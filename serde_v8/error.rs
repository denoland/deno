// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::fmt::Display;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
  #[error("{0}")]
  Message(String),

  #[error("serde_v8 error: invalid type; expected: boolean, got: {0}")]
  ExpectedBoolean(&'static str),

  #[error("serde_v8 error: invalid type; expected: integer, got: {0}")]
  ExpectedInteger(&'static str),

  #[error("serde_v8 error: invalid type; expected: number, got: {0}")]
  ExpectedNumber(&'static str),

  #[error("serde_v8 error: invalid type; expected: string, got: {0}")]
  ExpectedString(&'static str),

  #[error("serde_v8 error: invalid type; expected: array, got: {0}")]
  ExpectedArray(&'static str),

  #[error("serde_v8 error: invalid type; expected: map, got: {0}")]
  ExpectedMap(&'static str),

  #[error("serde_v8 error: invalid type; expected: enum, got: {0}")]
  ExpectedEnum(&'static str),

  #[error("serde_v8 error: invalid type; expected: object, got: {0}")]
  ExpectedObject(&'static str),

  #[error("serde_v8 error: invalid type; expected: buffer, got: {0}")]
  ExpectedBuffer(&'static str),

  #[error("serde_v8 error: invalid type; expected: detachable, got: {0}")]
  ExpectedDetachable(&'static str),

  #[error("serde_v8 error: invalid type; expected: external, got: {0}")]
  ExpectedExternal(&'static str),

  #[error("serde_v8 error: invalid type; expected: bigint, got: {0}")]
  ExpectedBigInt(&'static str),

  #[error("serde_v8 error: invalid type, expected: utf8")]
  ExpectedUtf8,
  #[error("serde_v8 error: invalid type, expected: latin1")]
  ExpectedLatin1,

  #[error("serde_v8 error: unsupported type")]
  UnsupportedType,

  #[error("serde_v8 error: length mismatch, got: {0}, expected: {1}")]
  LengthMismatch(usize, usize),

  #[error("serde_v8 error: can't create slice from resizable ArrayBuffer")]
  ResizableBackingStoreNotSupported,
}

impl serde::ser::Error for Error {
  fn custom<T: Display>(msg: T) -> Self {
    Error::Message(msg.to_string())
  }
}

impl serde::de::Error for Error {
  fn custom<T: Display>(msg: T) -> Self {
    Error::Message(msg.to_string())
  }
}

pub(crate) fn value_to_type_str(value: v8::Local<v8::Value>) -> &'static str {
  if value.is_module_namespace_object() {
    "Module"
  } else if value.is_wasm_module_object() {
    "WASM module"
  } else if value.is_wasm_memory_object() {
    "WASM memory object"
  } else if value.is_proxy() {
    "Proxy"
  } else if value.is_shared_array_buffer() {
    "SharedArrayBuffer"
  } else if value.is_data_view() {
    "DataView"
  } else if value.is_big_uint64_array() {
    "BigUint64Array"
  } else if value.is_big_int64_array() {
    "BigInt64Array"
  } else if value.is_float64_array() {
    "Float64Array"
  } else if value.is_float32_array() {
    "Float32Array"
  } else if value.is_int32_array() {
    "Int32Array"
  } else if value.is_uint32_array() {
    "Uint32Array"
  } else if value.is_int16_array() {
    "Int16Array"
  } else if value.is_uint16_array() {
    "Uint16Array"
  } else if value.is_int8_array() {
    "Int8Array"
  } else if value.is_uint8_clamped_array() {
    "Uint8ClampedArray"
  } else if value.is_uint8_array() {
    "Uint8Array"
  } else if value.is_typed_array() {
    "TypedArray"
  } else if value.is_array_buffer_view() {
    "ArrayBufferView"
  } else if value.is_array_buffer() {
    "ArrayBuffer"
  } else if value.is_weak_set() {
    "WeakSet"
  } else if value.is_weak_map() {
    "WeakMap"
  } else if value.is_set_iterator() {
    "Set Iterator"
  } else if value.is_map_iterator() {
    "Map Iterator"
  } else if value.is_set() {
    "Set"
  } else if value.is_map() {
    "Map"
  } else if value.is_promise() {
    "Promise"
  } else if value.is_generator_function() {
    "Generator function"
  } else if value.is_async_function() {
    "Async function"
  } else if value.is_reg_exp() {
    "RegExp"
  } else if value.is_date() {
    "Date"
  } else if value.is_number() {
    "Number"
  } else if value.is_boolean() {
    "Boolean"
  } else if value.is_big_int() {
    "bigint"
  } else if value.is_array() {
    "array"
  } else if value.is_function() {
    "function"
  } else if value.is_symbol() {
    "symbol"
  } else if value.is_string() {
    "string"
  } else if value.is_null() {
    "null"
  } else if value.is_undefined() {
    "undefined"
  } else {
    "unknown"
  }
}
