use super::const_data::ConstDataArena;
use super::function::*;
use id_arena::*;
use std::fmt;

pub struct MachineModule {
    pub name: String,
    pub functions: Arena<MachineFunction>,
    pub const_data: ConstDataArena,
}

impl MachineModule {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            functions: Arena::new(),
            const_data: ConstDataArena::new(),
        }
    }

    pub fn add_function(&mut self, f: MachineFunction) -> MachineFunctionId {
        self.functions.alloc(f)
    }

    pub fn function_ref(&self, id: MachineFunctionId) -> &MachineFunction {
        &self.functions[id]
    }

    pub fn function_ref_mut(&mut self, id: MachineFunctionId) -> &mut MachineFunction {
        &mut self.functions[id]
    }

    pub fn find_function_by_name(&self, name: &str) -> Option<MachineFunctionId> {
        for (id, func) in &self.functions {
            if func.name == name {
                return Some(id);
            }
        }
        None
    }
}

impl fmt::Debug for MachineModule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "MachineModule (name: {})", self.name)?;

        for (_, func) in &self.functions {
            func.fmt(f)?;
        }

        fmt::Result::Ok(())
    }
}
