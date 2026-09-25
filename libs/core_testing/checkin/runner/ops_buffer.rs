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

/// Returns whether the `JsBuffer` arm of [`BufView::into_bytes`] handed off the
/// backing store zero-copy (`true`) or copied it (`false`), by comparing the
/// resulting `Bytes` pointer against the source slice pointer. This pins the
/// safety guard: fixed-length stores must alias, resizable/shared stores must
/// copy. Deleting the guard flips the resizable/shared result and fails the
/// test.
#[op2]
pub fn op_v8slice_into_bytes_aliases(#[buffer] data: JsBuffer) -> bool {
  let slice = data.into_parts();
  let src_ptr = slice.as_ref().as_ptr();
  let bytes = BufView::from(JsBuffer::from_parts(slice)).into_bytes();
  std::ptr::eq(bytes.as_ptr(), src_ptr)
}
