use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_run_script(
  env: napi_env,
  script: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let script: v8::Local<v8::Value> = std::mem::transmute(script);
  if !script.is_string() {
    return Err(Error::StringExpected);
  }
  let script = script.to_string(env.scope).unwrap();

  let script = v8::Script::compile(env.scope, script, None);
  if script.is_none() {
    return Err(Error::GenericFailure);
  }
  let script = script.unwrap();
  let rv = script.run(env.scope);

  if let Some(rv) = rv {
    *result = std::mem::transmute(rv);
  } else {
    return Err(Error::GenericFailure);
  }

  Ok(())
}
