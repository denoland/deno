// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![allow(non_upper_case_globals)]

use deno_runtime::deno_napi::*;
use libc::INT_MAX;
use v8::BackingStore;
use v8::UniqueRef;

use super::util::get_array_buffer_ptr;
use deno_runtime::deno_napi::function::create_function;
use deno_runtime::deno_napi::function::create_function_template;
use deno_runtime::deno_napi::function::CallbackInfo;
use std::ptr::NonNull;

#[macro_export]
macro_rules! check_env {
  ($env: expr) => {
    if $env.is_null() {
      return napi_invalid_arg;
    }
  };
}

#[inline]
unsafe fn napi_value_unchecked(val: napi_value) -> v8::Local<v8::Value> {
  transmute::<napi_value, v8::Local<v8::Value>>(val)
}

#[macro_export]
macro_rules! return_error_status_if_false {
  ($env: expr, $condition: expr, $status: ident) => {
    if !$condition {
      return Err(
        $crate::napi::js_native_api::napi_set_last_error(
          $env,
          $status,
          0,
          std::ptr::null_mut(),
        )
        .into(),
      );
    }
  };
}

#[macro_export]
macro_rules! return_status_if_false {
  ($env: expr, $condition: expr, $status: ident) => {
    if !$condition {
      return $crate::napi::js_native_api::napi_set_last_error(
        $env,
        $status,
        0,
        std::ptr::null_mut(),
      );
    }
  };
}

fn check_new_from_utf8_len<'s>(
  env: *mut Env,
  str_: *const c_char,
  len: usize,
) -> Result<v8::Local<'s, v8::String>, napi_status> {
  return_error_status_if_false!(
    env,
    (len == NAPI_AUTO_LENGTH) || len <= INT_MAX as _,
    napi_invalid_arg
  );
  return_error_status_if_false!(env, !str_.is_null(), napi_invalid_arg);
  let string = if len == NAPI_AUTO_LENGTH {
    let result = unsafe { std::ffi::CStr::from_ptr(str_ as *const _) }.to_str();
    return_error_status_if_false!(env, result.is_ok(), napi_generic_failure);
    result.unwrap()
  } else {
    let string = unsafe { std::slice::from_raw_parts(str_ as *const u8, len) };
    let result = std::str::from_utf8(string);
    return_error_status_if_false!(env, result.is_ok(), napi_generic_failure);
    result.unwrap()
  };
  let result = {
    let env = unsafe { &mut *env };
    v8::String::new(&mut env.scope(), string)
  };
  return_error_status_if_false!(env, result.is_some(), napi_generic_failure);
  Ok(result.unwrap())
}

#[inline]
fn check_new_from_utf8<'s>(
  env: *mut Env,
  str_: *const c_char,
) -> Result<v8::Local<'s, v8::String>, napi_status> {
  check_new_from_utf8_len(env, str_, NAPI_AUTO_LENGTH)
}

#[macro_export]
macro_rules! status_call {
  ($call: expr) => {
    let status = $call;
    if status != napi_ok {
      return status;
    }
  };
}

// Macro to check napi arguments.
// If nullptr, return napi_invalid_arg.
#[macro_export]
macro_rules! check_arg {
  ($env: expr, $ptr: expr) => {
    $crate::return_status_if_false!($env, !$ptr.is_null(), napi_invalid_arg);
  };
}

macro_rules! check_arg_option {
  ($env: expr, $opt: expr) => {
    $crate::return_status_if_false!($env, $opt.is_some(), napi_invalid_arg);
  };
}

fn napi_clear_last_error(env: *mut Env) {
  let env = unsafe { &mut *env };
  env.last_error.error_code = napi_ok;
  env.last_error.engine_error_code = 0;
  env.last_error.engine_reserved = std::ptr::null_mut();
  env.last_error.error_message = std::ptr::null_mut();
}

pub(crate) fn napi_set_last_error(
  env: *mut Env,
  error_code: napi_status,
  engine_error_code: i32,
  engine_reserved: *mut c_void,
) -> napi_status {
  let env = unsafe { &mut *env };
  env.last_error.error_code = error_code;
  env.last_error.engine_error_code = engine_error_code;
  env.last_error.engine_reserved = engine_reserved;
  error_code
}

/// Returns napi_value that represents a new JavaScript Array.
#[napi_sym::napi_sym]
fn napi_create_array(env: *mut Env, result: *mut napi_value) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::Array::new(&mut env.scope(), 0).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_array_with_length(
  env: *mut Env,
  len: i32,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::Array::new(&mut env.scope(), len).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_arraybuffer(
  env: *mut Env,
  len: usize,
  data: *mut *mut u8,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };

  let value = v8::ArrayBuffer::new(&mut env.scope(), len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }

  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_bigint_int64(
  env: *mut Env,
  value: i64,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::BigInt::new_from_i64(&mut env.scope(), value).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_bigint_uint64(
  env: *mut Env,
  value: u64,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::BigInt::new_from_u64(&mut env.scope(), value).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_bigint_words(
  env: *mut Env,
  sign_bit: bool,
  word_count: usize,
  words: *const u64,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, words);
  let env = unsafe { &mut *env };
  check_arg!(env, result);

  if word_count > INT_MAX as _ {
    return napi_invalid_arg;
  }

  match v8::BigInt::new_from_words(
    &mut env.scope(),
    sign_bit,
    std::slice::from_raw_parts(words, word_count),
  ) {
    Some(value) => {
      *result = value.into();
    }
    None => {
      return napi_invalid_arg;
    }
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_buffer(
  env: *mut Env,
  len: usize,
  data: *mut *mut u8,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = v8::ArrayBuffer::new(&mut env.scope(), len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let value = v8::Uint8Array::new(&mut env.scope(), value, 0, len).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_buffer_copy(
  env: *mut Env,
  len: usize,
  data: *mut u8,
  result_data: *mut *mut u8,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = v8::ArrayBuffer::new(&mut env.scope(), len);
  let ptr = get_array_buffer_ptr(value);
  std::ptr::copy(data, ptr, len);
  if !result_data.is_null() {
    *result_data = ptr;
  }
  let value = v8::Uint8Array::new(&mut env.scope(), value, 0, len).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_coerce_to_bool(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let coerced = value.to_boolean(&mut env.scope());
  let value: v8::Local<v8::Value> = coerced.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_coerce_to_number(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let Some(coerced) = value.to_number(&mut env.scope()) else {
    return napi_number_expected;
  };
  let value: v8::Local<v8::Value> = coerced.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_coerce_to_object(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let coerced = value.to_object(&mut env.scope()).unwrap();
  let value: v8::Local<v8::Value> = coerced.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_coerce_to_string(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let coerced = value.to_string(&mut env.scope()).unwrap();
  let value: v8::Local<v8::Value> = coerced.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_dataview(
  env: *mut Env,
  len: usize,
  data: *mut *mut u8,
  byte_offset: usize,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, data);
  let env = unsafe { &mut *env };
  check_arg!(env, result);
  let value = v8::ArrayBuffer::new(&mut env.scope(), len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let context = &mut env.scope().get_current_context();
  let global = context.global(&mut env.scope());
  let data_view_name = v8::String::new(&mut env.scope(), "DataView").unwrap();
  let data_view = global.get(&mut env.scope(), data_view_name.into()).unwrap();
  let Ok(data_view) = v8::Local::<v8::Function>::try_from(data_view) else {
    return napi_function_expected;
  };
  let byte_offset = v8::Number::new(&mut env.scope(), byte_offset as f64);
  let byte_length = v8::Number::new(&mut env.scope(), len as f64);
  let value = data_view
    .new_instance(
      &mut env.scope(),
      &[value.into(), byte_offset.into(), byte_length.into()],
    )
    .unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_date(
  env: *mut Env,
  time: f64,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value: v8::Local<v8::Value> =
    v8::Date::new(&mut env.scope(), time).unwrap().into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_double(
  env: *mut Env,
  value: f64,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::Number::new(&mut env.scope(), value).into();
  napi_ok
}

fn set_error_code(
  env: *mut Env,
  error: v8::Local<v8::Value>,
  code: napi_value,
  code_cstring: *const c_char,
) -> napi_status {
  if code.is_some() || !code_cstring.is_null() {
    let err_object: v8::Local<v8::Object> = error.try_into().unwrap();

    let code_value: v8::Local<v8::Value> = if code.is_some() {
      let mut code_value = unsafe { napi_value_unchecked(code) };
      return_status_if_false!(
        env,
        code_value.is_string(),
        napi_string_expected
      );
      code_value
    } else {
      let name = match check_new_from_utf8(env, code_cstring) {
        Ok(s) => s,
        Err(status) => return status,
      };
      name.into()
    };

    let mut scope = unsafe { &mut *env }.scope();
    let code_key = v8::String::new(&mut scope, "code").unwrap();

    if err_object
      .set(&mut scope, code_key.into(), code_value)
      .is_none()
    {
      return napi_generic_failure;
    }
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_error(
  env: *mut Env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg_option!(env, msg);
  check_arg!(env, result);
  let mut message_value = napi_value_unchecked(msg);
  return_status_if_false!(env, message_value.is_string(), napi_string_expected);
  let error_obj = v8::Exception::error(
    &mut unsafe { &mut *env }.scope(),
    message_value.try_into().unwrap(),
  );
  status_call!(set_error_code(env, error_obj, code, std::ptr::null()));
  *result = error_obj.into();
  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_type_error(
  env: *mut Env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg_option!(env, msg);
  check_arg!(env, result);
  let mut message_value = napi_value_unchecked(msg);
  return_status_if_false!(env, message_value.is_string(), napi_string_expected);
  let error_obj = v8::Exception::type_error(
    &mut unsafe { &mut *env }.scope(),
    message_value.try_into().unwrap(),
  );
  status_call!(set_error_code(env, error_obj, code, std::ptr::null()));
  *result = error_obj.into();
  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_range_error(
  env: *mut Env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg_option!(env, msg);
  check_arg!(env, result);
  let mut message_value = napi_value_unchecked(msg);
  return_status_if_false!(env, message_value.is_string(), napi_string_expected);
  let error_obj = v8::Exception::range_error(
    &mut unsafe { &mut *env }.scope(),
    message_value.try_into().unwrap(),
  );
  status_call!(set_error_code(env, error_obj, code, std::ptr::null()));
  *result = error_obj.into();
  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_external(
  env_ptr: *mut Env,
  value: *mut c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env_ptr);
  let env = unsafe { &mut *env_ptr };
  let external: v8::Local<v8::Value> =
    v8::External::new(&mut env.scope(), value).into();

  let value = weak_local(env_ptr, external, value, finalize_cb, finalize_hint);

  *result = transmute(value);
  napi_ok
}

pub type BackingStoreDeleterCallback = unsafe extern "C" fn(
  data: *mut c_void,
  byte_length: usize,
  deleter_data: *mut c_void,
);

extern "C" {
  fn v8__ArrayBuffer__NewBackingStore__with_data(
    data: *mut c_void,
    byte_length: usize,
    deleter: BackingStoreDeleterCallback,
    deleter_data: *mut c_void,
  ) -> *mut BackingStore;
}

struct BufferFinalizer {
  env: *mut Env,
  finalize_cb: napi_finalize,
  finalize_data: *mut c_void,
  finalize_hint: *mut c_void,
}

impl BufferFinalizer {
  fn into_raw(self) -> *mut BufferFinalizer {
    Box::into_raw(Box::new(self))
  }
}

impl Drop for BufferFinalizer {
  fn drop(&mut self) {
    unsafe {
      (self.finalize_cb)(self.env as _, self.finalize_data, self.finalize_hint);
    }
  }
}

pub extern "C" fn backing_store_deleter_callback(
  data: *mut c_void,
  _byte_length: usize,
  deleter_data: *mut c_void,
) {
  let mut finalizer =
    unsafe { Box::from_raw(deleter_data as *mut BufferFinalizer) };

  finalizer.finalize_data = data;
}

#[napi_sym::napi_sym]
fn napi_create_external_arraybuffer(
  env_ptr: *mut Env,
  data: *mut c_void,
  byte_length: usize,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env_ptr);
  let env = unsafe { &mut *env_ptr };

  let finalizer = BufferFinalizer {
    env: env_ptr,
    finalize_data: ptr::null_mut(),
    finalize_cb,
    finalize_hint,
  };

  let store: UniqueRef<BackingStore> =
    transmute(v8__ArrayBuffer__NewBackingStore__with_data(
      data,
      byte_length,
      backing_store_deleter_callback,
      finalizer.into_raw() as _,
    ));

  let ab =
    v8::ArrayBuffer::with_backing_store(&mut env.scope(), &store.make_shared());
  let value: v8::Local<v8::Value> = ab.into();

  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_external_buffer(
  env_ptr: *mut Env,
  byte_length: usize,
  data: *mut c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env_ptr);
  let env = unsafe { &mut *env_ptr };
  let finalizer = BufferFinalizer {
    env: env_ptr,
    finalize_data: ptr::null_mut(),
    finalize_cb,
    finalize_hint,
  };

  let store: UniqueRef<BackingStore> =
    transmute(v8__ArrayBuffer__NewBackingStore__with_data(
      data,
      byte_length,
      backing_store_deleter_callback,
      finalizer.into_raw() as _,
    ));

  let ab =
    v8::ArrayBuffer::with_backing_store(&mut env.scope(), &store.make_shared());
  let value =
    v8::Uint8Array::new(&mut env.scope(), ab, 0, byte_length).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_function(
  env: *mut Env,
  name: *const c_char,
  length: usize,
  cb: napi_callback,
  cb_info: napi_callback_info,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  check_arg_option!(env, cb);

  let name = if let Some(name) = name.as_ref() {
    match check_new_from_utf8_len(env, name, length) {
      Ok(s) => Some(s),
      Err(status) => return status,
    }
  } else {
    None
  };

  *result = create_function(env, name, cb, cb_info).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_int32(
  env: *mut Env,
  value: i32,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::Integer::new(&mut env.scope(), value).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_uint32(
  env: *mut Env,
  value: u32,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::Integer::new_from_unsigned(&mut env.scope(), value).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_int64(
  env: *mut Env,
  value: i64,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::Number::new(&mut env.scope(), value as f64).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_object(env: *mut Env, result: *mut napi_value) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let object = v8::Object::new(&mut env.scope());
  *result = object.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_promise(
  env: *mut Env,
  deferred: *mut napi_deferred,
  promise_out: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let resolver = v8::PromiseResolver::new(&mut env.scope()).unwrap();
  let mut global = v8::Global::new(&mut env.scope(), resolver);
  let mut global_ptr = global.into_raw();
  let promise = resolver.get_promise(&mut env.scope());
  *deferred = global_ptr.as_mut() as *mut _ as napi_deferred;
  *promise_out = promise.into();

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_reference(
  env: *mut Env,
  value: napi_value,
  _initial_refcount: u32,
  result: *mut napi_ref,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };

  let value = napi_value_unchecked(value);
  let global = v8::Global::new(&mut env.scope(), value);
  let mut global_ptr = global.into_raw();
  *result = transmute::<NonNull<v8::Value>, napi_ref>(global_ptr);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_string_latin1(
  env: *mut Env,
  string: *const u8,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  if length > 0 {
    check_arg!(env, string);
  }
  check_arg!(env, result);
  return_status_if_false!(
    env,
    (length == NAPI_AUTO_LENGTH) || length <= INT_MAX as _,
    napi_invalid_arg
  );

  let string = if length == NAPI_AUTO_LENGTH {
    std::ffi::CStr::from_ptr(string as *const _)
      .to_str()
      .unwrap()
      .as_bytes()
  } else {
    std::slice::from_raw_parts(string, length)
  };
  let Some(v8str) = v8::String::new_from_one_byte(
    &mut env.scope(),
    string,
    v8::NewStringType::Normal,
  ) else {
    return napi_generic_failure;
  };
  *result = v8str.into();

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_string_utf16(
  env: *mut Env,
  string: *const u16,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  if length > 0 {
    check_arg!(env, string);
  }
  check_arg!(env, result);
  return_status_if_false!(
    env,
    (length == NAPI_AUTO_LENGTH) || length <= INT_MAX as _,
    napi_invalid_arg
  );

  let string = if length == NAPI_AUTO_LENGTH {
    let s = std::ffi::CStr::from_ptr(string as *const _)
      .to_str()
      .unwrap();
    std::slice::from_raw_parts(s.as_ptr() as *const u16, s.len())
  } else {
    std::slice::from_raw_parts(string, length)
  };

  match v8::String::new_from_two_byte(
    &mut env.scope(),
    string,
    v8::NewStringType::Normal,
  ) {
    Some(v8str) => {
      *result = v8str.into();
    }
    None => return napi_generic_failure,
  }
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_string_utf8(
  env: *mut Env,
  string: *const u8,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  if length > 0 {
    check_arg!(env, string);
  }
  check_arg!(env, result);
  return_status_if_false!(
    env,
    (length == NAPI_AUTO_LENGTH) || length <= INT_MAX as _,
    napi_invalid_arg
  );

  let string = if length == NAPI_AUTO_LENGTH {
    std::ffi::CStr::from_ptr(string as *const _)
      .to_str()
      .unwrap()
  } else {
    let string = std::slice::from_raw_parts(string, length);
    std::str::from_utf8(string).unwrap()
  };
  let v8str = v8::String::new(&mut env.scope(), string).unwrap();
  *result = v8str.into();

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_symbol(
  env: *mut Env,
  description: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };

  let scope = &mut env.scope();
  let description = if let Some(d) = *description {
    let Some(d) = d.to_string(scope) else {
      return napi_string_expected;
    };
    Some(d)
  } else {
    None
  };
  *result = v8::Symbol::new(scope, description).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_create_typedarray(
  env: *mut Env,
  ty: napi_typedarray_type,
  length: usize,
  arraybuffer: napi_value,
  byte_offset: usize,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let ab = napi_value_unchecked(arraybuffer);
  let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(ab) else {
    return napi_arraybuffer_expected;
  };
  let typedarray: v8::Local<v8::Value> = match ty {
    napi_uint8_array => {
      v8::Uint8Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_uint8_clamped_array => {
      v8::Uint8ClampedArray::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int8_array => {
      v8::Int8Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_uint16_array => {
      v8::Uint16Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int16_array => {
      v8::Int16Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_uint32_array => {
      v8::Uint32Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int32_array => {
      v8::Int32Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_float32_array => {
      v8::Float32Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_float64_array => {
      v8::Float64Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_bigint64_array => {
      v8::BigInt64Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_biguint64_array => {
      v8::BigUint64Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    _ => {
      return napi_invalid_arg;
    }
  };
  *result = typedarray.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_make_callback(
  env: *mut Env,
  async_context: *mut c_void,
  recv: napi_value,
  func: napi_value,
  argc: isize,
  argv: *const napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  check_arg_option!(env, recv);
  if argc > 0 {
    check_arg!(env, argv);
  }

  if !async_context.is_null() {
    log::info!("napi_make_callback: async_context is not supported");
  }

  let recv = napi_value_unchecked(recv);
  let func = napi_value_unchecked(func);

  let Ok(func) = v8::Local::<v8::Function>::try_from(func) else {
    return napi_function_expected;
  };
  let argv: &[v8::Local<v8::Value>] =
    transmute(std::slice::from_raw_parts(argv, argc as usize));
  let ret = func.call(&mut env.scope(), recv, argv);
  *result = transmute::<Option<v8::Local<v8::Value>>, napi_value>(ret);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_bigint_int64(
  env: *mut Env,
  value: napi_value,
  result: *mut i64,
  lossless: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let bigint = value.to_big_int(&mut env.scope()).unwrap();
  let (result_, lossless_) = bigint.i64_value();
  *result = result_;
  *lossless = lossless_;
  // TODO(bartlomieju):
  // napi_clear_last_error()
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_bigint_uint64(
  env: *mut Env,
  value: napi_value,
  result: *mut u64,
  lossless: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let bigint = value.to_big_int(&mut env.scope()).unwrap();
  let (result_, lossless_) = bigint.u64_value();
  *result = result_;
  *lossless = lossless_;
  // TODO(bartlomieju):
  // napi_clear_last_error()
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_bigint_words(
  env: *mut Env,
  value: napi_value,
  sign_bit: *mut i32,
  word_count: *mut usize,
  words: *mut u64,
) -> napi_status {
  check_env!(env);
  // TODO(bartlomieju):
  // check_arg!(env, value);
  check_arg!(env, word_count);
  let env = unsafe { &mut *env };

  let value = napi_value_unchecked(value);
  let big = match value.to_big_int(&mut env.scope()) {
    Some(b) => b,
    None => return napi_bigint_expected,
  };
  let word_count_int;

  if sign_bit.is_null() && words.is_null() {
    word_count_int = big.word_count();
  } else {
    check_arg!(env, sign_bit);
    check_arg!(env, words);
    let out_words = std::slice::from_raw_parts_mut(words, *word_count);
    let (sign, slice_) = big.to_words_array(out_words);
    word_count_int = slice_.len();
    *sign_bit = sign as i32;
  }

  *word_count = word_count_int;
  // TODO(bartlomieju):
  // napi_clear_last_error()
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_bool(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  *result = value.boolean_value(&mut env.scope());
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_double(
  env: *mut Env,
  value: napi_value,
  result: *mut f64,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  return_status_if_false!(env, value.is_number(), napi_number_expected);
  *result = value.number_value(&mut env.scope()).unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_external(
  _env: *mut Env,
  value: napi_value,
  result: *mut *mut c_void,
) -> napi_status {
  let value = napi_value_unchecked(value);
  let ext = v8::Local::<v8::External>::try_from(value).unwrap();
  *result = ext.value();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_int32(
  env: *mut Env,
  value: napi_value,
  result: *mut i32,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  *result = value.int32_value(&mut env.scope()).unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_int64(
  env: *mut Env,
  value: napi_value,
  result: *mut i64,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  *result = value.integer_value(&mut env.scope()).unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_string_latin1(
  env: *mut Env,
  value: napi_value,
  buf: *mut u8,
  bufsize: usize,
  result: *mut usize,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };

  let value = napi_value_unchecked(value);

  if !value.is_string() && !value.is_string_object() {
    return napi_string_expected;
  }

  let v8str = value.to_string(&mut env.scope()).unwrap();
  let string_len = v8str.utf8_length(&mut env.scope());

  if buf.is_null() {
    *result = string_len;
  } else if bufsize != 0 {
    let buffer = std::slice::from_raw_parts_mut(buf, bufsize - 1);
    let copied = v8str.write_one_byte(
      &mut env.scope(),
      buffer,
      0,
      v8::WriteOptions::NO_NULL_TERMINATION,
    );
    buf.add(copied).write(0);
    if !result.is_null() {
      *result = copied;
    }
  } else if !result.is_null() {
    *result = string_len;
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_string_utf8(
  env: *mut Env,
  value: napi_value,
  buf: *mut u8,
  bufsize: usize,
  result: *mut usize,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };

  let value = napi_value_unchecked(value);

  if !value.is_string() && !value.is_string_object() {
    return napi_string_expected;
  }

  let v8str = value.to_string(&mut env.scope()).unwrap();
  let string_len = v8str.utf8_length(&mut env.scope());

  if buf.is_null() {
    *result = string_len;
  } else if bufsize != 0 {
    let buffer = std::slice::from_raw_parts_mut(buf, bufsize - 1);
    let copied = v8str.write_utf8(
      &mut env.scope(),
      buffer,
      None,
      v8::WriteOptions::NO_NULL_TERMINATION
        | v8::WriteOptions::REPLACE_INVALID_UTF8,
    );
    buf.add(copied).write(0);
    if !result.is_null() {
      *result = copied;
    }
  } else if !result.is_null() {
    *result = string_len;
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_string_utf16(
  env: *mut Env,
  value: napi_value,
  buf: *mut u16,
  bufsize: usize,
  result: *mut usize,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };

  let value = napi_value_unchecked(value);

  if !value.is_string() && !value.is_string_object() {
    return napi_string_expected;
  }

  let v8str = value.to_string(&mut env.scope()).unwrap();
  let string_len = v8str.length();

  if buf.is_null() {
    *result = string_len;
  } else if bufsize != 0 {
    let buffer = std::slice::from_raw_parts_mut(buf, bufsize - 1);
    let copied = v8str.write(
      &mut env.scope(),
      buffer,
      0,
      v8::WriteOptions::NO_NULL_TERMINATION,
    );
    buf.add(copied).write(0);
    if !result.is_null() {
      *result = copied;
    }
  } else if !result.is_null() {
    *result = string_len;
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_value_uint32(
  env: *mut Env,
  value: napi_value,
  result: *mut u32,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  *result = value.uint32_value(&mut env.scope()).unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_add_finalizer(
  env_ptr: *mut Env,
  js_object: napi_value,
  native_object: *mut c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_ref,
) -> napi_status {
  check_env!(env_ptr);

  let value = napi_value_unchecked(js_object);
  let value =
    weak_local(env_ptr, value, native_object, finalize_cb, finalize_hint);

  if !result.is_null() {
    *result = transmute(value);
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_adjust_external_memory(
  env: *mut Env,
  change_in_bytes: i64,
  adjusted_value: *mut i64,
) -> napi_status {
  check_env!(env);
  check_arg!(env, adjusted_value);

  let env = unsafe { &mut *env };
  let isolate = &mut *env.isolate_ptr;
  *adjusted_value =
    isolate.adjust_amount_of_external_allocated_memory(change_in_bytes);

  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_call_function(
  env: *mut Env,
  recv: napi_value,
  func: napi_value,
  argc: usize,
  argv: *const napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let recv = napi_value_unchecked(recv);
  let func = napi_value_unchecked(func);
  let Ok(func) = v8::Local::<v8::Function>::try_from(func) else {
    return napi_function_expected;
  };

  let argv: &[v8::Local<v8::Value>] =
    transmute(std::slice::from_raw_parts(argv, argc));
  let ret = func.call(&mut env.scope(), recv, argv);
  if !result.is_null() {
    *result = transmute::<Option<v8::Local<v8::Value>>, napi_value>(ret);
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_close_escapable_handle_scope(
  _env: *mut Env,
  _scope: napi_escapable_handle_scope,
) -> napi_status {
  // TODO: do this properly
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_close_handle_scope(
  env: *mut Env,
  _scope: napi_handle_scope,
) -> napi_status {
  let env = &mut *env;
  if env.open_handle_scopes == 0 {
    return napi_handle_scope_mismatch;
  }
  // TODO: We are not opening a handle scope, therefore we cannot close it
  // TODO: this is also not implemented in napi_open_handle_scope
  // let _scope = &mut *(scope as *mut v8::HandleScope);
  env.open_handle_scopes -= 1;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_define_class(
  env_ptr: *mut Env,
  name: *const c_char,
  length: isize,
  constructor: napi_callback,
  callback_data: *mut c_void,
  property_count: usize,
  properties: *const napi_property_descriptor,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env_ptr);
  let env = unsafe { &mut *env_ptr };
  check_arg!(env, result);
  check_arg_option!(env, constructor);

  if property_count > 0 {
    check_arg!(env, properties);
  }

  let name = if length == -1 {
    let Ok(s) = std::ffi::CStr::from_ptr(name).to_str() else {
      return napi_invalid_arg;
    };
    s
  } else {
    let slice = std::slice::from_raw_parts(name as *const u8, length as usize);
    std::str::from_utf8(slice).unwrap()
  };

  let tpl =
    create_function_template(env_ptr, Some(name), constructor, callback_data);

  let scope = &mut env.scope();
  let napi_properties: &[napi_property_descriptor] =
    std::slice::from_raw_parts(properties, property_count);
  let mut static_property_count = 0;

  for p in napi_properties {
    if p.attributes & napi_static != 0 {
      // Will be handled below
      static_property_count += 1;
      continue;
    }

    let name = if !p.utf8name.is_null() {
      let name_str = CStr::from_ptr(p.utf8name).to_str().unwrap();
      v8::String::new(scope, name_str).unwrap()
    } else {
      transmute::<napi_value, v8::Local<v8::String>>(p.name)
    };

    let method = p.method;
    let getter = p.getter;
    let setter = p.setter;

    if getter.is_some() || setter.is_some() {
      let getter: Option<v8::Local<v8::FunctionTemplate>> = if getter.is_some()
      {
        Some(create_function_template(env_ptr, None, p.getter, p.data))
      } else {
        None
      };
      let setter: Option<v8::Local<v8::FunctionTemplate>> = if setter.is_some()
      {
        Some(create_function_template(env_ptr, None, p.setter, p.data))
      } else {
        None
      };

      let mut accessor_property = v8::PropertyAttribute::NONE;
      if getter.is_some()
        && setter.is_some()
        && (p.attributes & napi_writable) == 0
      {
        accessor_property =
          accessor_property | v8::PropertyAttribute::READ_ONLY;
      }
      if p.attributes & napi_enumerable == 0 {
        accessor_property =
          accessor_property | v8::PropertyAttribute::DONT_ENUM;
      }
      if p.attributes & napi_configurable == 0 {
        accessor_property =
          accessor_property | v8::PropertyAttribute::DONT_DELETE;
      }

      let proto = tpl.prototype_template(scope);
      proto.set_accessor_property(
        name.into(),
        getter,
        setter,
        accessor_property,
      );
    } else if method.is_some() {
      let function = create_function_template(env_ptr, None, p.method, p.data);
      let proto = tpl.prototype_template(scope);
      proto.set(name.into(), function.into());
    } else {
      let proto = tpl.prototype_template(scope);
      proto.set(
        name.into(),
        transmute::<napi_value, v8::Local<v8::Data>>(p.value),
      );
    }
  }

  let value: v8::Local<v8::Value> = tpl.get_function(scope).unwrap().into();
  *result = value.into();

  if static_property_count > 0 {
    let mut static_descriptors = Vec::with_capacity(static_property_count);

    for p in napi_properties {
      if p.attributes & napi_static != 0 {
        static_descriptors.push(*p);
      }
    }

    status_call!(napi_define_properties(
      env_ptr,
      *result,
      static_descriptors.len(),
      static_descriptors.as_ptr(),
    ));
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_define_properties(
  env_ptr: *mut Env,
  obj: napi_value,
  property_count: usize,
  properties: *const napi_property_descriptor,
) -> napi_status {
  check_env!(env_ptr);
  let env = unsafe { &mut *env_ptr };
  if property_count > 0 {
    check_arg!(env, properties);
  }

  let scope = &mut env.scope();

  let Ok(object) = v8::Local::<v8::Object>::try_from(napi_value_unchecked(obj))
  else {
    return napi_object_expected;
  };

  let properties = std::slice::from_raw_parts(properties, property_count);
  for property in properties {
    let name = if !property.utf8name.is_null() {
      let name_str = CStr::from_ptr(property.utf8name).to_str().unwrap();
      let Some(name_v8_str) = v8::String::new(scope, name_str) else {
        return napi_generic_failure;
      };
      name_v8_str.into()
    } else {
      let property_value = napi_value_unchecked(property.name);
      let Ok(prop) = v8::Local::<v8::Name>::try_from(property_value) else {
        return napi_name_expected;
      };
      prop
    };

    if property.getter.is_some() || property.setter.is_some() {
      let local_getter: v8::Local<v8::Value> = if property.getter.is_some() {
        create_function(env_ptr, None, property.getter, property.data).into()
      } else {
        v8::undefined(scope).into()
      };
      let local_setter: v8::Local<v8::Value> = if property.setter.is_some() {
        create_function(env_ptr, None, property.setter, property.data).into()
      } else {
        v8::undefined(scope).into()
      };

      let mut desc =
        v8::PropertyDescriptor::new_from_get_set(local_getter, local_setter);
      desc.set_enumerable(property.attributes & napi_enumerable != 0);
      desc.set_configurable(property.attributes & napi_configurable != 0);

      let define_maybe = object.define_property(scope, name, &desc);
      return_status_if_false!(
        env_ptr,
        define_maybe.is_some(),
        napi_invalid_arg
      );
    } else if property.method.is_some() {
      let value: v8::Local<v8::Value> = {
        let function =
          create_function(env_ptr, None, property.method, property.data);
        function.into()
      };
      return_status_if_false!(
        env_ptr,
        object.set(scope, name.into(), value).is_some(),
        napi_invalid_arg
      );
    } else {
      let value = napi_value_unchecked(property.value);
      return_status_if_false!(
        env_ptr,
        object.set(scope, name.into(), value).is_some(),
        napi_invalid_arg
      );
    }
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_delete_element(
  env: *mut Env,
  value: napi_value,
  index: u32,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  *result = obj.delete_index(&mut env.scope(), index).unwrap_or(false);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_delete_property(
  env: *mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  check_arg_option!(env, key);
  check_arg!(env, result);

  let scope = &mut env.scope();
  let Some(object) = object.map(|o| o.to_object(scope)).flatten() else {
    return napi_invalid_arg;
  };

  let Some(deleted) = object.delete(scope, key.unwrap_unchecked()) else {
    return napi_generic_failure;
  };

  *result = deleted;
  napi_ok
}

// TODO: properly implement ref counting stuff
#[napi_sym::napi_sym]
fn napi_delete_reference(_env: *mut Env, _nref: napi_ref) -> napi_status {
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_detach_arraybuffer(env: *mut Env, value: napi_value) -> napi_status {
  check_env!(env);

  let value = napi_value_unchecked(value);
  let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(value) else {
    return napi_arraybuffer_expected;
  };

  if !ab.is_detachable() {
    return napi_detachable_arraybuffer_expected;
  }

  // Expected to crash for None.
  ab.detach(None).unwrap();

  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_escape_handle<'s>(
  _env: *mut Env,
  _handle_scope: napi_escapable_handle_scope,
  escapee: napi_value<'s>,
  result: *mut napi_value<'s>,
) -> napi_status {
  // TODO
  *result = escapee;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_all_property_names(_env: *mut Env) -> napi_status {
  // TODO
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_and_clear_last_exception(
  env: *mut Env,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  // TODO: just return undefined for now we don't cache
  // exceptions in env.
  let value: v8::Local<v8::Value> = v8::undefined(&mut env.scope()).into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_array_length(
  _env: *mut Env,
  value: napi_value,
  result: *mut u32,
) -> napi_status {
  let value = napi_value_unchecked(value);
  *result = v8::Local::<v8::Array>::try_from(value).unwrap().length();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_arraybuffer_info(
  _env: *mut Env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> napi_status {
  let value = napi_value_unchecked(value);
  let buf = v8::Local::<v8::ArrayBuffer>::try_from(value).unwrap();
  if !data.is_null() {
    *data = get_array_buffer_ptr(buf);
  }
  if !length.is_null() {
    *length = buf.byte_length();
  }
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_boolean(
  env: *mut Env,
  value: bool,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::Boolean::new(env.isolate(), value).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_buffer_info(
  env: *mut Env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let Ok(buf) = v8::Local::<v8::ArrayBufferView>::try_from(value) else {
    return napi_arraybuffer_expected;
  };
  let buffer_name = v8::String::new(&mut env.scope(), "buffer").unwrap();
  let Ok(abuf) = v8::Local::<v8::ArrayBuffer>::try_from(
    buf.get(&mut env.scope(), buffer_name.into()).unwrap(),
  ) else {
    return napi_arraybuffer_expected;
  };
  if !data.is_null() {
    *data = get_array_buffer_ptr(abuf);
  }
  if !length.is_null() {
    *length = abuf.byte_length();
  }
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_cb_info(
  env: *mut Env,
  cbinfo: napi_callback_info,
  argc: *mut i32,
  argv: *mut napi_value,
  this_arg: *mut napi_value,
  data: *mut *mut c_void,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  check_arg!(env, cbinfo);

  let cbinfo: &CallbackInfo = &*(cbinfo as *const CallbackInfo);
  let args = &*(cbinfo.args as *const v8::FunctionCallbackArguments);

  if !argv.is_null() {
    check_arg!(env, argc);
    let mut v_argv = std::slice::from_raw_parts_mut(argv, argc as usize);
    for i in 0..*argc {
      let mut arg = args.get(i);
      v_argv[i as usize] = arg.into();
    }
  }

  if !argc.is_null() {
    *argc = args.length();
  }

  if !this_arg.is_null() {
    let mut this = args.this();
    *this_arg = this.into();
  }

  if !data.is_null() {
    *data = cbinfo.cb_info;
  }

  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_dataview_info(
  env: *mut Env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let Ok(buf) = v8::Local::<v8::DataView>::try_from(value) else {
    return napi_invalid_arg;
  };
  let buffer_name = v8::String::new(&mut env.scope(), "buffer").unwrap();
  let Ok(abuf) = v8::Local::<v8::ArrayBuffer>::try_from(
    buf.get(&mut env.scope(), buffer_name.into()).unwrap(),
  ) else {
    return napi_invalid_arg;
  };
  if !data.is_null() {
    *data = get_array_buffer_ptr(abuf);
  }
  *length = abuf.byte_length();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_date_value(
  env: *mut Env,
  value: napi_value,
  result: *mut f64,
) -> napi_status {
  check_env!(env);
  let value = napi_value_unchecked(value);
  return_status_if_false!(env, value.is_date(), napi_date_expected);
  let env = unsafe { &mut *env };
  let Ok(date) = v8::Local::<v8::Date>::try_from(value) else {
    return napi_date_expected;
  };
  // TODO: should be value of
  *result = date.number_value(&mut env.scope()).unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_element(
  env: *mut Env,
  object: napi_value,
  index: u32,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let object = napi_value_unchecked(object);
  let Ok(object) = v8::Local::<v8::Object>::try_from(object) else {
    return napi_invalid_arg;
  };
  let value: v8::Local<v8::Value> =
    object.get_index(&mut env.scope(), index).unwrap();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_global(env: *mut Env, result: *mut napi_value) -> napi_status {
  check_env!(env);
  check_arg!(env, result);

  let value: v8::Local<v8::Value> =
    transmute::<NonNull<v8::Value>, v8::Local<v8::Value>>((*env).global);
  *result = value.into();
  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_instance_data(
  env: *mut Env,
  result: *mut *mut c_void,
) -> napi_status {
  let env = &mut *env;
  let shared = env.shared();
  *result = shared.instance_data;
  napi_ok
}

// TODO(bartlomieju): this function is broken
#[napi_sym::napi_sym]
fn napi_get_last_error_info(
  _env: *mut Env,
  error_code: *mut *const napi_extended_error_info,
) -> napi_status {
  let err_info = Box::new(napi_extended_error_info {
    error_message: std::ptr::null(),
    engine_reserved: std::ptr::null_mut(),
    engine_error_code: 0,
    error_code: napi_ok,
  });

  *error_code = Box::into_raw(err_info);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_named_property(
  env: *mut Env,
  object: napi_value,
  utf8_name: *const c_char,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let object = napi_value_unchecked(object);
  let utf8_name = std::ffi::CStr::from_ptr(utf8_name);
  let name =
    v8::String::new(&mut env.scope(), &utf8_name.to_string_lossy()).unwrap();
  let value: v8::Local<v8::Value> = object
    .to_object(&mut env.scope())
    .unwrap()
    .get(&mut env.scope(), name.into())
    .unwrap();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_new_target(
  _env: &mut Env,
  cbinfo: &CallbackInfo,
  result: &mut v8::Local<v8::Value>,
) -> napi_status {
  let info = &*(cbinfo.args as *const v8::FunctionCallbackArguments);
  *result = info.new_target();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_null(env: *mut Env, result: *mut napi_value) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::null(env.isolate()).into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_property(
  env: *mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let object = transmute::<napi_value, v8::Local<v8::Object>>(object);
  let key = napi_value_unchecked(key);
  let value: v8::Local<v8::Value> = object.get(&mut env.scope(), key).unwrap();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_property_names(
  env: *mut Env,
  object: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let object = napi_value_unchecked(object);
  let array: v8::Local<v8::Array> = object
    .to_object(&mut env.scope())
    .unwrap()
    .get_property_names(&mut env.scope(), Default::default())
    .unwrap();
  let value: v8::Local<v8::Value> = array.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_prototype(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let proto = obj.get_prototype(&mut env.scope()).unwrap();
  *result = proto.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_reference_value(
  env: *mut Env,
  reference: napi_ref,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let value = transmute::<napi_ref, v8::Local<v8::Value>>(reference);
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_typedarray_info(
  env: *mut Env,
  typedarray: napi_value,
  type_: *mut napi_typedarray_type,
  length: *mut usize,
  data: *mut *mut c_void,
  arraybuffer: *mut napi_value,
  byte_offset: *mut usize,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(typedarray);
  let Ok(array) = v8::Local::<v8::TypedArray>::try_from(value) else {
    return napi_invalid_arg;
  };

  if !type_.is_null() {
    if value.is_int8_array() {
      *type_ = napi_int8_array;
    } else if value.is_uint8_array() {
      *type_ = napi_uint8_array;
    } else if value.is_uint8_clamped_array() {
      *type_ = napi_uint8_clamped_array;
    } else if value.is_int16_array() {
      *type_ = napi_int16_array;
    } else if value.is_uint16_array() {
      *type_ = napi_uint16_array;
    } else if value.is_int32_array() {
      *type_ = napi_int32_array;
    } else if value.is_uint32_array() {
      *type_ = napi_uint32_array;
    } else if value.is_float32_array() {
      *type_ = napi_float32_array;
    } else if value.is_float64_array() {
      *type_ = napi_float64_array;
    }
  }

  if !length.is_null() {
    *length = array.length();
  }

  if !data.is_null() || !arraybuffer.is_null() {
    let buf = array.buffer(&mut env.scope()).unwrap();
    if !data.is_null() {
      *data = get_array_buffer_ptr(buf) as *mut c_void;
    }
    if !arraybuffer.is_null() {
      *arraybuffer = buf.into();
    }
  }

  if !byte_offset.is_null() {
    *byte_offset = array.byte_offset();
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_undefined(env: *mut Env, result: *mut napi_value) -> napi_status {
  check_env!(env);
  check_arg!(env, result);
  let env = unsafe { &mut *env };
  *result = v8::undefined(env.isolate()).into();
  napi_ok
}

pub const NAPI_VERSION: u32 = 8;

#[napi_sym::napi_sym]
fn napi_get_version(_: napi_env, version: *mut u32) -> napi_status {
  *version = NAPI_VERSION;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_has_element(
  env: *mut Env,
  value: napi_value,
  index: u32,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  *result = obj.has_index(&mut env.scope(), index).unwrap_or(false);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_has_named_property(
  env: *mut Env,
  value: napi_value,
  key: *const c_char,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let key = CStr::from_ptr(key).to_str().unwrap();
  let key = v8::String::new(&mut env.scope(), key).unwrap();
  *result = obj.has(&mut env.scope(), key.into()).unwrap_or(false);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_has_own_property(
  env: *mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  check_arg_option!(env, key);
  check_arg!(env, result);

  let scope = &mut env.scope();
  let Some(object) = object.map(|o| o.to_object(scope)).flatten() else {
    return napi_invalid_arg;
  };

  if key.is_none() {
    return napi_invalid_arg;
  }
  let Ok(key) = v8::Local::<v8::Name>::try_from(key.unwrap()) else {
    return napi_name_expected;
  };

  let Some(has_own) = object.has_own_property(scope, key) else {
    return napi_generic_failure;
  };

  *result = has_own;

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_has_property(
  env: *mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  check_arg_option!(env, key);
  check_arg!(env, result);

  let scope = &mut env.scope();
  let Some(object) = object.map(|o| o.to_object(scope)).flatten() else {
    return napi_invalid_arg;
  };

  let Some(has) = object.has(scope, key.unwrap_unchecked()) else {
    return napi_generic_failure;
  };
  *result = has;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_instanceof(
  env: *mut Env,
  value: napi_value,
  constructor: napi_value,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  check_arg_option!(env, constructor);
  check_arg_option!(env, value);

  let value = napi_value_unchecked(value);
  let constructor = napi_value_unchecked(constructor);
  let Some(ctor) = constructor.to_object(&mut env.scope()) else {
    return napi_object_expected;
  };
  if !ctor.is_function() {
    return napi_function_expected;
  }
  let Some(res) = value.instance_of(&mut env.scope(), ctor) else {
    return napi_generic_failure;
  };

  *result = res;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_array(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let value = napi_value_unchecked(value);
  *result = value.is_array();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_arraybuffer(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let value = napi_value_unchecked(value);
  *result = value.is_array_buffer();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_buffer(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let value = napi_value_unchecked(value);
  // TODO: should we assume Buffer as Uint8Array in Deno?
  // or use std/node polyfill?
  *result = value.is_typed_array();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_dataview(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let value = napi_value_unchecked(value);
  *result = value.is_data_view();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_date(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let value = napi_value_unchecked(value);
  *result = value.is_date();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_detached_arraybuffer(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  check_arg!(env, result);

  let value = napi_value_unchecked(value);

  *result = match v8::Local::<v8::ArrayBuffer>::try_from(value) {
    Ok(array_buffer) => array_buffer.was_detached(),
    Err(_) => false,
  };

  napi_clear_last_error(env);

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_error(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  {
    check_env!(env);
    if value.is_none() {
      return napi_invalid_arg;
    }
    check_arg!(env, result);

    let value = napi_value_unchecked(value);
    *result = value.is_native_error();
  }
  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_exception_pending(_env: *mut Env, result: *mut bool) -> napi_status {
  // TODO
  *result = false;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_promise(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let value = napi_value_unchecked(value);
  *result = value.is_promise();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_is_typedarray(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let value = napi_value_unchecked(value);
  *result = value.is_typed_array();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_new_instance(
  env: *mut Env,
  constructor: napi_value,
  argc: usize,
  argv: *const napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let constructor = napi_value_unchecked(constructor);
  let Ok(constructor) = v8::Local::<v8::Function>::try_from(constructor) else {
    return napi_function_expected;
  };
  let args: &[v8::Local<v8::Value>] =
    transmute(std::slice::from_raw_parts(argv, argc));
  let inst = constructor.new_instance(&mut env.scope(), args).unwrap();
  let value: v8::Local<v8::Value> = inst.into();
  *result = value.into();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_object_freeze(
  env: &mut Env,
  object: v8::Local<v8::Value>,
) -> napi_status {
  let object = object.to_object(&mut env.scope()).unwrap();
  if object
    .set_integrity_level(&mut env.scope(), v8::IntegrityLevel::Frozen)
    .is_none()
  {
    return napi_generic_failure;
  };

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_object_seal(
  env: &mut Env,
  object: v8::Local<v8::Value>,
) -> napi_status {
  let object = object.to_object(&mut env.scope()).unwrap();
  if object
    .set_integrity_level(&mut env.scope(), v8::IntegrityLevel::Sealed)
    .is_none()
  {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_open_escapable_handle_scope(
  _env: *mut Env,
  _result: *mut napi_escapable_handle_scope,
) -> napi_status {
  // TODO: do this properly
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_open_handle_scope(
  env: *mut Env,
  _result: *mut napi_handle_scope,
) -> napi_status {
  let env = &mut *env;

  // TODO: this is also not implemented in napi_close_handle_scope
  // *result = &mut env.scope() as *mut _ as napi_handle_scope;
  env.open_handle_scopes += 1;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_reference_ref() -> napi_status {
  // TODO
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_reference_unref() -> napi_status {
  // TODO
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_reject_deferred(
  env: *mut Env,
  deferred: napi_deferred,
  error: napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };

  let deferred_ptr =
    NonNull::new_unchecked(deferred as *mut v8::PromiseResolver);
  // TODO(@littledivy): Use Global::from_raw instead casting to local.
  // SAFETY: Isolate is still alive unless the module is doing something weird,
  // global data is the size of a pointer.
  // Global pointer is obtained from napi_create_promise
  let resolver = transmute::<
    NonNull<v8::PromiseResolver>,
    v8::Local<v8::PromiseResolver>,
  >(deferred_ptr);
  resolver
    .reject(&mut env.scope(), napi_value_unchecked(error))
    .unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_remove_wrap(env: *mut Env, value: napi_value) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let shared = &*(env.shared as *const EnvShared);
  let napi_wrap = v8::Local::new(&mut env.scope(), &shared.napi_wrap);
  obj.delete_private(&mut env.scope(), napi_wrap).unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_resolve_deferred(
  env: *mut Env,
  deferred: napi_deferred,
  result: napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let deferred_ptr =
    NonNull::new_unchecked(deferred as *mut v8::PromiseResolver);
  // TODO(@littledivy): Use Global::from_raw instead casting to local.
  // SAFETY: Isolate is still alive unless the module is doing something weird,
  // global data is the size of a pointer.
  // Global pointer is obtained from napi_create_promise
  let resolver = transmute::<
    NonNull<v8::PromiseResolver>,
    v8::Local<v8::PromiseResolver>,
  >(deferred_ptr);
  resolver
    .resolve(&mut env.scope(), napi_value_unchecked(result))
    .unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_run_script(
  env: *mut Env,
  script: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };

  let script = napi_value_unchecked(script);
  if !script.is_string() {
    // TODO:
    // napi_set_last_error
    return napi_string_expected;
  }
  let script = script.to_string(&mut env.scope()).unwrap();

  let script = v8::Script::compile(&mut env.scope(), script, None);
  if script.is_none() {
    return napi_generic_failure;
  }
  let script = script.unwrap();
  let rv = script.run(&mut env.scope());

  if let Some(rv) = rv {
    *result = rv.into();
  } else {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_set_element(
  env: *mut Env,
  object: napi_value,
  index: u32,
  value: napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let object = napi_value_unchecked(object);
  let Ok(object) = v8::Local::<v8::Object>::try_from(object) else {
    return napi_invalid_arg;
  };
  let value = napi_value_unchecked(value);
  object.set_index(&mut env.scope(), index, value).unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_set_instance_data(
  env: *mut Env,
  data: *mut c_void,
  finalize_cb: Option<napi_finalize>,
  finalize_hint: *mut c_void,
) -> napi_status {
  let env = &mut *env;
  let shared = env.shared_mut();
  shared.instance_data = data;
  shared.data_finalize = if finalize_cb.is_some() {
    finalize_cb
  } else {
    None
  };
  shared.data_finalize_hint = finalize_hint;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_set_named_property(
  env: *mut Env,
  object: napi_value,
  name: *const c_char,
  value: napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let name = CStr::from_ptr(name).to_str().unwrap();
  let object = transmute::<napi_value, v8::Local<v8::Object>>(object);
  let value = napi_value_unchecked(value);
  let name = v8::String::new(&mut env.scope(), name).unwrap();
  object.set(&mut env.scope(), name.into(), value).unwrap();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_set_property(
  env: *mut Env,
  object: napi_value,
  key: napi_value,
  value: napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  check_arg_option!(env, key);
  check_arg_option!(env, value);

  let scope = &mut env.scope();
  let Some(object) = object.map(|o| o.to_object(scope)).flatten() else {
    return napi_invalid_arg;
  };

  if object
    .set(scope, key.unwrap_unchecked(), value.unwrap_unchecked())
    .is_none()
  {
    return napi_generic_failure;
  };

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_strict_equals(
  env: *mut Env,
  lhs: napi_value,
  rhs: napi_value,
  result: *mut bool,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  check_arg_option!(env, lhs);
  check_arg_option!(env, rhs);

  *result = lhs.unwrap_unchecked().strict_equals(rhs.unwrap_unchecked());
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_throw(env: *mut Env, error: napi_value) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let error = napi_value_unchecked(error);
  env.scope().throw_exception(error);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_throw_error(
  env: *mut Env,
  code: *const c_char,
  msg: *const c_char,
) -> napi_status {
  // TODO: add preamble here

  {
    check_env!(env);
    let str_ = match check_new_from_utf8(env, msg) {
      Ok(s) => s,
      Err(status) => return status,
    };

    let error = {
      let env = unsafe { &mut *env };
      let scope = &mut env.scope();
      v8::Exception::error(scope, str_)
    };
    status_call!(set_error_code(
      env,
      error,
      transmute::<*mut (), napi_value>(std::ptr::null_mut()),
      code,
    ));

    unsafe { &mut *env }.scope().throw_exception(error);
  }
  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_throw_range_error(
  env: *mut Env,
  code: *const c_char,
  msg: *const c_char,
) -> napi_status {
  // TODO: add preamble here

  {
    check_env!(env);
    let str_ = match check_new_from_utf8(env, msg) {
      Ok(s) => s,
      Err(status) => return status,
    };
    let error = {
      let env = unsafe { &mut *env };
      let scope = &mut env.scope();
      v8::Exception::range_error(scope, str_)
    };
    status_call!(set_error_code(
      env,
      error,
      transmute::<*mut (), napi_value>(std::ptr::null_mut()),
      code,
    ));
    unsafe { &mut *env }.scope().throw_exception(error);
  }
  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_throw_type_error(
  env: *mut Env,
  code: *const c_char,
  msg: *const c_char,
) -> napi_status {
  // TODO: add preamble here

  {
    check_env!(env);
    let str_ = match check_new_from_utf8(env, msg) {
      Ok(s) => s,
      Err(status) => return status,
    };
    let error = {
      let env = unsafe { &mut *env };
      let scope = &mut env.scope();
      v8::Exception::type_error(scope, str_)
    };
    status_call!(set_error_code(
      env,
      error,
      transmute::<*mut (), napi_value>(std::ptr::null_mut()),
      code,
    ));
    unsafe { &mut *env }.scope().throw_exception(error);
  }
  napi_clear_last_error(env);
  napi_ok
}

pub fn get_value_type(value: v8::Local<v8::Value>) -> Option<napi_valuetype> {
  if value.is_undefined() {
    Some(napi_undefined)
  } else if value.is_null() {
    Some(napi_null)
  } else if value.is_external() {
    Some(napi_external)
  } else if value.is_boolean() {
    Some(napi_boolean)
  } else if value.is_number() {
    Some(napi_number)
  } else if value.is_big_int() {
    Some(napi_bigint)
  } else if value.is_string() {
    Some(napi_string)
  } else if value.is_symbol() {
    Some(napi_symbol)
  } else if value.is_function() {
    Some(napi_function)
  } else if value.is_object() {
    Some(napi_object)
  } else {
    None
  }
}

#[napi_sym::napi_sym]
fn napi_typeof(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_valuetype,
) -> napi_status {
  check_env!(env);
  check_arg_option!(env, value);
  check_arg!(env, result);

  let Some(ty) = get_value_type(value.unwrap()) else {
    return napi_invalid_arg;
  };
  *result = ty;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_unwrap(
  env: *mut Env,
  value: napi_value,
  result: *mut *mut c_void,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let shared = &*(env.shared as *const EnvShared);
  let napi_wrap = v8::Local::new(&mut env.scope(), &shared.napi_wrap);
  let ext = obj.get_private(&mut env.scope(), napi_wrap).unwrap();
  let Some(ext) = v8::Local::<v8::External>::try_from(ext).ok() else {
    return napi_invalid_arg;
  };
  *result = ext.value();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_wrap(
  env: *mut Env,
  value: napi_value,
  native_object: *mut c_void,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };
  let value = napi_value_unchecked(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let shared = &*(env.shared as *const EnvShared);
  let napi_wrap = v8::Local::new(&mut env.scope(), &shared.napi_wrap);
  let ext = v8::External::new(&mut env.scope(), native_object);
  obj.set_private(&mut env.scope(), napi_wrap, ext.into());
  napi_ok
}

#[napi_sym::napi_sym]
fn node_api_throw_syntax_error(
  env: *mut Env,
  _code: *const c_char,
  msg: *const c_char,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };

  // let code = CStr::from_ptr(code).to_str().unwrap();
  let msg = CStr::from_ptr(msg).to_str().unwrap();

  // let code = v8::String::new(&mut env.scope(), code).unwrap();
  let msg = v8::String::new(&mut env.scope(), msg).unwrap();

  let error = v8::Exception::syntax_error(&mut env.scope(), msg);
  env.scope().throw_exception(error);

  napi_ok
}

#[napi_sym::napi_sym]
fn node_api_create_syntax_error(
  env: *mut Env,
  _code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> napi_status {
  check_env!(env);
  let env = unsafe { &mut *env };

  // let code = napi_value_unchecked(code);
  let msg = napi_value_unchecked(msg);

  let msg = msg.to_string(&mut env.scope()).unwrap();

  let error = v8::Exception::syntax_error(&mut env.scope(), msg);
  *result = error.into();

  napi_ok
}
