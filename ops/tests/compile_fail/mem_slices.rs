// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_ops::op;

#[op]
fn sync_test(slice: &mut [u32]) {
  //
}

#[op]
async fn async_test(slice: &[u8]) {
  // Memory slices are not allowed in async ops.
}

#[op]
fn async_test2(slice: &mut [u8]) -> impl Future<Output = ()> {
  // Memory slices are not allowed in async ops, even when not implemented as an
  // async function.
  async {}
}

fn main() {
  // pass
}
