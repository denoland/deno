 
use deno_core::napi::*;

#[no_mangle]
pub unsafe extern "C" fn napi_fatal_exception(
  env: napi_env,
  value: napi_value,
) -> ! {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let error = value.to_rust_string_lossy(env.scope);
  panic!(
    "Fatal exception triggered by napi_fatal_exception!\n{}",
    error
  );
}
