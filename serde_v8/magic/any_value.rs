use num_bigint::BigInt;

// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use super::buffer::ZeroCopyBuf;
use super::transl8::FromV8;
use super::transl8::ToV8;
use crate::magic::transl8::impl_magic;
use crate::Error;

/// An untagged enum type that can be any of number, string, bool, bigint, or
/// buffer.
#[derive(Debug)]
pub enum AnyValue {
  Buffer(ZeroCopyBuf),
  String(String),
  Number(f64),
  BigInt(BigInt),
  Bool(bool),
}

impl_magic!(AnyValue);

impl ToV8 for AnyValue {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    match self {
      Self::Buffer(buf) => buf.to_v8(scope),
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
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
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
      let buf = ZeroCopyBuf::from_v8(scope, value)?;
      Ok(AnyValue::Buffer(buf))
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
