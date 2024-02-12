// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_runtime::deno_napi::*;

pub fn get_array_buffer_ptr(ab: v8::Local<v8::ArrayBuffer>) -> *mut u8 {
  // SAFETY: Thanks to the null pointer optimization, NonNull<T> and Option<NonNull<T>> are guaranteed
  // to have the same size and alignment.
  unsafe { std::mem::transmute(ab.data()) }
}
