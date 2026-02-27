// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;

use super::TestData;

#[op2]
pub fn op_v8slice_store(
  state: &mut OpState,
  #[string] name: String,
  #[buffer] data: JsBuffer,
) {
  state.borrow_mut::<TestData>().insert(name, data);
}

#[op2]
#[buffer]
pub fn op_v8slice_clone(state: &OpState, #[string] name: String) -> Vec<u8> {
  state.borrow::<TestData>().get::<JsBuffer>(name).to_vec()
}
