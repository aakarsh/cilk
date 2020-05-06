use super::super::dag::mc_convert::mov_rx;
use super::{
    function::MachineFunction,
    // basic_block::{MachineBasicBlock, MachineBasicBlockId},
    inst::MachineOpcode,
    module::MachineModule,
};
use crate::ir::types::Types;

pub struct ReplaceCopyWithProperMInst {}

impl ReplaceCopyWithProperMInst {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, f) in &mut module.functions {
            self.run_on_function(&module.types, f);
        }
    }

    pub fn run_on_function(&mut self, tys: &Types, f: &mut MachineFunction) {
        for (_, bb) in f.body.basic_blocks.id_and_block() {
            for inst_id in &*bb.iseq_ref() {
                let inst = &mut f.body.inst_arena[*inst_id];

                if inst.opcode != MachineOpcode::Copy {
                    continue;
                }

                let mov = mov_rx(tys, &f.regs_info, &inst.operand[0]).unwrap();
                inst.opcode = mov;
            }
        }
    }
}
