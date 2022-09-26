// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use super::buffer::ZeroCopyBuf;
use super::transl8::{FromV8, ToV8};
use crate::magic::transl8::impl_magic;
use crate::Error;
use std::ops::Deref;

#[derive(Debug)]
pub enum StringOrBuffer {
  Buffer(ZeroCopyBuf),
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

impl ToV8 for StringOrBuffer {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    match self {
      Self::Buffer(buf) => crate::to_v8(scope, buf),
      Self::String(s) => crate::to_v8(scope, s),
    }
  }
}

impl FromV8 for StringOrBuffer {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    if let Ok(buf) = ZeroCopyBuf::from_v8(scope, value) {
      return Ok(Self::Buffer(buf));
    } else if let Ok(s) = crate::from_v8(scope, value) {
      return Ok(Self::String(s));
    }
    Err(Error::ExpectedBuffer)
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
