use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_named_property(
  env: napi_env,
  object: napi_value,
  utf8_name: *const c_char,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let object: v8::Local<v8::Value> = std::mem::transmute(object);
  let utf8_name = std::ffi::CStr::from_ptr(utf8_name);
  let name = v8::String::new(env.scope, &utf8_name.to_string_lossy()).unwrap();
  let value: v8::Local<v8::Value> = object
    .to_object(env.scope)
    .unwrap()
    .get(env.scope, name.into())
    .unwrap();
  *result = std::mem::transmute(value);
  Ok(())
}
