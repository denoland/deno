use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_global(env: napi_env, result: *mut napi_value) -> Result {
  let mut env = &mut *(env as *mut Env);
  let context = env.scope.get_current_context();
  let global = context.global(env.scope);
  let value: v8::Local<v8::Value> = global.into();
  *result = std::mem::transmute(value);
  Ok(())
}
