 
use deno_core::napi::*;
use super::function::CallbackInfo;

#[napi_sym::napi_sym]
fn napi_get_cb_info(
  env: napi_env,
  cbinfo: napi_callback_info,
  argc: *mut i32,
  argv: *mut napi_value,
  this_arg: *mut napi_value,
  cb_data: *mut *mut c_void,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let cbinfo: &CallbackInfo = &*(cbinfo as *const CallbackInfo);
  let args = &*(cbinfo.args as *const v8::FunctionCallbackArguments);

  if !cb_data.is_null() {
    *cb_data = cbinfo.cb_info;
  }

  if !this_arg.is_null() {
    let mut this: v8::Local<v8::Value> = args.this().into();
    *this_arg = std::mem::transmute(this);
  }

  let len = args.length();
  let mut v_argc = len;
  if !argc.is_null() {
    v_argc = *argc;
    *argc = len;
  }

  if !argv.is_null() {
    let mut v_argv = std::slice::from_raw_parts_mut(argv, v_argc as usize);
    for i in 0..v_argc {
      let mut arg = args.get(i);
      v_argv[i as usize] = std::mem::transmute(arg);
    }
  }

  Ok(())
}
