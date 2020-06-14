use super::{super::machine::register::*, node::*};
use crate::codegen::common::dag::{
    function::{DAGFunction, DAGHeap},
    module::DAGModule,
};
use crate::{ir::types::*, traits::pass::ModulePassTrait, util::allocator::*};
use defs::isel_pat;
use rustc_hash::FxHashMap;

impl ModulePassTrait for MISelector {
    type M = DAGModule;

    fn name(&self) -> &'static str {
        "MachineInstSelector"
    }

    fn run_on_module(&mut self, module: &mut Self::M) {
        self.run_on_module(module);
    }
}

pub struct MISelector {
    selected: FxHashMap<Raw<DAGNode>, Raw<DAGNode>>,
}

impl MISelector {
    pub fn new() -> Self {
        Self {
            selected: FxHashMap::default(),
        }
    }

    pub fn run_on_module(&mut self, module: &mut DAGModule) {
        for (_, func) in &mut module.functions {
            if func.is_internal {
                continue;
            }
            self.run_on_function(&module.types, func)
        }
    }

    fn run_on_function(&mut self, tys: &Types, func: &mut DAGFunction) {
        for bb_id in &func.dag_basic_blocks {
            let bb = &func.dag_basic_block_arena[*bb_id];
            self.run_on_node(tys, &func.regs_info, &mut func.dag_heap, bb.entry.unwrap());
        }
    }

    fn run_on_node(
        &mut self,
        tys: &Types,
        regs_info: &RegistersInfo,
        heap: &mut DAGHeap,
        mut node: Raw<DAGNode>,
    ) -> Raw<DAGNode> {
        if !node.may_contain_children() {
            return node;
        }

        if let Some(node) = self.selected.get(&node) {
            return *node;
        }

        let mut selected = isel_pat!(
            // TODO: Refactoring
            // (ir.Call _a) => { self.select_call(tys, regs_info, heap, node) }
            (ir.Add a, b): Int32 {
                GPR a {
                    imm12 b => (mi.ADDIW a, b)
                    imm32 b => (mi.ADDW  a, (mi.LI b))
                    GPR   b => (mi.ADDW  a, b) } }
            (ir.Mul a, b): Int32 {
                GPR a {
                    imm32 b => (mi.MULW a, (mi.LI b))
                    GPR   b => (mi.MULW a, b) } }
            (ir.Div a, b): Int32 {
                GPR a {
                    imm32 b => (mi.DIVW a, (mi.LI b))
                    GPR   b => (mi.DIVW a, b) } }
            (ir.Br a) => (mi.J a)
            // (ir.Mul a, b) {
            //     GR32 a {
            //         GR32  b => (mi.IMULrr32  a, b)
            //         imm32 b => (mi.IMULrri32 a, b) }
            //     GR64 a {
            //         imm32 b => (mi.IMULrr64i32 a, b) }
            //     XMM a {
            //         (ir.Load c) b {
            //             (ir.FIAddr d) c {
            //                 f64mem d => (mi.MULSDrm a, [BaseFi %rbp, d]) } }
            //         imm_f64 b => (mi.MULSDrr a, (mi.MOVSDrm64 b))
            //         XMM    b => (mi.MULSDrr a, b)
            //     }
            // }
            // (ir.Div a, b) {
            //     XMM a {
            //         (ir.Load c) b {
            //             (ir.FIAddr d) c {
            //                 f64mem d => (mi.DIVSDrm a, [BaseFi %rbp, d]) } }
            //         imm_f64 b => (mi.DIVSDrr a, (mi.MOVSDrm64 b))
            //         XMM    b => (mi.DIVSDrr a, b)
            //     }
            //     imm_f64 a {
            //         XMM b => (mi.DIVSDrr (mi.MOVSDrm64 a), b)
            //     }
            // }
            // (ir.Shl a, b) {
            //     GR64 a {
            //         imm8 b => (mi.SHLr64i8 a, b) }
            //     GR32 a {
            //         imm8 b => (mi.SHLr32i8 a, b) }
            // }
            (ir.Load a) {
                (ir.FIAddr b) a {
                    mem32 b => (mi.LW [FiReg b, %s0])
                }
            }
            (ir.Store a, b) {
                (ir.FIAddr c) a {
                    mem32 c {
                        imm32 b => (mi.SW (mi.LI b), [FiReg c, %s0])
                    }
                }
            }
            // (ir.Load a) {
            //     (ir.FIAddr b) a {
            //         f64mem b => (mi.MOVSDrm [BaseFi %rbp, b])
            //         mem32  b => (mi.MOVrm32 [BaseFi %rbp, b])
            //         mem64  b => (mi.MOVrm64 [BaseFi %rbp, b])
            //     }
            //     // a is pointer
            //     GR64  a => {
            //         let a = self.run_on_node(tys, regs_info, heap, a);
            //         let ty = tys.get_element_ty(a.ty, None).unwrap();
            //         let mem = heap.alloc(DAGNode::new_mem(MemNodeKind::Base, vec![a]));
            //         match ty {
            //             Type::Int32 => heap.alloc(DAGNode::new(NodeKind::MI(MINodeKind::MOVrm32),
            //                     vec![mem], Type::Int32)),
            //             Type::Int64 | Type::Pointer(_) | Type::Array(_) =>
            //                 heap.alloc(DAGNode::new(NodeKind::MI(MINodeKind::MOVrm64),
            //                     vec![mem], node.ty)),
            //             Type::F64 => heap.alloc(DAGNode::new(NodeKind::MI(MINodeKind::MOVSDrm),
            //                     vec![mem], Type::F64)),
            //             _ => unimplemented!()
            //         }
            //     }
            // }
            // (ir.Store a, b) {
            //     (ir.FIAddr c) a {
            //         f64mem c {
            //             imm_f64 b => (mi.MOVSDmr [BaseFi %rbp, c], (mi.MOVSDrm64 b))
            //         }
            //         mem32  c {
            //             GR32  b => (mi.MOVmr32 [BaseFi %rbp, c], b)
            //             imm32 b => (mi.MOVmi32 [BaseFi %rbp, c], b) }
            //         mem64  c {
            //             GR64  b => (mi.MOVmr64 [BaseFi %rbp, c], b) }
            //     }
            //     GR64   a {
            //         imm32 b => (mi.MOVmi32 [Base a], b)
            //         GR32  b => (mi.MOVmr32 [Base a], b)
            //         GR64  b => (mi.MOVmr64 [Base a], b)
            //         imm_f64 b => (mi.MOVSDmr [Base a], (mi.MOVSDrm64 b))
            //         XMM    b => (mi.MOVSDmr [Base a], b)
            //     }
            // }
            // (ir.FIAddr a) {
            //     mem a => (mi.LEAr64m [BaseFi %rbp, a])
            // }
            // (ir.CopyFromReg a) => (mi.Copy a)
        );

        self.selected.insert(node, selected);

        if let Some(next) = node.next {
            selected.next = Some(self.run_on_node(tys, regs_info, heap, next));
        }

        selected
    }

    // fn select_call(
    //     &mut self,
    //     tys: &Types,
    //     regs_info: &RegistersInfo,
    //     heap: &mut DAGHeap,
    //     mut node: Raw<DAGNode>,
    // ) -> Raw<DAGNode> {
    //     unimplemented!()
    // const SQRT_F64: &str = "cilk.sqrt.f64";
    // let supported = [SQRT_F64];
    //
    // let name = match &node.operand[0].kind {
    //     NodeKind::Operand(OperandNodeKind::Address(AddressKind::FunctionName(name)))
    //         if supported.contains(&name.as_str()) =>
    //     {
    //         name.as_str()
    //     }
    //     _ => {
    //         node.operand = node
    //             .operand
    //             .iter()
    //             .map(|op| self.run_on_node(tys, regs_info, heap, *op))
    //             .collect();
    //         return node;
    //     }
    // };
    //
    // match name {
    //     SQRT_F64 => {
    //         let x = self.run_on_node(tys, regs_info, heap, node.operand[1]);
    //         heap.alloc(DAGNode::new(
    //             NodeKind::MI(MINodeKind::SQRTSDrr),
    //             vec![x],
    //             Type::F64,
    //         ))
    //     }
    //     _ => unreachable!(),
    // }
    // }
}
