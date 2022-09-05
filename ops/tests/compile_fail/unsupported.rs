// Copyright 2019-2020 the Deno authors. All rights reserved. MIT license.

use deno_ops::op;

#[op(fast)]
fn op_result_return(a: i32, b: i32) -> Result<(), ()> {
  a + b
}

#[op(fast)]
fn op_u8_arg(a: u8, b: u8) {
  //
}

#[op(fast)]
fn op_u16_arg(a: u16, b: u16) {
  //
}

use deno_core::v8::fast_api::FastApiCallbackOptions;

#[op(fast)]
fn op_callback_options(options: &mut FastApiCallbackOptions) {
  // fast callback options must be an Option.
}

#[op(fast)]
async fn op_async_fn(a: i32, b: i32) -> i32 {
  a + b
}

fn main() {
  // pass
}
