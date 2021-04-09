// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
//!  This example shows you how to define ops in Rust and then call them from
//!  JavaScript.

use deno_core::json_op_sync;
use deno_core::JsRuntime;
use std::io::Write;

fn main() {
  // Initialize a runtime instance
  let mut runtime = JsRuntime::new(Default::default());

  // The first thing we do is define two ops.  They will be used to show how to
  // pass data to Rust and back to JavaScript.
  //
  // The first one is used to print data to stdout, because by default the
  // JavaScript console functions are just stubs (they don't do anything).
  //
  // The second one just transforms some input and returns it to JavaScript.

  // Register the op for outputting a string to stdout.
  // It can be invoked with Deno.core.dispatch and the id this method returns
  // or Deno.core.dispatchByName and the name provided.
  runtime.register_op(
    "op_print",
    // The op_fn callback takes a state object OpState,
    // a structured arg of type `T` and an optional ZeroCopyBuf,
    // a mutable reference to a JavaScript ArrayBuffer
    json_op_sync(|_state, msg: Option<String>, zero_copy| {
      let mut out = std::io::stdout();

      // Write msg to stdout
      if let Some(msg) = msg {
        out.write_all(msg.as_bytes()).unwrap();
      }

      // Write the contents of every buffer to stdout
      if let Some(buf) = zero_copy {
        out.write_all(&buf).unwrap();
      }

      Ok(()) // No meaningful result
    }),
  );

  // Register the JSON op for summing a number array.
  runtime.register_op(
    "op_sum",
    // The json_op_sync function automatically deserializes
    // the first ZeroCopyBuf and serializes the return value
    // to reduce boilerplate
    json_op_sync(|_state, nums: Vec<f64>, _| {
      // Sum inputs
      let sum = nums.iter().fold(0.0, |a, v| a + v);
      // return as a Result<f64, AnyError>
      Ok(sum)
    }),
  );

  // Now we see how to invoke the ops we just defined. The runtime automatically
  // contains a Deno.core object with several functions for interacting with it.
  // You can find its definition in core.js.
  runtime
    .execute(
      "<init>",
      r#"
// First we initialize the ops cache.
// This maps op names to their id's.
Deno.core.ops();

// Then we define a print function that uses
// our op_print op to display the stringified argument.
const _newline = new Uint8Array([10]);
function print(value) {
  Deno.core.dispatchByName('op_print', 0, value.toString(), _newline);
}

// Finally we register the error class used by op_sum
// so that it throws the correct class.
Deno.core.registerErrorClass('Error', Error);
"#,
    )
    .unwrap();

  // Now we can finally use this in an example.
  runtime
    .execute(
      "<usage>",
      r#"
const arr = [1, 2, 3];
print("The sum of");
print(arr);
print("is");
print(Deno.core.jsonOpSync('op_sum', arr));

// And incorrect usage
try {
  print(Deno.core.jsonOpSync('op_sum', 0));
} catch(e) {
  print('Exception:');
  print(e);
}
"#,
    )
    .unwrap();
}
