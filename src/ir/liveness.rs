use crate::ir::{basic_block::*, function::*, module::*, opcode::*, types::*};

pub struct IRLivenessAnalyzer<'a> {
    module: &'a Module,
}

impl<'a> IRLivenessAnalyzer<'a> {
    pub fn new(module: &'a Module) -> Self {
        Self { module }
    }

    pub fn analyze(&mut self) {
        for (_, f) in &self.module.functions {
            self.set_def(f);
            self.visit(&f);
        }
    }

    pub fn set_def(&mut self, f: &Function) {
        for (_, bb) in &f.basic_block_arena {
            let def = &mut bb.liveness.borrow_mut().def;

            for instr_val in &*bb.iseq.borrow() {
                let instr_id = instr_val.get_instr_id().unwrap();
                let instr = &f.instr_table[instr_id];

                match instr.opcode {
                    Opcode::Add(_, _)
                    | Opcode::Mul(_, _)
                    | Opcode::Rem(_, _)
                    | Opcode::Sub(_, _)
                    | Opcode::Alloca(_)
                    | Opcode::ICmp(_, _, _)
                    | Opcode::Load(_)
                    | Opcode::Phi(_)
                    | Opcode::GetElementPtr(_, _) => {
                        def.insert(instr_id);
                    }
                    Opcode::Call(f, _) => {
                        if f.get_type(&self.module).get_function_ty().unwrap().ret_ty != Type::Void
                        {
                            def.insert(instr_id);
                        }
                    }
                    Opcode::Store(_, _dst) => {
                        // some_then!(id, dst.get_instr_id(), {
                        //     // def.insert(f.instr_table[id].vreg);
                        // });
                    }
                    Opcode::Br(_) | Opcode::CondBr(_, _, _) | Opcode::Ret(_) => {}
                }
            }
        }
    }

    pub fn visit(&mut self, f: &Function) {
        for (bb_id, bb) in &f.basic_block_arena {
            for instr_val in &*bb.iseq.borrow() {
                let instr = &f.instr_table[instr_val.get_instr_id().unwrap()];

                match &instr.opcode {
                    Opcode::Call(func, args) => {
                        some_then!(id, func.get_instr_id(), self.propagate(f, bb_id, id));
                        for arg in args {
                            some_then!(id, arg.get_instr_id(), self.propagate(f, bb_id, id));
                        }
                    }
                    Opcode::CondBr(v, _, _) | Opcode::Ret(v) | Opcode::Load(v) => {
                        some_then!(id, v.get_instr_id(), self.propagate(f, bb_id, id));
                    }
                    Opcode::Phi(vals) => {
                        for (val, _) in vals {
                            some_then!(id, val.get_instr_id(), self.propagate(f, bb_id, id));
                        }
                    }
                    Opcode::GetElementPtr(base, idx) => {
                        some_then!(id, base.get_instr_id(), self.propagate(f, bb_id, id));
                        for idx in idx {
                            some_then!(id, idx.get_instr_id(), self.propagate(f, bb_id, id));
                        }
                    }
                    Opcode::Store(v1, v2)
                    | Opcode::ICmp(_, v1, v2)
                    | Opcode::Add(v1, v2)
                    | Opcode::Sub(v1, v2) => {
                        some_then!(id, v1.get_instr_id(), self.propagate(f, bb_id, id));
                        some_then!(id, v2.get_instr_id(), self.propagate(f, bb_id, id));
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn propagate(&mut self, f: &Function, bb_id: BasicBlockId, instr_id: InstructionId) {
        let bb = &f.basic_block_arena[bb_id];

        {
            let mut bb_liveness = bb.liveness.borrow_mut();

            if bb_liveness.def.contains(&instr_id) {
                return;
            }

            if !bb_liveness.live_in.insert(instr_id) {
                // live_in already had the value instr_id
                return;
            }
        }

        for pred_id in &bb.pred {
            let pred = &f.basic_block_arena[*pred_id];
            if pred.liveness.borrow_mut().live_out.insert(instr_id) {
                // live_out didn't have the value instr_id
                self.propagate(f, *pred_id, instr_id);
            }
        }
    }
}
