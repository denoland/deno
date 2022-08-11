// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
//!  This example shows you how to define ops in Rust and then call them from
//!  JavaScript.

use deno_core::op;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

// This is a hack to make the `#[op]` macro work with
// deno_core examples.
// You can remove this:
use deno_core::*;

#[op]
fn op_sum(nums: Vec<f64>) -> Result<f64, deno_core::error::AnyError> {
  // Sum inputs
  let sum = nums.iter().fold(0.0, |a, v| a + v);
  // return as a Result<f64, AnyError>
  Ok(sum)
}

fn main() {
  // Build a deno_core::Extension providing custom ops
  let ext = Extension::builder()
    .ops(vec![
      // An op for summing an array of numbers
      // The op-layer automatically deserializes inputs
      // and serializes the returned Result & value
      op_sum::decl(),
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
print(Deno.core.ops.op_sum(arr));

// And incorrect usage
try {
  print(Deno.core.ops.op_sum(0));
} catch(e) {
  print('Exception:');
  print(e);
}
"#,
    )
    .unwrap();
}
