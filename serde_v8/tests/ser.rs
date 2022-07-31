// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use serde::Serialize;
use serde_json::json;
use serde_v8::utils::{js_exec, v8_do};

#[derive(Debug, Serialize, PartialEq)]
struct MathOp {
  pub a: u64,
  pub b: u64,
  pub operator: Option<String>,
}

// Utility JS code (obj equality, etc...)
const JS_UTILS: &str = r#"
// Shallow obj equality (don't use deep objs for now)
function objEqual(a, b) {
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  return ka.length === kb.length && ka.every(k => a[k] === b[k]);
}

function arrEqual(a, b) {
  return a.length === b.length && a.every((v, i) => v === b[i]);
}
"#;
const JS_POLLUTE: &str = r#"
Object.defineProperty(Array.prototype, "0", {
  set: function (v) {
    throw new Error("Polluted Array 0 set");
  },
});

Object.defineProperty(Object.prototype, "a", {
  set: (v) => {
    throw new Error("Polluted Object 'a' set");
  }
});
"#;

fn sercheck<T: Serialize>(val: T, code: &str, pollute: bool) -> bool {
  let mut equal = false;

  v8_do(|| {
    // Setup isolate
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let handle_scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(handle_scope);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    // Load util functions
    js_exec(scope, JS_UTILS);
    if pollute {
      js_exec(scope, JS_POLLUTE);
    }
    // TryCatch scope (to catch pollution exceptions)
    let scope = &mut v8::TryCatch::new(scope);

    // Set value as "x" in global scope
    let global = context.global(scope);
    let v8_key = serde_v8::to_v8(scope, "x").unwrap();
    let v8_val = serde_v8::to_v8(scope, val).unwrap();
    global.set(scope, v8_key, v8_val);

    // Pollution check
    if let Some(message) = scope.message() {
      let msg = message.get(scope).to_rust_string_lossy(scope);
      panic!("JS Exception: {}", msg);
    }

    // Execute equality check in JS (e.g: x == ...)
    let v = js_exec(scope, code);
    // Cast to bool
    equal = serde_v8::from_v8(scope, v).unwrap();
  });

  equal
}

macro_rules! sertest {
  ($fn_name:ident, $rust:expr, $src:expr) => {
    #[test]
    fn $fn_name() {
      assert!(
        sercheck($rust, $src, false),
        "Expected: {} where x={:?}",
        $src,
        $rust,
      );
    }
  };
}

macro_rules! sertest_polluted {
  ($fn_name:ident, $rust:expr, $src:expr) => {
    #[test]
    fn $fn_name() {
      assert!(
        sercheck($rust, $src, true),
        "Expected: {} where x={:?}",
        $src,
        $rust,
      );
    }
  };
}

sertest!(ser_char, 'é', "x === 'é'");
sertest!(ser_option_some, Some(true), "x === true");
sertest!(ser_option_null, None as Option<bool>, "x === null");
sertest!(ser_unit_null, (), "x === null");
sertest!(ser_bool, true, "x === true");
sertest!(ser_u64, 32, "x === 32");
sertest!(ser_f64, 12345.0, "x === 12345.0");
sertest!(ser_string, "Hello", "x === 'Hello'");
sertest!(ser_bytes, b"\x01\x02\x03", "arrEqual(x, [1, 2, 3])");
sertest!(ser_vec_u64, vec![1, 2, 3, 4, 5], "arrEqual(x, [1,2,3,4,5])");
sertest!(
  ser_vec_string,
  vec!["hello", "world"],
  "arrEqual(x, ['hello', 'world'])"
);
sertest!(ser_tuple, (123, true, ()), "arrEqual(x, [123, true, null])");
sertest!(
  ser_mathop,
  MathOp {
    a: 1,
    b: 3,
    operator: None
  },
  "objEqual(x, {a: 1, b: 3, operator: null})"
);

sertest!(
  ser_map,
  {
    let map: std::collections::BTreeMap<&str, u32> =
      vec![("a", 1), ("b", 2), ("c", 3)].drain(..).collect();
    map
  },
  "objEqual(x, {a: 1, b: 2, c: 3})"
);

////
// JSON tests: json!() compatibility
////
sertest!(ser_json_bool, json!(true), "x === true");
sertest!(ser_json_null, json!(null), "x === null");
sertest!(ser_json_int, json!(123), "x === 123");
sertest!(ser_json_f64, json!(123.45), "x === 123.45");
sertest!(ser_json_string, json!("Hello World"), "x === 'Hello World'");
sertest!(ser_json_obj_empty, json!({}), "objEqual(x, {})");
sertest!(
  ser_json_obj,
  json!({"a": 1, "b": 2, "c": true}),
  "objEqual(x, {a: 1, b: 2, c: true})"
);
sertest!(
  ser_json_vec_int,
  json!([1, 2, 3, 4, 5]),
  "arrEqual(x, [1,2,3,4,5])"
);
sertest!(
  ser_json_vec_string,
  json!(["Goodbye", "Dinosaurs 👋☄️"]),
  "arrEqual(x, ['Goodbye', 'Dinosaurs 👋☄️'])"
);
sertest!(
  ser_json_tuple,
  json!([true, 42, "nabla"]),
  "arrEqual(x, [true, 42, 'nabla'])"
);

////
// Pollution tests
////

sertest_polluted!(
  ser_polluted_obj,
  MathOp {
    a: 1,
    b: 2,
    operator: None
  },
  "objEqual(x, { a: 1, b: 2, operator: null })"
);

sertest_polluted!(
  ser_polluted_tuple,
  (true, 123, false),
  "arrEqual(x, [true, 123, false])"
);

sertest_polluted!(ser_polluted_vec, vec![1, 2, 3], "arrEqual(x, [1, 2, 3])");
