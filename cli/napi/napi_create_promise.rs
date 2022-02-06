use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_promise(
  env: napi_env,
  deferred: *mut napi_deferred,
  promise_out: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let resolver = v8::PromiseResolver::new(env.scope).unwrap();
  let promise: v8::Local<v8::Value> = resolver.get_promise(env.scope).into();
  *deferred = std::mem::transmute(resolver);
  *promise_out = std::mem::transmute(promise);
  Ok(())
}
