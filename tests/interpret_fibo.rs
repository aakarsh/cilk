use cilk::{
    exec::interpreter::interp,
    ir::{builder, function, module, opcode, types, value},
};

#[test]
fn interpret_fibo() {
    // int fibo(int n) {
    //   if (n <= 2) return 1;
    //   return fibo(n - 1) + fibo(n - 2);
    // }

    let mut m = module::Module::new("cilk");

    let fibo = m.add_function(function::Function::new(
        "fibo",
        types::Type::Int32,
        vec![types::Type::Int32],
    ));
    let mut builder = builder::Builder::new(&mut m, fibo);

    let entry = builder.append_basic_block();
    let br1 = builder.append_basic_block();
    let br2 = builder.append_basic_block();
    builder.set_insert_point(entry);
    let arg0 = builder.get_param(0).unwrap();
    let eq1 = builder.build_icmp(
        opcode::ICmpKind::Le,
        arg0,
        value::Value::Immediate(value::ImmediateValue::Int32(2)),
    );
    builder.build_cond_br(eq1, br1, br2);
    builder.set_insert_point(br1);
    builder.build_ret(value::Value::Immediate(value::ImmediateValue::Int32(1)));
    builder.set_insert_point(br2);
    let fibo1arg = builder.build_sub(
        arg0,
        value::Value::Immediate(value::ImmediateValue::Int32(1)),
    );
    let fibo1 = builder.build_call(value::Value::Function(fibo), vec![fibo1arg]);
    let fibo2arg = builder.build_sub(
        arg0,
        value::Value::Immediate(value::ImmediateValue::Int32(2)),
    );
    let fibo2 = builder.build_call(value::Value::Function(fibo), vec![fibo2arg]);
    let add = builder.build_add(fibo1, fibo2);
    builder.build_ret(add);

    let f = m.function_ref(fibo);
    println!("{}", f.to_string(&m));

    let mut interp = interp::Interpreter::new(&m);
    let ret = interp.run_function(fibo, vec![interp::ConcreteValue::Int32(10)]);
    assert_eq!(ret, interp::ConcreteValue::Int32(55));

    println!("exec: fibo(10) = {:?}", ret);
}
