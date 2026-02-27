// Copyright 2018-2025 the Deno authors. MIT license.

use core::ops::Range;
use std::ops::Deref;
use std::ops::DerefMut;

use super::transl8::FromV8;
use super::transl8::ToV8;
use super::v8slice::V8Slice;
use super::v8slice::to_ranged_buffer;
use crate::magic::transl8::impl_magic;

// A buffer that detaches when deserialized from JS
pub struct DetachedBuffer(V8Slice<u8>);
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
  fn to_v8<'scope, 'i>(
    &self,
    scope: &mut v8::PinScope<'scope, 'i>,
  ) -> Result<v8::Local<'scope, v8::Value>, crate::Error> {
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &self.0.store);
    let Range { start, end } = self.0.range;
    let (off, len) = (start, end - start);
    let v = v8::Uint8Array::new(scope, buffer, off, len).unwrap();
    Ok(v.into())
  }
}

impl FromV8 for DetachedBuffer {
  fn from_v8<'scope, 'i>(
    scope: &mut v8::PinScope<'scope, 'i>,
    value: v8::Local<'scope, v8::Value>,
  ) -> Result<Self, crate::Error> {
    let (b, range) = to_ranged_buffer(scope, value)
      .map_err(|_| crate::Error::ExpectedBuffer(value.type_repr()))?;
    if !b.is_detachable() {
      return Err(crate::Error::ExpectedDetachable(value.type_repr()));
    }
    let store = b.get_backing_store();
    b.detach(None); // Detach
    // SAFETY: We got these values from to_ranged_buffer
    Ok(Self(unsafe { V8Slice::from_parts(store, range) }))
  }
}
