// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr;

use napi_sys::Status::napi_pending_exception;
use napi_sys::ValueType::napi_bigint;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::cstr;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn is_lossless(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);
  assert_eq!(argc, 2);

  let mut is_signed = false;
  assert_napi_ok!(napi_get_value_bool(env, args[1], &mut is_signed));

  let mut lossless = false;

  if is_signed {
    let mut input: i64 = 0;
    assert_napi_ok!(napi_get_value_bigint_int64(
      env,
      args[0],
      &mut input,
      &mut lossless
    ));
  } else {
    let mut input: u64 = 0;
    assert_napi_ok!(napi_get_value_bigint_uint64(
      env,
      args[0],
      &mut input,
      &mut lossless
    ));
  }

  let mut output: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, lossless, &mut output));

  output
}

extern "C" fn test_int64(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, _argc, _) = napi_get_callback_info!(env, info, 2);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_bigint);

  let mut input: i64 = 0;
  let mut lossless = false;
  assert_napi_ok!(napi_get_value_bigint_int64(
    env,
    args[0],
    &mut input,
    &mut lossless
  ));

  let mut output: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_bigint_int64(env, input, &mut output));

  output
}

extern "C" fn test_uint64(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, _argc, _) = napi_get_callback_info!(env, info, 2);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_bigint);

  let mut input: u64 = 0;
  let mut lossless = false;
  assert_napi_ok!(napi_get_value_bigint_uint64(
    env,
    args[0],
    &mut input,
    &mut lossless
  ));

  let mut output: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_bigint_uint64(env, input, &mut output));

  output
}

extern "C" fn test_words(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, _argc, _) = napi_get_callback_info!(env, info, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_bigint);

  let mut expected_work_count = 0;
  assert_napi_ok!(napi_get_value_bigint_words(
    env,
    args[0],
    ptr::null_mut(),
    &mut expected_work_count,
    ptr::null_mut()
  ));

  let mut sign_bit = 0;
  let mut word_count: usize = 10;
  let mut words: Vec<u64> = Vec::with_capacity(10);

  assert_napi_ok!(napi_get_value_bigint_words(
    env,
    args[0],
    &mut sign_bit,
    &mut word_count,
    words.as_mut_ptr(),
  ));

  assert_eq!(word_count, expected_work_count);
  let mut output: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_create_bigint_words(
    env,
    sign_bit,
    word_count,
    words.as_ptr(),
    &mut output,
  ));
  output
}

extern "C" fn create_too_big_big_int(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let sign_bit = 0;
  let word_count = usize::MAX;
  let words: Vec<u64> = Vec::with_capacity(10);

  let mut output: napi_value = ptr::null_mut();
  let result = unsafe {
    napi_create_bigint_words(
      env,
      sign_bit,
      word_count,
      words.as_ptr(),
      &mut output,
    )
  };
  assert_eq!(result, 1);

  output
}

extern "C" fn make_big_int_words_throw(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let words: Vec<u64> = Vec::with_capacity(10);
  let mut output = ptr::null_mut();

  let status = unsafe {
    napi_create_bigint_words(env, 0, usize::MAX, words.as_ptr(), &mut output)
  };

  if status != napi_pending_exception {
    unsafe {
      napi_throw_error(
        env,
        ptr::null_mut(),
        cstr!("Expected status 'napi_pending_exception'"),
      )
    };
  }

  ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "isLossless", is_lossless),
    napi_new_property!(env, "testInt64", test_int64),
    napi_new_property!(env, "testUint64", test_uint64),
    napi_new_property!(env, "testWords", test_words),
    napi_new_property!(env, "createTooBigBigInt", create_too_big_big_int),
    napi_new_property!(env, "makeBigIntWordsThrow", make_big_int_words_throw),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
