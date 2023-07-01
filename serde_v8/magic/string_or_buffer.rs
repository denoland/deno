// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use super::buffer::JsBuffer;
use super::transl8::FromV8;
use crate::error::value_to_type_str;
use crate::magic::transl8::impl_magic;
use crate::Error;
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
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    if let Ok(buf) = JsBuffer::from_v8(scope, value) {
      return Ok(Self::Buffer(buf));
    } else if let Ok(s) = crate::from_v8(scope, value) {
      return Ok(Self::String(s));
    }
    Err(Error::ExpectedBuffer(value_to_type_str(value)))
  }
}

impl From<StringOrBuffer> for bytes::Bytes {
  fn from(sob: StringOrBuffer) -> Self {
    match sob {
      StringOrBuffer::Buffer(b) => b.into(),
      StringOrBuffer::String(s) => s.into_bytes().into(),
    }
  }
}
