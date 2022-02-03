 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_string_utf8(
  env: napi_env,
  string: *const u8,
  length: isize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let string = if length == -1 {
    std::ffi::CStr::from_ptr(string as *const _)
      .to_str()
      .unwrap()
  } else {
    let string = std::slice::from_raw_parts(string, length as usize);
    std::str::from_utf8(string).unwrap()
  };
  let v8str = v8::String::new(env.scope, string).unwrap();
  let value: v8::Local<v8::Value> = v8str.into();
  *result = std::mem::transmute(value);

  Ok(())
}
