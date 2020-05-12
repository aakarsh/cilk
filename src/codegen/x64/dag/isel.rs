use super::super::register::*;
use super::{
    function::{DAGFunction, DAGHeap},
    module::DAGModule,
    node::*,
};
use crate::ir::types::*;
use crate::util::allocator::*;
use defs::isel_pat;
use rustc_hash::FxHashMap;

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
            (ir.Add a, b) {
                GR32 a {
                    GR32  b => (mi.ADDrr32   a, b)
                    imm32 b => (mi.ADDri32   a, b) }
                GR64 a {
                    imm32 b => (mi.ADDr64i32 a, b)
                    GR64  b => (mi.ADDrr64   a, b) }
                XMM a {
                    imm_f64 b => (mi.ADDSDrr a, (mi.MOVSDrm64 b))
                    XMM    b => (mi.ADDSDrr a, b)
                }
            }
            (ir.Sub a, b) {
                GR32 a {
                    GR32  b => (mi.SUBrr32   a, b)
                    imm32 b => (mi.SUBri32   a, b) }
                imm32 a {
                    GR32 b => (mi.SUBrr32 (mi.MOVri32 a), b)
                }
                GR64 a {
                    imm32 b => (mi.SUBr64i32 a, b) }
                XMM  a {
                    imm_f64 b => (mi.SUBSDrr a, (mi.MOVSDrm64 b))
                    XMM    b => (mi.SUBSDrr a, b)
                }
                imm_f64 a {
                    XMM b => (mi.SUBSDrr (mi.MOVSDrm64 a), b)
                }
            }
            (ir.Mul a, b) {
                GR32 a {
                    GR32  b => (mi.IMULrr32  a, b)
                    imm32 b => (mi.IMULrri32 a, b) }
                GR64 a {
                    imm32 b => (mi.IMULrr64i32 a, b) }
                XMM a {
                    imm_f64 b => (mi.MULSDrr a, (mi.MOVSDrm64 b))
                    XMM    b => (mi.MULSDrr a, b)
                }
            }
            (ir.Div a, b) {
                XMM a {
                    imm_f64 b => (mi.DIVSDrr a, (mi.MOVSDrm64 b))
                    XMM    b => (mi.DIVSDrr a, b)
                }
                imm_f64 a {
                    XMM b => (mi.DIVSDrr (mi.MOVSDrm64 a), b)
                }
            }
            (ir.Load a) {
                (ir.FIAddr b) a {
                    f64mem b => (mi.MOVSDrm [BaseFi %rbp, b])
                    mem32  b => (mi.MOVrm32 [BaseFi %rbp, b])
                    mem64  b => (mi.MOVrm64 [BaseFi %rbp, b])
                }
                // a is pointer
                GR64  a => {
                    let a = self.run_on_node(tys, regs_info, heap, a);
                    let ty = tys.get_element_ty(a.ty, None).unwrap();
                    let mem = heap.alloc(DAGNode::new_mem(MemNodeKind::Base, vec![a]));
                    match ty {
                        Type::Int32 => heap.alloc(DAGNode::new(NodeKind::MI(MINodeKind::MOVrm32),
                                vec![mem], Type::Int32)),
                        Type::Int64 | Type::Pointer(_) | Type::Array(_) =>
                            heap.alloc(DAGNode::new(NodeKind::MI(MINodeKind::MOVrm64),
                                vec![mem], node.ty)),
                        Type::F64 => heap.alloc(DAGNode::new(NodeKind::MI(MINodeKind::MOVSDrm),
                                vec![mem], Type::F64)),
                        _ => unimplemented!()
                    }
                }
            }
            (ir.Store a, b) {
                (ir.FIAddr c) a {
                    f64mem c {
                        imm_f64 b => (mi.MOVSDmr [BaseFi %rbp, c], (mi.MOVSDrm64 b))
                    }
                    mem32  c {
                        GR32  b => (mi.MOVmr32 [BaseFi %rbp, c], b)
                        imm32 b => (mi.MOVmi32 [BaseFi %rbp, c], b) } }
                GR64   a {
                    imm32 b => (mi.MOVmi32 [Base a], b)
                    GR32  b => (mi.MOVmr32 [Base a], b)
                    GR64  b => (mi.MOVmr64 [Base a], b)
                    imm_f64 b => (mi.MOVSDmr [Base a], (mi.MOVSDrm64 b))
                    XMM    b => (mi.MOVSDmr [Base a], b)
                }
            }
            (ir.FIAddr a) {
                mem a => (mi.LEAr64m [BaseFi %rbp, a])
            }
            (ir.CopyFromReg a) => (mi.Copy a)
        );

        self.selected.insert(node, selected);

        if let Some(next) = node.next {
            selected.next = Some(self.run_on_node(tys, regs_info, heap, next));
        }

        selected
    }
}
