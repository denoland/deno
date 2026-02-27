// Copyright 2018-2025 the Deno authors. MIT license.
//!  This example shows you how to define ops in Rust and then call them from
//!  JavaScript.

use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

fn main() {
  let my_ext = Extension {
    name: "my_ext",
    middleware_fn: Some(Box::new(|op| match op.name {
      "op_print" => op.disable(),
      _ => op,
    })),
    ..Default::default()
  };

  // Initialize a runtime instance
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![my_ext],
    ..Default::default()
  });

  // Deno.core.print() will now be a NOP
  runtime
    .execute_script("<usage>", r#"Deno.core.print("I'm broken")"#)
    .unwrap();
}
