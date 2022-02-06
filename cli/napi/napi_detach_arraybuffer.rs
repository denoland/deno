use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_detach_arraybuffer(env: napi_env, value: napi_value) -> Result {
  let mut env = &mut (env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let ab = v8::Local::<v8::ArrayBuffer>::try_from(value).unwrap();
  ab.detach();
  Ok(())
}
