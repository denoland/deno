 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_symbol(
  env: napi_env,
  description: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let description = match result.is_null() {
    true => None,
    false => Some(
      std::mem::transmute::<napi_value, v8::Local<v8::Value>>(description)
        .to_string(env.scope)
        .unwrap(),
    ),
  };
  let sym = v8::Symbol::new(env.scope, description);
  let local: v8::Local<v8::Value> = sym.into();
  *result = transmute(local);
  Ok(())
}
