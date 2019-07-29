use super::{basic_block::*, function::*, module::*, node::*};
use crate::ir::{basic_block::*, function::*, module::*, opcode::*, types::*, value::*};
use id_arena::*;
use rustc_hash::FxHashMap;

pub struct ConvertToDAG<'a> {
    pub module: &'a Module,
    pub instr_id_to_dag_node_id: FxHashMap<InstructionId, DAGNodeId>,
    pub cur_conversion_info: Option<ConversionInfo>,
}

pub struct ConversionInfo {
    pub dag_arena: Arena<DAGNode>,
    pub locals_ty: Vec<Type>,
    pub bb_to_dag_bb: FxHashMap<BasicBlockId, DAGBasicBlockId>,
    pub last_chain_node: Option<DAGNodeId>,
}

impl<'a> ConvertToDAG<'a> {
    pub fn new(module: &'a Module) -> Self {
        Self {
            module,
            instr_id_to_dag_node_id: FxHashMap::default(),
            cur_conversion_info: None,
        }
    }

    pub fn convert_module(&mut self) -> DAGModule {
        let mut dag_module = DAGModule::new(self.module.name.as_str());
        for (f_id, _) in &self.module.functions {
            dag_module.add_function(self.construct_dag(f_id));
        }
        dag_module
    }

    pub fn construct_dag(&mut self, func_id: FunctionId) -> DAGFunction {
        self.cur_conversion_info = Some(ConversionInfo::new());
        self.instr_id_to_dag_node_id.clear();

        let func = self.module.function_ref(func_id);
        let mut dag_bb_arena: Arena<DAGBasicBlock> = Arena::new();

        for (bb_id, _) in &func.basic_blocks {
            self.cur_conv_info_mut()
                .bb_to_dag_bb
                .insert(bb_id, dag_bb_arena.alloc(DAGBasicBlock::new()));
        }

        self.set_dag_bb_pred_and_succ(func, &mut dag_bb_arena);

        for (bb_id, bb) in &func.basic_blocks {
            let id = self.construct_dag_from_basic_block(func, bb);
            dag_bb_arena[self.cur_conv_info_ref().get_dag_bb(bb_id)].set_entry(id);
            // when_debug!({
            //     let dag_arena = &self.cur_conv_info_mut().dag_arena;
            //     println!("{}", dag_arena[id].to_dot(id, dag_arena))
            // });
        }

        let conv_info = ::std::mem::replace(&mut self.cur_conversion_info, None).unwrap();
        DAGFunction::new(func, conv_info.dag_arena, dag_bb_arena, conv_info.locals_ty)
    }

    pub fn get_dag_id_from_value(&mut self, v: &Value, arg_load: bool) -> DAGNodeValue {
        match v {
            Value::Instruction(iv) => DAGNodeValue::Id(self.instr_id_to_dag_node_id[&iv.id]),
            Value::Immediate(ImmediateValue::Int32(i)) => {
                DAGNodeValue::Constant(ConstantKind::Int32(*i))
            }
            Value::Argument(av) => {
                let ty = self
                    .module
                    .function_ref(av.func_id)
                    .get_param_type(av.index)
                    .unwrap();
                let fi = DAGNodeValue::FrameIndex(-(av.index as i32 + 1), ty.clone());
                if arg_load {
                    let load_id = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::Load,
                        vec![fi],
                        Some(ty.clone()),
                    ));
                    self.make_chain(load_id);
                    DAGNodeValue::Id(load_id)
                } else {
                    fi
                }
            }
            Value::Function(fid) => {
                let f = self.module.function_ref(*fid);
                DAGNodeValue::GlobalAddress(GlobalValueKind::FunctionName(f.name.to_string()))
            }
            Value::None => DAGNodeValue::None,
        }
    }

    pub fn construct_dag_from_basic_block(
        &mut self,
        func: &Function,
        bb: &BasicBlock,
    ) -> DAGNodeId {
        let mut local_count = 0i32;
        let entry_node = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
            DAGNodeKind::Entry,
            vec![],
            None,
        ));
        self.cur_conv_info_mut().last_chain_node = Some(entry_node);

        macro_rules! make_chain {
            ($dag_id:expr) => {{
                let dag_id = $dag_id;
                let conv_info = self.cur_conv_info_mut();
                if let Some(last_node_id) = &mut conv_info.last_chain_node {
                    conv_info.dag_arena[*last_node_id].next = Some(dag_id);
                    *last_node_id = dag_id;
                }
            }};
        }

        for instr_val in bb.iseq_ref().iter() {
            let instr_id = instr_val.get_instr_id().unwrap();
            let instr = &func.instr_table[instr_id];

            match instr.opcode {
                Opcode::Alloca(ref ty) => {
                    self.cur_conv_info_mut().locals_ty.push(ty.clone());
                    local_count += 1;
                    let id = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::FrameIndex,
                        vec![DAGNodeValue::FrameIndex(local_count, ty.clone())],
                        Some(ty.clone()),
                    ));
                    self.instr_id_to_dag_node_id.insert(instr_id, id);
                }
                Opcode::Load(ref v) => {
                    let v = self.get_dag_id_from_value(v, true);
                    let load_id = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::Load,
                        vec![v],
                        Some(instr.ty.clone()),
                    ));
                    make_chain!(load_id);
                    self.instr_id_to_dag_node_id.insert(instr_id, load_id);
                }
                Opcode::Store(ref src, ref dst) => {
                    let dst = self.get_dag_id_from_value(dst, true);
                    let src = self.get_dag_id_from_value(src, true);
                    let id = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::Store,
                        vec![dst, src],
                        None,
                    ));
                    make_chain!(id);
                }
                Opcode::GetElementPtr(ref ptr, ref indices) => {
                    let gep = self.construct_dag_for_gep(instr, ptr, indices);
                    self.instr_id_to_dag_node_id.insert(instr_id, gep);
                    if bb.liveness.borrow().live_out.contains(&instr_id) {
                        make_chain!(gep);
                    }
                }
                Opcode::Call(ref f, ref args) => {
                    let mut operands: Vec<DAGNodeValue> = args
                        .iter()
                        .map(|a| self.get_dag_id_from_value(a, true))
                        .collect();
                    operands.insert(0, self.get_dag_id_from_value(f, true));
                    let id = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::Call,
                        operands,
                        Some(instr.ty.clone()),
                    ));
                    make_chain!(id);
                    self.instr_id_to_dag_node_id.insert(instr_id, id);
                }
                Opcode::Add(ref v1, ref v2)
                | Opcode::Sub(ref v1, ref v2)
                | Opcode::Mul(ref v1, ref v2)
                | Opcode::Rem(ref v1, ref v2) => {
                    let v1 = self.get_dag_id_from_value(v1, true);
                    let v2 = self.get_dag_id_from_value(v2, true);
                    let bin_id = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        match instr.opcode {
                            Opcode::Add(_, _) => DAGNodeKind::Add,
                            Opcode::Sub(_, _) => DAGNodeKind::Sub,
                            Opcode::Mul(_, _) => DAGNodeKind::Mul,
                            Opcode::Rem(_, _) => DAGNodeKind::Rem,
                            _ => unreachable!(),
                        },
                        vec![v1, v2],
                        Some(instr.ty.clone()),
                    ));
                    self.instr_id_to_dag_node_id.insert(instr_id, bin_id);
                    if bb.liveness.borrow().live_out.contains(&instr_id) {
                        make_chain!(bin_id);
                    }
                }
                Opcode::Br(bb) => make_chain!(self.cur_conv_info_mut_with(|c| {
                    c.dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::Br,
                        vec![DAGNodeValue::BasicBlock(c.get_dag_bb(bb))],
                        None,
                    ))
                })),
                Opcode::CondBr(ref v, then_, else_) => {
                    let v = self.get_dag_id_from_value(v, true);
                    make_chain!({
                        let c = self.cur_conv_info_mut();
                        c.dag_arena.alloc(DAGNode::new(
                            DAGNodeKind::BrCond,
                            vec![v, DAGNodeValue::BasicBlock(c.get_dag_bb(then_))],
                            None,
                        ))
                    });
                    make_chain!(self.cur_conv_info_mut_with(|c| {
                        c.dag_arena.alloc(DAGNode::new(
                            DAGNodeKind::Br,
                            vec![DAGNodeValue::BasicBlock(c.get_dag_bb(else_))],
                            None,
                        ))
                    }));
                }
                Opcode::ICmp(ref c, ref v1, ref v2) => {
                    let v1 = self.get_dag_id_from_value(v1, true);
                    let v2 = self.get_dag_id_from_value(v2, true);
                    let id = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::Setcc,
                        vec![DAGNodeValue::CondKind((*c).into()), v1, v2],
                        Some(instr.ty.clone()),
                    ));
                    self.instr_id_to_dag_node_id.insert(instr_id, id);
                }
                Opcode::Phi(ref pairs) => {
                    let mut operands = vec![];
                    for (val, bb) in pairs {
                        operands.push(self.get_dag_id_from_value(val, true));
                        operands.push(DAGNodeValue::BasicBlock(
                            self.cur_conv_info_mut().get_dag_bb(*bb),
                        ))
                    }
                    let id = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::Phi,
                        operands,
                        Some(instr.ty.clone()),
                    ));
                    self.instr_id_to_dag_node_id.insert(instr_id, id);
                }
                Opcode::Ret(ref v) => {
                    let v = self.get_dag_id_from_value(v, true);
                    make_chain!(self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                        DAGNodeKind::Ret,
                        vec![v],
                        None
                    )))
                }
            }
        }

        entry_node
    }

    fn construct_dag_for_gep(
        &mut self,
        instr: &Instruction,
        ptr: &Value,
        indices: &[Value],
    ) -> DAGNodeId {
        let mut gep = self.get_dag_id_from_value(ptr, false).id();

        for idx in indices {
            let idx = self.get_dag_id_from_value(idx, true);
            gep = self.cur_conv_info_mut().dag_arena.alloc(DAGNode::new(
                DAGNodeKind::Add,
                vec![DAGNodeValue::Id(gep), idx],
                Some(instr.ty.clone()), // TODO
            ));
        }

        gep
    }

    fn cur_conv_info_mut_with<F, T>(&mut self, mut f: F) -> T
    where
        F: FnMut(&mut ConversionInfo) -> T,
    {
        f(self.cur_conversion_info.as_mut().unwrap())
    }

    fn cur_conv_info_mut(&mut self) -> &mut ConversionInfo {
        self.cur_conversion_info.as_mut().unwrap()
    }

    fn cur_conv_info_ref(&mut self) -> &ConversionInfo {
        self.cur_conversion_info.as_ref().unwrap()
    }

    fn set_dag_bb_pred_and_succ(
        &mut self,
        func: &Function,
        dag_bb_arena: &mut Arena<DAGBasicBlock>,
    ) {
        let conv_info = self.cur_conv_info_ref();
        for (bb, dag_bb) in &conv_info.bb_to_dag_bb {
            dag_bb_arena[*dag_bb].pred = func.basic_blocks[*bb]
                .pred
                .iter()
                .map(|bb| conv_info.get_dag_bb(*bb))
                .collect();
            dag_bb_arena[*dag_bb].succ = func.basic_blocks[*bb]
                .succ
                .iter()
                .map(|bb| conv_info.get_dag_bb(*bb))
                .collect();
        }
    }

    fn make_chain(&mut self, dag_id: DAGNodeId) {
        let conv_info = self.cur_conv_info_mut();
        if let Some(last_node_id) = &mut conv_info.last_chain_node {
            conv_info.dag_arena[*last_node_id].next = Some(dag_id);
            *last_node_id = dag_id;
        }
    }
}

impl ConversionInfo {
    pub fn new() -> Self {
        ConversionInfo {
            dag_arena: Arena::new(),
            locals_ty: vec![],
            bb_to_dag_bb: FxHashMap::default(),
            last_chain_node: None,
        }
    }

    pub fn get_dag_bb(&self, bb_id: BasicBlockId) -> DAGBasicBlockId {
        *self.bb_to_dag_bb.get(&bb_id).unwrap()
    }
}
