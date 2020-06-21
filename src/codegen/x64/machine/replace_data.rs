use super::inst::*;
use crate::codegen::common::machine::{function::*, module::*};
use crate::traits::pass::ModulePassTrait;

pub struct ReplaceConstFPWithMemoryRef {}

impl ModulePassTrait for ReplaceConstFPWithMemoryRef {
    type M = MachineModule;

    fn name(&self) -> &'static str {
        "ReplaceConstFPWithMemoryRef"
    }

    fn run_on_module(&mut self, module: &mut Self::M) {
        self.run_on_module(module);
    }
}

impl ReplaceConstFPWithMemoryRef {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, func) in &mut module.functions {
            if func.is_internal {
                continue;
            }
            self.run_on_function(func);
        }
    }

    pub fn run_on_function(&mut self, cur_func: &mut MachineFunction) {
        for (_, bb) in cur_func.body.basic_blocks.id_and_block() {
            for inst_id in &*bb.iseq_ref() {
                let inst = &mut cur_func.body.inst_arena[*inst_id];
                let replace = matches!(inst.opcode, MachineOpcode::MOVSDrm64);
                if !replace {
                    continue;
                }
                for operand in &mut inst.operand {
                    match operand {
                        MachineOperand::Constant(MachineConstant::F64(f)) => {
                            let id = cur_func.const_data.alloc(MachineConstant::F64(*f));
                            *operand = MachineOperand::Mem(MachineMemOperand::Address(
                                AddressKind::Label(id),
                            ));
                        }
                        _ => {}
                    };
                }
            }
        }
    }
}
