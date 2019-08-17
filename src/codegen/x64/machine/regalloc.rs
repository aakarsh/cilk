use super::super::frame_object::*;
use super::{builder::*, function::*, instr::*, liveness::*, module::*};
use crate::ir::types::*;
use rustc_hash::{FxHashMap, FxHashSet};

pub struct RegisterAllocator {}

impl RegisterAllocator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, func) in &mut module.functions {
            self.run_on_function(func);
        }
    }

    pub fn run_on_function(&mut self, cur_func: &mut MachineFunction) {
        let mut matrix = LivenessAnalysis::new().analyze_function(cur_func);

        let vregs = matrix.map.iter().map(|(vreg, _)| *vreg).collect::<Vec<_>>();
        for vreg in vregs {
            for reg in 0..10 {
                if !matrix.interferes(vreg, reg) {
                    break;
                }
            }
        }

        for (_, x) in matrix.map {
            println!("{:?}", x)
        }

        // self.assign(cur_func, &matrix);
        // self.collect_regs(cur_func);
        // self.scan(cur_func);
    }

    // fn assign(&mut self, cur_func: &mut MachineFunction) {}
}

pub struct PhysicalRegisterAllocator {}

impl PhysicalRegisterAllocator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, func) in &mut module.functions {
            self.run_on_function(func);
        }
    }

    pub fn run_on_function(&mut self, cur_func: &mut MachineFunction) {
        self.collect_regs(cur_func);
        self.scan(cur_func);
    }

    fn insert_instr_to_save_reg(
        &mut self,
        cur_func: &mut MachineFunction,
        occupied: &mut FxHashSet<i32>,
        call_instr_id: MachineInstrId,
    ) {
        fn find_unused_slot(
            cur_func: &mut MachineFunction,
            occupied: &mut FxHashSet</*idx=*/ i32>,
            r: &MachineRegister,
        ) -> FrameIndexInfo {
            for slot in &*cur_func.local_mgr.locals {
                if occupied.contains(&slot.idx) {
                    continue;
                }
                if r.info_ref().ty == slot.ty {
                    occupied.insert(slot.idx);
                    return slot.clone();
                }
            }
            let slot = cur_func.local_mgr.alloc(&r.info_ref().ty);
            occupied.insert(slot.idx);
            slot
        }

        let call_instr_vreg = cur_func.instr_arena[call_instr_id].get_vreg();
        let mut regs_to_save = vec![];

        // TODO
        for (_, i) in &cur_func.instr_arena {
            if i.def[0].info_ref().reg.is_none() {
                continue;
            }
            let bgn = i.get_vreg();
            let end = match i.get_last_use() {
                Some(last_use) => cur_func.instr_arena[last_use].get_vreg(),
                None => continue,
            };
            if bgn < call_instr_vreg && call_instr_vreg < end {
                regs_to_save.push(i.def[0].clone());
            }
        }

        when_debug!(println!("SAVED REG: {:?}", regs_to_save));

        let mut slots_to_save_regs = vec![];
        for r in &regs_to_save {
            slots_to_save_regs.push(find_unused_slot(cur_func, occupied, r));
        }

        println!("NEW SLOT: {:?}", slots_to_save_regs);

        for (frinfo, reg) in slots_to_save_regs.into_iter().zip(regs_to_save) {
            let store_instr_id = cur_func.instr_arena.alloc(MachineInstr::new(
                &cur_func.vreg_gen,
                MachineOpcode::Store,
                vec![
                    MachineOperand::FrameIndex(frinfo.clone()),
                    MachineOperand::Register(reg.clone()),
                ],
                Type::Void,
            ));

            let load_instr_id = cur_func.instr_arena.alloc(MachineInstr::new_with_def_reg(
                MachineOpcode::Load,
                vec![MachineOperand::FrameIndex(frinfo)],
                reg.info_ref().ty.clone(),
                vec![reg.clone()],
            ));

            let mut builder = Builder::new(cur_func);

            builder.set_insert_point_before_instr(call_instr_id);
            builder.insert_instr_id(store_instr_id);

            builder.set_insert_point_after_instr(call_instr_id);
            builder.insert_instr_id(load_instr_id);
        }
    }

    fn scan(&mut self, cur_func: &mut MachineFunction) {
        let mut used = FxHashMap::default();
        let mut call_instr_id = vec![];

        for bb_id in &cur_func.basic_blocks {
            let bb = &cur_func.basic_block_arena[*bb_id];
            for instr_id in bb.iseq_ref().iter() {
                self.scan_on_instr(cur_func, &mut used, *instr_id);

                let instr = &cur_func.instr_arena[*instr_id];
                if instr.opcode == MachineOpcode::Call {
                    call_instr_id.push(*instr_id)
                }
            }
        }

        let occupied = cur_func
            .local_mgr
            .locals
            .iter()
            .map(|l| l.idx)
            .collect::<FxHashSet<_>>();
        for instr_id in call_instr_id {
            self.insert_instr_to_save_reg(cur_func, &mut occupied.clone(), instr_id);
        }
    }

    fn scan_on_instr(
        &mut self,
        cur_func: &MachineFunction,
        used: &mut FxHashMap<usize, MachineInstrId>,
        instr_id: MachineInstrId,
    ) {
        // TODO: Refactor

        let instr = &cur_func.instr_arena[instr_id];
        let num_reg = 4;

        if instr.def.len() == 0 {
            return;
        }
        if instr.def[0].info_ref().last_use.is_none() {
            return;
        }

        let mut found = false;

        for i in 0..num_reg - 1 {
            if used.contains_key(&i) {
                let target_last_use_id = cur_func.instr_arena[*used.get(&i).unwrap()]
                    .get_last_use()
                    .unwrap();
                let target_last_use = cur_func.instr_arena[target_last_use_id].get_vreg();
                if instr.get_vreg() < target_last_use {
                    continue;
                }
            }

            instr.set_phy_reg(i, false);
            used.insert(i, instr_id);
            found = true;
            break;
        }

        if found {
            return;
        }

        used.insert(num_reg - 1, instr_id);

        let mut k = 0;
        for i in 1..num_reg {
            let l1 = cur_func.instr_arena[*used.get(&k).unwrap()]
                .get_last_use()
                .unwrap();
            let l2 = cur_func.instr_arena[*used.get(&i).unwrap()]
                .get_last_use()
                .unwrap();
            if l1 < l2 {
                k = i;
            }
        }

        instr.set_phy_reg(k, false);
        cur_func.instr_arena[*used.get(&k).unwrap()].set_phy_reg(num_reg - 1, true);

        *used.get_mut(&k).unwrap() = instr_id;
    }

    fn collect_regs(&mut self, cur_func: &MachineFunction) {
        for bb_id in &cur_func.basic_blocks {
            let bb = &cur_func.basic_block_arena[*bb_id];
            let mut last_instr = None;

            for instr_id in &*bb.iseq_ref() {
                self.collect_regs_on_instr(cur_func, *instr_id);
                last_instr = Some(*instr_id);
            }

            for out in &bb.liveness.borrow().live_out {
                out.set_last_use(last_instr);
            }
        }
    }

    fn collect_regs_on_instr(&mut self, cur_func: &MachineFunction, instr_id: MachineInstrId) {
        let instr = &cur_func.instr_arena[instr_id];
        for operand in &instr.operand {
            match_then!(
                MachineOperand::Register(reg),
                operand,
                reg.set_last_use(Some(instr_id))
            );
        }
    }
}
