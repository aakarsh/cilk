use super::{basic_block::*, function::*, instr::*, module::*};
use rustc_hash::FxHashMap;

pub struct PhiElimination {}

impl PhiElimination {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, f) in &mut module.functions {
            self.run_on_function(f);
        }
    }

    pub fn run_on_function(&mut self, f: &mut MachineFunction) {
        // TODO: Rewrite with MachineInstr Builder
        let phi_pos = self.collect_phi(f);
        for (bb_id, bb) in &f.basic_blocks {
            let (phi, pos) = if let Some(phi_pos) = phi_pos.get(&bb_id) {
                (f.instr_arena[bb.iseq_ref()[*phi_pos]].clone(), *phi_pos)
            } else {
                continue;
            };
            let mut i = 0;
            while i < phi.operand.len() {
                let val = &phi.operand[i + 0];
                let bb = match phi.operand[i + 1] {
                    MachineOperand::Branch(bb) => bb,
                    _ => unreachable!(),
                };

                let mut iseq = f.basic_blocks[bb].iseq_ref_mut();
                for k in 0..iseq.len() {
                    if !f.instr_arena[iseq[iseq.len() - 1 - k]]
                        .opcode
                        .is_terminator()
                    {
                        let mut copy = MachineInstr::new(
                            MachineOpcode::CopyToReg,
                            vec![val.clone()],
                            phi.ty.clone(),
                        );
                        copy.reg = phi.reg.clone();
                        let id = f.instr_arena.alloc(copy);
                        let pt = iseq.len() - k;
                        iseq.insert(pt, id);
                        break;
                    }
                }

                i += 2;
            }

            bb.iseq_ref_mut().remove(pos);
        }
    }

    fn collect_phi(&mut self, f: &MachineFunction) -> FxHashMap<MachineBasicBlockId, usize> {
        let mut phi_pos = FxHashMap::default();
        for (bb_id, bb) in &f.basic_blocks {
            for (i, instr) in bb.iseq_ref().iter().enumerate() {
                if f.instr_arena[*instr].opcode == MachineOpcode::Phi {
                    phi_pos.insert(bb_id, i);
                }
            }
        }
        phi_pos
    }
}
