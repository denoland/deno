// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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
    // TODO: restore range ?
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &self.0.store);
    let (off, len) =
      (self.0.range.start, self.0.range.end - self.0.range.start);
    let v = v8::Uint8Array::new(scope, buffer, off, len).unwrap();
    Ok(v.into())

    // Re-box
    // let boxed = Box::from(self.0.as_ref());
    // let store = v8::ArrayBuffer::new_backing_store_from_boxed_slice(boxed);
    // let store = store.make_shared();
    // let ab = v8::ArrayBuffer::with_backing_store(scope, &store);
    // let (off, len) = (self.0.range.start, self.0.range.end-self.0.range.start);
    // Ok(
    //   v8::Uint8Array::new(scope, ab, off, len)
    //     .expect("Failed to create Uint8Array")
    //     .into(),
    // )
  }
}

impl FromV8 for DetachedBuffer {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let (b, range) =
      to_ranged_buffer(scope, value).or(Err(crate::Error::ExpectedBuffer))?;
    let store = b.get_backing_store();
    b.detach(); // Detach
    Ok(Self(ZeroCopyBuf { store, range }))
  }
}
