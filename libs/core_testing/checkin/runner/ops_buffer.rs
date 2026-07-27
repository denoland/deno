// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::BufView;
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

/// Round-trips a buffer through the `JsBuffer` arm of [`BufView::into_bytes`]
/// and returns the resulting bytes, so both the zero-copy handoff and the
/// resizable-store copy fallback can be exercised from JS.
#[op2]
#[buffer]
pub fn op_v8slice_into_bytes(#[buffer] data: JsBuffer) -> Vec<u8> {
  BufView::from(data).into_bytes().to_vec()
}
