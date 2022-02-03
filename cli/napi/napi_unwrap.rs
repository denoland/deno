use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_unwrap(
  env: napi_env,
  value: napi_value,
  result: *mut *mut c_void,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let obj = value.to_object(env.scope).unwrap();
  let shared = &*(env.shared as *const EnvShared);
  let napi_wrap = v8::Local::new(env.scope, &shared.napi_wrap);
  let ext = obj.get_private(env.scope, napi_wrap).unwrap();
  let ext = v8::Local::<v8::External>::try_from(ext).unwrap();
  *result = ext.value();
  Ok(())
}
