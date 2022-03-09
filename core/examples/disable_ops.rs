// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
//!  This example shows you how to define ops in Rust and then call them from
//!  JavaScript.

use deno_core::v8::MapFnTo;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

fn main() {
  let my_ext = Extension::builder()
    .middleware(|name, opfn| match name {
      "op_print" => deno_core::void_op_sync.map_fn_to(),
      _ => opfn,
    })
    .build();

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
