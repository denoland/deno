// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::ops::Deref;
use std::ops::DerefMut;

use bytes::Buf;
use serde_v8::JsBuffer;

/// BufView is a wrapper around an underlying contiguous chunk  of bytes. It can
/// be created from a [JsBuffer], [bytes::Bytes], or [Vec<u8>] and implements
/// `Deref<[u8]>` and `AsRef<[u8]>`.
///
/// The wrapper has the ability to constrain the exposed view to a sub-region of
/// the underlying buffer. This is useful for write operations, because they may
/// have to be called multiple times, with different views onto the buffer to be
/// able to write it entirely.
pub struct BufView {
  inner: BufViewInner,
  cursor: usize,
}

enum BufViewInner {
  Empty,
  Bytes(bytes::Bytes),
  JsBuffer(JsBuffer),
  Vec(Vec<u8>),
}

impl BufView {
  const fn from_inner(inner: BufViewInner) -> Self {
    Self { inner, cursor: 0 }
  }

  pub const fn empty() -> Self {
    Self::from_inner(BufViewInner::Empty)
  }

  /// Get the length of the buffer view. This is the length of the underlying
  /// buffer minus the cursor position.
  pub fn len(&self) -> usize {
    match &self.inner {
      BufViewInner::Empty => 0,
      BufViewInner::Bytes(bytes) => bytes.len() - self.cursor,
      BufViewInner::JsBuffer(js_buf) => js_buf.len() - self.cursor,
      BufViewInner::Vec(vec) => vec.len() - self.cursor,
    }
  }

  /// Is the buffer view empty?
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Advance the internal cursor of the buffer view by `n` bytes.
  pub fn advance_cursor(&mut self, n: usize) {
    assert!(self.len() >= n);
    self.cursor += n;
  }

  /// Reset the internal cursor of the buffer view to the beginning of the
  /// buffer. Returns the old cursor position.
  pub fn reset_cursor(&mut self) -> usize {
    let old = self.cursor;
    self.cursor = 0;
    old
  }
}

impl Buf for BufView {
  fn remaining(&self) -> usize {
    self.len()
  }

  fn chunk(&self) -> &[u8] {
    self.deref()
  }

  fn advance(&mut self, cnt: usize) {
    self.advance_cursor(cnt)
  }
}

impl Deref for BufView {
  type Target = [u8];

  fn deref(&self) -> &[u8] {
    let buf = match &self.inner {
      BufViewInner::Empty => &[],
      BufViewInner::Bytes(bytes) => bytes.deref(),
      BufViewInner::JsBuffer(js_buf) => js_buf.deref(),
      BufViewInner::Vec(vec) => vec.deref(),
    };
    &buf[self.cursor..]
  }
}

impl AsRef<[u8]> for BufView {
  fn as_ref(&self) -> &[u8] {
    self.deref()
  }
}

impl From<JsBuffer> for BufView {
  fn from(buf: JsBuffer) -> Self {
    Self::from_inner(BufViewInner::JsBuffer(buf))
  }
}

impl From<Vec<u8>> for BufView {
  fn from(vec: Vec<u8>) -> Self {
    Self::from_inner(BufViewInner::Vec(vec))
  }
}

impl From<bytes::Bytes> for BufView {
  fn from(buf: bytes::Bytes) -> Self {
    Self::from_inner(BufViewInner::Bytes(buf))
  }
}

impl From<BufView> for bytes::Bytes {
  fn from(buf: BufView) -> Self {
    match buf.inner {
      BufViewInner::Empty => bytes::Bytes::new(),
      BufViewInner::Bytes(bytes) => bytes,
      BufViewInner::JsBuffer(js_buf) => js_buf.into(),
      BufViewInner::Vec(vec) => vec.into(),
    }
  }
}

/// BufMutView is a wrapper around an underlying contiguous chunk of writable
/// bytes. It can be created from a `JsBuffer` or a `Vec<u8>` and implements
/// `DerefMut<[u8]>` and `AsMut<[u8]>`.
///
/// The wrapper has the ability to constrain the exposed view to a sub-region of
/// the underlying buffer. This is useful for write operations, because they may
/// have to be called multiple times, with different views onto the buffer to be
/// able to write it entirely.
///
/// A `BufMutView` can be turned into a `BufView` by calling `BufMutView::into_view`.
pub struct BufMutView {
  inner: BufMutViewInner,
  cursor: usize,
}

enum BufMutViewInner {
  JsBuffer(JsBuffer),
  Vec(Vec<u8>),
}

impl BufMutView {
  fn from_inner(inner: BufMutViewInner) -> Self {
    Self { inner, cursor: 0 }
  }

  pub fn new(len: usize) -> Self {
    Self::from_inner(BufMutViewInner::Vec(vec![0; len]))
  }

  /// Get the length of the buffer view. This is the length of the underlying
  /// buffer minus the cursor position.
  pub fn len(&self) -> usize {
    match &self.inner {
      BufMutViewInner::JsBuffer(js_buf) => js_buf.len() - self.cursor,
      BufMutViewInner::Vec(vec) => vec.len() - self.cursor,
    }
  }

  /// Is the buffer view empty?
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Advance the internal cursor of the buffer view by `n` bytes.
  pub fn advance_cursor(&mut self, n: usize) {
    assert!(self.len() >= n);
    self.cursor += n;
  }

  /// Reset the internal cursor of the buffer view to the beginning of the
  /// buffer. Returns the old cursor position.
  pub fn reset_cursor(&mut self) -> usize {
    let old = self.cursor;
    self.cursor = 0;
    old
  }

  /// Turn this `BufMutView` into a `BufView`.
  pub fn into_view(self) -> BufView {
    let inner = match self.inner {
      BufMutViewInner::JsBuffer(js_buf) => BufViewInner::JsBuffer(js_buf),
      BufMutViewInner::Vec(vec) => BufViewInner::Vec(vec),
    };
    BufView {
      inner,
      cursor: self.cursor,
    }
  }

  /// Unwrap the underlying buffer into a `Vec<u8>`, consuming the `BufMutView`.
  ///
  /// This method panics when called on a `BufMutView` that was created from a
  /// `JsBuffer`.
  pub fn unwrap_vec(self) -> Vec<u8> {
    match self.inner {
      BufMutViewInner::JsBuffer(_) => {
        panic!("Cannot unwrap a JsBuffer backed BufMutView into a Vec");
      }
      BufMutViewInner::Vec(vec) => vec,
    }
  }

  /// Get a mutable reference to an underlying `Vec<u8>`.
  ///
  /// This method panics when called on a `BufMutView` that was created from a
  /// `JsBuffer`.
  pub fn get_mut_vec(&mut self) -> &mut Vec<u8> {
    match &mut self.inner {
      BufMutViewInner::JsBuffer(_) => {
        panic!("Cannot unwrap a JsBuffer backed BufMutView into a Vec");
      }
      BufMutViewInner::Vec(vec) => vec,
    }
  }
}

impl Buf for BufMutView {
  fn remaining(&self) -> usize {
    self.len()
  }

  fn chunk(&self) -> &[u8] {
    self.deref()
  }

  fn advance(&mut self, cnt: usize) {
    self.advance_cursor(cnt)
  }
}

impl Deref for BufMutView {
  type Target = [u8];

  fn deref(&self) -> &[u8] {
    let buf = match &self.inner {
      BufMutViewInner::JsBuffer(js_buf) => js_buf.deref(),
      BufMutViewInner::Vec(vec) => vec.deref(),
    };
    &buf[self.cursor..]
  }
}

impl DerefMut for BufMutView {
  fn deref_mut(&mut self) -> &mut [u8] {
    let buf = match &mut self.inner {
      BufMutViewInner::JsBuffer(js_buf) => js_buf.deref_mut(),
      BufMutViewInner::Vec(vec) => vec.deref_mut(),
    };
    &mut buf[self.cursor..]
  }
}

impl AsRef<[u8]> for BufMutView {
  fn as_ref(&self) -> &[u8] {
    self.deref()
  }
}

impl AsMut<[u8]> for BufMutView {
  fn as_mut(&mut self) -> &mut [u8] {
    self.deref_mut()
  }
}

impl From<JsBuffer> for BufMutView {
  fn from(buf: JsBuffer) -> Self {
    Self::from_inner(BufMutViewInner::JsBuffer(buf))
  }
}

impl From<Vec<u8>> for BufMutView {
  fn from(buf: Vec<u8>) -> Self {
    Self::from_inner(BufMutViewInner::Vec(buf))
  }
}

pub enum WriteOutcome {
  Partial { nwritten: usize, view: BufView },
  Full { nwritten: usize },
}

impl WriteOutcome {
  pub fn nwritten(&self) -> usize {
    match self {
      WriteOutcome::Partial { nwritten, .. } => *nwritten,
      WriteOutcome::Full { nwritten } => *nwritten,
    }
  }
}
