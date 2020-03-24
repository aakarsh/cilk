use super::super::dag::mc_convert::mov_rx;
use super::{
    function::MachineFunction,
    // basic_block::{MachineBasicBlock, MachineBasicBlockId},
    instr::MachineOpcode,
    module::MachineModule,
};

pub struct CopyToMachineInst {}

impl CopyToMachineInst {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, f) in &mut module.functions {
            self.run_on_function(f);
        }
    }

    pub fn run_on_function(&mut self, f: &mut MachineFunction) {
        for bb_id in &f.basic_blocks {
            let bb = &f.basic_block_arena[*bb_id];
            for inst_id in &*bb.iseq_ref() {
                let inst = &mut f.instr_arena[*inst_id];

                if inst.opcode != MachineOpcode::Copy {
                    continue;
                }

                let mov = mov_rx(&inst.operand[0]).unwrap();
                inst.opcode = mov;
            }
        }
    }
}
