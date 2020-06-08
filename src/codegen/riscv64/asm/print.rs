use super::super::frame_object::FrameObjectsInfo;
use super::super::machine::{
    basic_block::MachineBasicBlockId,
    function::{InstIter, MachineFunction},
    inst::*,
    module::MachineModule,
};
use super::super::register::RegistersInfo;
use crate::ir::types::{TypeSize, Types};

pub struct MachineAsmPrinter {
    pub output: String,
    cur_bb_id_base: usize,
}

impl MachineAsmPrinter {
    pub fn new() -> Self {
        Self {
            output: "".to_string(),
            cur_bb_id_base: 0,
        }
    }

    pub fn run_on_module(&mut self, m: &MachineModule) {
        self.output.push_str("  .text\n");

        for (_, func) in &m.functions {
            self.run_on_function(&m.types, &func)
        }
    }

    fn run_on_function(&mut self, tys: &Types, f: &MachineFunction) {
        if f.is_internal {
            return;
        }

        let fo = FrameObjectsInfo::new(tys, f);

        self.output
            .push_str(format!("  .globl {}\n", f.name).as_str()); // TODO

        self.output.push_str(format!("{}:\n", f.name).as_str());

        self.run_on_basic_blocks(tys, f, &fo);
    }

    fn run_on_basic_blocks(&mut self, tys: &Types, f: &MachineFunction, fo: &FrameObjectsInfo) {
        for (id, _, inst_iter) in f.body.mbb_iter() {
            self.output
                .push_str(format!("{}:\n", self.bb_id_to_label_id(&id)).as_str());
            self.run_on_basic_block(tys, &f.regs_info, inst_iter, fo);
        }
        self.cur_bb_id_base += f.body.basic_blocks.order.len();
    }

    fn run_on_basic_block<'a>(
        &mut self,
        tys: &Types,
        regs_info: &RegistersInfo,
        inst_iter: InstIter<'a>,
        fo: &FrameObjectsInfo,
    ) {
        for (_, inst) in inst_iter {
            self.run_on_inst(tys, regs_info, inst, fo);
        }
    }

    fn run_on_inst(
        &mut self,
        tys: &Types,
        regs_info: &RegistersInfo,
        inst: &MachineInst,
        fo: &FrameObjectsInfo,
    ) {
        self.output.push_str("  ");

        match inst.opcode {
            MachineOpcode::ADDI => self.run_on_inst_addi(inst),
            MachineOpcode::MV => self.run_on_inst_mv(inst),
            MachineOpcode::LI => self.run_on_inst_li(inst),
            MachineOpcode::LW => self.run_on_inst_lw(fo, inst),
            MachineOpcode::LD => self.run_on_inst_ld(fo, inst),
            MachineOpcode::SW => self.run_on_inst_sw(fo, inst),
            MachineOpcode::SD => self.run_on_inst_sd(fo, inst),
            MachineOpcode::JR => self.run_on_inst_jr(inst),
            // MachineOpcode::MOVSDrm64 => {}
            // MachineOpcode::MOVSXDr64m32 => {}
            // MachineOpcode::MOVrr32
            // | MachineOpcode::MOVri32
            // | MachineOpcode::MOVrr64
            // | MachineOpcode::MOVri64 => self.run_on_inst_mov_rx(inst),
            // MachineOpcode::MOVrm32 | MachineOpcode::MOVrm64 => self.run_on_inst_mov_rm(inst, fo),
            // MachineOpcode::MOVmr32 | MachineOpcode::MOVmi32 => {
            //     self.run_on_inst_mov_mx(tys, regs_info, inst, fo)
            // }
            // MachineOpcode::LEAr64m => self.run_on_inst_lea_rm(inst, fo),
            // MachineOpcode::ADDrr32 | MachineOpcode::ADDri32 | MachineOpcode::ADDr64i32 => {
            //     self.run_on_inst_add(inst)
            // }
            // MachineOpcode::SUBrr32 | MachineOpcode::SUBri32 | MachineOpcode::SUBr64i32 => {
            //     self.run_on_inst_sub(inst)
            // }
            // MachineOpcode::IMULrr32 => self.run_on_inst_imul_rr(inst),
            // MachineOpcode::IMULrri32 | MachineOpcode::IMULrr64i32 => {
            //     self.run_on_inst_imul_rri(inst)
            // }
            // MachineOpcode::CDQ => self.run_on_inst_cdq(),
            // MachineOpcode::IDIV => self.run_on_inst_idiv(inst),
            // MachineOpcode::PUSH64 => self.run_on_inst_push(inst),
            // MachineOpcode::POP64 => self.run_on_inst_pop(inst),
            // MachineOpcode::RET => self.run_on_inst_ret(),
            // MachineOpcode::CALL => self.run_on_inst_call(inst),
            // MachineOpcode::Seteq | MachineOpcode::Setle | MachineOpcode::Setlt => {}
            // MachineOpcode::JMP => self.run_on_inst_jmp(inst),
            // MachineOpcode::BrCond => {}
            // MachineOpcode::CMPri | MachineOpcode::CMPrr => self.run_on_inst_cmp(inst),
            // MachineOpcode::JE => self.run_on_inst_je(inst),
            // MachineOpcode::JLE => self.run_on_inst_jle(inst),
            // MachineOpcode::JL => self.run_on_inst_jl(inst),
            _ => panic!("{:?}", inst),
        }

        self.output.push('\n');
    }

    fn run_on_inst_addi(&mut self, i: &MachineInst) {
        self.output.push_str(
            format!(
                "addi {}, {}, {}",
                i.def[0].as_phys_reg().name(),
                operand2string(None, &i.operand[0]),
                operand2string(None, &i.operand[1])
            )
            .as_str(),
        )
    }

    fn run_on_inst_mv(&mut self, i: &MachineInst) {
        self.output.push_str(
            format!(
                "mv {}, {}",
                i.def[0].as_phys_reg().name(),
                i.operand[0].as_register().as_phys_reg().name()
            )
            .as_str(),
        )
    }

    fn run_on_inst_li(&mut self, i: &MachineInst) {
        self.output.push_str(
            format!(
                "li {}, {}",
                i.def[0].as_phys_reg().name(),
                operand2string(None, &i.operand[0]),
            )
            .as_str(),
        );
    }

    fn run_on_inst_lw(&mut self, fo: &FrameObjectsInfo, i: &MachineInst) {
        self.output.push_str(
            format!(
                "lw {}, {}",
                i.def[0].as_phys_reg().name(),
                operand2string(Some(fo), &i.operand[0]),
            )
            .as_str(),
        );
    }

    fn run_on_inst_ld(&mut self, fo: &FrameObjectsInfo, i: &MachineInst) {
        self.output.push_str(
            format!(
                "ld {}, {}",
                i.def[0].as_phys_reg().name(),
                operand2string(Some(fo), &i.operand[0]),
            )
            .as_str(),
        );
    }

    fn run_on_inst_sw(&mut self, fo: &FrameObjectsInfo, i: &MachineInst) {
        self.output.push_str(
            format!(
                "sw {}, {}",
                i.operand[0].as_register().as_phys_reg().name(),
                operand2string(Some(fo), &i.operand[1]),
            )
            .as_str(),
        );
    }

    fn run_on_inst_sd(&mut self, fo: &FrameObjectsInfo, i: &MachineInst) {
        self.output.push_str(
            format!(
                "sd {}, {}",
                i.operand[0].as_register().as_phys_reg().name(),
                operand2string(Some(fo), &i.operand[1]),
            )
            .as_str(),
        );
    }

    fn run_on_inst_jr(&mut self, i: &MachineInst) {
        self.output
            .push_str(format!("jr {}", operand2string(None, &i.operand[0])).as_str());
    }

    // fn run_on_inst_mov_rx(&mut self, i: &MachineInst) {
    //     self.output.push_str("mov ");
    //     self.output
    //         .push_str(format!("{}, ", i.def[0].as_phys_reg().name()).as_str());
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_mov_rm(&mut self, i: &MachineInst, fo: &FrameObjectsInfo) {
    //     let word = byte2word(i.def[0].as_phys_reg().reg_class().size_in_byte());
    //     self.output.push_str("mov ");
    //     self.output
    //         .push_str(format!("{}, ", i.def[0].as_phys_reg().name()).as_str());
    //     self.run_on_mem_operand(&i.operand[0], fo, word);
    // }
    //
    // fn run_on_inst_mov_mx(
    //     &mut self,
    //     tys: &Types,
    //     regs_info: &RegistersInfo,
    //     i: &MachineInst,
    //     fo: &FrameObjectsInfo,
    // ) {
    //     let word = byte2word(i.operand[1].get_type(regs_info).unwrap().size_in_byte(tys));
    //     self.output.push_str("mov ");
    //     self.run_on_mem_operand(&i.operand[0], fo, word);
    //     self.output.push_str(", ");
    //     self.run_on_operand(&i.operand[1]);
    // }
    //
    // fn run_on_inst_lea_rm(&mut self, i: &MachineInst, fo: &FrameObjectsInfo) {
    //     self.output.push_str("lea ");
    //     self.output
    //         .push_str(format!("{}, ", i.def[0].as_phys_reg().name()).as_str());
    //
    //     // out = lea rbp, fi, none, none
    //     if i.operand[0].is_register() // must be rbp
    //         && i.operand[1].is_frame_index()
    //         && i.operand[2].is_none()
    //         && i.operand[3].is_none()
    //     {
    //         let offset = fo.offset(i.operand[1].as_frame_index().idx).unwrap();
    //         self.output.push_str(format!("[rbp - {}]", offset).as_str());
    //     }
    //
    //     // out = lea rbp, fi, none, const.off
    //     if i.operand[0].is_register() // must be rbp
    //         && i.operand[1].is_frame_index()
    //         && i.operand[2].is_none()
    //         && i.operand[3].is_const_i32()
    //     {
    //         let off1 = fo.offset(i.operand[1].as_frame_index().idx).unwrap();
    //         let off2 = i.operand[3].as_constant().as_i32();
    //         assert!(off1 >= off2);
    //         let offset = off1 - off2;
    //         self.output.push_str(format!("[rbp - {}]", offset).as_str());
    //     }
    //
    //     // out = lea rbp, fi, align, off
    //     if i.operand[0].is_register() // must be rbp
    //         && i.operand[1].is_frame_index()
    //         && i.operand[2].is_const_i32()
    //         && i.operand[3].is_register()
    //     {
    //         let offset = fo.offset(i.operand[1].as_frame_index().idx).unwrap();
    //         let align = i.operand[2].as_constant().as_i32();
    //         let reg = i.operand[3].as_register().as_phys_reg();
    //         self.output
    //             .push_str(format!("[rbp + {}*{} - {}]", align, reg.name(), offset).as_str());
    //     }
    //
    //     // out = lea base, none, align, off
    //     if i.operand[0].is_register()
    //         && i.operand[1].is_none()
    //         && i.operand[2].is_const_i32()
    //         && i.operand[3].is_register()
    //     {
    //         let base = i.operand[0].as_register().as_phys_reg();
    //         let align = i.operand[2].as_constant().as_i32();
    //         let reg = i.operand[3].as_register().as_phys_reg();
    //         self.output
    //             .push_str(format!("[{} + {}*{}]", base.name(), align, reg.name()).as_str());
    //     }
    //
    //     // out = lea base, none, none, none
    //     if i.operand[0].is_register()
    //         && i.operand[1].is_none()
    //         && i.operand[2].is_none()
    //         && i.operand[3].is_none()
    //     {
    //         let base = i.operand[0].as_register().as_phys_reg();
    //         self.output.push_str(format!("[{}]", base.name()).as_str());
    //     }
    // }
    //
    // fn run_on_mem_operand(&mut self, op: &MachineOperand, fo: &FrameObjectsInfo, word: &str) {
    //     match op {
    //         MachineOperand::Mem(MachineMemOperand::BaseFi(base, fi)) => {
    //             let base = base.as_phys_reg();
    //             let offset = fo.offset(fi.idx).unwrap();
    //             self.output
    //                 .push_str(format!("{} ptr [{} - {}]", word, base.name(), offset).as_str());
    //         }
    //         MachineOperand::Mem(MachineMemOperand::BaseAlignOff(base, align, off)) => {
    //             let base = base.as_phys_reg();
    //             let reg = off.as_phys_reg();
    //             self.output.push_str(
    //                 format!("{} ptr [{} + {}*{}]", word, base.name(), align, reg.name()).as_str(),
    //             );
    //         }
    //         MachineOperand::Mem(MachineMemOperand::BaseFiAlignOff(base, fi, align, off)) => {
    //             let offset = fo.offset(fi.idx).unwrap();
    //             let base = base.as_phys_reg();
    //             let reg = off.as_phys_reg();
    //             self.output.push_str(
    //                 format!(
    //                     "{} ptr [{} + {}*{} - {}]",
    //                     word,
    //                     base.name(),
    //                     align,
    //                     reg.name(),
    //                     offset
    //                 )
    //                 .as_str(),
    //             );
    //         }
    //         MachineOperand::Mem(MachineMemOperand::BaseFiOff(base, fi, off)) => {
    //             let base = base.as_phys_reg();
    //             let off1 = fo.offset(fi.idx).unwrap();
    //             let off2 = *off;
    //             assert!(off1 >= off2);
    //             let offset = off1 - off2;
    //             self.output
    //                 .push_str(format!("{} ptr [{} - {}]", word, base.name(), offset).as_str());
    //         }
    //         MachineOperand::Mem(MachineMemOperand::Base(base)) => {
    //             let base = base.as_phys_reg();
    //             self.output
    //                 .push_str(format!("{} ptr [{}]", word, base.name()).as_str());
    //         }
    //         MachineOperand::Mem(MachineMemOperand::BaseOff(base, off)) => {
    //             let base = base.as_phys_reg();
    //             self.output
    //                 .push_str(format!("{} ptr [{} + {}]", word, base.name(), off).as_str());
    //         }
    //         e => panic!("{:?}", e),
    //     }
    // }
    //
    // fn run_on_inst_add(&mut self, i: &MachineInst) {
    //     self.output.push_str("add ");
    //     self.run_on_operand(&i.operand[0]);
    //     self.output.push_str(", ");
    //     self.run_on_operand(&i.operand[1]);
    // }
    //
    // fn run_on_inst_sub(&mut self, i: &MachineInst) {
    //     self.output.push_str("sub ");
    //     self.run_on_operand(&i.operand[0]);
    //     self.output.push_str(", ");
    //     self.run_on_operand(&i.operand[1]);
    // }
    //
    // fn run_on_inst_imul_rr(&mut self, i: &MachineInst) {
    //     self.output.push_str("imul ");
    //     self.output
    //         .push_str(format!("{}, ", i.def[0].as_phys_reg().name()).as_str());
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_imul_rri(&mut self, i: &MachineInst) {
    //     self.output.push_str("imul ");
    //     self.output
    //         .push_str(format!("{}, ", i.def[0].as_phys_reg().name()).as_str());
    //     self.run_on_operand(&i.operand[0]);
    //     self.output.push_str(", ");
    //     self.run_on_operand(&i.operand[1]);
    // }
    //
    // fn run_on_inst_cdq(&mut self) {
    //     self.output.push_str("cdq");
    // }
    //
    // fn run_on_inst_idiv(&mut self, i: &MachineInst) {
    //     self.output.push_str("idiv ");
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_push(&mut self, i: &MachineInst) {
    //     self.output.push_str("push ");
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_pop(&mut self, i: &MachineInst) {
    //     self.output.push_str("pop ");
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_ret(&mut self) {
    //     self.output.push_str("ret");
    // }
    //
    // fn run_on_inst_call(&mut self, i: &MachineInst) {
    //     self.output.push_str("call ");
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_cmp(&mut self, i: &MachineInst) {
    //     self.output.push_str("cmp ");
    //     self.run_on_operand(&i.operand[0]);
    //     self.output.push_str(", ");
    //     self.run_on_operand(&i.operand[1]);
    // }
    //
    // fn run_on_inst_jmp(&mut self, i: &MachineInst) {
    //     self.output.push_str("jmp ");
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_je(&mut self, i: &MachineInst) {
    //     self.output.push_str("je ");
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_jle(&mut self, i: &MachineInst) {
    //     self.output.push_str("jle ");
    //     self.run_on_operand(&i.operand[0]);
    // }
    //
    // fn run_on_inst_jl(&mut self, i: &MachineInst) {
    //     self.output.push_str("jl ");
    //     self.run_on_operand(&i.operand[0]);
    // }

    //
    fn bb_id_to_label_id(&self, bb_id: &MachineBasicBlockId) -> String {
        format!(".L{}", bb_id.index() + self.cur_bb_id_base)
    }
}

fn operand2string(fo: Option<&FrameObjectsInfo>, operand: &MachineOperand) -> String {
    match operand {
        // MachineOperand::Branch(id) => self.output.push_str(self.bb_id_to_label_id(id).as_str()),
        MachineOperand::Constant(MachineConstant::Int32(i)) => format!("{}", i),
        MachineOperand::Register(r) => r.as_phys_reg().name().to_string(),
        MachineOperand::FrameIndex(i) => format!("{}", fo.unwrap().offset(i.idx).unwrap()),
        MachineOperand::Mem(MachineMemOperand::FiReg(fi, r)) => format!(
            "{}({})",
            fo.unwrap().offset(fi.idx).unwrap(),
            r.as_phys_reg().name()
        ),
        MachineOperand::Mem(MachineMemOperand::ImmReg(imm, r)) => {
            format!("{}({})", imm, r.as_phys_reg().name())
        }
        MachineOperand::Mem(MachineMemOperand::Address(AddressKind::FunctionName(name))) => {
            name.clone()
        }
        _ => unimplemented!(),
    }
}

fn byte2word(byte: usize) -> &'static str {
    match byte {
        4 => "dword",
        8 => "qword",
        _ => unimplemented!(),
    }
}
