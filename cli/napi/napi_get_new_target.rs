use super::function::CallbackInfo;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_new_target(
  env: &mut Env,
  cbinfo: &CallbackInfo,
  result: &mut v8::Local<v8::Value>,
) -> Result {
  let info = &*(cbinfo.args as *const v8::FunctionCallbackArguments);
  *result = info.new_target();
  Ok(())
}
