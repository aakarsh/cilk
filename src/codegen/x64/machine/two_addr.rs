use super::{builder::*, function::*, inst::*, module::*};
use std::mem;

pub struct TwoAddressConverter {}

impl TwoAddressConverter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, f) in &mut module.functions {
            self.run_on_function(f);
        }
    }

    pub fn run_on_function(&mut self, f: &mut MachineFunction) {
        if f.internal {
            return;
        }

        let mut tied = vec![];

        for (_, bb) in f.body.basic_blocks.id_and_block() {
            for inst_id in &*bb.iseq_ref() {
                let inst = &mut f.body.inst_arena[*inst_id];

                // No tied values then skip
                if inst.tie.len() == 0 {
                    continue;
                }

                for (def, use_) in &mut inst.tie {
                    tied.push((*inst_id, def.clone(), use_.clone()));

                    // Tied registers are assigned to one register (def register).
                    // e.g.
                    //   before: v1 = add v2, 1 (tied: v1 and v2)
                    //   after:  v1 = add v1, 1
                    *inst
                        .operand
                        .iter_mut()
                        .find(|op| op.as_register() == use_)
                        .unwrap() = MachineOperand::Register(def.clone());
                }

                // Delete the map of all the tied registers because they are assigned to one
                // register so that no registers tied.
                inst.tie.clear();
            }
        }

        for (inst_id, def, use_) in tied {
            let inst_bb = f.body.inst_arena[inst_id].parent;

            // before: v1 = add v1, 1
            // after:  v1 = copy v2
            //         v1 = add v1, 1

            let old_inst = mem::replace(
                &mut f.body.inst_arena[inst_id],
                MachineInst::new_simple(
                    MachineOpcode::Copy,
                    vec![MachineOperand::Register(use_)],
                    inst_bb,
                )
                .with_def(vec![def]),
            );

            let inst = f.body.inst_arena.alloc(old_inst);

            let mut builder = Builder::new(f);
            builder.set_insert_point_after_inst(inst_id).unwrap();
            builder.insert(inst);
        }
    }
}
