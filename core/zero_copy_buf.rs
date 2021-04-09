// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::bindings;
use rusty_v8 as v8;
use std::ops::Deref;
use std::ops::DerefMut;

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
  backing_store: v8::SharedRef<v8::BackingStore>,
  byte_offset: usize,
  byte_length: usize,
}

unsafe impl Send for ZeroCopyBuf {}

impl ZeroCopyBuf {
  pub fn new<'s>(
    scope: &mut v8::HandleScope<'s>,
    view: v8::Local<v8::ArrayBufferView>,
  ) -> Self {
    let backing_store = view.buffer(scope).unwrap().get_backing_store();
    let byte_offset = view.byte_offset();
    let byte_length = view.byte_length();
    Self {
      backing_store,
      byte_offset,
      byte_length,
    }
  }
}

impl Deref for ZeroCopyBuf {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    unsafe {
      bindings::get_backing_store_slice(
        &self.backing_store,
        self.byte_offset,
        self.byte_length,
      )
    }
  }
}

impl DerefMut for ZeroCopyBuf {
  fn deref_mut(&mut self) -> &mut [u8] {
    unsafe {
      bindings::get_backing_store_slice_mut(
        &self.backing_store,
        self.byte_offset,
        self.byte_length,
      )
    }
  }
}

impl AsRef<[u8]> for ZeroCopyBuf {
  fn as_ref(&self) -> &[u8] {
    &*self
  }
}

impl AsMut<[u8]> for ZeroCopyBuf {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut *self
  }
}
