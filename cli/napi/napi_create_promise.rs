use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_promise(
  env: napi_env,
  deferred: *mut napi_deferred,
  promise_out: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let resolver = v8::PromiseResolver::new(env.scope).unwrap();
  let mut global = v8::Global::new(env.scope, resolver);
  let mut global_ptr = global.into_raw();
  let promise: v8::Local<v8::Value> = resolver.get_promise(env.scope).into();
  *deferred = (global_ptr.as_mut() as *mut _ as napi_deferred);
  *promise_out = std::mem::transmute(promise);

  Ok(())
}
