 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_resolve_deferred(
  env: napi_env,
  deferred: napi_deferred,
  result: napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let resolver: v8::Local<v8::PromiseResolver> = std::mem::transmute(deferred);
  resolver
    .resolve(env.scope, std::mem::transmute(result))
    .unwrap();
  Ok(())
}
