use napi_sys::*;

pub mod strings;

#[no_mangle]
unsafe extern "C" fn napi_register_module_v1(
  env: napi_env,
  exports: napi_value,
) -> napi_value {
  strings::init(env, exports);
  exports
}