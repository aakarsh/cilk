use super::super::register::*;
use super::{basic_block::*, function::*, instr::*, module::*};
use rustc_hash::FxHashMap;
use std::cmp::Ordering;

const IDX_STEP: usize = 16;

#[derive(Debug, Clone)]
pub struct LiveRegMatrix {
    pub id2pp: FxHashMap<MachineInstrId, ProgramPoint>,
    pub vreg_interval: FxHashMap<VirtReg, LiveInterval>,
    pub reg_range: FxHashMap<PhysReg, LiveRange>,
}

#[derive(Debug, Clone)]
pub struct LiveInterval {
    pub vreg: VirtReg,
    pub reg: Option<PhysReg>,
    pub range: LiveRange,
}

#[derive(Debug, Clone)]
pub struct LiveRange {
    pub segments: Vec<LiveSegment>,
}

#[derive(Debug, Clone)]
pub struct LiveSegment {
    start: ProgramPoint,
    end: ProgramPoint,
}

#[derive(Debug, Clone, Copy)]
pub struct ProgramPoint {
    bb: usize,
    idx: usize,
}

impl LiveRegMatrix {
    pub fn new(
        id2pp: FxHashMap<MachineInstrId, ProgramPoint>,
        vreg_interval: FxHashMap<VirtReg, LiveInterval>,
        reg_range: FxHashMap<PhysReg, LiveRange>,
    ) -> Self {
        Self {
            id2pp,
            vreg_interval,
            reg_range,
        }
    }

    pub fn get_program_point_of_instr(&self, id: MachineInstrId) -> Option<ProgramPoint> {
        self.id2pp.get(&id).map(|x| *x)
    }

    /// Return false if it's legal to allocate reg for vreg
    pub fn interferes(&self, vreg: VirtReg, reg: PhysReg) -> bool {
        if !self.reg_range.contains_key(&reg) {
            return false;
        }

        let r1 = self.reg_range.get(&reg).unwrap();
        let r2 = &self.vreg_interval.get(&vreg).unwrap().range;

        r1.interferes(r2)
    }

    pub fn interferes_with_range(&self, vreg: VirtReg, range: LiveRange) -> bool {
        match self.vreg_interval.get(&vreg) {
            Some(interval) => range.interferes(&interval.range),
            None => false,
        }
    }

    pub fn assign_reg(&mut self, vreg: VirtReg, reg: PhysReg) {
        // assign reg to vreg
        self.vreg_interval.get_mut(&vreg).unwrap().reg = Some(reg);

        let vreg_range = self.get_vreg_interval(vreg).unwrap().range.clone();
        self.get_or_create_reg_live_range(reg)
            .unite_range(vreg_range);
    }

    pub fn collect_vregs(&self) -> Vec<VirtReg> {
        self.vreg_interval
            .iter()
            .map(|(vreg, _)| *vreg)
            .collect::<Vec<_>>()
    }

    pub fn get_vreg_interval(&self, vreg: VirtReg) -> Option<&LiveInterval> {
        self.vreg_interval.get(&vreg)
    }

    pub fn get_or_create_reg_live_range(&mut self, reg: PhysReg) -> &mut LiveRange {
        self.reg_range.entry(reg).or_insert(LiveRange::new_empty())
    }
}

impl LiveInterval {
    pub fn new(vreg: VirtReg, range: LiveRange) -> Self {
        Self {
            vreg,
            range,
            reg: None,
        }
    }

    pub fn interferes(&self, other: &LiveInterval) -> bool {
        self.range.interferes(&other.range)
    }
}

impl LiveRange {
    pub fn new(segments: Vec<LiveSegment>) -> Self {
        Self { segments }
    }

    pub fn new_empty() -> Self {
        Self { segments: vec![] }
    }

    pub fn add_segment(&mut self, seg: LiveSegment) {
        self.segments.push(seg)
    }

    pub fn unite_range(&mut self, mut range: LiveRange) {
        self.segments.append(&mut range.segments)
    }

    pub fn interferes(&self, other: &LiveRange) -> bool {
        for seg1 in &self.segments {
            for seg2 in &other.segments {
                if seg1.interferes(seg2) {
                    return true;
                }
            }
        }
        false
    }
}

impl LiveSegment {
    pub fn new(start: ProgramPoint, end: ProgramPoint) -> Self {
        Self { start, end }
    }

    pub fn interferes(&self, seg: &LiveSegment) -> bool {
        self.start < seg.end && self.end > seg.start
    }
}

impl ProgramPoint {
    pub fn new(bb: usize, idx: usize) -> Self {
        Self { bb, idx }
    }

    pub fn next_idx(mut self) -> Self {
        self.idx += 1;
        self
    }
}

impl PartialOrd for ProgramPoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.bb < other.bb {
            return Some(Ordering::Less);
        }

        if self.bb > other.bb {
            return Some(Ordering::Greater);
        }

        Some(self.idx.cmp(&other.idx))
    }
}

impl PartialEq for ProgramPoint {
    fn eq(&self, other: &Self) -> bool {
        self.bb == other.bb && self.idx == other.idx
    }
}

pub struct LivenessAnalysis {}

impl LivenessAnalysis {
    pub fn new() -> Self {
        Self {}
    }

    pub fn analyze_module(&mut self, module: &MachineModule) {
        for (_, func) in &module.functions {
            self.analyze_function(func);
        }
    }

    pub fn analyze_function(&mut self, cur_func: &MachineFunction) -> LiveRegMatrix {
        self.clear_bb_liveness_info(cur_func);
        self.set_def(cur_func);
        self.visit(cur_func);
        self.construct_live_reg_matrix(cur_func)
    }

    fn clear_bb_liveness_info(&mut self, cur_func: &MachineFunction) {
        for (_, bb) in &cur_func.basic_block_arena {
            bb.liveness_ref_mut().clear();
        }
    }

    fn set_def(&mut self, cur_func: &MachineFunction) {
        for bb_id in &cur_func.basic_blocks {
            let bb = &cur_func.basic_block_arena[*bb_id];
            for instr_id in &*bb.iseq_ref() {
                self.set_def_on_instr(cur_func, bb, *instr_id);
            }
        }
    }

    fn set_def_on_instr(
        &mut self,
        cur_func: &MachineFunction,
        bb: &MachineBasicBlock,
        instr_id: MachineInstrId,
    ) {
        let instr = &cur_func.instr_arena[instr_id];

        if instr.def.len() > 0 {
            bb.liveness.borrow_mut().def.insert(instr.def[0].clone());
        }
    }

    fn visit(&mut self, cur_func: &MachineFunction) {
        for bb_id in &cur_func.basic_blocks {
            let bb = &cur_func.basic_block_arena[*bb_id];
            for instr_id in &*bb.iseq_ref() {
                self.visit_instr(cur_func, *bb_id, *instr_id);
            }
        }
    }

    fn visit_instr(
        &mut self,
        cur_func: &MachineFunction,
        bb: MachineBasicBlockId,
        instr_id: MachineInstrId,
    ) {
        let instr = &cur_func.instr_arena[instr_id];

        for (i, operand) in instr.operand.iter().enumerate() {
            if let MachineOperand::Register(reg) = operand {
                self.propagate(
                    cur_func,
                    bb,
                    if instr.opcode == MachineOpcode::Phi {
                        Some(instr.operand[i + 1].as_basic_block())
                    } else {
                        None
                    },
                    reg,
                )
            }
        }
    }

    fn propagate(
        &self,
        cur_func: &MachineFunction,
        bb: MachineBasicBlockId,
        pred_bb: Option<MachineBasicBlockId>,
        reg: &MachineRegister,
    ) {
        let bb = &cur_func.basic_block_arena[bb];

        {
            let mut bb_liveness = bb.liveness.borrow_mut();

            if bb_liveness.def.contains(reg) {
                return;
            }

            if !bb_liveness.live_in.insert(reg.clone()) {
                // live_in already had the reg
                return;
            }
        }

        if let Some(pred_bb) = pred_bb {
            let pred = &cur_func.basic_block_arena[pred_bb];
            if pred.liveness.borrow_mut().live_out.insert(reg.clone()) {
                // live_out didn't have the reg
                self.propagate(cur_func, pred_bb, None, reg);
            }
            return;
        }

        for pred_id in &bb.pred {
            let pred = &cur_func.basic_block_arena[*pred_id];
            if pred.liveness.borrow_mut().live_out.insert(reg.clone()) {
                // live_out didn't have the reg
                self.propagate(cur_func, *pred_id, None, reg);
            }
        }
    }

    pub fn construct_live_reg_matrix(&self, cur_func: &MachineFunction) -> LiveRegMatrix {
        let mut vreg2range: FxHashMap<VirtReg, LiveRange> = FxHashMap::default();
        let mut reg2range: FxHashMap<PhysReg, LiveRange> = FxHashMap::default();
        let mut id2pp: FxHashMap<MachineInstrId, ProgramPoint> = FxHashMap::default();

        let mut bb_idx = 0;

        for bb_id in &cur_func.basic_blocks {
            let mut index = 0;
            let bb = &cur_func.basic_block_arena[*bb_id];
            let liveness = bb.liveness_ref();

            #[rustfmt::skip]
            macro_rules! cur_pp { () => { ProgramPoint::new(bb_idx, index) };}

            for livein in &liveness.live_in {
                if livein.get_reg().is_some() {
                    continue;
                }

                vreg2range
                    .entry(livein.get_vreg())
                    .or_insert_with(|| LiveRange::new_empty())
                    .add_segment(LiveSegment::new(cur_pp!(), cur_pp!()))
            }

            for instr_id in &*bb.iseq_ref() {
                let instr = &cur_func.instr_arena[*instr_id];

                id2pp.insert(*instr_id, cur_pp!());

                for operand in &instr.operand {
                    if let MachineOperand::Register(reg) = operand {
                        if let Some(phy_reg) = reg.get_reg() {
                            if let Some(range) = reg2range.get_mut(&phy_reg) {
                                range.segments.last_mut().unwrap().end = cur_pp!();
                            }
                        }

                        if reg.get_reg().is_none() {
                            vreg2range
                                .get_mut(&reg.get_vreg())
                                .unwrap()
                                .segments
                                .last_mut()
                                .unwrap()
                                .end = cur_pp!();
                        }
                    }
                }

                // TODO: def.len() > 0 is no easy to understand
                if instr.def.len() > 0 {
                    if instr.def[0].get_reg().is_some() {
                        reg2range
                            .entry(instr.def[0].get_reg().unwrap())
                            .or_insert_with(|| LiveRange::new_empty())
                            .add_segment(LiveSegment::new(cur_pp!(), cur_pp!()));
                    } else {
                        vreg2range
                            .entry(instr.def[0].get_vreg())
                            .or_insert_with(|| LiveRange::new_empty())
                            .add_segment(LiveSegment::new(cur_pp!(), cur_pp!()));
                    }
                }

                for use_ in &instr.imp_use {
                    if let Some(range) = reg2range.get_mut(&use_.get_reg().unwrap()) {
                        range.segments.last_mut().unwrap().end = cur_pp!();
                    }
                }

                for def in &instr.imp_def {
                    reg2range
                        .entry(def.get_reg().unwrap())
                        .or_insert_with(|| LiveRange::new_empty())
                        .add_segment(LiveSegment::new(cur_pp!(), cur_pp!()))
                }

                index += IDX_STEP;
            }

            for liveout in &liveness.live_out {
                if liveout.get_reg().is_some() {
                    continue;
                }

                vreg2range
                    .get_mut(&liveout.get_vreg())
                    .unwrap()
                    .segments
                    .last_mut()
                    .unwrap()
                    .end = cur_pp!();
            }

            bb_idx += 1;
        }

        debug!(for (vreg, range) in &vreg2range {
            println!("{:?}: {:?}", vreg, range)
        });

        LiveRegMatrix::new(
            id2pp,
            vreg2range
                .into_iter()
                .map(|(vreg, range)| (vreg, LiveInterval::new(vreg, range)))
                .collect(),
            reg2range,
        )
    }
}
