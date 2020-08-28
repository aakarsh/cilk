use crate::{
    analysis::{
        dom_tree::{DominatorTree, DominatorTreeConstructor},
        loops::{Loop, Loops, LoopsConstructor},
    },
    ir::{
        basic_block::{BasicBlock, BasicBlockId},
        builder::{Builder, FunctionEntity},
        function::Function,
        module::Module,
        opcode::{Instruction, Opcode, Operand},
        value::*,
    },
};
use id_arena::Id;
use rustc_hash::FxHashMap;

pub struct LoopInvariantCodeMotion {}

struct LoopInvariantCodeMotionOnFunction<'a> {
    func: &'a mut Function,
}

impl LoopInvariantCodeMotion {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut Module) {
        for (_, func) in &mut module.functions {
            if func.is_internal {
                continue;
            }
            LoopInvariantCodeMotionOnFunction::new(func).run();
        }
    }
}

impl<'a> LoopInvariantCodeMotionOnFunction<'a> {
    pub fn new(func: &'a mut Function) -> Self {
        Self { func }
    }

    pub fn run(&mut self) {
        let dom_tree = DominatorTreeConstructor::new(&self.func.basic_blocks).construct();
        let loops = LoopsConstructor::new(&dom_tree, &self.func.basic_blocks).analyze();

        let mut count = 0;
        let pre_headers = self.insert_pre_headers(&loops);

        // TODO: VERY INEFFICIENT!
        let dom_tree = DominatorTreeConstructor::new(&self.func.basic_blocks).construct();
        let loops = LoopsConstructor::new(&dom_tree, &self.func.basic_blocks).analyze();

        for (id, loop_) in &loops.arena {
            let mut insts_to_hoist = vec![];
            for &bb_id in &loop_.set {
                let bb = &self.func.basic_blocks.arena[bb_id];
                for inst_id in bb.iseq.borrow().iter().map(|v| v.as_instruction().id) {
                    let inst = &self.func.inst_table[inst_id];
                    if inst.opcode.access_memory() || inst.opcode == Opcode::Call {
                        continue;
                    }
                    let invariant = inst.operands.iter().all(|operand| match operand {
                        Operand::Value(v) => match v {
                            Value::Instruction(InstructionValue { id, .. }) => {
                                let inst = &self.func.inst_table[*id];
                                !loop_.contains(&inst.parent)
                            }
                            _ => true,
                        },
                        _ => false,
                    });
                    if invariant {
                        count += 1;
                        insts_to_hoist.push(inst_id);
                    }
                }
            }

            for inst_id in insts_to_hoist {
                let pre_header = pre_headers[&loop_.header];
                let val = self.func.remove_inst_from_block(inst_id);
                let inst = &mut self.func.inst_table[inst_id];
                inst.parent = pre_header;
                let mut builder = Builder::new(FunctionEntity(self.func));
                builder.set_insert_point_at(0, pre_header);
                builder.insert(val);
            }
        }

        debug!(println!("LICM: {} invariants hoisted", count));
    }

    fn insert_pre_headers(
        &mut self,
        loops: &Loops<BasicBlock>,
    ) -> FxHashMap<BasicBlockId, BasicBlockId> {
        let mut pre_headers = FxHashMap::default();

        for (id, loop_) in &loops.arena {
            let pre_header = self.insert_pre_header(loop_);
            pre_headers.insert(loop_.header, pre_header);
        }

        pre_headers
    }

    fn insert_pre_header(&mut self, loop_: &Loop<BasicBlock>) -> BasicBlockId {
        let pre_header = self.func.append_basic_block_before(loop_.header);

        let mut preds = self.func.basic_blocks.arena[loop_.header].pred.clone();
        preds.retain(|p| !loop_.contains(p));
        let preds_not_in_loop = preds;

        let header_preds = &mut self.func.basic_blocks.arena[loop_.header].pred;
        header_preds.retain(|p| !preds_not_in_loop.contains(p)); // retain preds in loop
        header_preds.insert(pre_header);

        for &pred in &preds_not_in_loop {
            let block = &mut self.func.basic_blocks.arena[pred];
            block.succ.retain(|&s| s != loop_.header);
            block.succ.insert(pre_header);
            for &id in block.iseq_ref().iter().rev() {
                let id = id.as_instruction().id;
                if !self.func.inst_table[id].opcode.is_terminator() {
                    break;
                }
                Instruction::replace_operand(
                    &mut self.func.inst_table,
                    id,
                    &Operand::BasicBlock(loop_.header),
                    Operand::BasicBlock(pre_header),
                );
            }
        }

        self.func.basic_blocks.arena[pre_header]
            .pred
            .extend(preds_not_in_loop);

        let mut builder = Builder::new(FunctionEntity(self.func));
        builder.set_insert_point(pre_header);
        builder.build_br(loop_.header);

        pre_header
    }
}
