use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_external_buffer(
  env: napi_env,
  byte_length: isize,
  data: *mut c_void,
  _finalize_cb: napi_finalize,
  _finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let slice = if byte_length == -1 {
    std::ffi::CStr::from_ptr(data as *const _).to_bytes()
  } else {
    std::slice::from_raw_parts(data as *mut u8, byte_length as usize)
  };
  // TODO: make this not copy the slice
  // TODO: finalization
  let store = v8::ArrayBuffer::new_backing_store_from_boxed_slice(
    slice.to_vec().into_boxed_slice(),
  );
  let ab = v8::ArrayBuffer::with_backing_store(env.scope, &store.make_shared());
  let value = v8::Uint8Array::new(env.scope, ab, 0, slice.len()).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}
