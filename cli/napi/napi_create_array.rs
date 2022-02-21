use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_array(
  env: napi_env,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::Array::new(env.scope, 0).into();
  *result = std::mem::transmute::<v8::Local<v8::Value>, napi_value>(value);
  Ok(())
}
