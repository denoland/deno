 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_has_named_property(
  env: napi_env,
  value: napi_value,
  key: *const c_char,
  result: *mut bool,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = transmute(value);
  let obj = value.to_object(env.scope).unwrap();
  let key = CStr::from_ptr(key).to_str().unwrap();
  let key = v8::String::new(env.scope, key).unwrap();
  *result = obj.has(env.scope, key.into()).unwrap_or(false);
  Ok(())
}
