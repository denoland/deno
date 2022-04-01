// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::ops::Deref;
use std::ops::DerefMut;
use std::ops::Range;

use super::transl8::FromV8;

/// A ZeroCopyBuf encapsulates a slice that's been borrowed from a JavaScript
/// ArrayBuffer object. JavaScript objects can normally be garbage collected,
/// but the existence of a ZeroCopyBuf inhibits this until it is dropped. It
/// behaves much like an Arc<[u8]>.
///
/// # Cloning
/// Cloning a ZeroCopyBuf does not clone the contents of the buffer,
/// it creates a new reference to that buffer.
///
/// To actually clone the contents of the buffer do
/// `let copy = Vec::from(&*zero_copy_buf);`
#[derive(Clone)]
pub struct ZeroCopyBuf {
  store: v8::SharedRef<v8::BackingStore>,
  range: Range<usize>,
}

unsafe impl Send for ZeroCopyBuf {}

impl ZeroCopyBuf {
  pub fn from_buffer(
    buffer: v8::Local<v8::ArrayBuffer>,
    range: Range<usize>,
  ) -> Result<Self, v8::DataError> {
    let store = buffer.get_backing_store();
    if store.is_shared() {
      return Err(v8::DataError::BadType {
        actual: "shared ArrayBufferView",
        expected: "non-shared ArrayBufferView",
      });
    }
    Ok(Self { store, range })
  }

  pub fn from_view(
    scope: &mut v8::HandleScope,
    view: v8::Local<v8::ArrayBufferView>,
  ) -> Result<Self, v8::DataError> {
    let buffer = view.buffer(scope).ok_or(v8::DataError::NoData {
      expected: "view to have a buffer",
    })?;
    let (offset, len) = (view.byte_offset(), view.byte_length());
    Self::from_buffer(buffer, offset..offset + len)
  }

  fn as_slice(&self) -> &[u8] {
    unsafe { &*(&self.store[self.range.clone()] as *const _ as *const [u8]) }
  }

  #[allow(clippy::cast_ref_to_mut)]
  fn as_slice_mut(&mut self) -> &mut [u8] {
    unsafe { &mut *(&self.store[self.range.clone()] as *const _ as *mut [u8]) }
  }
}

impl FromV8 for ZeroCopyBuf {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    if value.is_array_buffer() {
      value
        .try_into()
        .and_then(|b| Self::from_buffer(b, 0..b.byte_length()))
    } else {
      value
        .try_into()
        .and_then(|view| Self::from_view(scope, view))
    }
    .map_err(|_| crate::Error::ExpectedBuffer)
  }
}

impl Deref for ZeroCopyBuf {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    self.as_slice()
  }
}

impl DerefMut for ZeroCopyBuf {
  fn deref_mut(&mut self) -> &mut [u8] {
    self.as_slice_mut()
  }
}

impl AsRef<[u8]> for ZeroCopyBuf {
  fn as_ref(&self) -> &[u8] {
    self.as_slice()
  }
}

impl AsMut<[u8]> for ZeroCopyBuf {
  fn as_mut(&mut self) -> &mut [u8] {
    self.as_slice_mut()
  }
}
