use deno_core::napi::*;

use v8::BackingStore;
use v8::UniqueRef;

pub type BackingStoreDeleterCallback = unsafe extern "C" fn(
  data: *mut c_void,
  byte_length: usize,
  deleter_data: *mut c_void,
);
extern "C" {
  fn v8__ArrayBuffer__NewBackingStore__with_data(
    data: *mut c_void,
    byte_length: usize,
    deleter: BackingStoreDeleterCallback,
    deleter_data: *mut c_void,
  ) -> *mut BackingStore;
}

pub extern "C" fn backing_store_deleter_callback(
  data: *mut c_void,
  byte_length: usize,
  _deleter_data: *mut c_void,
) {
  let slice_ptr = ptr::slice_from_raw_parts_mut(data as *mut u8, byte_length);
  let b = unsafe { Box::from_raw(slice_ptr) };
  drop(b);
}

#[napi_sym::napi_sym]
fn napi_create_external_arraybuffer(
  env: napi_env,
  data: *mut c_void,
  byte_length: usize,
  _finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let _slice = std::slice::from_raw_parts(data as *mut u8, byte_length);
  // TODO: finalization
  let store: UniqueRef<BackingStore> =
    std::mem::transmute(v8__ArrayBuffer__NewBackingStore__with_data(
      data,
      byte_length,
      backing_store_deleter_callback,
      finalize_hint,
    ));

  let ab = v8::ArrayBuffer::with_backing_store(env.scope, &store.make_shared());
  let value: v8::Local<v8::Value> = ab.into();
  *result = std::mem::transmute(value);
  Ok(())
}
