// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::v8;
use deno_core::v8::ExternalReference;
use deno_core::v8::MapFnTo;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
enum CryptoBindingError {
  #[error("{0}")]
  #[class(type)]
  #[property("code" = "ERR_INVALID_ARG_TYPE")]
  InvalidArgType(String),
  #[error("Input buffers must have the same byte length")]
  #[class(range)]
  #[property("code" = "ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH")]
  TimingSafeEqualLength,
  #[error("FIPS mode is not supported in Deno.")]
  #[class(generic)]
  FipsUnsupported,
}

fn set_value(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: v8::Local<v8::Value>,
) {
  let key = v8::String::new(scope, name).unwrap();
  obj.set(scope, key.into(), value);
}

fn set_function(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  export_name: &str,
  function: v8::Local<v8::Function>,
) {
  let name = v8::String::new(scope, export_name).unwrap();
  function.set_name(name);
  set_value(scope, obj, export_name, function.into());
}

fn received_type(
  scope: &mut v8::PinScope,
  value: v8::Local<v8::Value>,
) -> String {
  if value.is_null() {
    return "Received null".to_string();
  }
  if value.is_undefined() {
    return "Received undefined".to_string();
  }
  if value.is_string() {
    let value = value.to_string(scope).unwrap().to_rust_string_lossy(scope);
    return format!("Received type string ('{}')", value.replace('\'', "\\'"));
  }
  if value.is_number() {
    let value = value.to_string(scope).unwrap().to_rust_string_lossy(scope);
    return format!("Received type number ({value})");
  }
  if value.is_boolean() {
    let value = if value.is_true() { "true" } else { "false" };
    return format!("Received type boolean ({value})");
  }
  if value.is_big_int() {
    let value = value.to_string(scope).unwrap().to_rust_string_lossy(scope);
    return format!("Received type bigint ({value})");
  }
  if value.is_symbol() {
    return "Received type symbol".to_string();
  }
  if value.is_function() {
    return "Received function".to_string();
  }
  "Received an instance of Object".to_string()
}

fn invalid_buffer_arg(
  scope: &mut v8::PinScope,
  name: &str,
  value: v8::Local<v8::Value>,
) -> CryptoBindingError {
  CryptoBindingError::InvalidArgType(format!(
    "The \"{name}\" argument must be an instance of Buffer, ArrayBuffer, TypedArray, or DataView. {}",
    received_type(scope, value)
  ))
}

fn array_buffer_bytes(
  value: v8::Local<v8::ArrayBuffer>,
) -> Result<Vec<u8>, CryptoBindingError> {
  let length = value.byte_length();
  let mut bytes = vec![0; length];
  if length == 0 {
    return Ok(bytes);
  }
  if let Some(data) = value.data() {
    // SAFETY: V8 owns the ArrayBuffer backing store and the byte length came
    // from the same ArrayBuffer. The copy is bounded by that byte length.
    let slice =
      unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), length) };
    bytes.copy_from_slice(slice);
  }
  Ok(bytes)
}

fn shared_array_buffer_bytes(
  value: v8::Local<v8::SharedArrayBuffer>,
) -> Result<Vec<u8>, CryptoBindingError> {
  let length = value.byte_length();
  let mut bytes = vec![0; length];
  if length == 0 {
    return Ok(bytes);
  }
  let backing_store = value.get_backing_store();
  if let Some(data) = backing_store.data() {
    // SAFETY: V8 owns the SharedArrayBuffer backing store and the byte length
    // came from the same SharedArrayBuffer.
    let slice =
      unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), length) };
    bytes.copy_from_slice(slice);
  }
  Ok(bytes)
}

fn array_buffer_view_bytes(value: v8::Local<v8::ArrayBufferView>) -> Vec<u8> {
  let mut bytes = vec![0; value.byte_length()];
  let copied = value.copy_contents(&mut bytes);
  debug_assert_eq!(copied, bytes.len());
  bytes
}

fn buffer_source_bytes(
  scope: &mut v8::PinScope,
  value: v8::Local<v8::Value>,
  name: &str,
) -> Result<Vec<u8>, CryptoBindingError> {
  if value.is_array_buffer_view() {
    let value = v8::Local::<v8::ArrayBufferView>::try_from(value).unwrap();
    return Ok(array_buffer_view_bytes(value));
  }
  if value.is_array_buffer() {
    let value = v8::Local::<v8::ArrayBuffer>::try_from(value).unwrap();
    return array_buffer_bytes(value);
  }
  if value.is_shared_array_buffer() {
    let value = v8::Local::<v8::SharedArrayBuffer>::try_from(value).unwrap();
    return shared_array_buffer_bytes(value);
  }
  Err(invalid_buffer_arg(scope, name, value))
}

fn timing_safe_equal_impl(
  scope: &mut v8::PinScope<'_, '_>,
  buf1: v8::Local<v8::Value>,
  buf2: v8::Local<v8::Value>,
) -> Result<bool, CryptoBindingError> {
  let buf1 = buffer_source_bytes(scope, buf1, "buf1")?;
  let buf2 = buffer_source_bytes(scope, buf2, "buf2")?;
  if buf1.len() != buf2.len() {
    return Err(CryptoBindingError::TimingSafeEqualLength);
  }
  let mut out = 0;
  for i in 0..buf1.len() {
    out |= buf1[i] ^ buf2[i];
  }
  Ok(out == 0)
}

fn throw_crypto_error(scope: &mut v8::PinScope, error: CryptoBindingError) {
  let exception = deno_core::error::to_v8_error(scope, &error);
  scope.throw_exception(exception);
}

fn timing_safe_equal_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let buf1 = args.get(0);
  let buf2 = args.get(1);
  match timing_safe_equal_impl(scope, buf1, buf2) {
    Ok(equal) => rv.set_bool(equal),
    Err(error) => throw_crypto_error(scope, error),
  }
}

fn get_fips_callback(
  _scope: &mut v8::PinScope,
  _args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  rv.set_bool(false);
}

fn set_fips_callback(
  scope: &mut v8::PinScope,
  _args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  throw_crypto_error(scope, CryptoBindingError::FipsUnsupported);
}

thread_local! {
  static TIMING_SAFE_EQUAL_CALLBACK: v8::FunctionCallback = timing_safe_equal_callback.map_fn_to();
  static GET_FIPS_CALLBACK: v8::FunctionCallback = get_fips_callback.map_fn_to();
  static SET_FIPS_CALLBACK: v8::FunctionCallback = set_fips_callback.map_fn_to();
}

fn function_from_callback<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  callback: v8::FunctionCallback,
) -> v8::Local<'s, v8::Function> {
  v8::FunctionTemplate::new_raw(scope, callback)
    .get_function(scope)
    .unwrap()
}

pub(crate) fn external_references() -> [ExternalReference; 3] {
  [
    TIMING_SAFE_EQUAL_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    GET_FIPS_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    SET_FIPS_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
  ]
}

#[op2(fast)]
pub fn op_node_crypto_timing_safe_equal(
  scope: &mut v8::PinScope<'_, '_>,
  buf1: v8::Local<v8::Value>,
  buf2: v8::Local<v8::Value>,
) -> Result<bool, CryptoBindingError> {
  timing_safe_equal_impl(scope, buf1, buf2)
}

#[op2(fast)]
pub fn op_node_crypto_get_fips() -> bool {
  false
}

#[op2(fast)]
pub fn op_node_crypto_set_fips() -> Result<(), CryptoBindingError> {
  Err(CryptoBindingError::FipsUnsupported)
}

#[op2]
pub fn op_node_internal_binding_crypto<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  let timing_safe_equal = TIMING_SAFE_EQUAL_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "timingSafeEqual", timing_safe_equal);
  let get_fips =
    GET_FIPS_CALLBACK.with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "getFipsCrypto", get_fips);
  let set_fips =
    SET_FIPS_CALLBACK.with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "setFipsCrypto", set_fips);
  obj
}

#[op2]
pub fn op_node_internal_binding_timing_safe_equal<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  let timing_safe_equal = TIMING_SAFE_EQUAL_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "timingSafeEqual", timing_safe_equal);
  let default_obj = v8::Object::new(scope);
  let timing_safe_equal = TIMING_SAFE_EQUAL_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, default_obj, "timingSafeEqual", timing_safe_equal);
  set_value(scope, obj, "default", default_obj.into());
  obj
}
