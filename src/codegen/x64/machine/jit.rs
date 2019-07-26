use super::{basic_block::*, function::*, instr::*, module::*};
use crate::ir::types::*;
use dynasmrt::*;
use rustc_hash::FxHashMap;

#[rustfmt::skip]
macro_rules! register {
    ($f:expr; $instr_id:expr) => {{
        ($f.instr_arena[$instr_id].get_reg().unwrap()
         + REGISTER_OFFSET) as u8
    }};
    ($reg:expr) => {{
        ($reg.get_reg().unwrap() + REGISTER_OFFSET) as u8
    }};
}

#[rustfmt::skip]
macro_rules! typ {
    ($f:expr; $instr_id:expr) => {{
        $f.instr_arena[$instr_id].ty.as_ref().unwrap()
    }};
    ($reg:expr) => {{
        &$reg.info_ref().ty
    }};
}

#[rustfmt::skip]
macro_rules! vregister {
    ($f:expr ; $instr_id:expr) => {{
        $f.instr_arena[$instr_id].reg.borrow().vreg
    }};
    ($instr:expr) => {{
        $instr.reg.borrow().vreg
    }};
}

const REGISTER_OFFSET: usize = 10; // Instruction.reg.reg=0 means r10

#[derive(Debug, Clone, PartialEq)]
pub enum GenericValue {
    Int32(i32),
    None,
}

pub struct JITCompiler<'a> {
    module: &'a MachineModule,
    asm: x64::Assembler,
    function_map: FxHashMap<MachineFunctionId, DynamicLabel>,
    bb_to_label: FxHashMap<MachineBasicBlockId, DynamicLabel>,
}

#[derive(Debug)]
pub struct FrameObjectsInfo {
    offset_map: FxHashMap<i32, usize>, // frame index -> offset
    total_size: usize,
}

impl<'a> JITCompiler<'a> {
    pub fn new(module: &'a MachineModule) -> Self {
        Self {
            module,
            asm: x64::Assembler::new().unwrap(),
            function_map: FxHashMap::default(),
            bb_to_label: FxHashMap::default(),
        }
    }

    pub fn run(&mut self, id: MachineFunctionId, args: Vec<GenericValue>) -> GenericValue {
        let f_entry = self.get_function_entry_label(id);
        let entry = self.asm.offset();

        for (idx, arg) in args.iter().enumerate() {
            match arg {
                GenericValue::Int32(i) => dynasm!(self.asm; mov Ra(reg4arg(idx).unwrap()), *i),
                GenericValue::None => unreachable!(),
            }
        }

        dynasm!(self.asm
                ; call =>f_entry
                ; ret);

        self.asm.commit();
        let executor = self.asm.reader();
        let buf = executor.lock();
        let f: extern "C" fn() -> u64 = unsafe { ::std::mem::transmute(buf.ptr(entry)) };

        match &self
            .module
            .function_ref(id)
            .ty
            .get_function_ty()
            .unwrap()
            .ret_ty
        {
            Type::Int32 => GenericValue::Int32(f() as i32),
            _ => unimplemented!(),
        }
    }

    pub fn compile_module(&mut self) {
        for (f_id, _) in &self.module.functions {
            self.compile_function(f_id);
        }
    }

    fn compile_function(&mut self, id: MachineFunctionId) {
        self.bb_to_label.clear();

        let f = self.module.function_ref(id);
        let f_entry = self.get_function_entry_label(id);

        let frame_objects = FrameObjectsInfo::new(f);

        dynasm!(self.asm
            ; => f_entry
            ; push rbp
            ; mov rbp, rsp
            ; sub rsp, roundup(frame_objects.total_size() + /*push rbp=*/8, 16) - 8
        );

        for (i, ty) in f.ty.get_function_ty().unwrap().params_ty.iter().enumerate() {
            match ty {
                Type::Int32 => {
                    let off = frame_objects.offset(-(i as i32 + 1)).unwrap();
                    dynasm!(self.asm; mov [rbp - off], Rd(reg4arg(i).unwrap()));
                }
                _ => unimplemented!(),
            }
        }

        for (bb_id, bb) in &f.basic_blocks {
            if bb_id.index() != 0 {
                let label = self.get_label_of_bb(bb_id);
                dynasm!(self.asm; =>label);
            }

            for instr in &*bb.iseq_ref() {
                let instr = &f.instr_arena[*instr];
                match instr.opcode {
                    MachineOpcode::Add => self.compile_add(f, &frame_objects, instr),
                    MachineOpcode::Sub => self.compile_sub(f, &frame_objects, instr),
                    MachineOpcode::Load => self.compile_load(&frame_objects, instr),
                    MachineOpcode::Store => self.compile_store(f, &frame_objects, instr),
                    MachineOpcode::Call => self.compile_call(f, &frame_objects, instr),
                    MachineOpcode::CopyToReg => self.compile_copy2reg(f, instr),
                    MachineOpcode::BrccEq | MachineOpcode::BrccLe => self.compile_brcc(f, instr),
                    MachineOpcode::Br => self.compile_br(instr),
                    MachineOpcode::Ret => self.compile_return(f, &frame_objects, instr),
                    _ => unimplemented!(),
                }
            }
        }
    }

    fn compile_copy2reg(&mut self, f: &MachineFunction, instr: &MachineInstr) {
        let rn = register!(instr);
        let op0 = &instr.oprand[0];
        match op0 {
            MachineOprand::Register(reg) => self.reg_copy(typ!(reg), rn, register!(reg)),
            _ => unimplemented!(),
        }
    }

    fn compile_brcc(&mut self, f: &MachineFunction, instr: &MachineInstr) {
        let op0 = &instr.oprand[0];
        let op1 = &instr.oprand[1];
        let br = &instr.oprand[2];
        match op0 {
            MachineOprand::Register(i0) => match op1 {
                MachineOprand::Constant(c) => match c {
                    MachineConstant::Int32(x) => dynasm!(self.asm; cmp Rd(register!(i0)), *x),
                },
                MachineOprand::Register(i1) => match typ!(i0) {
                    Type::Int32 => dynasm!(self.asm; cmp Rd(register!(i0)), Rd(register!(i1))),
                    _ => unimplemented!(),
                },
                _ => unimplemented!(),
            },
            e => unimplemented!("{:?}", e),
        }
        match br {
            MachineOprand::Branch(bb) => {
                let l = self.get_label_of_bb(*bb);
                match instr.opcode {
                    MachineOpcode::BrccEq => dynasm!(self.asm; jz => l),
                    MachineOpcode::BrccLe => dynasm!(self.asm; jle => l),
                    _ => unreachable!(),
                }
            }
            _ => unimplemented!(),
        }
    }

    fn compile_load(&mut self, fo: &FrameObjectsInfo, instr: &MachineInstr) {
        let rn = register!(instr);
        let op0 = &instr.oprand[0];
        match op0 {
            MachineOprand::FrameIndex(fi) => {
                let off = fo.offset(fi.idx).unwrap();
                match fi.ty {
                    Type::Int32 => dynasm!(self.asm; mov Rd(rn), [rbp - off]),
                    _ => unimplemented!(),
                }
            }
            MachineOprand::Register(_) => unimplemented!(),
            _ => unreachable!(),
        }
    }

    fn compile_store(&mut self, f: &MachineFunction, fo: &FrameObjectsInfo, instr: &MachineInstr) {
        let dst = &instr.oprand[0];
        let src = &instr.oprand[1];
        match dst {
            MachineOprand::FrameIndex(fi) => {
                let off = fo.offset(fi.idx).unwrap();
                match src {
                    MachineOprand::Constant(c) => match c {
                        MachineConstant::Int32(i) => dynasm!(self.asm; mov DWORD [rbp - off], *i),
                    },
                    MachineOprand::Register(i) => match fi.ty {
                        Type::Int32 => dynasm!(self.asm; mov [rbp - off], Rd(register!(i))),
                        _ => unimplemented!(),
                    },
                    _ => unreachable!(),
                }
            }
            _ => unimplemented!(),
        }
    }

    fn compile_call(&mut self, f: &MachineFunction, _fo: &FrameObjectsInfo, instr: &MachineInstr) {
        let callee_id = self
            .module
            .find_function_by_name(match &instr.oprand[0] {
                MachineOprand::GlobalAddress(GlobalValueInfo::FunctionName(n)) => n.as_str(),
                _ => unimplemented!(),
            })
            .unwrap();
        let callee_entity = self.module.function_ref(callee_id);
        let ret_ty = &callee_entity.ty.get_function_ty().unwrap().ret_ty;
        // let mut rsp_offset = 0;

        // TODO
        let mut save_regs = vec![];
        for (_, i) in &f.instr_arena {
            // TODO
            if i.reg.borrow().reg.is_none() {
                continue;
            }
            let bgn = vregister!(i);
            let end = match i.reg.borrow().last_use {
                Some(last_use) => vregister!(f; last_use),
                None => continue,
            };
            if bgn < vregister!(instr) && vregister!(instr) < end {
                save_regs.push(register!(i));
            }
        }

        when_debug!(println!("saved register: {:?}", save_regs));

        for save_reg in &save_regs {
            dynasm!(self.asm; push Ra(*save_reg));
            // rsp_offset += 8;
        }

        if !callee_entity.internal {
            // rsp_offset += self.push_args(f, args);
            self.assign_args_to_regs(f, &instr.oprand[1..]);
        }

        if callee_entity.internal {
            // self.assign_args_to_regs(f, args);
            // let callee = self.internal_func.get(&callee_entity.name).unwrap();
            // let rsp_adjust = roundup(rsp_offset as i32, 16) - rsp_offset as i32; // rsp is 16-byte aligned
            // dynasm!(self.asm
            //     ; sub rsp, rsp_adjust
            //     ; mov rax, QWORD *callee as _
            //     ; call rax
            //     ; add rsp, rsp_adjust
            // );
        } else {
            let f_entry = self.get_function_entry_label(callee_id);
            dynasm!(self.asm; call => f_entry);
        }

        match ret_ty {
            Type::Int32 => self.reg_copy(ret_ty, register!(instr), 0),
            _ => {}
        }

        if !callee_entity.internal {
            // dynasm!(self.asm; add rsp, 8*(args.len() as i32));
        }

        for save_reg in save_regs.iter().rev() {
            dynasm!(self.asm; pop Ra(*save_reg));
        }
    }

    fn compile_add(&mut self, f: &MachineFunction, _fo: &FrameObjectsInfo, instr: &MachineInstr) {
        let rn = register!(instr);
        let op0 = &instr.oprand[0];
        let op1 = &instr.oprand[1];
        match op0 {
            MachineOprand::FrameIndex(_) => unimplemented!(), // TODO: Address
            MachineOprand::Register(i0) => {
                self.reg_copy(typ!(i0), rn, register!(i0));
                match op1 {
                    MachineOprand::Constant(c) => match c {
                        MachineConstant::Int32(x) => dynasm!(self.asm; add Rd(rn), *x),
                    },
                    MachineOprand::Register(i1) => match typ!(i1) {
                        Type::Int32 => dynasm!(self.asm; add Rd(rn), Rd(register!(i1))),
                        _ => unimplemented!(),
                    },
                    _ => unimplemented!(),
                };
            }
            _ => unimplemented!(),
        }
    }

    fn compile_sub(&mut self, f: &MachineFunction, _fo: &FrameObjectsInfo, instr: &MachineInstr) {
        let rn = register!(instr);
        let op0 = &instr.oprand[0];
        let op1 = &instr.oprand[1];
        match op0 {
            MachineOprand::FrameIndex(_) => unimplemented!(), // TODO: Address
            MachineOprand::Register(i0) => {
                self.reg_copy(typ!(i0), rn, register!(i0));
                match op1 {
                    MachineOprand::Constant(MachineConstant::Int32(x)) => {
                        dynasm!(self.asm; sub Rd(rn), *x)
                    }
                    MachineOprand::Register(i1) => match typ!(i1) {
                        Type::Int32 => dynasm!(self.asm; sub Rd(rn), Rd(register!(i1))),
                        _ => unimplemented!(),
                    },
                    _ => unimplemented!(),
                };
            }
            _ => unimplemented!(),
        }
    }

    fn compile_br(&mut self, instr: &MachineInstr) {
        match &instr.oprand[0] {
            MachineOprand::Branch(bb) => {
                let label = self.get_label_of_bb(*bb);
                dynasm!(self.asm; jmp =>label)
            }
            _ => unimplemented!(),
        }
    }

    fn compile_return(&mut self, f: &MachineFunction, fo: &FrameObjectsInfo, instr: &MachineInstr) {
        match &instr.oprand[0] {
            MachineOprand::Constant(c) => match c {
                MachineConstant::Int32(i) => dynasm!(self.asm; mov rax, *i),
            },
            MachineOprand::Register(i) => dynasm!(self.asm; mov rax, Ra(register!(i))),
            MachineOprand::FrameIndex(fi) => {
                dynasm!(self.asm; mov rax, [rbp - fo.offset(fi.idx).unwrap()])
            }
            _ => unreachable!(),
        }
        dynasm!(self.asm; mov rsp, rbp; pop rbp; ret);
    }

    fn assign_args_to_regs(&mut self, f: &MachineFunction, args: &[MachineOprand]) {
        for (idx, arg) in args.iter().enumerate() {
            match arg {
                MachineOprand::Constant(MachineConstant::Int32(i)) => {
                    dynasm!(self.asm; mov Rd(reg4arg(idx).unwrap()), *i)
                }
                MachineOprand::Register(id) => {
                    self.reg_copy(typ!(id), reg4arg(idx).unwrap(), register!(id))
                }
                _ => unimplemented!(),
            }
        }
    }

    fn reg_copy(&mut self, ty: &Type, r0: u8, r1: u8) {
        if r0 == r1 {
            return;
        }

        match ty {
            Type::Int32 => dynasm!(self.asm; mov Rd(r0), Rd(r1)),
            _ => unimplemented!(),
        }
    }

    fn get_label_of_bb(&mut self, bb_id: MachineBasicBlockId) -> DynamicLabel {
        if let Some(label) = self.bb_to_label.get(&bb_id) {
            return *label;
        }

        let new_label = self.asm.new_dynamic_label();
        self.bb_to_label.insert(bb_id, new_label);
        new_label
    }

    fn get_function_entry_label(&mut self, f_id: MachineFunctionId) -> DynamicLabel {
        if self.function_map.contains_key(&f_id) {
            return *self.function_map.get(&f_id).unwrap();
        }

        let f_entry = self.asm.new_dynamic_label();
        self.function_map.insert(f_id, f_entry);
        f_entry
    }
}

impl FrameObjectsInfo {
    pub fn new(f: &MachineFunction) -> Self {
        let mut offset_map = FxHashMap::default();
        let mut offset = 0;
        for (i, param_ty) in f.ty.get_function_ty().unwrap().params_ty.iter().enumerate() {
            offset += param_ty.size_in_byte();
            offset_map.insert(-(i as i32 + 1), offset);
        }
        for (i, param_ty) in f.locals_ty.iter().enumerate() {
            offset += param_ty.size_in_byte();
            offset_map.insert(i as i32 + 1, offset);
        }

        Self {
            offset_map,
            total_size: offset,
        }
    }

    pub fn offset(&self, frame_index: i32) -> Option<i32> {
        self.offset_map.get(&frame_index).map(|x| *x as i32)
    }

    pub fn total_size(&self) -> i32 {
        self.total_size as i32
    }
}

fn reg4arg(idx: usize) -> Option<u8> {
    let regs = [7, 6, 2, 1, 8, 9]; // rdi, rsi, rdx, rcx, r8, r9
    regs.get(idx).map(|x| *x as u8)
}

fn roundup(n: i32, align: i32) -> i32 {
    (n + align - 1) & !(align - 1)
}
