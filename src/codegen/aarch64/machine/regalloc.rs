use crate::codegen::arch::{
    frame_object::FrameIndexInfo,
    machine::inst::{MachineMemOperand, MachineOpcode},
    machine::register::*,
};
pub use crate::codegen::common::machine::regalloc::*;
use crate::codegen::common::machine::{
    basic_block::MachineBasicBlockId,
    function::MachineFunction,
    inst::{MachineInst, MachineInstId, MachineOperand, RegisterOperand},
};

impl RegisterAllocator {
    pub fn store_and_load_for_reg_preservation(
        &mut self,
        f: &mut MachineFunction,
        reg: RegisterId,
        frinfo: FrameIndexInfo,
        parent: MachineBasicBlockId,
    ) -> (MachineInstId, MachineInstId) {
        let src = MachineOperand::Register(RegisterOperand::new(reg));
        let x29 = RegisterOperand::new(f.regs_info.get_phys_reg(GR64::X29));
        let store_inst_id = f.alloc_inst(MachineInst::new(
            &f.regs_info,
            MachineOpcode::STR,
            vec![
                src,
                MachineOperand::Mem(MachineMemOperand::RegFi(x29, frinfo)),
            ],
            None,
            parent,
        ));

        let load_inst_id = f.alloc_inst(
            MachineInst::new_simple(
                MachineOpcode::LDR32,
                vec![MachineOperand::Mem(MachineMemOperand::RegFi(x29, frinfo))],
                parent,
            )
            .with_def(vec![RegisterOperand::new(reg)]),
        );

        (store_inst_id, load_inst_id)
    }

    pub fn get_regs_used_to_preserve(
        &mut self,
        func: &mut MachineFunction,
        store_id: MachineInstId,
        load_id: MachineInstId,
    ) -> Vec<(RegisterId, RegisterId)> {
        let store = &func.body.inst_arena[store_id];
        let r1 = store.operand[0].as_register().id;

        let load = &func.body.inst_arena[load_id];
        let r2 = load.def[0].id;

        vec![(r1, r2)]
    }
}
