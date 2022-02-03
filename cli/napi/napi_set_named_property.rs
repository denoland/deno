 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_set_named_property(
  env: napi_env,
  object: napi_value,
  name: *const c_char,
  value: napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let name = CStr::from_ptr(name).to_str().unwrap();
  let object: v8::Local<v8::Object> = std::mem::transmute(object);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let name = v8::String::new(env.scope, name).unwrap();
  object.set(env.scope, name.into(), value).unwrap();
  Ok(())
}
