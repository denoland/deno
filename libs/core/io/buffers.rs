// Copyright 2018-2025 the Deno authors. MIT license.

use bytes::Buf;
use bytes::BytesMut;
use serde_v8::JsBuffer;
use serde_v8::V8Slice;
use std::ops::Deref;
use std::ops::DerefMut;

/// BufView is a wrapper around an underlying contiguous chunk of bytes. It can
/// be created from a [JsBuffer], [bytes::Bytes], or [Vec<u8>] and implements
/// `Deref<[u8]>` and `AsRef<[u8]>`.
///
/// The wrapper has the ability to constrain the exposed view to a sub-region of
/// the underlying buffer. This is useful for write operations, because they may
/// have to be called multiple times, with different views onto the buffer to be
/// able to write it entirely.
#[derive(Debug)]
pub struct BufView {
  inner: BufViewInner,
  cursor: usize,
}

#[derive(Debug)]
enum BufViewInner {
  Empty,
  Bytes(bytes::Bytes),
  JsBuffer(V8Slice<u8>),
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

  /// Adjust the length of the remaining buffer. If the requested size is greater than the current
  /// length, no changes are made.
  pub fn truncate(&mut self, size: usize) {
    match &mut self.inner {
      BufViewInner::Empty => {}
      BufViewInner::Bytes(bytes) => bytes.truncate(size + self.cursor),
      BufViewInner::JsBuffer(buffer) => buffer.truncate(size + self.cursor),
    }
  }

  /// Split the underlying buffer. The other piece will maintain the current cursor position while this buffer
  /// will have a cursor of zero.
  pub fn split_off(&mut self, at: usize) -> Self {
    let at = at + self.cursor;
    assert!(at <= self.len());
    let other = match &mut self.inner {
      BufViewInner::Empty => BufViewInner::Empty,
      BufViewInner::Bytes(bytes) => BufViewInner::Bytes(bytes.split_off(at)),
      BufViewInner::JsBuffer(buffer) => {
        BufViewInner::JsBuffer(buffer.split_off(at))
      }
    };
    Self {
      inner: other,
      cursor: 0,
    }
  }

  /// Split the underlying buffer. The other piece will have a cursor of zero while this buffer
  /// will maintain the current cursor position.
  pub fn split_to(&mut self, at: usize) -> Self {
    assert!(at <= self.len());
    let at = at + self.cursor;
    let other = match &mut self.inner {
      BufViewInner::Empty => BufViewInner::Empty,
      BufViewInner::Bytes(bytes) => BufViewInner::Bytes(bytes.split_to(at)),
      BufViewInner::JsBuffer(buffer) => {
        BufViewInner::JsBuffer(buffer.split_to(at))
      }
    };
    let cursor = std::mem::take(&mut self.cursor);
    Self {
      inner: other,
      cursor,
    }
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
    Self::from_inner(BufViewInner::JsBuffer(buf.into_parts()))
  }
}

impl From<Vec<u8>> for BufView {
  fn from(vec: Vec<u8>) -> Self {
    Self::from_inner(BufViewInner::Bytes(vec.into()))
  }
}

impl From<Box<[u8]>> for BufView {
  fn from(data: Box<[u8]>) -> Self {
    Self::from_inner(BufViewInner::Bytes(data.into()))
  }
}

impl From<bytes::Bytes> for BufView {
  fn from(buf: bytes::Bytes) -> Self {
    Self::from_inner(BufViewInner::Bytes(buf))
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
#[derive(Debug)]
pub struct BufMutView {
  inner: BufMutViewInner,
  cursor: usize,
}

#[derive(Debug)]
enum BufMutViewInner {
  JsBuffer(V8Slice<u8>),
  Bytes(BytesMut),
}

impl Default for BufMutView {
  fn default() -> Self {
    BufMutView {
      inner: BufMutViewInner::Bytes(BytesMut::default()),
      cursor: 0,
    }
  }
}

impl BufMutView {
  fn from_inner(inner: BufMutViewInner) -> Self {
    Self { inner, cursor: 0 }
  }

  pub fn new(len: usize) -> Self {
    let bytes = BytesMut::zeroed(len);
    Self::from_inner(BufMutViewInner::Bytes(bytes))
  }

  /// Get the length of the buffer view. This is the length of the underlying
  /// buffer minus the cursor position.
  pub fn len(&self) -> usize {
    match &self.inner {
      BufMutViewInner::JsBuffer(js_buf) => js_buf.len() - self.cursor,
      BufMutViewInner::Bytes(bytes) => bytes.len() - self.cursor,
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
      BufMutViewInner::Bytes(bytes) => BufViewInner::Bytes(bytes.into()),
    };
    BufView {
      inner,
      cursor: self.cursor,
    }
  }

  /// Attempts to unwrap the underlying buffer into a [`BytesMut`], consuming the `BufMutView`. If
  /// this buffer does not have a [`BytesMut`], returns `Self`.
  pub fn maybe_unwrap_bytes(self) -> Result<BytesMut, Self> {
    match self.inner {
      BufMutViewInner::JsBuffer(_) => Err(self),
      BufMutViewInner::Bytes(bytes) => Ok(bytes),
    }
  }

  /// This attempts to grow the `BufMutView` to a target size, by a maximum increment. This method
  /// will be replaced by a better API in the future and should not be used at this time.
  #[must_use = "The result of this method should be tested"]
  #[deprecated = "API will be replaced in the future"]
  #[doc(hidden)]
  pub fn maybe_resize(
    &mut self,
    target_size: usize,
    maximum_increment: usize,
  ) -> Option<usize> {
    match &mut self.inner {
      BufMutViewInner::Bytes(bytes) => {
        use std::cmp::Ordering::*;
        let len = bytes.len();
        let target_size = target_size + self.cursor;
        match target_size.cmp(&len) {
          Greater => {
            bytes
              .resize(std::cmp::min(target_size, len + maximum_increment), 0);
          }
          Less => {
            bytes.truncate(target_size);
          }
          Equal => {}
        }
        Some(bytes.len())
      }
      _ => None,
    }
  }

  /// This attempts to grow the `BufMutView` to a target size, by a maximum increment. This method
  /// will be replaced by a better API in the future and should not be used at this time.
  #[must_use = "The result of this method should be tested"]
  #[deprecated = "API will be replaced in the future"]
  #[doc(hidden)]
  pub fn maybe_grow(&mut self, target_size: usize) -> Option<usize> {
    match &mut self.inner {
      BufMutViewInner::Bytes(bytes) => {
        let len = bytes.len();
        let target_size = target_size + self.cursor;
        if target_size > len {
          bytes.resize(target_size, 0);
        }
        Some(bytes.len())
      }
      _ => None,
    }
  }

  /// Adjust the length of the remaining buffer and ensure that the cursor continues to
  /// stay in-bounds.
  pub fn truncate(&mut self, size: usize) {
    match &mut self.inner {
      BufMutViewInner::Bytes(bytes) => bytes.truncate(size + self.cursor),
      BufMutViewInner::JsBuffer(buffer) => buffer.truncate(size + self.cursor),
    }
    self.cursor = std::cmp::min(self.cursor, self.len());
  }

  /// Split the underlying buffer. The other piece will maintain the current cursor position while this buffer
  /// will have a cursor of zero.
  pub fn split_off(&mut self, at: usize) -> Self {
    let at = at + self.cursor;
    assert!(at <= self.len());
    let other = match &mut self.inner {
      BufMutViewInner::Bytes(bytes) => {
        BufMutViewInner::Bytes(bytes.split_off(at))
      }
      BufMutViewInner::JsBuffer(buffer) => {
        BufMutViewInner::JsBuffer(buffer.split_off(at))
      }
    };
    Self {
      inner: other,
      cursor: 0,
    }
  }

  /// Split the underlying buffer. The other piece will have a cursor of zero while this buffer
  /// will maintain the current cursor position.
  pub fn split_to(&mut self, at: usize) -> Self {
    assert!(at <= self.len());
    let at = at + self.cursor;
    let other = match &mut self.inner {
      BufMutViewInner::Bytes(bytes) => {
        BufMutViewInner::Bytes(bytes.split_to(at))
      }
      BufMutViewInner::JsBuffer(buffer) => {
        BufMutViewInner::JsBuffer(buffer.split_to(at))
      }
    };
    let cursor = std::mem::take(&mut self.cursor);
    Self {
      inner: other,
      cursor,
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
      BufMutViewInner::Bytes(vec) => vec.deref(),
    };
    &buf[self.cursor..]
  }
}

impl DerefMut for BufMutView {
  fn deref_mut(&mut self) -> &mut [u8] {
    let buf = match &mut self.inner {
      BufMutViewInner::JsBuffer(js_buf) => js_buf.deref_mut(),
      BufMutViewInner::Bytes(vec) => vec.deref_mut(),
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
    Self::from_inner(BufMutViewInner::JsBuffer(buf.into_parts()))
  }
}

impl From<BytesMut> for BufMutView {
  fn from(buf: BytesMut) -> Self {
    Self::from_inner(BufMutViewInner::Bytes(buf))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  pub fn bufview_read_and_truncate() {
    let mut buf = BufView::from(vec![1, 2, 3, 4]);
    assert_eq!(4, buf.len());
    assert_eq!(0, buf.cursor);
    assert_eq!(1, buf.get_u8());
    assert_eq!(3, buf.len());
    // The cursor is at position 1, so this truncates the underlying buffer to 2+1
    buf.truncate(2);
    assert_eq!(2, buf.len());
    assert_eq!(2, buf.get_u8());
    assert_eq!(1, buf.len());

    buf.reset_cursor();
    assert_eq!(3, buf.len());
  }

  #[test]
  pub fn bufview_split() {
    let mut buf = BufView::from(Vec::from_iter(0..100));
    assert_eq!(100, buf.len());
    buf.advance_cursor(25);
    assert_eq!(75, buf.len());
    let mut other = buf.split_off(10);
    assert_eq!(25, buf.cursor);
    assert_eq!(10, buf.len());
    assert_eq!(65, other.len());

    let other2 = other.split_to(20);
    assert_eq!(20, other2.len());
    assert_eq!(45, other.len());

    assert_eq!(100, buf.cursor + buf.len() + other.len() + other2.len());
    buf.reset_cursor();
    assert_eq!(100, buf.cursor + buf.len() + other.len() + other2.len());
  }

  #[test]
  pub fn bufmutview_read_and_truncate() {
    let mut buf = BufMutView::from(BytesMut::from([1, 2, 3, 4].as_slice()));
    assert_eq!(4, buf.len());
    assert_eq!(0, buf.cursor);
    assert_eq!(1, buf.get_u8());
    assert_eq!(3, buf.len());
    // The cursor is at position 1, so this truncates the underlying buffer to 2+1
    buf.truncate(2);
    assert_eq!(2, buf.len());
    assert_eq!(2, buf.get_u8());
    assert_eq!(1, buf.len());

    buf.reset_cursor();
    assert_eq!(3, buf.len());
  }

  #[test]
  pub fn bufmutview_split() {
    let mut buf =
      BufMutView::from(BytesMut::from(Vec::from_iter(0..100).as_slice()));
    assert_eq!(100, buf.len());
    buf.advance_cursor(25);
    assert_eq!(75, buf.len());
    let mut other = buf.split_off(10);
    assert_eq!(25, buf.cursor);
    assert_eq!(10, buf.len());
    assert_eq!(65, other.len());

    let other2 = other.split_to(20);
    assert_eq!(20, other2.len());
    assert_eq!(45, other.len());

    assert_eq!(100, buf.cursor + buf.len() + other.len() + other2.len());
    buf.reset_cursor();
    assert_eq!(100, buf.cursor + buf.len() + other.len() + other2.len());
  }

  #[test]
  #[allow(deprecated)]
  fn bufmutview_resize() {
    let new =
      || BufMutView::from(BytesMut::from(Vec::from_iter(0..100).as_slice()));
    let mut buf = new();
    assert_eq!(100, buf.len());
    buf.maybe_resize(200, 10).unwrap();
    assert_eq!(110, buf.len());

    let mut buf = new();
    assert_eq!(100, buf.len());
    buf.maybe_resize(200, 100).unwrap();
    assert_eq!(200, buf.len());

    let mut buf = new();
    assert_eq!(100, buf.len());
    buf.maybe_resize(200, 1000).unwrap();
    assert_eq!(200, buf.len());

    let mut buf = new();
    buf.advance_cursor(50);
    assert_eq!(50, buf.len());
    buf.maybe_resize(100, 100).unwrap();
    assert_eq!(100, buf.len());
    buf.reset_cursor();
    assert_eq!(150, buf.len());
  }

  #[test]
  #[allow(deprecated)]
  fn bufmutview_grow() {
    let new =
      || BufMutView::from(BytesMut::from(Vec::from_iter(0..100).as_slice()));
    let mut buf = new();
    assert_eq!(100, buf.len());
    buf.maybe_grow(200).unwrap();
    assert_eq!(200, buf.len());

    let mut buf = new();
    buf.advance_cursor(50);
    assert_eq!(50, buf.len());
    buf.maybe_grow(100).unwrap();
    assert_eq!(100, buf.len());
    buf.reset_cursor();
    assert_eq!(150, buf.len());
  }
}
