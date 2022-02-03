 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_date_value(
  env: napi_env,
  value: napi_value,
  result: *mut f64,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let date = v8::Local::<v8::Date>::try_from(value).unwrap();
  *result = date.number_value(env.scope).unwrap();
  Ok(())
}
