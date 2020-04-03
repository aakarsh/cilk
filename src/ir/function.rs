use super::{basic_block::*, module::Module, opcode::*, types::*, value::*, DumpToString};
use id_arena::*;

pub type FunctionId = Id<Function>;

pub struct FunctionName<'a>(pub &'a str);

impl<'a> From<&'a Function> for FunctionName<'a> {
    fn from(f: &'a Function) -> Self {
        FunctionName(&f.name)
    }
}

impl<'a> From<&'a str> for FunctionName<'a> {
    fn from(s: &'a str) -> Self {
        FunctionName(s)
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    /// Function name
    pub name: String,

    /// Function type
    pub ty: Type,

    /// Basic blocks
    pub basic_block_arena: Arena<BasicBlock>,

    pub basic_blocks: Vec<BasicBlockId>,

    /// Instruction arena
    pub inst_table: Arena<Instruction>,

    pub id: Option<FunctionId>,
}

impl Function {
    pub fn new(module: &mut Module, name: &str, ret_ty: Type, params_ty: Vec<Type>) -> FunctionId {
        let ty = module.types.new_function_ty(ret_ty, params_ty);
        module.add_function(Self {
            name: name.to_string(),
            ty,
            basic_block_arena: Arena::new(),
            basic_blocks: vec![],
            inst_table: Arena::new(),
            id: None,
            // TODO
            // internal: match name {
            //     "cilk.memset.p0i32.i32" | "cilk.println.i32" | "cilk.printch.i32" => true, // TODO
            //     _ => false,
            // },
        })
    }

    pub fn append_basic_block(&mut self) -> BasicBlockId {
        let id = self.basic_block_arena.alloc(BasicBlock::new());
        // self.basic_blocks.push(id);
        id
    }

    pub fn append_existing_basic_block(&mut self, bb_id: BasicBlockId) {
        self.basic_blocks.push(bb_id);
    }

    pub fn basic_block_ref(&self, id: BasicBlockId) -> &BasicBlock {
        &self.basic_block_arena[id]
    }

    pub fn basic_block_ref_mut(&mut self, id: BasicBlockId) -> &mut BasicBlock {
        &mut self.basic_block_arena[id]
    }

    pub fn get_param_value(&self, tys: &Types, func_id: FunctionId, idx: usize) -> Option<Value> {
        if idx >= tys.as_function_ty(self.ty).unwrap().params_ty.len() {
            return None;
        }
        Some(Value::Argument(ArgumentValue {
            func_id,
            index: idx,
        }))
    }

    pub fn get_param_type(&self, tys: &Types, idx: usize) -> Option<Type> {
        let params_ty = &tys.as_function_ty(self.ty).unwrap().params_ty;
        if idx >= params_ty.len() {
            return None;
        }
        Some(params_ty[idx])
    }

    pub fn inst_id(&mut self, inst: Instruction) -> InstructionId {
        self.inst_table.alloc(inst)
    }
}

impl DumpToString for &Function {
    fn dump(&self, module: &Module) -> String {
        let ty = module.types.as_function_ty(self.ty).unwrap();
        format!(
            "define {} {}({}) {{\n{}}}",
            module.types.to_string(ty.ret_ty),
            self.name,
            ty.params_ty
                .iter()
                .fold("".to_string(), |mut s, p| {
                    s += &(module.types.to_string(*p) + ", ");
                    s
                })
                .trim_matches(&[',', ' '][0..]),
            self.basic_block_arena.dump(module)
        )
    }
}

impl DumpToString for FunctionId {
    fn dump(&self, module: &Module) -> String {
        module.function_ref(*self).dump(module)
    }
}

impl DumpToString for Arena<BasicBlock> {
    fn dump(&self, module: &Module) -> String {
        self.iter().fold("".to_string(), |s, (id, b)| {
            format!(
                "{}label.{}:\t// pred({}), succ({}), def({}), in({}), out({})\n{}\n",
                s,
                id.index(),
                &b.pred
                    .iter()
                    .fold("".to_string(), |s, x| format!("{}{},", s, x.index()))
                    .trim_matches(','),
                &b.succ
                    .iter()
                    .fold("".to_string(), |s, x| format!("{}{},", s, x.index()))
                    .trim_matches(','),
                &b.liveness
                    .borrow()
                    .def
                    .iter()
                    .fold("".to_string(), |s, x| format!("{}{},", s, x.index()))
                    .trim_matches(','),
                &b.liveness
                    .borrow()
                    .live_in
                    .iter()
                    .fold("".to_string(), |s, x| format!("{}{},", s, x.index()))
                    .trim_matches(','),
                &b.liveness
                    .borrow()
                    .live_out
                    .iter()
                    .fold("".to_string(), |s, x| format!("{}{},", s, x.index()))
                    .trim_matches(','),
                b.dump(module)
            )
        })
    }
}

impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
