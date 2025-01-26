// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::cstr;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn check_error(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);
  let mut r = false;
  assert_napi_ok!(napi_is_error(env, args[0], &mut r));
  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, r, &mut result));
  result
}

extern "C" fn create_error(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut result: napi_value = ptr::null_mut();
  let mut message: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("error"),
    usize::MAX,
    &mut message
  ));
  assert_napi_ok!(napi_create_error(
    env,
    ptr::null_mut(),
    message,
    &mut result
  ));
  result
}

extern "C" fn create_range_error(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut result: napi_value = ptr::null_mut();
  let mut message: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("range error"),
    usize::MAX,
    &mut message
  ));
  assert_napi_ok!(napi_create_range_error(
    env,
    ptr::null_mut(),
    message,
    &mut result
  ));
  result
}

extern "C" fn create_type_error(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut result: napi_value = ptr::null_mut();
  let mut message: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("type error"),
    usize::MAX,
    &mut message
  ));
  assert_napi_ok!(napi_create_type_error(
    env,
    ptr::null_mut(),
    message,
    &mut result
  ));
  result
}

extern "C" fn create_error_code(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut result: napi_value = ptr::null_mut();
  let mut message: napi_value = ptr::null_mut();
  let mut code: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("Error [error]"),
    usize::MAX,
    &mut message
  ));
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("ERR_TEST_CODE"),
    usize::MAX,
    &mut code
  ));
  assert_napi_ok!(napi_create_error(env, code, message, &mut result));
  result
}

extern "C" fn create_range_error_code(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut result: napi_value = ptr::null_mut();
  let mut message: napi_value = ptr::null_mut();
  let mut code: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("RangeError [range error]"),
    usize::MAX,
    &mut message
  ));
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("ERR_TEST_CODE"),
    usize::MAX,
    &mut code
  ));
  assert_napi_ok!(napi_create_range_error(env, code, message, &mut result));
  result
}

extern "C" fn create_type_error_code(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut result: napi_value = ptr::null_mut();
  let mut message: napi_value = ptr::null_mut();
  let mut code: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("TypeError [type error]"),
    usize::MAX,
    &mut message
  ));
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("ERR_TEST_CODE"),
    usize::MAX,
    &mut code
  ));
  assert_napi_ok!(napi_create_type_error(env, code, message, &mut result));
  result
}

extern "C" fn throw_existing_error(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut message: napi_value = ptr::null_mut();
  let mut error: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("existing error"),
    usize::MAX,
    &mut message
  ));
  assert_napi_ok!(napi_create_error(
    env,
    std::ptr::null_mut(),
    message,
    &mut error
  ));
  assert_napi_ok!(napi_throw(env, error));
  std::ptr::null_mut()
}

extern "C" fn throw_error(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  assert_napi_ok!(napi_throw_error(env, std::ptr::null_mut(), cstr!("error"),));
  std::ptr::null_mut()
}

extern "C" fn throw_range_error(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  assert_napi_ok!(napi_throw_range_error(
    env,
    std::ptr::null_mut(),
    cstr!("range error"),
  ));
  std::ptr::null_mut()
}

extern "C" fn throw_type_error(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  assert_napi_ok!(napi_throw_type_error(
    env,
    std::ptr::null_mut(),
    cstr!("type error"),
  ));
  std::ptr::null_mut()
}

extern "C" fn throw_arbitrary(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);
  assert_napi_ok!(napi_throw(env, args[0]));
  std::ptr::null_mut()
}

extern "C" fn throw_error_code(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  assert_napi_ok!(napi_throw_error(
    env,
    cstr!("ERR_TEST_CODE"),
    cstr!("Error [error]"),
  ));
  std::ptr::null_mut()
}

extern "C" fn throw_range_error_code(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  assert_napi_ok!(napi_throw_range_error(
    env,
    cstr!("ERR_TEST_CODE"),
    cstr!("RangeError [range error]"),
  ));
  std::ptr::null_mut()
}

extern "C" fn throw_type_error_code(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  assert_napi_ok!(napi_throw_type_error(
    env,
    cstr!("ERR_TEST_CODE"),
    cstr!("TypeError [type error]"),
  ));
  std::ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "checkError", check_error),
    napi_new_property!(env, "throwExistingError", throw_existing_error),
    napi_new_property!(env, "throwError", throw_error),
    napi_new_property!(env, "throwRangeError", throw_range_error),
    napi_new_property!(env, "throwTypeError", throw_type_error),
    // NOTE(bartlomieju): currently experimental api
    // napi_new_property!(env, "throwSyntaxError", throw_syntax_error),
    napi_new_property!(env, "throwErrorCode", throw_error_code),
    napi_new_property!(env, "throwRangeErrorCode", throw_range_error_code),
    napi_new_property!(env, "throwTypeErrorCode", throw_type_error_code),
    // NOTE(bartlomieju): currently experimental api
    // napi_new_property!(env, "throwSyntaxErrorCode", throw_syntax_error_code),
    napi_new_property!(env, "throwArbitrary", throw_arbitrary),
    napi_new_property!(env, "createError", create_error),
    napi_new_property!(env, "createRangeError", create_range_error),
    napi_new_property!(env, "createTypeError", create_type_error),
    // NOTE(bartlomieju): currently experimental api
    // napi_new_property!(env, "createSyntaxError", create_syntax_error),
    napi_new_property!(env, "createErrorCode", create_error_code),
    napi_new_property!(env, "createRangeErrorCode", create_range_error_code),
    napi_new_property!(env, "createTypeErrorCode", create_type_error_code),
    // NOTE(bartlomieju): currently experimental api
    // napi_new_property!(env, "createSyntaxErrorCode", create_syntax_error_code),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
