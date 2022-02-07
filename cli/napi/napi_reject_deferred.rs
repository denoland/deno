use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_reject_deferred(
  env: napi_env,
  deferred: napi_deferred,
  error: napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let resolver = &*(deferred as *const v8::Global<v8::Value>);
  let resolver = v8::Local::new(env.scope, resolver);
  let resolver: v8::Local<v8::PromiseResolver> = v8::Local::cast(resolver);
  resolver
    .reject(env.scope, std::mem::transmute(error))
    .unwrap();
  Ok(())
}
