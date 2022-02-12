// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
//!  This example shows you how to define ops in Rust and then call them from
//!  JavaScript.

use deno_core::op_sync;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

fn main() {
  // Build a deno_core::Extension providing custom ops
  let ext = Extension::builder()
    .ops(vec![
      // An op for summing an array of numbers
      (
        "op_sum",
        // The op-layer automatically deserializes inputs
        // and serializes the returned Result & value
        op_sync(|_state, nums: Vec<f64>, _: ()| {
          // Sum inputs
          let sum = nums.iter().fold(0.0, |a, v| a + v);
          // return as a Result<f64, AnyError>
          Ok(sum)
        }),
      ),
    ])
    .build();

  // Initialize a runtime instance
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![ext],
    ..Default::default()
  });

  // Now we see how to invoke the op we just defined. The runtime automatically
  // contains a Deno.core object with several functions for interacting with it.
  // You can find its definition in core.js.
  runtime
    .execute_script(
      "<usage>",
      r#"
// Print helper function, calling Deno.core.print()
function print(value) {
  Deno.core.print(value.toString()+"\n");
}

const arr = [1, 2, 3];
print("The sum of");
print(arr);
print("is");
print(Deno.core.opSync('op_sum', arr));

// And incorrect usage
try {
  print(Deno.core.opSync('op_sum', 0));
} catch(e) {
  print('Exception:');
  print(e);
}
"#,
    )
    .unwrap();
}
