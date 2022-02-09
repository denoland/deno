use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_throw_error(
  env: napi_env,
  _code: *const c_char,
  msg: *const c_char,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  // let code = CStr::from_ptr(code).to_str().unwrap();
  let msg = CStr::from_ptr(msg).to_str().unwrap();

  // let code = v8::String::new(env.scope, code).unwrap();
  let msg = v8::String::new(env.scope, msg).unwrap();

  let error = v8::Exception::error(env.scope, msg);
  env.scope.throw_exception(error);

  Ok(())
}
