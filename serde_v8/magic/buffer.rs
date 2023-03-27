// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fmt::Debug;

use super::transl8::FromV8;
use super::transl8::ToV8;
use super::v8slice::V8Slice;
use crate::magic::transl8::impl_magic;

// An asymmetric wrapper around V8Slice,
// allowing us to use a single type for familiarity
pub enum ZeroCopyBuf {
  FromV8(V8Slice),
  ToV8(Option<Box<[u8]>>),
}

impl_magic!(ZeroCopyBuf);

impl Debug for ZeroCopyBuf {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.open(|bytes| f.debug_list().entries(bytes.iter()).finish())
  }
}

impl ZeroCopyBuf {
  pub fn empty() -> Self {
    ZeroCopyBuf::ToV8(Some(vec![0_u8; 0].into_boxed_slice()))
  }

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
    match self {
      Self::FromV8(v8slice) => v8slice.open(cb),
      Self::ToV8(Some(data)) => cb(&data),
      Self::ToV8(_) => {
        panic!("tried to read a V8Slice already sent to V8")
      }
    }
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
    match self {
      Self::FromV8(v8slice) => v8slice.open_mut(cb),
      Self::ToV8(Some(data)) => cb(&mut *data),
      Self::ToV8(_) => {
        panic!("tried to read a V8Slice already sent to V8")
      }
    }
  }

  /// Copy the contents of the underlying byte slice into a new [Vec].
  pub fn to_vec(&self) -> Vec<u8> {
    self.open(|bytes| bytes.to_vec())
  }
}

impl Clone for ZeroCopyBuf {
  /// Clone a ZeroCopyBuf. This creates a new reference to the underlying data,
  /// and does not clone the data.
  ///
  /// ### Panics
  ///
  /// Must not be called on a V8Slice destined for V8.
  fn clone(&self) -> Self {
    match self {
      Self::FromV8(zbuf) => Self::FromV8(zbuf.clone()),
      Self::ToV8(_) => {
        panic!("ZeroCopyBufs that will be sent to v8 can not be cloned")
      }
    }
  }
}

impl From<Box<[u8]>> for ZeroCopyBuf {
  fn from(buf: Box<[u8]>) -> Self {
    ZeroCopyBuf::ToV8(Some(buf))
  }
}

impl From<Vec<u8>> for ZeroCopyBuf {
  fn from(vec: Vec<u8>) -> Self {
    vec.into_boxed_slice().into()
  }
}

impl ToV8 for ZeroCopyBuf {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let data: Box<[u8]> = match self {
      Self::FromV8(buf) => buf.to_vec().into(),
      Self::ToV8(ref mut x) => x
        .take()
        .expect("tried to serialize a V8Slice already sent to V8"),
    };

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

impl FromV8 for ZeroCopyBuf {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    Ok(Self::FromV8(V8Slice::from_v8(scope, value)?))
  }
}
