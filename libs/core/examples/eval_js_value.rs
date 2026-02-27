// Copyright 2018-2025 the Deno authors. MIT license.
//!  This example shows you how to evaluate JavaScript expression and deserialize
//!  return value into a Rust object.

// NOTE:
// Here we are deserializing to `serde_json::Value` but you can
// deserialize to any other type that implements the `Deserialize` trait.

use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use deno_core::v8;

fn main() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());

  // Evaluate some code
  let code = "let a = 1+4; a*2";
  let output: serde_json::Value =
    eval(&mut runtime, code).expect("Eval failed");

  println!("Output: {output:?}");

  let expected_output = serde_json::json!(10);
  assert_eq!(expected_output, output);
}

fn eval(
  context: &mut JsRuntime,
  code: &'static str,
) -> Result<serde_json::Value, String> {
  let res = context.execute_script("<anon>", code);
  match res {
    Ok(global) => {
      deno_core::scope!(scope, context);
      let local = v8::Local::new(scope, global);
      // Deserialize a `v8` object into a Rust type using `serde_v8`,
      // in this case deserialize to a JSON `Value`.
      let deserialized_value =
        serde_v8::from_v8::<serde_json::Value>(scope, local);

      match deserialized_value {
        Ok(value) => Ok(value),
        Err(err) => Err(format!("Cannot deserialize value: {err:?}")),
      }
    }
    Err(err) => Err(format!("Evaling error: {err:?}")),
  }
}
