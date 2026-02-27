// Copyright 2018-2025 the Deno authors. MIT license.

use super::buffer::JsBuffer;
use super::transl8::FromV8;
use super::transl8::ToV8;
use crate::Error;
use crate::ToJsBuffer;
use crate::magic::transl8::impl_magic;
use num_bigint::BigInt;

/// An untagged enum type that can be any of number, string, bool, bigint, or
/// buffer.
#[derive(Debug)]
pub enum AnyValue {
  RustBuffer(ToJsBuffer),
  V8Buffer(JsBuffer),
  String(String),
  Number(f64),
  BigInt(BigInt),
  Bool(bool),
}

impl_magic!(AnyValue);

impl ToV8 for AnyValue {
  fn to_v8<'scope, 'i>(
    &self,
    scope: &mut v8::PinScope<'scope, 'i>,
  ) -> Result<v8::Local<'scope, v8::Value>, crate::Error> {
    match self {
      Self::RustBuffer(buf) => crate::to_v8(scope, buf),
      Self::V8Buffer(_) => unreachable!(),
      Self::String(s) => crate::to_v8(scope, s),
      Self::Number(num) => crate::to_v8(scope, num),
      Self::BigInt(bigint) => {
        crate::to_v8(scope, crate::BigInt::from(bigint.clone()))
      }
      Self::Bool(b) => crate::to_v8(scope, b),
    }
  }
}

impl FromV8 for AnyValue {
  fn from_v8<'scope, 'i>(
    scope: &mut v8::PinScope<'scope, 'i>,
    value: v8::Local<'scope, v8::Value>,
  ) -> Result<Self, crate::Error> {
    if value.is_string() {
      let string = crate::from_v8(scope, value)?;
      Ok(AnyValue::String(string))
    } else if value.is_number() {
      let string = crate::from_v8(scope, value)?;
      Ok(AnyValue::Number(string))
    } else if value.is_big_int() {
      let bigint = crate::BigInt::from_v8(scope, value)?;
      Ok(AnyValue::BigInt(bigint.into()))
    } else if value.is_array_buffer_view() {
      let buf = JsBuffer::from_v8(scope, value)?;
      Ok(AnyValue::V8Buffer(buf))
    } else if value.is_boolean() {
      let string = crate::from_v8(scope, value)?;
      Ok(AnyValue::Bool(string))
    } else {
      Err(Error::Message(
        "expected string, number, bigint, ArrayBufferView, boolean".into(),
      ))
    }
  }
}
