// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_runtime::deno_napi::*;
use std::cell::Cell;

unsafe fn get_backing_store_slice(
  backing_store: &mut v8::SharedRef<v8::BackingStore>,
  byte_offset: usize,
  byte_length: usize,
) -> &mut [u8] {
  let cells: *const [Cell<u8>] =
    &backing_store[byte_offset..byte_offset + byte_length];
  let mut bytes = cells as *mut [u8];
  &mut *bytes
}

pub fn get_array_buffer_ptr(ab: v8::Local<v8::ArrayBuffer>) -> *mut u8 {
  let mut backing_store = ab.get_backing_store();
  let byte_length = ab.byte_length();
  let mut slice =
    unsafe { get_backing_store_slice(&mut backing_store, 0, byte_length) };
  slice.as_mut_ptr()
}
