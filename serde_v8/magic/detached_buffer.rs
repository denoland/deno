// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::transl8::FromV8;
use super::transl8::ToV8;
use super::v8slice::value_to_array_buffer;
use super::v8slice::V8Slice;
use crate::magic::transl8::impl_magic;

// A buffer that detaches when deserialized from JS
pub struct DetachedBuffer(V8Slice);
impl_magic!(DetachedBuffer);

impl DetachedBuffer {
  /// View the contents of the underlying byte slice.
  ///
  /// ### Safety
  ///
  /// V8 must never be invoked or the underlying [v8::BackingStore] accessed or
  /// resized for the duration of the callback `cb`'s execution.
  pub fn open<'s, F, R>(&'s self, cb: F) -> R
  where
    F: FnOnce(&[u8]) -> R,
  {
    self.0.open(cb)
  }

  /// Access a mutable slice the contents of the underlying byte slice.
  ///
  /// ### Panics
  ///
  /// Must not be called on a V8Slice destined for V8.
  ///
  /// ### Safety
  ///
  /// V8 must never be invoked or the underlying [v8::BackingStore] accessed or
  /// resized for the duration of the callback `cb`'s execution.
  pub fn open_mut<'s, F, R>(&'s mut self, cb: F) -> R
  where
    F: FnOnce(&mut [u8]) -> R,
  {
    self.0.open_mut(cb)
  }

  /// Copy the contents of the underlying byte slice into a new [Vec].
  pub fn to_vec(&self) -> Vec<u8> {
    self.open(|bytes| bytes.to_vec())
  }
}

impl ToV8 for DetachedBuffer {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let data: Box<[u8]> = self.0.to_vec().into();
    let len = data.len();
    let backing_store =
      v8::ArrayBuffer::new_backing_store_from_boxed_slice(data).make_shared();
    let array_buffer =
      v8::ArrayBuffer::with_backing_store(scope, &backing_store);
    Ok(
      v8::Uint8Array::new(scope, array_buffer, 0, len)
        .expect("Uint8Array creation to succeed")
        .into(),
    )
  }
}

impl FromV8 for DetachedBuffer {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let (buffer, range) = value_to_array_buffer(scope, value)
      .map_err(|_| crate::Error::ExpectedBuffer)?;
    if !buffer.is_detachable() {
      return Err(crate::Error::ExpectedDetachable);
    }
    let v8slice = V8Slice::from_array_buffer(buffer, range)
      .map_err(|_| crate::Error::ExpectedBuffer)?;
    buffer.detach(None); // Detach
    Ok(DetachedBuffer(v8slice))
  }
}
