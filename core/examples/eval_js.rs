// NOTE:
// Here we are using serde_json::Value
// but you can use anything that implementes the Deserialize trait

use deno_core::{JsRuntime, RuntimeOptions};

fn main() {
  // Create the js runtime
  let opts = RuntimeOptions::default();
  let mut runtime = JsRuntime::new(opts);

  // Evaluate some code
  let code = "let a = 1+4; a*2";
  let expected_output = serde_json::json!(10);
  let output: serde_json::Value =
    eval(&mut runtime, code).expect("Eval failed");
  println!("Output: {:?}", output);
  assert_eq!(expected_output, output);
}

fn eval(
  context: &mut JsRuntime,
  code: &str,
) -> Result<serde_json::Value, String> {
  let res = context.execute_script("<anon>", code);
  match res {
    Ok(global) => {
      let mut scope = context.handle_scope();
      let local = deno_core::v8::Local::new(&mut scope, global);
      let value_str_res =
        serde_v8::from_v8::<serde_json::Value>(&mut scope, local);
      match value_str_res {
        Ok(value) => Ok(value),
        Err(err) => Err(format!("Cannot deserialize value: {:?}", err)),
      }
    }
    Err(err) => Err(format!("Evaling error: {:?}", err)),
  }
}
