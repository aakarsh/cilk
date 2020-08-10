pub use crate::codegen::common::machine::register::*;
use crate::ir::types::Type;
use defs::registers;
use id_arena::Arena;
use rustc_hash::FxHashMap;
use std::cell::RefCell;

registers! {
    // register nubmering: https://corsix.github.io/dynasm-doc/instructions.html#registers
    class GR32 (32, Int32, [Int32], [EAX]) < GR64 {
        EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI,
        R8D, R9D, R10D, R11D, R12D, R13D, R14D, R15D
    }

    class GR64 (64, Int64, [Int64, Pointer!], [RAX]) {
        RAX, RCX, RDX, RBX, RSP, RBP, RSI, RDI,
        R8, R9, R10, R11, R12, R13, R14, R15
    }

    class XMM (128, F64, [F64], [XMM0]) {
        XMM0, XMM1, XMM2, XMM3, XMM4, XMM5, XMM6, XMM7,
        XMM8, XMM9, XMM10, XMM11, XMM12, XMM13, XMM14, XMM15
    }

    // Normal order of registers used to pass arguments
    // TODO: This is System V AMD64 ABI.
    // https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI
    order arg GR32 { EDI, ESI, EDX, ECX, R8D, R9D }
    order arg GR64 { RDI, RSI, RDX, RCX, R8,  R9 }
    order arg XMM  { XMM0, XMM1, XMM2, XMM3, XMM4, XMM5, XMM6, XMM7 }

    // Normal order of general-purpose registers
    order gp GR32 { EAX, ECX, EDX, R8D, R9D, R10D, R11D }
    order gp GR64 { RAX, RCX, RDX, R8, R9, R10, R11 }
    order gp XMM { XMM0, XMM1, XMM2, XMM3, XMM4, XMM5 }
}

macro_rules! to_phys {
    ($($r:path),*) => {
        vec![$(($r.as_phys_reg())),*]
    };
}

thread_local! {
    pub static CALLEE_SAVED_REGS: PhysRegSet = {
        let mut bits = PhysRegSet::new();
        let regs = to_phys![
            GR32::EBX,
            GR32::EBP,
            GR32::R12D,
            GR32::R13D,
            GR32::R14D,
            GR32::R15D,
            GR64::RBX,
            GR64::RBP,
            GR64::R12,
            GR64::R13,
            GR64::R14,
            GR64::R15,
            XMM::XMM6,
            XMM::XMM7,
            XMM::XMM8,
            XMM::XMM15
        ];
        for reg in regs {
            bits.set(reg)
        }
        bits
    };

    pub static REG_FILE: RefCell<FxHashMap<PhysReg, PhysRegSet>> = {
        RefCell::new(FxHashMap::default())
    }
}

// TODO: Auto generate
impl RegisterClassKind {
    pub fn arg_regs() -> ArgRegs {
        ArgRegs::new()
    }

    pub fn ret_regs() -> RetRegs {
        RetRegs::new()
    }
}

pub struct ArgRegs {
    nths: Vec<usize>, // GR(32|64), XMM
}

// TODO: Integrate into ArgRegs
pub struct RetRegs {
    nths: Vec<usize>, // GR(32|64), XMM
    regs: Vec<Vec<PhysReg>>,
}

impl ArgRegs {
    pub fn new() -> Self {
        Self { nths: vec![0, 0] }
    }

    pub fn next(&mut self, rc: RegisterClassKind) -> Option<PhysReg> {
        match rc {
            RegisterClassKind::GR32 | RegisterClassKind::GR64 => {
                let nth = self.nths[0];
                self.nths[0] += 1;
                rc.get_nth_arg_reg(nth)
            }
            RegisterClassKind::XMM => {
                let nth = self.nths[1];
                self.nths[1] += 1;
                rc.get_nth_arg_reg(nth)
            }
        }
    }
}

impl RetRegs {
    pub fn new() -> Self {
        Self {
            nths: vec![0, 0],
            regs: vec![
                to_phys![GR32::EAX, GR32::EDX],
                to_phys![GR64::RAX, GR64::RDX],
                to_phys![XMM::XMM0, XMM::XMM1],
            ],
        }
    }

    pub fn next(&mut self, rc: RegisterClassKind) -> Option<PhysReg> {
        match rc {
            RegisterClassKind::GR32 => {
                let nth = self.nths[0];
                self.nths[0] += 1;
                self.regs[0].get(nth).map_or(None, |a| Some(*a))
            }
            RegisterClassKind::GR64 => {
                let nth = self.nths[0];
                self.nths[0] += 1;
                self.regs[1].get(nth).map_or(None, |a| Some(*a))
            }
            RegisterClassKind::XMM => {
                let nth = self.nths[1];
                self.nths[1] += 1;
                self.regs[2].get(nth).map_or(None, |a| Some(*a))
            }
        }
    }
}
