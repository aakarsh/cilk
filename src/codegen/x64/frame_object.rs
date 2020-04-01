use super::machine::function::MachineFunction;
use crate::ir::types::*;
use rustc_hash::FxHashMap;
use std::fmt;

#[derive(Debug, Clone)]
pub struct LocalVariables {
    pub locals: Vec<FrameIndexInfo>,
    pub cur_idx: usize,
}

#[derive(Debug)]
pub struct FrameObjectsInfo {
    offset_map: FxHashMap<FrameIndexKind, usize>, // frame index -> offset
    total_size: usize,
}

impl LocalVariables {
    pub fn new() -> Self {
        Self {
            locals: vec![],
            cur_idx: 0,
        }
    }

    pub fn alloc(&mut self, ty: &Type) -> FrameIndexInfo {
        let info = FrameIndexInfo::new(ty.clone(), FrameIndexKind::Local(self.cur_idx));
        self.cur_idx += 1;
        self.locals.push(info.clone());
        info
    }
}

impl FrameObjectsInfo {
    pub fn new(f: &MachineFunction) -> Self {
        let mut offset_map = FxHashMap::default();
        let mut offset = 0;

        for (i, param_ty) in f.ty.get_function_ty().unwrap().params_ty.iter().enumerate() {
            offset += param_ty.size_in_byte();
            offset_map.insert(FrameIndexKind::Arg(i), offset);
        }

        for FrameIndexInfo { idx, ty } in &f.local_mgr.locals {
            offset += ty.size_in_byte();
            offset_map.insert(*idx, offset);
        }

        Self {
            offset_map,
            total_size: offset,
        }
    }

    pub fn offset(&self, kind: FrameIndexKind) -> Option<i32> {
        self.offset_map.get(&kind).map(|x| *x as i32)
    }

    pub fn total_size(&self) -> i32 {
        self.total_size as i32
    }
}

#[derive(Clone, PartialEq)]
pub struct FrameIndexInfo {
    pub ty: Type,
    pub idx: FrameIndexKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Copy)]
pub enum FrameIndexKind {
    Arg(usize),
    Local(usize),
}

impl FrameIndexKind {
    pub fn new_arg(idx: usize) -> Self {
        FrameIndexKind::Arg(idx)
    }

    pub fn new_local(idx: usize) -> Self {
        FrameIndexKind::Local(idx)
    }
}

impl FrameIndexInfo {
    pub fn new(ty: Type, idx: FrameIndexKind) -> Self {
        Self { ty, idx }
    }
}

impl fmt::Debug for FrameIndexInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FI<{:?}, {:?}>", self.ty, self.idx)
    }
}
