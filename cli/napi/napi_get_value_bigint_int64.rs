use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_value_bigint_int64(
  env: napi_env,
  value: napi_value,
  result: *mut i64,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let bigint = value.to_big_int(env.scope).unwrap();
  *result = bigint.i64_value().0;
  Ok(())
}
