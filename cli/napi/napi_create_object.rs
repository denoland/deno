 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_object(env: napi_env, result: *mut napi_value) -> Result {
  let mut env = &mut *(env as *mut Env);
  let object = v8::Object::new(env.scope);
  *result = std::mem::transmute(object);
  Ok(())
}
