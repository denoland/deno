 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_new_instance(
  env: napi_env,
  constructor: napi_value,
  argc: usize,
  argv: *const napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let constructor: v8::Local<v8::Value> = std::mem::transmute(constructor);
  let constructor = v8::Local::<v8::Function>::try_from(constructor).unwrap();
  let args: &[v8::Local<v8::Value>] =
    std::mem::transmute(std::slice::from_raw_parts(argv, argc));
  let inst = constructor.new_instance(env.scope, args).unwrap();
  let value: v8::Local<v8::Value> = inst.into();
  *result = std::mem::transmute(value);
  Ok(())
}
