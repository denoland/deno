use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_instanceof(
  _env: napi_env,
  _value: napi_value,
  _constructor: napi_value,
  _result: *mut bool,
) -> Result {
  // let mut env = &mut (env as *mut Env);
  // let value: v8::Local<v8::Value> = transmute(value);
  // let constructor: v8::Local<v8::Value> = transmute(constructor);
  // TODO: https://github.com/denoland/rusty_v8/pull/879
  // *result = value.instance_of(constructor);
  Ok(())
}
