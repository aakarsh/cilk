use super::{basic_block::*, function::*, module::Module, opcode::*, types::*, value::*};

#[derive(Debug)]
pub struct Builder<F: FuncRef> {
    pub func: F,
    cur_bb: Option<BasicBlockId>,
    insert_point: usize,
}

pub struct FunctionEntity<'a>(pub &'a mut Function);

pub struct FunctionIdWithModule<'a> {
    pub module: &'a mut Module,
    pub func_id: FunctionId,
}

impl<'a> FunctionIdWithModule<'a> {
    pub fn new(module: &'a mut Module, func_id: FunctionId) -> Self {
        Self { module, func_id }
    }
}

pub trait FuncRef {
    fn func_ref(&self) -> &Function;
    fn func_ref_mut(&mut self) -> &mut Function;
}

impl<'a> FuncRef for FunctionEntity<'a> {
    fn func_ref(&self) -> &Function {
        self.0
    }

    fn func_ref_mut(&mut self) -> &mut Function {
        self.0
    }
}

impl<'a> FuncRef for FunctionIdWithModule<'a> {
    fn func_ref(&self) -> &Function {
        self.module.function_ref(self.func_id)
    }

    fn func_ref_mut(&mut self) -> &mut Function {
        self.module.function_ref_mut(self.func_id)
    }
}

impl<F: FuncRef> Builder<F> {
    pub fn new(func: F) -> Self {
        Self {
            func,
            cur_bb: None,
            insert_point: 0,
        }
    }

    pub fn get_param(&self, idx: usize) -> Option<Value> {
        self.func.func_ref().get_param_value(idx)
    }

    pub fn append_basic_block(&mut self) -> BasicBlockId {
        self.func.func_ref_mut().append_basic_block()
    }

    pub fn set_insert_point(&mut self, id: BasicBlockId) {
        self.cur_bb = Some(id);
        let iseq_len = self
            .func
            .func_ref_mut()
            .basic_block_ref(id)
            .iseq_ref()
            .len();
        self.insert_point = iseq_len;
    }

    pub fn set_insert_point_at(&mut self, pt: usize, id: BasicBlockId) {
        self.cur_bb = Some(id);
        self.insert_point = pt;
    }

    pub fn set_insert_point_before_inst(&mut self, inst_id: InstructionId) -> Option<()> {
        let (bb_id, inst_pos) = self.func.func_ref_mut().find_inst_pos(inst_id)?;
        self.set_insert_point_at(inst_pos, bb_id);
        Some(())
    }

    pub fn build_alloca(&mut self, ty: Type) -> Value {
        let ptr_ty = self.func.func_ref_mut().types.new_pointer_ty(ty);
        let inst = self.create_inst_value(Opcode::Alloca, vec![Operand::Type(ty)], ptr_ty);
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_gep(&mut self, v: Value, indices: Vec<Value>) -> Value {
        let elem_ty = self
            .func
            .func_ref()
            .types
            .get_element_ty_with_indices(v.get_type(), &indices)
            .unwrap();
        let ptr_ty = self.func.func_ref_mut().types.new_pointer_ty(elem_ty);
        let mut operands = vec![Operand::Value(v)];
        operands.extend(indices.iter().map(|v| Operand::Value(*v)));
        let inst = self.create_inst_value(Opcode::GetElementPtr, operands, ptr_ty);
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_load(&mut self, v: Value) -> Value {
        let inst = self.create_inst_value(
            Opcode::Load,
            vec![Operand::Value(v)],
            self.func
                .func_ref()
                .types
                .get_element_ty(v.get_type(), None)
                .unwrap()
                .clone(),
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_store(&mut self, src: Value, dst: Value) -> Value {
        let inst = self.create_inst_value(
            Opcode::Store,
            vec![Operand::Value(src), Operand::Value(dst)],
            Type::Void,
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_add(&mut self, v1: Value, v2: Value) -> Value {
        if let Some(konst) = v1.const_add(&v2) {
            return konst;
        }

        let inst = self.create_inst_value(
            Opcode::Add,
            vec![Operand::Value(v1), Operand::Value(v2)],
            v1.get_type(),
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_sub(&mut self, v1: Value, v2: Value) -> Value {
        if let Some(konst) = v1.const_sub(&v2) {
            return konst;
        }

        let inst = self.create_inst_value(
            Opcode::Sub,
            vec![Operand::Value(v1), Operand::Value(v2)],
            v1.get_type(),
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_mul(&mut self, v1: Value, v2: Value) -> Value {
        if let Some(konst) = v1.const_mul(&v2) {
            return konst;
        }

        let inst = self.create_inst_value(
            Opcode::Mul,
            vec![Operand::Value(v1), Operand::Value(v2)],
            v1.get_type(),
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_div(&mut self, v1: Value, v2: Value) -> Value {
        if let Some(konst) = v1.const_div(&v2) {
            return konst;
        }

        let inst = self.create_inst_value(
            Opcode::Div,
            vec![Operand::Value(v1), Operand::Value(v2)],
            v1.get_type(),
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_rem(&mut self, v1: Value, v2: Value) -> Value {
        if let Some(konst) = v1.const_rem(&v2) {
            return konst;
        }

        let inst = self.create_inst_value(
            Opcode::Rem,
            vec![Operand::Value(v1), Operand::Value(v2)],
            v1.get_type(),
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_shl(&mut self, v1: Value, v2: Value) -> Value {
        // if let Some(konst) = v1.const_shl(&v2) {
        //     return konst;
        // }

        let inst = self.create_inst_value(
            Opcode::Shl,
            vec![Operand::Value(v1), Operand::Value(v2)],
            v1.get_type(),
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_sitofp(&mut self, v: Value, ty: Type) -> Value {
        let inst = self.create_inst_value(Opcode::SIToFP, vec![Operand::Value(v)], ty);
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_fptosi(&mut self, v: Value, ty: Type) -> Value {
        let inst = self.create_inst_value(Opcode::FPToSI, vec![Operand::Value(v)], ty);
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_sext(&mut self, v: Value, ty: Type) -> Value {
        let inst = self.create_inst_value(Opcode::Sext, vec![Operand::Value(v)], ty);
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_icmp(&mut self, kind: ICmpKind, v1: Value, v2: Value) -> Value {
        let inst = self.create_inst_value(
            Opcode::ICmp,
            vec![
                Operand::ICmpKind(kind),
                Operand::Value(v1),
                Operand::Value(v2),
            ],
            Type::Int1,
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_fcmp(&mut self, kind: FCmpKind, v1: Value, v2: Value) -> Value {
        let inst = self.create_inst_value(
            Opcode::FCmp,
            vec![
                Operand::FCmpKind(kind),
                Operand::Value(v1),
                Operand::Value(v2),
            ],
            Type::Int1,
        );
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_br(&mut self, dst_id: BasicBlockId) -> Value {
        let inst =
            self.create_inst_value(Opcode::Br, vec![Operand::BasicBlock(dst_id)], Type::Void);
        self.append_inst_to_cur_bb(inst);

        let cur_bb_id = self.cur_bb.unwrap();
        self.with_function(|f| {
            f.basic_block_ref_mut(cur_bb_id).succ.insert(dst_id);
            f.basic_block_ref_mut(dst_id).pred.insert(cur_bb_id);
        });

        inst
    }

    pub fn build_cond_br(&mut self, cond: Value, bb1: BasicBlockId, bb2: BasicBlockId) -> Value {
        let cur_bb_id = self.cur_bb.unwrap();
        let inst = self.create_inst_value(
            Opcode::CondBr,
            vec![
                Operand::Value(cond),
                Operand::BasicBlock(bb1),
                Operand::BasicBlock(bb2),
            ],
            Type::Void,
        );
        self.append_inst_to_cur_bb(inst);

        self.with_function(|f| {
            let cur_bb = f.basic_block_ref_mut(cur_bb_id);
            cur_bb.succ.insert(bb1);
            cur_bb.succ.insert(bb2);

            f.basic_block_ref_mut(bb1).pred.insert(cur_bb_id);
            f.basic_block_ref_mut(bb2).pred.insert(cur_bb_id);
        });

        inst
    }

    pub fn build_phi(&mut self, pairs: Vec<(Value, BasicBlockId)>) -> Value {
        let ty = pairs.get(0).unwrap().0.get_type();
        let mut operands = vec![];
        for (v, bb) in pairs {
            operands.push(Operand::Value(v));
            operands.push(Operand::BasicBlock(bb));
        }
        let inst = self.create_inst_value(Opcode::Phi, operands, ty);
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_call(&mut self, f: Value, args: Vec<Value>) -> Value {
        let ret_ty = self
            .func
            .func_ref()
            .types
            .base
            .borrow()
            .as_function_ty(f.get_type())
            .unwrap()
            .ret_ty;
        let mut operands = vec![Operand::Value(f)];
        operands.extend(args.iter().map(|&v| Operand::Value(v)));
        let inst = self.create_inst_value(Opcode::Call, operands, ret_ty);
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn build_ret(&mut self, v: Value) -> Value {
        let inst = self.create_inst_value(Opcode::Ret, vec![Operand::Value(v)], Type::Void);
        self.append_inst_to_cur_bb(inst);
        inst
    }

    pub fn is_last_inst_terminator(&self) -> bool {
        let bb = self.func.func_ref().basic_block_ref(self.cur_bb.unwrap());
        bb.iseq_ref().last().map_or(false, |i| {
            self.func.func_ref().inst_table[i.as_instruction().id]
                .opcode
                .is_terminator()
        })
    }

    // Utils

    fn create_inst_value(&mut self, opcode: Opcode, operands: Vec<Operand>, ret_ty: Type) -> Value {
        let inst = Instruction::new(opcode, operands, ret_ty, self.cur_bb.unwrap());
        let inst_id = self.func.func_ref_mut().alloc_inst(inst);
        Value::Instruction(InstructionValue {
            func_id: self.func.func_ref().id.unwrap(),
            id: inst_id,
            ty: ret_ty,
        })
    }

    fn append_inst_to_cur_bb(&mut self, inst: Value) {
        let bb_id = self.cur_bb.unwrap();
        let insert_point = self.insert_point;
        self.insert_point += 1;

        let bb = self.func.func_ref().basic_block_ref(bb_id);
        bb.iseq_ref_mut().insert(insert_point, inst);
    }

    fn with_function<Func, T>(&mut self, mut f: Func) -> T
    where
        Func: FnMut(&mut Function) -> T,
    {
        let function = self.func.func_ref_mut();
        f(function)
    }
}
