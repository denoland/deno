use bencher::{benchmark_group, benchmark_main, Bencher};

use rusty_v8 as v8;

use serde::Serialize;

use serde_v8::utils::v8_do;

#[derive(Serialize)]
struct MathOp {
  arg1: u64,
  arg2: u64,
  operator: Option<String>,
}

fn serdo(f: impl FnOnce(&mut v8::HandleScope)) {
  v8_do(|| {
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let handle_scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(handle_scope);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    f(scope);
  })
}

macro_rules! dualbench {
  ($v8_fn:ident, $json_fn:ident, $src:expr) => {
    fn $v8_fn(b: &mut Bencher) {
      serdo(|scope| {
        let v = $src;
        b.iter(move || {
          let _ = serde_v8::to_v8(scope, &v).unwrap();
        });
      });
    }

    fn $json_fn(b: &mut Bencher) {
      let v = $src;
      b.iter(move || {
        let _ = serde_json::to_string(&v).unwrap();
      });
    }
  };
}

dualbench!(
  ser_struct_v8,
  ser_struct_json,
  MathOp {
    arg1: 10,
    arg2: 123,
    operator: None
  }
);
dualbench!(ser_bool_v8, ser_bool_json, true);
dualbench!(ser_int_v8, ser_int_json, 12345);
dualbench!(
  ser_array_v8,
  ser_array_json,
  vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
);
dualbench!(ser_str_v8, ser_str_json, "hello world");
dualbench!(ser_tuple_v8, ser_tuple_json, (1, false));

fn ser_struct_v8_manual(b: &mut Bencher) {
  serdo(|scope| {
    let v = MathOp {
      arg1: 10,
      arg2: 123,
      operator: None,
    };
    b.iter(|| {
      let obj = v8::Object::new(scope);
      let k1 = v8::String::new(scope, "arg1").unwrap();
      let k2 = v8::String::new(scope, "arg2").unwrap();
      let k3 = v8::String::new(scope, "operator").unwrap();
      //    let k1 = v8::String::new_from_utf8(scope, "arg1".as_ref(), v8::NewStringType::Internalized).unwrap();
      //    let k2 = v8::String::new_from_utf8(scope, "arg2".as_ref(), v8::NewStringType::Internalized).unwrap();
      //    let k3 = v8::String::new_from_utf8(scope, "operator".as_ref(), v8::NewStringType::Internalized).unwrap();
      let v1 = v8::Number::new(scope, v.arg1 as f64);
      let v2 = v8::Number::new(scope, v.arg2 as f64);
      let v3 = v8::null(scope);
      obj.set(scope, k1.into(), v1.into()).unwrap();
      obj.set(scope, k2.into(), v2.into()).unwrap();
      obj.set(scope, k3.into(), v3.into()).unwrap();
    });
  });
}

benchmark_group!(
  benches,
  ser_struct_v8,
  ser_struct_json,
  ser_bool_v8,
  ser_bool_json,
  ser_int_v8,
  ser_int_json,
  ser_array_v8,
  ser_array_json,
  ser_str_v8,
  ser_str_json,
  ser_tuple_v8,
  ser_tuple_json,
  ser_struct_v8_manual,
);
benchmark_main!(benches);
