macro_rules! some_then {
    ($x:pat, $e:expr, $t:expr) => {{
        if let Some($x) = $e {
            $t
        }
    }};
}

macro_rules! when_debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            $($arg)*;
        }
    };
}

macro_rules! matches {
    ($e:expr, $p:pat) => {
        match $e {
            $p => true,
            _ => false,
        }
    };
}

#[rustfmt::skip]
macro_rules! reg {
    ($f:expr; $instr_id:expr) => {{
        $f.instr_table[$instr_id].reg.borrow().reg.unwrap()
            .shift(REGISTER_OFFSET).as_u8()
    }};
    ($instr:expr) => {{
        $instr.reg.borrow().reg.unwrap()
            .shift(REGISTER_OFFSET).as_u8()
    }};
}

#[rustfmt::skip]
macro_rules! vreg {
    ($f:expr ; $instr_id:expr) => {{
        $f.instr_table[$instr_id].reg.borrow().vreg
    }};
    ($instr:expr) => {{
        $instr.reg.borrow().vreg
    }};
}


#[macro_export]
macro_rules! cilk_parse_ty {
    (i32) => {
        types::Type::Int32
    };
    (void) => {
        types::Type::Void
    };
}

#[macro_export]
macro_rules! cilk_value {
    ($builder:expr; %arg . $n:expr) => {{
        $builder.get_param($n).unwrap()
    }};
    ($builder:expr; void) => {{
        value::Value::None
    }};
    ($builder:expr; i32 $n:expr) => {{
        value::Value::Immediate(value::ImmediateValue::Int32($n))
    }};
    ($builder:expr; % $n:expr) => {{
        $n
    }};
}

#[macro_export]
macro_rules! icmp_kind {
    (le) => {
        opcode::ICmpKind::Le
    };
    (eq) => {
        opcode::ICmpKind::Eq
    };
}

#[macro_export]
macro_rules! cilk_expr {
    ($builder:expr; $bb_map:expr; $label:ident : $($remain:tt)*) => {
        let bb = *$bb_map.entry(stringify!($label)).or_insert_with(|| $builder.append_basic_block());
        $builder.set_insert_point(bb);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = alloca $ty:ident; $($remain:tt)*) => {
        let $x = $builder.build_alloca(cilk_parse_ty!($ty));
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = load ($($val:tt)*); $($remain:tt)*) => {
        let val= cilk_value!($builder; $( $val )*);
        let $x = $builder.build_load(val);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; store ($($val1:tt)*), ($($val2:tt)*); $($remain:tt)*) => {
        let src = cilk_value!($builder; $( $val1 )*);
        let dst = cilk_value!($builder; $( $val2 )*);
        $builder.build_store(src, dst);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = add ($($val1:tt)*), ($($val2:tt)*); $($remain:tt)*) => {
        let val1 = cilk_value!($builder; $( $val1 )*);
        let val2 = cilk_value!($builder; $( $val2 )*);
        let $x = $builder.build_add(val1, val2);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = sub ($($val1:tt)*), ($($val2:tt)*); $($remain:tt)*) => {
        let val1 = cilk_value!($builder; $( $val1 )*);
        let val2 = cilk_value!($builder; $( $val2 )*);
        let $x = $builder.build_sub(val1, val2);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = mul ($($val1:tt)*), ($($val2:tt)*); $($remain:tt)*) => {
        let val1 = cilk_value!($builder; $( $val1 )*);
        let val2 = cilk_value!($builder; $( $val2 )*);
        let $x = $builder.build_mul(val1, val2);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = rem ($($val1:tt)*), ($($val2:tt)*); $($remain:tt)*) => {
        let val1 = cilk_value!($builder; $( $val1 )*);
        let val2 = cilk_value!($builder; $( $val2 )*);
        let $x = $builder.build_rem(val1, val2);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = phi [$( [ ($($arg:tt)*), $bb:ident ] ),*] ; $($remain:tt)*) => {
        let args = vec![$(
                           (cilk_value!($builder; $( $arg )*),
                            *$bb_map.entry(stringify!($bb)).or_insert_with(|| $builder.append_basic_block()))
                       ),*];
        let $x = $builder.build_phi(args);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = call $name:ident [$( ( $($arg:tt)* ) ),*] ; $($remain:tt)*) => {
        let args = vec![ $( cilk_value!($builder; $( $arg )*) ),* ];
        let $x = $builder.build_call(value::Value::Function($builder.module.find_function_by_name(stringify!($name)).unwrap()), args);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = call (->$id:expr) [$( ( $($arg:tt)* ) ),*] ; $($remain:tt)*) => {
        let args = vec![ $( cilk_value!($builder; $( $arg )*) ),* ];
        let $x = $builder.build_call(value::Value::Function($id), args);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; $x:ident = icmp $kind:ident ($($val1:tt)*), ($($val2:tt)*); $($remain:tt)*) => {
        let val1 = cilk_value!($builder; $( $val1 )*);
        let val2 = cilk_value!($builder; $( $val2 )*);
        let $x = $builder.build_icmp(icmp_kind!($kind), val1, val2);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; br ($($cond:tt)*) $l1:ident, $l2:ident; $($remain:tt)*) => {
        let bb1 = *$bb_map.entry(stringify!($l1)).or_insert_with(|| $builder.append_basic_block());
        let bb2 = *$bb_map.entry(stringify!($l2)).or_insert_with(|| $builder.append_basic_block());
        let cond = cilk_value!($builder; $( $cond )*);
        $builder.build_cond_br(cond, bb1, bb2);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; br $label:ident; $($remain:tt)*) => {
        let bb = *$bb_map.entry(stringify!($label)).or_insert_with(|| $builder.append_basic_block());
        $builder.build_br(bb);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };
    ($builder:expr; $bb_map:expr; ret ($($val:tt)*) ; $($remain:tt)*) => {
        let val = cilk_value!($builder; $( $val )*);
        $builder.build_ret(val);
        cilk_expr!($builder; $bb_map; $( $remain )*);
    };

    ($builder:expr; $bb_map:expr; ) => {{}};
}

#[macro_export]
macro_rules! cilk_ir {
    ($m:expr; define [$ret_ty:ident] $name:ident ($( $arg:ident ),* ) { $($exp:tt)* }) => {{
        let f = $m.add_function(function::Function::new(
                stringify!($name),
                cilk_parse_ty!($ret_ty),
                vec![$( cilk_parse_ty!($arg) ),*],
                ));
        let mut builder = builder::Builder::new(&mut $m, f);
        let mut bb_map: FxHashMap<&str, basic_block::BasicBlockId> = FxHashMap::default();
        cilk_expr!(builder; bb_map; $( $exp )*);
        f
    }};
}
