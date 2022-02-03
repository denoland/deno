 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_reject_deferred(
  env: napi_env,
  deferred: napi_deferred,
  error: napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let resolver: v8::Local<v8::PromiseResolver> = std::mem::transmute(deferred);
  resolver
    .reject(env.scope, std::mem::transmute(error))
    .unwrap();
  Ok(())
}
