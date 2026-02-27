// Copyright 2018-2025 the Deno authors. MIT license.

use super::buffer::JsBuffer;
use super::transl8::FromV8;
use crate::Error;
use crate::magic::transl8::impl_magic;
use std::ops::Deref;

#[derive(Debug)]
pub enum StringOrBuffer {
  Buffer(JsBuffer),
  String(String),
}

impl_magic!(StringOrBuffer);

impl Deref for StringOrBuffer {
  type Target = [u8];
  fn deref(&self) -> &Self::Target {
    match self {
      Self::Buffer(b) => b.as_ref(),
      Self::String(s) => s.as_bytes(),
    }
  }
}

impl<'a> TryFrom<&'a StringOrBuffer> for &'a str {
  type Error = std::str::Utf8Error;
  fn try_from(value: &'a StringOrBuffer) -> Result<Self, Self::Error> {
    match value {
      StringOrBuffer::String(s) => Ok(s.as_str()),
      StringOrBuffer::Buffer(b) => std::str::from_utf8(b.as_ref()),
    }
  }
}

impl FromV8 for StringOrBuffer {
  fn from_v8<'scope, 'i>(
    scope: &mut v8::PinScope<'scope, 'i>,
    value: v8::Local<'scope, v8::Value>,
  ) -> Result<Self, crate::Error> {
    match JsBuffer::from_v8(scope, value) {
      Ok(buf) => {
        return Ok(Self::Buffer(buf));
      }
      _ => {
        if let Ok(s) = crate::from_v8(scope, value) {
          return Ok(Self::String(s));
        }
      }
    }
    Err(Error::ExpectedBuffer(value.type_repr()))
  }
}
