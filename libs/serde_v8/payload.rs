// Copyright 2018-2025 the Deno authors. MIT license.

// TODO: maybe add a Payload type that holds scope & v8::Value
// so it can implement Deserialize by itself

// Classifies v8::Values into sub-types
#[derive(Debug)]
pub enum ValueType {
  Null,
  Bool,
  Number,
  BigInt,
  String,
  Array,
  ArrayBuffer,
  ArrayBufferView,
  Object,
}

impl ValueType {
  pub fn from_v8(v: v8::Local<v8::Value>) -> ValueType {
    if v.is_boolean() {
      return Self::Bool;
    } else if v.is_number() {
      return Self::Number;
    } else if v.is_string() {
      return Self::String;
    } else if v.is_array() {
      return Self::Array;
    } else if v.is_big_int() {
      return Self::BigInt;
    } else if v.is_array_buffer() {
      return Self::ArrayBuffer;
    } else if v.is_array_buffer_view() {
      return Self::ArrayBufferView;
    } else if v.is_object() {
      return Self::Object;
    } else if v.is_null_or_undefined() {
      return Self::Null;
    }
    panic!("serde_v8: unknown ValueType for v8::Value")
  }
}
