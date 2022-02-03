 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_date(
  env: napi_env,
  time: f64,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> =
    v8::Date::new(env.scope, time).unwrap().into();
  *result = std::mem::transmute(value);
  Ok(())
}
