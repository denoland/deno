use deno_core::napi::*;
use std::ptr::NonNull;

#[napi_sym::napi_sym]
fn napi_reject_deferred(
  env: napi_env,
  deferred: napi_deferred,
  error: napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let deferred_ptr =
    NonNull::new_unchecked(deferred as *mut v8::PromiseResolver);
  let resolver_global = v8::Global::<v8::PromiseResolver>::from_raw(
    &mut *env.isolate_ptr,
    deferred_ptr,
  );
  let resolver =
    v8::Local::<v8::PromiseResolver>::new(env.scope, resolver_global);
  resolver
    .reject(env.scope, std::mem::transmute(error))
    .unwrap();
  Ok(())
}
