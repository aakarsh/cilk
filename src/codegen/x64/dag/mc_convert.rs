// TODO: dirty code

use super::super::inst::DefOrUseReg;
use super::super::machine::{basic_block::*, function::*, inst, inst::*, module::*};
use super::super::register::*;
use super::{basic_block::*, function::*, module::*, node, node::*};
use crate::ir::types::*;
use crate::util::allocator::*;
use id_arena::*;
use rustc_hash::FxHashMap;
use std::cell::RefCell;

pub struct ConversionInfo<'a> {
    types: &'a Types,
    cur_func: &'a DAGFunction,
    machine_inst_arena: &'a mut InstructionArena,
    dag_to_reg: FxHashMap<Raw<DAGNode>, Option<MachineRegister>>,
    cur_bb: MachineBasicBlockId,
    iseq: &'a mut Vec<MachineInstId>,
    bb_map: &'a FxHashMap<DAGBasicBlockId, MachineBasicBlockId>,
    node2minst: &'a mut FxHashMap<Raw<DAGNode>, MachineInstId>,
}

pub fn convert_module(module: DAGModule) -> MachineModule {
    let mut functions = Arena::new();
    for (_, func) in module.functions {
        functions.alloc(convert_function(&module.types, func));
    }
    MachineModule::new(module.name, functions, module.types)
}

pub fn convert_function(types: &Types, dag_func: DAGFunction) -> MachineFunction {
    let mut bb_map = FxHashMap::default();
    let mut mbbs = MachineBasicBlocks::new();

    for dag_bb_id in &dag_func.dag_basic_blocks {
        let mbb_id = mbbs.arena.alloc(MachineBasicBlock::new());
        bb_map.insert(*dag_bb_id, mbb_id);
        mbbs.order.push(mbb_id);
    }

    for (dag, machine) in &bb_map {
        mbbs.arena[*machine].pred = dag_func.dag_basic_block_arena[*dag]
            .pred
            .iter()
            .map(|bb| *bb_map.get(bb).unwrap())
            .collect();
        mbbs.arena[*machine].succ = dag_func.dag_basic_block_arena[*dag]
            .succ
            .iter()
            .map(|bb| *bb_map.get(bb).unwrap())
            .collect();
    }

    let mut machine_inst_arena = InstructionArena::new();
    let mut node2minst = FxHashMap::default();

    for dag_bb_id in &dag_func.dag_basic_blocks {
        let node = &dag_func.dag_basic_block_arena[*dag_bb_id];
        let bb_id = *bb_map.get(dag_bb_id).unwrap();
        let mut iseq = vec![];

        ConversionInfo {
            types,
            cur_func: &dag_func,
            machine_inst_arena: &mut machine_inst_arena,
            dag_to_reg: FxHashMap::default(),
            cur_bb: bb_id,
            iseq: &mut iseq,
            bb_map: &bb_map,
            node2minst: &mut node2minst,
        }
        .convert_dag(node.entry.unwrap());

        mbbs.arena[bb_id].iseq = RefCell::new(iseq);
    }

    MachineFunction::new(dag_func, mbbs, machine_inst_arena)
}

impl<'a> ConversionInfo<'a> {
    pub fn convert_dag(&mut self, node: Raw<DAGNode>) -> Option<MachineRegister> {
        if let Some(machine_register) = self.dag_to_reg.get(&node) {
            return machine_register.clone();
        }

        let machine_register = match self.convert_dag_to_machine_inst(node) {
            Some(id) => {
                let inst = &self.machine_inst_arena[id];
                inst.get_def_reg().map(|r| r.clone())
            }
            None => None,
        };
        self.dag_to_reg.insert(node, machine_register.clone());

        some_then!(next, node.next, {
            self.convert_dag(next);
        });

        machine_register
    }

    fn convert_dag_to_machine_inst(&mut self, node: Raw<DAGNode>) -> Option<MachineInstId> {
        if let Some(machine_inst_id) = self.node2minst.get(&node) {
            return Some(*machine_inst_id);
        }

        #[rustfmt::skip]
        macro_rules! bb {($id:expr)=>{ $id.as_basic_block() };}
        #[rustfmt::skip]
        macro_rules! cond_kind {($id:expr)=>{ $id.as_cond_kind() };}

        // there should be no NodeKind::IRs here
        let machine_inst_id = match &node.kind {
            NodeKind::MI(_) => {
                fn reg(inst: &MachineInst, x: &DefOrUseReg) -> MachineRegister {
                    match x {
                        DefOrUseReg::Def(i) => inst.def[*i].clone(),
                        DefOrUseReg::Use(i) => inst.operand[*i].as_register().clone(),
                    }
                }
                let mi = node.kind.as_mi();
                let inst_def = mi.inst_def().unwrap();
                let operands = node
                    .operand
                    .iter()
                    .map(|op| self.normal_operand(*op))
                    .collect();
                let mut inst = MachineInst::new(
                    &self.cur_func.vreg_gen,
                    mi,
                    operands,
                    ty2rc(&node.ty),
                    self.cur_bb,
                );
                for (def_, use_) in &inst_def.tie {
                    inst.tie_regs(reg(&inst, def_), reg(&inst, use_));
                }
                Some(self.push_inst(inst))
            }
            NodeKind::IR(IRNodeKind::Entry) => None,
            NodeKind::IR(IRNodeKind::CopyToReg) => {
                let val = self.normal_operand(node.operand[1]);
                let dst = match &node.operand[0].kind {
                    NodeKind::Operand(OperandNodeKind::Register(r)) => {
                        MachineRegister::new(r.clone())
                    }
                    _ => unreachable!(),
                };
                Some(self.push_inst(MachineInst::new_with_def_reg(
                    MachineOpcode::Copy,
                    vec![val],
                    vec![dst],
                    self.cur_bb,
                )))
            }
            NodeKind::IR(IRNodeKind::Call) => Some(self.convert_call_dag(&*node)),
            NodeKind::IR(IRNodeKind::Phi) => {
                let mut operands = vec![];
                let mut i = 0;
                while i < node.operand.len() {
                    operands.push(self.normal_operand(node.operand[i]));
                    operands.push(MachineOperand::Branch(
                        self.get_machine_bb(bb!(node.operand[i + 1])),
                    ));
                    i += 2;
                }
                Some(self.push_inst(MachineInst::new(
                    &self.cur_func.vreg_gen,
                    MachineOpcode::Phi,
                    operands,
                    ty2rc(&node.ty),
                    self.cur_bb,
                )))
            }
            NodeKind::IR(IRNodeKind::Div) => {
                let eax = MachineRegister::phys_reg(GR32::EAX);
                let edx = MachineRegister::phys_reg(GR32::EDX);

                let op1 = self.normal_operand(node.operand[0]);
                let op2 = self.normal_operand(node.operand[1]);

                self.push_inst(
                    MachineInst::new_simple(mov_n_rx(32, &op1).unwrap(), vec![op1], self.cur_bb)
                        .with_def(vec![eax.clone()]),
                );

                self.push_inst(
                    MachineInst::new_simple(MachineOpcode::CDQ, vec![], self.cur_bb)
                        .with_imp_def(edx.clone())
                        .with_imp_use(eax.clone()),
                );

                assert_eq!(op2.get_type(), Some(Type::Int32));
                let inst1 = MachineInst::new(
                    &self.cur_func.vreg_gen,
                    mov_n_rx(32, &op2).unwrap(),
                    vec![op2],
                    Some(RegisterClassKind::GR32), // TODO: support other types
                    self.cur_bb,
                );
                let op2 = MachineOperand::Register(inst1.def[0].clone());
                self.push_inst(inst1);

                self.push_inst(
                    MachineInst::new_simple(MachineOpcode::IDIV, vec![op2], self.cur_bb)
                        .with_imp_defs(vec![eax.clone(), edx.clone()])
                        .with_imp_uses(vec![eax.clone(), edx]),
                );

                Some(self.push_inst(MachineInst::new(
                    &self.cur_func.vreg_gen,
                    MachineOpcode::Copy,
                    vec![MachineOperand::Register(eax)],
                    Some(RegisterClassKind::GR32), // TODO
                    self.cur_bb,
                )))
            }
            NodeKind::IR(IRNodeKind::Rem) => {
                let eax = MachineRegister::phys_reg(GR32::EAX);
                let edx = MachineRegister::phys_reg(GR32::EDX);

                let op1 = self.normal_operand(node.operand[0]);
                let op2 = self.normal_operand(node.operand[1]);

                self.push_inst(
                    MachineInst::new_simple(mov_n_rx(32, &op1).unwrap(), vec![op1], self.cur_bb)
                        .with_def(vec![eax.clone()]),
                );

                self.push_inst(
                    MachineInst::new_simple(MachineOpcode::CDQ, vec![], self.cur_bb)
                        .with_imp_def(edx.clone())
                        .with_imp_use(eax.clone()),
                );

                assert_eq!(op2.get_type(), Some(Type::Int32));
                let inst1 = MachineInst::new(
                    &self.cur_func.vreg_gen,
                    mov_n_rx(32, &op2).unwrap(),
                    vec![op2],
                    Some(RegisterClassKind::GR32), // TODO: support other types
                    self.cur_bb,
                );
                let op2 = MachineOperand::Register(inst1.def[0].clone());
                self.push_inst(inst1);

                self.push_inst(
                    MachineInst::new_simple(MachineOpcode::IDIV, vec![op2], self.cur_bb)
                        .with_imp_defs(vec![eax.clone(), edx.clone()])
                        .with_imp_uses(vec![eax, edx.clone()]),
                );

                Some(self.push_inst(MachineInst::new(
                    &self.cur_func.vreg_gen,
                    MachineOpcode::Copy,
                    vec![MachineOperand::Register(edx)],
                    Some(RegisterClassKind::GR32), // TODO
                    self.cur_bb,
                )))
            }
            NodeKind::IR(IRNodeKind::Setcc) => {
                let new_op1 = self.normal_operand(node.operand[1]);
                let new_op2 = self.normal_operand(node.operand[2]);
                Some(self.push_inst(MachineInst::new(
                    &self.cur_func.vreg_gen,
                    match cond_kind!(node.operand[0]) {
                        CondKind::Eq => MachineOpcode::Seteq,
                        CondKind::Le => MachineOpcode::Setle,
                        CondKind::Lt => MachineOpcode::Setlt,
                        _ => unimplemented!(),
                    },
                    vec![new_op1, new_op2],
                    ty2rc(&node.ty),
                    self.cur_bb,
                )))
            }
            NodeKind::IR(IRNodeKind::Br) => Some(self.push_inst(MachineInst::new(
                &self.cur_func.vreg_gen,
                MachineOpcode::JMP,
                vec![MachineOperand::Branch(
                    self.get_machine_bb(bb!(node.operand[0])),
                )],
                None,
                self.cur_bb,
            ))),
            NodeKind::IR(IRNodeKind::BrCond) => {
                let new_cond = self.normal_operand(node.operand[0]);
                Some(self.push_inst(MachineInst::new(
                    &self.cur_func.vreg_gen,
                    MachineOpcode::BrCond,
                    vec![
                        new_cond,
                        MachineOperand::Branch(self.get_machine_bb(bb!(node.operand[1]))),
                    ],
                    None,
                    self.cur_bb,
                )))
            }
            NodeKind::IR(IRNodeKind::Brcc) => {
                let op0 = self.normal_operand(node.operand[1]);
                let op1 = self.normal_operand(node.operand[2]);

                self.push_inst(MachineInst::new_simple(
                    if op0.is_register() && op1.is_constant() {
                        MachineOpcode::CMPri
                    } else if op0.is_register() && op1.is_register() {
                        MachineOpcode::CMPrr
                    } else {
                        unreachable!()
                    },
                    vec![op0, op1],
                    self.cur_bb,
                ));

                Some(self.push_inst(MachineInst::new_simple(
                    match cond_kind!(node.operand[0]) {
                        CondKind::Eq => MachineOpcode::JE,
                        CondKind::Le => MachineOpcode::JLE,
                        CondKind::Lt => MachineOpcode::JL,
                        CondKind::Ge => MachineOpcode::JGE,
                        CondKind::Gt => MachineOpcode::JG,
                    },
                    vec![MachineOperand::Branch(
                        self.get_machine_bb(bb!(node.operand[3])),
                    )],
                    self.cur_bb,
                )))
            }
            NodeKind::IR(IRNodeKind::Ret) => Some(self.convert_ret(&*node)),
            NodeKind::IR(IRNodeKind::CopyToLiveOut) => {
                self.convert_dag_to_machine_inst(node.operand[0])
            }
            e => {
                println!("{:?}", e);
                None
            }
        };

        if machine_inst_id.is_some() {
            self.node2minst.insert(node, machine_inst_id.unwrap());
        }

        machine_inst_id
    }

    fn convert_ret(&mut self, node: &DAGNode) -> MachineInstId {
        let val = self.normal_operand(node.operand[0]);
        if let Some(ty) = val.get_type() {
            let ret_reg = ty2rc(&ty).unwrap().return_value_register();
            let set_ret_val =
                MachineInst::new_simple(mov_rx(self.types, &val).unwrap(), vec![val], self.cur_bb)
                    .with_def(vec![MachineRegister::phys_reg(ret_reg)]);
            self.push_inst(set_ret_val);
        }
        self.push_inst(MachineInst::new_simple(
            MachineOpcode::RET,
            vec![],
            self.cur_bb,
        ))
    }

    fn move2reg(&self, r: MachineRegister, src: MachineOperand) -> MachineInst {
        match src {
            MachineOperand::Branch(_) => unimplemented!(),
            MachineOperand::Constant(_) | MachineOperand::Register(_) => {
                MachineInst::new_simple(mov_rx(self.types, &src).unwrap(), vec![src], self.cur_bb)
                    .with_def(vec![r])
            }
            MachineOperand::FrameIndex(fi) => MachineInst::new_with_def_reg(
                MachineOpcode::LEAr64m,
                vec![MachineOperand::Mem(MachineMemOperand::BaseFi(
                    MachineRegister::phys_reg(GR64::RBP),
                    fi,
                ))],
                vec![r],
                self.cur_bb,
            ),
            _ => unimplemented!(),
        }
    }

    fn convert_call_dag(&mut self, node: &DAGNode) -> MachineInstId {
        let mut arg_regs = vec![MachineRegister::phys_reg(GR64::RSP)]; // call uses RSP
        let mut off = 0;

        for (i, operand) in node.operand[1..].iter().enumerate() {
            let arg = self.normal_operand(*operand);
            let ty = arg.get_type().unwrap();

            if !matches!(
                ty,
                Type::Int32 | Type::Int64 | Type::F64 | Type::Pointer(_) | Type::Array(_)
            ) {
                unimplemented!()
            };

            let reg_class = ty2rc(&ty).unwrap();
            let inst = match reg_class.get_nth_arg_reg(i) {
                Some(arg_reg) => {
                    let r = MachineRegister::phys_reg(arg_reg);
                    arg_regs.push(r.clone());
                    self.move2reg(r, arg)
                }
                None => {
                    // Put the exceeded value onto the stack
                    let inst = MachineInst::new_simple(
                        mov_mx(&arg).unwrap(),
                        vec![
                            MachineOperand::Mem(MachineMemOperand::BaseOff(
                                MachineRegister::phys_reg(GR64::RSP),
                                off,
                            )),
                            arg,
                        ],
                        self.cur_bb,
                    );
                    off += 8;
                    inst
                }
            };

            self.push_inst(inst);
        }

        self.push_inst(
            MachineInst::new_simple(
                MachineOpcode::AdjStackDown,
                vec![MachineOperand::imm_i32(off)],
                self.cur_bb,
            )
            .with_imp_def(MachineRegister::phys_reg(GR64::RSP))
            .with_imp_use(MachineRegister::phys_reg(GR64::RSP)),
        );

        let callee = self.normal_operand(node.operand[0]);
        let ret_reg = MachineRegister::phys_reg(GR32::EAX); // TODO: Support other types than i32
        let call_inst = self.push_inst(
            MachineInst::new_simple(MachineOpcode::CALL, vec![callee], self.cur_bb)
                .with_imp_uses(arg_regs)
                .with_imp_def(MachineRegister::phys_reg(GR64::RSP))
                .with_def(match node.ty {
                    Type::Void => vec![],
                    _ => vec![ret_reg.clone()],
                }),
        );

        self.push_inst(
            MachineInst::new_simple(
                MachineOpcode::AdjStackUp,
                vec![MachineOperand::imm_i32(off)],
                self.cur_bb,
            )
            .with_imp_def(MachineRegister::phys_reg(GR64::RSP))
            .with_imp_use(MachineRegister::phys_reg(GR64::RSP)),
        );

        if node.ty == Type::Void {
            return call_inst;
        }

        let reg_class = ret_reg.info_ref().reg_class;
        self.push_inst(MachineInst::new(
            &self.cur_func.vreg_gen,
            MachineOpcode::Copy,
            vec![MachineOperand::Register(ret_reg)],
            Some(reg_class),
            self.cur_bb,
        ))
    }

    fn normal_operand(&mut self, node: Raw<DAGNode>) -> MachineOperand {
        match node.kind {
            NodeKind::Operand(OperandNodeKind::Constant(c)) => match c {
                ConstantKind::Int32(i) => MachineOperand::Constant(MachineConstant::Int32(i)),
                ConstantKind::Int64(i) => MachineOperand::Constant(MachineConstant::Int64(i)),
                ConstantKind::F64(f) => MachineOperand::Constant(MachineConstant::F64(f)),
            },
            NodeKind::Operand(OperandNodeKind::FrameIndex(ref kind)) => {
                MachineOperand::FrameIndex(kind.clone())
            }
            NodeKind::Operand(OperandNodeKind::Address(ref g)) => match g {
                node::AddressKind::FunctionName(n) => MachineOperand::Mem(
                    MachineMemOperand::Address(inst::AddressKind::FunctionName(n.clone())),
                ),
            },
            NodeKind::Operand(OperandNodeKind::BasicBlock(_)) => unimplemented!(),
            NodeKind::Operand(OperandNodeKind::Register(ref r)) => {
                MachineOperand::Register(MachineRegister::new(r.clone()))
            }
            NodeKind::Operand(OperandNodeKind::Mem(ref mem)) => match mem {
                MemNodeKind::Base => MachineOperand::Mem(MachineMemOperand::Base(
                    self.normal_operand(node.operand[0]).as_register().clone(),
                )),
                MemNodeKind::BaseAlignOff => MachineOperand::Mem(MachineMemOperand::BaseAlignOff(
                    self.normal_operand(node.operand[0]).as_register().clone(),
                    self.normal_operand(node.operand[1]).as_constant().as_i32(),
                    self.normal_operand(node.operand[2]).as_register().clone(),
                )),
                MemNodeKind::BaseFi => MachineOperand::Mem(MachineMemOperand::BaseFi(
                    self.normal_operand(node.operand[0]).as_register().clone(),
                    *self.normal_operand(node.operand[1]).as_frame_index(),
                )),
                MemNodeKind::BaseFiAlignOff => {
                    MachineOperand::Mem(MachineMemOperand::BaseFiAlignOff(
                        self.normal_operand(node.operand[0]).as_register().clone(),
                        *self.normal_operand(node.operand[1]).as_frame_index(),
                        self.normal_operand(node.operand[2]).as_constant().as_i32(),
                        self.normal_operand(node.operand[3]).as_register().clone(),
                    ))
                }
                MemNodeKind::BaseFiOff => MachineOperand::Mem(MachineMemOperand::BaseFiOff(
                    self.normal_operand(node.operand[0]).as_register().clone(),
                    *self.normal_operand(node.operand[1]).as_frame_index(),
                    self.normal_operand(node.operand[2]).as_constant().as_i32(),
                )),
            },
            NodeKind::None => MachineOperand::None,
            _ => MachineOperand::Register(self.convert_dag(node).unwrap()),
        }
    }

    fn get_machine_bb(&self, dag_bb_id: DAGBasicBlockId) -> MachineBasicBlockId {
        *self.bb_map.get(&dag_bb_id).unwrap()
    }

    pub fn push_inst(&mut self, inst: MachineInst) -> MachineInstId {
        let inst_id = self.machine_inst_arena.alloc(inst);
        self.iseq.push(inst_id);
        inst_id
    }
}

pub fn mov_n_rx(bit: usize, x: &MachineOperand) -> Option<MachineOpcode> {
    // TODO: refine code
    assert!(bit > 0 && ((bit & (bit - 1)) == 0));

    let mov32rx = [MachineOpcode::MOVrr32, MachineOpcode::MOVri32];
    let xidx = match x {
        MachineOperand::Register(_) => 0,
        MachineOperand::Constant(_) => 1,
        _ => return None, // TODO: Support Address?
    };
    match bit {
        32 => Some(mov32rx[xidx]),
        _ => None,
    }
}

pub fn mov_rx(tys: &Types, x: &MachineOperand) -> Option<MachineOpcode> {
    // TODO: special handling for float
    if x.get_type().unwrap() == Type::F64 {
        return match x {
            MachineOperand::Mem(_) => Some(MachineOpcode::MOVSDrm64),
            MachineOperand::Register(_) => Some(MachineOpcode::MOVSDrr),
            _ => None,
        };
    }

    let mov32rx = [
        MachineOpcode::MOVrr32,
        MachineOpcode::MOVri32,
        MachineOpcode::MOVrm32,
    ];
    let mov64rx = [
        MachineOpcode::MOVrr64,
        MachineOpcode::MOVri64,
        MachineOpcode::MOVrm64,
    ];
    let (bit, xidx) = match x {
        MachineOperand::Register(r) => (r.info_ref().reg_class.size_in_bits(), 0),
        MachineOperand::Constant(c) => (c.size_in_bits(), 1),
        MachineOperand::FrameIndex(f) => (f.ty.size_in_bits(tys), 2),
        _ => return None, // TODO: Support Address?
    };
    match bit {
        32 => Some(mov32rx[xidx]),
        64 => Some(mov64rx[xidx]),
        _ => None,
    }
}

pub fn mov_mx(x: &MachineOperand) -> Option<MachineOpcode> {
    let mov32mx = [MachineOpcode::MOVmr32, MachineOpcode::MOVmi32];
    let mov64mx = [MachineOpcode::MOVmr64, MachineOpcode::MOVmi64];
    // let mov64rx = [
    //     MachineOpcode::MOVrr64,
    //     MachineOpcode::MOVri64,
    //     MachineOpcode::MOVrm64,
    // ];
    let (bit, n) = match x {
        MachineOperand::Register(r) => (r.info_ref().reg_class.size_in_bits(), 0),
        MachineOperand::Constant(c) => (c.size_in_bits(), 1),
        _ => return None, // TODO: Support Address?
    };
    match bit {
        32 => Some(mov32mx[n]),
        64 => Some(mov64mx[n]),
        _ => None,
    }
}
