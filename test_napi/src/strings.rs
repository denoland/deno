use napi_sys::Status::napi_ok;
use napi_sys::*;
use std::ptr;

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

extern "C" fn test_utf8(env: napi_env, info: napi_callback_info) -> napi_value {
  let (args, argc) = get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  args[0]
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    // utf8
    new_property!(env, "test_utf8", test_utf8),
    // utf16
    // latin1
  ];

  unsafe { napi_define_properties(env, exports, 1, properties.as_ptr()) };
  std::mem::forget(properties);
}
