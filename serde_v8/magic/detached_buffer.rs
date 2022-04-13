// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use core::ops::Range;
use std::ops::Deref;
use std::ops::DerefMut;

use super::transl8::FromV8;
use super::transl8::ToV8;
use super::zero_copy_buf::to_ranged_buffer;
use super::zero_copy_buf::ZeroCopyBuf;
use crate::magic::transl8::impl_magic;

// A buffer that detaches when deserialized from JS
pub struct DetachedBuffer(ZeroCopyBuf);
impl_magic!(DetachedBuffer);

impl AsRef<[u8]> for DetachedBuffer {
  fn as_ref(&self) -> &[u8] {
    self.0.as_ref()
  }
}

impl AsMut<[u8]> for DetachedBuffer {
  fn as_mut(&mut self) -> &mut [u8] {
    self.0.as_mut()
  }
}

impl Deref for DetachedBuffer {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    self.0.deref()
  }
}

impl DerefMut for DetachedBuffer {
  fn deref_mut(&mut self) -> &mut [u8] {
    self.0.deref_mut()
  }
}

impl ToV8 for DetachedBuffer {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &self.0.store);
    let Range { start, end } = self.0.range;
    let (off, len) = (start, end - start);
    let v = v8::Uint8Array::new(scope, buffer, off, len).unwrap();
    Ok(v.into())
  }
}

impl FromV8 for DetachedBuffer {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let (b, range) =
      to_ranged_buffer(scope, value).or(Err(crate::Error::ExpectedBuffer))?;
    if !b.is_detachable() {
      return Err(crate::Error::ExpectedDetachable);
    }
    let store = b.get_backing_store();
    b.detach(); // Detach
    Ok(Self(ZeroCopyBuf { store, range }))
  }
}
