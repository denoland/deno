// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use super::buffer::ZeroCopyBuf;
use super::transl8::FromV8;
use super::transl8::ToV8;
use crate::magic::transl8::impl_magic;
use crate::Error;

#[derive(Debug)]
pub enum StringOrBuffer {
  Buffer(ZeroCopyBuf),
  String(String),
}

impl_magic!(StringOrBuffer);

impl StringOrBuffer {
  /// View the byte representation of the underlying byte slice or string.
  ///
  /// ### Safety
  ///
  /// V8 must never be invoked or the underlying [v8::BackingStore] accessed or
  /// resized for the duration of the callback `cb`'s execution.
  pub fn open<'s, F, R>(&'s self, cb: F) -> R
  where
    F: FnOnce(&[u8]) -> R,
  {
    match self {
      StringOrBuffer::Buffer(buf) => buf.open(cb),
      StringOrBuffer::String(str) => cb(str.as_bytes()),
    }
  }

  /// View the string representation of the underlying byte slice or string. If
  /// the underlying bytes can not be represented as a UTF-8 string, an error is
  /// passed to `cb`.
  ///
  /// ### Safety
  ///
  /// V8 must never be invoked or the underlying [v8::BackingStore] accessed or
  /// resized for the duration of the callback `cb`'s execution.
  pub fn open_str<'s, F, R>(&'s self, cb: F) -> R
  where
    F: FnOnce(Result<&str, std::str::Utf8Error>) -> R,
  {
    match self {
      StringOrBuffer::String(str) => cb(Ok(str)),
      StringOrBuffer::Buffer(buf) => {
        buf.open(|bytes| cb(std::str::from_utf8(bytes)))
      }
    }
  }
}

// impl<'a> TryFrom<&'a StringOrBuffer> for &'a str {
//   type Error = std::str::Utf8Error;
//   fn try_from(value: &'a StringOrBuffer) -> Result<Self, Self::Error> {
//     match value {
//       StringOrBuffer::String(s) => Ok(s.as_str()),
//       StringOrBuffer::Buffer(b) => std::str::from_utf8(b.as_ref()),
//     }
//   }
// }

impl ToV8 for StringOrBuffer {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    match self {
      Self::Buffer(buf) => buf.to_v8(scope),
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
