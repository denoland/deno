// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
//!  This example shows that op-panics currently result in UB (likely "failed to initiate panic")
//!  without a custom panic hook that aborts the process or -C panic=abort.
//!
//!  This happens due to the UB of panicking in an extern "C",
//!  given how ops are reduced via rusty_v8::MapFnTo
//!  See:
//!    - https://github.com/rust-lang/rust/issues/74990
//!    - https://rust-lang.github.io/rfcs/2945-c-unwind-abi.html

use deno_core::op;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

// This is a hack to make the `#[op]` macro work with
// deno_core examples.
// You can remove this:
use deno_core::*;

fn main() {
  #[op]
  fn op_panik() {
    panic!("panik !!!")
  }

  let extensions = vec![Extension::builder("my_ext")
    .ops(vec![op_panik::decl()])
    .build()];
  let mut rt = JsRuntime::new(RuntimeOptions {
    extensions,
    ..Default::default()
  });
  rt.execute_script("panik", "Deno.core.ops.op_panik()")
    .unwrap();
}
