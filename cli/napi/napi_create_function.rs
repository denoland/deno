use super::function::create_function;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_function(
  env: &mut Env,
  name: *const u8,
  length: isize,
  cb: napi_callback,
  cb_info: napi_callback_info,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let name = match name.is_null() {
    true => None,
    false => Some(name),
  };
  let name = name.map(|name| {
    if length == -1 {
      std::ffi::CStr::from_ptr(name as *const _).to_str().unwrap()
    } else {
      let name = std::slice::from_raw_parts(name, length as usize);
      std::str::from_utf8(name).unwrap()
    }
  });
  let function = create_function(env, name, cb, cb_info);
  let value: v8::Local<v8::Value> = function.into();
  *result = std::mem::transmute(value);
  Ok(())
}
