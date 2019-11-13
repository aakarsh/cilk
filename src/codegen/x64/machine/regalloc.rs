use super::super::frame_object::*;
use super::super::register::*;
use super::{builder::*, function::*, instr::*, liveness::*, module::*};
use crate::ir::types::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

pub struct RegisterAllocator {
    queue: VecDeque<VirtReg>,
}

impl RegisterAllocator {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, func) in &mut module.functions {
            self.run_on_function(func);
        }
    }

    pub fn run_on_function(&mut self, cur_func: &mut MachineFunction) {
        self.preserve_vreg_uses_across_call(cur_func);

        let mut matrix = LivenessAnalysis::new().analyze_function(cur_func);

        self.queue = matrix.collect_vregs().into_iter().collect();

        while let Some(vreg) = self.queue.pop_front() {
            // TODO: 0..8 ???
            let mut allocated = false;
            for reg in 0..8 {
                let reg = get_general_reg(reg).unwrap();

                if matrix.interferes(vreg, reg) {
                    continue;
                }

                matrix.assign_reg(vreg, reg);

                allocated = true;
                break;
            }

            assert!(allocated);
        }

        debug!(for (_, x) in &matrix.vreg_interval {
            println!("{:?}", x)
        });

        self.rewrite_vregs(cur_func, &matrix);
    }

    fn rewrite_vregs(&mut self, cur_func: &mut MachineFunction, matrix: &LiveRegMatrix) {
        for bb_id in &cur_func.basic_blocks {
            let bb = &cur_func.basic_block_arena[*bb_id];
            for instr_id in &*bb.iseq_ref() {
                let instr = &cur_func.instr_arena[*instr_id];
                for def in &instr.def {
                    if def.get_reg().is_some() {
                        continue;
                    }

                    let reg = matrix
                        .get_vreg_interval(def.get_vreg())
                        .unwrap()
                        .reg
                        .unwrap();
                    // if phys reg is not assigned
                    if def.get_reg().is_none() {
                        def.set_phy_reg(reg, false);
                    }
                }
            }
        }
    }

    fn insert_instr_to_save_reg(
        &mut self,
        cur_func: &mut MachineFunction,
        occupied: &mut FxHashSet<i32>,
        call_instr_id: MachineInstrId,
    ) {
        // TODO: Refine code. It's hard to understand.
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

        let matrix = LivenessAnalysis::new().analyze_function(cur_func);

        let call_instr_pp = matrix.get_program_point_of_instr(call_instr_id).unwrap();
        let mut regs_to_save = FxHashSet::default();

        // TODO: It's expensive to check all the elements in ``instr_arena``
        for (_, instr) in &cur_func.instr_arena.arena {
            if instr.def.len() == 0 {
                continue;
            }

            if matrix.interferes_with_range(
                instr.get_vreg(),
                LiveRange::new(vec![LiveSegment::new(
                    call_instr_pp,
                    call_instr_pp.next_idx(),
                )]),
            ) {
                regs_to_save.insert(instr.def[0].clone());
            }
        }

        debug!(println!("REG TO SAVE: {:?}", regs_to_save));

        let mut slots_to_save_regs = vec![];
        for r in &regs_to_save {
            slots_to_save_regs.push(find_unused_slot(cur_func, occupied, r));
        }

        debug!(println!("NEW SLOTS: {:?}", slots_to_save_regs));

        let call_instr_parent = cur_func.instr_arena[call_instr_id].parent;

        for (frinfo, reg) in slots_to_save_regs.into_iter().zip(regs_to_save.iter()) {
            let store_instr_id = cur_func.instr_arena.alloc(MachineInstr::new(
                &cur_func.vreg_gen,
                MachineOpcode::Store,
                vec![
                    MachineOperand::FrameIndex(frinfo.clone()),
                    MachineOperand::Register(reg.clone()),
                ],
                &Type::Void,
                call_instr_parent,
            ));
            cur_func.instr_arena[store_instr_id].add_use(store_instr_id);
            cur_func.instr_arena[store_instr_id].add_def(store_instr_id);

            let load_instr_id = cur_func.instr_arena.alloc(
                MachineInstr::new_simple(
                    MachineOpcode::Load,
                    vec![MachineOperand::FrameIndex(frinfo)],
                    call_instr_parent,
                )
                .with_def(vec![reg.clone()]),
            );
            cur_func.instr_arena[load_instr_id].add_def(load_instr_id);

            let mut builder = Builder::new(cur_func);

            builder.set_insert_point_before_instr(call_instr_id);
            builder.insert_instr_id(store_instr_id);

            builder.set_insert_point_after_instr(call_instr_id);
            builder.insert_instr_id(load_instr_id);
        }
    }

    fn preserve_vreg_uses_across_call(&mut self, cur_func: &mut MachineFunction) {
        let mut call_instr_id = vec![];

        for bb_id in &cur_func.basic_blocks {
            let bb = &cur_func.basic_block_arena[*bb_id];
            for instr_id in bb.iseq_ref().iter() {
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

        let call_instr_vreg = cur_func.instr_arena[call_instr_id].get_vreg().get();
        let mut regs_to_save = vec![];

        // TODO
        for (_, i) in &cur_func.instr_arena.arena {
            if i.def[0].info_ref().reg.is_none() {
                continue;
            }
            let bgn = i.get_vreg().get();
            let end = match i.get_last_use() {
                Some(last_use) => cur_func.instr_arena[last_use].get_vreg(),
                None => continue,
            }
            .get();
            if bgn < call_instr_vreg && call_instr_vreg < end {
                regs_to_save.push(i.def[0].clone());
            }
        }

        debug!(println!("SAVED REG: {:?}", regs_to_save));

        let mut slots_to_save_regs = vec![];
        for r in &regs_to_save {
            slots_to_save_regs.push(find_unused_slot(cur_func, occupied, r));
        }

        println!("NEW SLOT: {:?}", slots_to_save_regs);

        for (_frinfo, _reg) in slots_to_save_regs.into_iter().zip(regs_to_save) {
            // let store_instr_id = cur_func.instr_arena.alloc(MachineInstr::new(
            //     &cur_func.vreg_gen,
            //     MachineOpcode::Store,
            //     vec![
            //         MachineOperand::FrameIndex(frinfo.clone()),
            //         MachineOperand::Register(reg.clone()),
            //     ],
            //     Type::Void,
            // ));
            //
            // let load_instr_id = cur_func.instr_arena.alloc(MachineInstr::new_with_def_reg(
            //     MachineOpcode::Load,
            //     vec![MachineOperand::FrameIndex(frinfo)],
            //     reg.info_ref().ty.clone(),
            //     vec![reg.clone()],
            // ));
            //
            // let mut builder = Builder::new(cur_func);
            //
            // builder.set_insert_point_before_instr(call_instr_id);
            // builder.insert_instr_id(store_instr_id);
            //
            // builder.set_insert_point_after_instr(call_instr_id);
            // builder.insert_instr_id(load_instr_id);
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
                let target_last_use = cur_func.instr_arena[target_last_use_id].get_vreg().get();
                if instr.get_vreg().get() < target_last_use {
                    continue;
                }
            }

            instr.set_phy_reg(PhysReg(i), false);
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

        instr.set_phy_reg(PhysReg(k), false);
        cur_func.instr_arena[*used.get(&k).unwrap()].set_phy_reg(PhysReg(num_reg - 1), true);

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
