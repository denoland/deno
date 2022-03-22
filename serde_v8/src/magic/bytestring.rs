// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::ops::{Deref, DerefMut};

use super::transl8::{FromV8, ToV8};
use crate::magic::transl8::impl_magic;
use crate::Error;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ByteString(pub Vec<u8>);
impl_magic!(ByteString);

impl ByteString {
  pub fn new() -> ByteString {
    ByteString(Vec::new())
  }

  pub fn with_capacity(capacity: usize) -> ByteString {
    ByteString(Vec::with_capacity(capacity))
  }

  pub fn capacity(&self) -> usize {
    self.0.capacity()
  }

  pub fn reserve(&mut self, additional: usize) {
    self.0.reserve(additional)
  }

  pub fn reserve_exact(&mut self, additional: usize) {
    self.0.reserve_exact(additional)
  }

  pub fn shrink_to_fit(&mut self) {
    self.0.shrink_to_fit()
  }

  pub fn truncate(&mut self, len: usize) {
    self.0.truncate(len)
  }

  pub fn push(&mut self, value: u8) {
    self.0.push(value)
  }

  pub fn pop(&mut self) -> Option<u8> {
    self.0.pop()
  }
}

impl Default for ByteString {
  fn default() -> Self {
    ByteString::new()
  }
}

impl Deref for ByteString {
  type Target = [u8];

  fn deref(&self) -> &[u8] {
    self.0.deref()
  }
}

impl DerefMut for ByteString {
  fn deref_mut(&mut self) -> &mut [u8] {
    self.0.deref_mut()
  }
}

impl AsRef<[u8]> for ByteString {
  fn as_ref(&self) -> &[u8] {
    self.0.as_ref()
  }
}

impl AsMut<[u8]> for ByteString {
  fn as_mut(&mut self) -> &mut [u8] {
    self.0.as_mut()
  }
}

impl ToV8 for ByteString {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let v =
      v8::String::new_from_one_byte(scope, self, v8::NewStringType::Normal)
        .unwrap();
    Ok(v.into())
  }
}

impl FromV8 for ByteString {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let v8str = v8::Local::<v8::String>::try_from(value)
      .map_err(|_| Error::ExpectedString)?;
    if !v8str.contains_only_onebyte() {
      return Err(Error::ExpectedLatin1);
    }
    let len = v8str.length();
    let mut buffer = Vec::with_capacity(len);
    // SAFETY: we set length == capacity (see previous line),
    // before immediately writing into that buffer and sanity check with an assert
    #[allow(clippy::uninit_vec)]
    unsafe {
      buffer.set_len(len);
      let written = v8str.write_one_byte(
        scope,
        &mut buffer,
        0,
        v8::WriteOptions::NO_NULL_TERMINATION,
      );
      assert!(written == len);
    }
    Ok(ByteString(buffer))
  }
}
