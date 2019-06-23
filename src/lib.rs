pub mod ir;

extern crate id_arena;
extern crate rustc_hash;

#[cfg(test)]
mod tests {
    use crate::ir::{function, module, types};

    #[test]
    fn module() {
        let mut m = module::Module::new("cilk");
        m.add_function(function::Function::new("f", types::Type::Int32, vec![]));
    }
}
