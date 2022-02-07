use napi_sys::*;

pub mod numbers;
pub mod strings;
pub mod typedarray;

#[macro_export]
macro_rules! get_callback_info {
  ($env: expr, $callback_info: expr, $size: literal) => {{
    let mut args = [ptr::null_mut(); $size];
    let mut argc = $size;
    unsafe {
      assert!(
        napi_get_cb_info(
          $env,
          $callback_info,
          &mut argc,
          args.as_mut_ptr(),
          ptr::null_mut(),
          ptr::null_mut(),
        ) == napi_ok,
      )
    };
    (args, argc)
  }};
}

#[macro_export]
macro_rules! new_property {
  ($env: expr, $name: expr, $value: expr) => {
    napi_property_descriptor {
      utf8name: $name.as_ptr() as *const i8,
      name: ptr::null_mut(),
      method: Some($value),
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: 0,
      value: ptr::null_mut(),
    }
  };
}

#[no_mangle]
unsafe extern "C" fn napi_register_module_v1(
  env: napi_env,
  exports: napi_value,
) -> napi_value {
  strings::init(env, exports);
  numbers::init(env, exports);
  typedarray::init(env, exports);
  exports
}
