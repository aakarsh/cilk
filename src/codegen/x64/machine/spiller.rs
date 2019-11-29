use super::super::{frame_object::FrameIndexInfo, register::VirtReg};
use super::{
    builder::BuilderWithLiveInfoEdit,
    function::MachineFunction,
    instr::{MachineInstr, MachineOpcode, MachineOperand, MachineRegister},
    liveness::{LiveRange, LiveRegMatrix, LiveSegment, ProgramPoint},
};

pub struct Spiller<'a> {
    func: &'a mut MachineFunction,
    matrix: &'a mut LiveRegMatrix,
}

impl<'a> Spiller<'a> {
    pub fn new(func: &'a mut MachineFunction, matrix: &'a mut LiveRegMatrix) -> Self {
        Self { func, matrix }
    }

    pub fn insert_evict(&mut self, r: MachineRegister, slot: &FrameIndexInfo) {
        let r_def_list = r.info_ref().def_list.clone();
        assert_eq!(r_def_list.len(), 1); // TODO: r is maybe tied register if this assertion fails

        for def_id in r_def_list {
            let def_instr = &mut self.func.instr_arena[def_id];
            let store = MachineInstr::new_simple(
                MachineOpcode::Store,
                vec![
                    MachineOperand::FrameIndex(slot.clone()),
                    MachineOperand::Register(r.clone()),
                ],
                def_instr.parent,
            );
            let store_id = self.func.instr_arena.alloc(store);

            let mut builder = BuilderWithLiveInfoEdit::new(self.matrix, self.func);
            builder.set_insert_point_after_instr(def_id).unwrap();
            builder.insert_instr_id(store_id);
        }
    }

    pub fn insert_reload(
        &mut self,
        r: MachineRegister,
        slot: &FrameIndexInfo,
    ) -> Vec<MachineRegister> {
        let mut new_regs = vec![];
        let r_ty = r.info_ref().ty.clone();

        for use_id in &r.info_ref().use_list {
            let new_r = self
                .func
                .vreg_gen
                .gen_vreg(r_ty.clone())
                .into_machine_register();
            new_regs.push(new_r.clone());
            self.matrix
                .vreg_entity
                .insert(new_r.get_vreg(), new_r.clone());

            let use_instr = &mut self.func.instr_arena[*use_id];
            use_instr.replace_operand_reg(&r, &new_r);
            new_r.add_use(*use_id);

            let load = MachineInstr::new_with_def_reg(
                MachineOpcode::Load,
                vec![MachineOperand::FrameIndex(slot.clone())],
                vec![new_r.clone()],
                use_instr.parent,
            );
            let load_id = self.func.instr_arena.alloc(load);

            // self.matrix.add_live_interval(
            //     new_r.get_vreg(),
            //     LiveRange::new(vec![LiveSegment::new(start, end)]),
            // );

            let mut builder = BuilderWithLiveInfoEdit::new(self.matrix, self.func);
            builder.set_insert_point_before_instr(*use_id);
            builder.insert_instr_id(load_id);
        }

        r.info_ref_mut().use_list.clear();

        new_regs
    }

    pub fn spill(&mut self, vreg: VirtReg) -> Vec<MachineRegister> {
        let r = self.matrix.get_vreg_entity(vreg).unwrap().clone();
        let slot = self.func.local_mgr.alloc(&r.info_ref().ty); // TODO

        let new_regs = self.insert_reload(r.clone(), &slot);
        self.insert_evict(r, &slot);

        new_regs
    }
}
