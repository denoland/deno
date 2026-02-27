// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ops::Range;

use super::transl8::FromV8;

/// A type that may be represented as a [`V8Slice`].
pub trait V8Sliceable: Copy + Clone {
  /// The concrete V8 data view type.
  type V8;
  fn new_buf<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    buf: v8::Local<v8::ArrayBuffer>,
    byte_offset: usize,
    length: usize,
  ) -> Option<v8::Local<'s, Self::V8>>;
}

impl V8Sliceable for u8 {
  type V8 = v8::Uint8Array;
  fn new_buf<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    buf: v8::Local<v8::ArrayBuffer>,
    byte_offset: usize,
    length: usize,
  ) -> Option<v8::Local<'s, Self::V8>> {
    v8::Uint8Array::new(scope, buf, byte_offset, length)
  }
}

impl V8Sliceable for u32 {
  type V8 = v8::Uint32Array;
  fn new_buf<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    buf: v8::Local<v8::ArrayBuffer>,
    byte_offset: usize,
    length: usize,
  ) -> Option<v8::Local<'s, Self::V8>> {
    v8::Uint32Array::new(scope, buf, byte_offset, length)
  }
}

impl V8Sliceable for f32 {
  type V8 = v8::Float32Array;
  fn new_buf<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    buf: v8::Local<v8::ArrayBuffer>,
    byte_offset: usize,
    length: usize,
  ) -> Option<v8::Local<'s, Self::V8>> {
    v8::Float32Array::new(scope, buf, byte_offset, length)
  }
}

impl V8Sliceable for f64 {
  type V8 = v8::Float64Array;
  fn new_buf<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    buf: v8::Local<v8::ArrayBuffer>,
    byte_offset: usize,
    length: usize,
  ) -> Option<v8::Local<'s, Self::V8>> {
    v8::Float64Array::new(scope, buf, byte_offset, length)
  }
}

/// A V8Slice encapsulates a slice that's been borrowed from a JavaScript
/// ArrayBuffer object. JavaScript objects can normally be garbage collected,
/// but the existence of a V8Slice inhibits this until it is dropped. It
/// behaves much like an Arc<[u8]>.
///
/// # Cloning
/// Cloning a V8Slice does not clone the contents of the buffer,
/// it creates a new reference to that buffer.
///
/// To actually clone the contents of the buffer do
/// `let copy = Vec::from(&*zero_copy_buf);`
#[derive(Clone)]
pub struct V8Slice<T>
where
  T: V8Sliceable,
{
  pub(crate) store: v8::SharedRef<v8::BackingStore>,
  pub(crate) range: Range<usize>,
  _phantom: PhantomData<T>,
}

impl<T> Debug for V8Slice<T>
where
  T: V8Sliceable,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "V8Slice({:?} of {} {})",
      self.range,
      self.store.len(),
      std::any::type_name::<T>()
    ))
  }
}

// SAFETY: unsafe trait must have unsafe implementation
unsafe impl<T> Send for V8Slice<T> where T: V8Sliceable {}

impl<T> V8Slice<T>
where
  T: V8Sliceable,
{
  /// Create one of these for testing. We create and forget an isolate here. If we decide to perform more v8-requiring tests,
  /// this code will probably need to be hoisted to another location.
  #[cfg(test)]
  fn very_unsafe_new_only_for_test(byte_length: usize) -> Self {
    static V8_ONCE: std::sync::Once = std::sync::Once::new();

    V8_ONCE.call_once(|| {
      let platform =
        v8::new_unprotected_default_platform(0, false).make_shared();
      v8::V8::initialize_platform(platform);
      v8::V8::initialize();
    });

    let mut isolate = v8::Isolate::new(Default::default());
    // SAFETY: This is not safe in any way whatsoever, but it's only for testing non-buffer functions.
    unsafe {
      let ptr = v8::ArrayBuffer::new_backing_store(&mut isolate, byte_length);
      std::mem::forget(isolate);
      Self::from_parts(ptr.into(), 0..byte_length)
    }
  }

  /// Create a V8Slice from raw parts.
  ///
  /// # Safety
  ///
  /// The `range` passed to this function *must* be within the bounds of the backing store, as we may
  /// create a slice from this. The [`v8::BackingStore`] must be valid, and valid for use for the purposes
  /// of this `V8Slice` (ie: the caller must understand the repercussions of using shared/resizable
  /// buffers).
  pub unsafe fn from_parts(
    store: v8::SharedRef<v8::BackingStore>,
    range: Range<usize>,
  ) -> Self {
    Self {
      store,
      range: range.start / std::mem::size_of::<T>()
        ..range.end / std::mem::size_of::<T>(),
      _phantom: PhantomData,
    }
  }

  fn as_slice(&self) -> &[T] {
    let store = &self.store;
    let Some(ptr) = store.data() else {
      return &[];
    };
    let clamped_end =
      std::cmp::min(self.range.end, store.len() / std::mem::size_of::<T>());
    let clamped_len = clamped_end.saturating_sub(self.range.start);
    if clamped_len == 0 {
      return &mut [];
    }
    let ptr = ptr.cast::<T>().as_ptr();
    // SAFETY: v8::SharedRef<v8::BackingStore> is similar to Arc<[u8]>,
    // it points to a fixed continuous slice of bytes on the heap.
    // We assume it's initialized and thus safe to read (though may not contain
    // meaningful data).
    // Note that we are likely violating Rust's safety rules here by assuming
    // nobody is mutating this buffer elsewhere, however in practice V8Slices
    // do not have overlapping read/write phases.
    unsafe {
      let ptr = ptr.add(self.range.start);
      std::slice::from_raw_parts(ptr, clamped_len)
    }
  }

  fn as_slice_mut(&mut self) -> &mut [T] {
    let store = &self.store;
    let Some(ptr) = store.data() else {
      return &mut [];
    };
    let clamped_end =
      std::cmp::min(self.range.end, store.len() / std::mem::size_of::<T>());
    let clamped_len = clamped_end.saturating_sub(self.range.start);
    if clamped_len == 0 {
      return &mut [];
    }
    let ptr = ptr.cast::<T>().as_ptr();
    // SAFETY: v8::SharedRef<v8::BackingStore> is similar to Arc<[u8]>,
    // it points to a fixed continuous slice of bytes on the heap.
    // We assume it's initialized and thus safe to read (though may not contain
    // meaningful data).
    // Note that we are likely violating Rust's safety rules here by assuming
    // nobody is mutating this buffer elsewhere, however in practice V8Slices
    // do not have overlapping read/write phases.
    unsafe {
      let ptr = ptr.add(self.range.start);
      std::slice::from_raw_parts_mut(ptr, clamped_len)
    }
  }

  /// Returns the underlying length of the range of this slice. If the range of this slice would exceed the range
  /// of the underlying backing store, the range is clamped so that it falls within the underlying backing store's
  /// valid length.
  pub fn len(&self) -> usize {
    let store = &self.store;
    let clamped_end =
      std::cmp::min(self.range.end, store.len() / std::mem::size_of::<T>());
    clamped_end.saturating_sub(self.range.start)
  }

  /// Returns whether this slice is empty. See `len` for notes about how the length is treated when the range of this
  /// slice exceeds that of the underlying backing store.
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Create a [`Vec<T>`] copy of this slice data.
  pub fn to_vec(&self) -> Vec<T> {
    self.as_slice().to_vec()
  }

  /// Create a [`Box<[T]>`] copy of this slice data.
  pub fn to_boxed_slice(&self) -> Box<[T]> {
    self.to_vec().into_boxed_slice()
  }

  /// Takes this slice and converts it into a strongly-typed v8 array.
  pub fn into_v8_local<'a, 'b>(
    self,
    scope: &mut v8::PinScope<'a, 'b>,
  ) -> Option<v8::Local<'a, T::V8>> {
    let (store, range) = self.into_parts();
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &store);
    T::new_buf(
      scope,
      buffer,
      range.start,
      range.len() / std::mem::size_of::<T>(),
    )
  }

  /// Takes this slice and converts it into a strongly-typed v8 array, ignoring the underlying range.
  pub fn into_v8_unsliced_arraybuffer_local<'a, 'b>(
    self,
    scope: &mut v8::PinScope<'a, 'b>,
  ) -> v8::Local<'a, v8::ArrayBuffer> {
    let (store, _range) = self.into_parts();
    v8::ArrayBuffer::with_backing_store(scope, &store)
  }

  /// Returns the slice to the parts it came from.
  pub fn into_parts(self) -> (v8::SharedRef<v8::BackingStore>, Range<usize>) {
    (
      self.store,
      self.range.start * std::mem::size_of::<T>()
        ..self.range.end * std::mem::size_of::<T>(),
    )
  }

  /// Splits the buffer into two at the given index.
  ///
  /// Afterwards `self` contains elements `[at, len)`, and the returned `V8Slice` contains elements `[0, at)`.
  ///
  /// # Panics
  ///
  /// Panics if `at > len`.
  pub fn split_to(&mut self, at: usize) -> Self {
    let len = self.len();
    assert!(at <= len);
    let offset = self.range.start;
    let mut other = self.clone();
    self.range = offset + at..offset + len;
    other.range = offset..offset + at;
    other
  }

  /// Splits the buffer into two at the given index.
  ///
  /// Afterwards `self` contains elements `[0, at)`, and the returned `V8Slice` contains elements `[at, len)`.
  ///
  /// # Panics
  ///
  /// Panics if `at > len`.
  pub fn split_off(&mut self, at: usize) -> Self {
    let len = self.len();
    assert!(at <= len);
    let offset = self.range.start;
    let mut other = self.clone();
    self.range = offset..offset + at;
    other.range = offset + at..offset + len;
    other
  }

  /// Shortens the buffer, keeping the first `len` bytes and dropping the rest.
  ///
  /// If `len` is greater than the buffer's current length, this has no effect.
  pub fn truncate(&mut self, len: usize) {
    let offset = self.range.start;
    self.range.end = std::cmp::min(offset + len, self.range.end)
  }
}

pub(crate) fn to_ranged_buffer<'scope, 'i>(
  scope: &mut v8::PinScope<'scope, 'i>,
  value: v8::Local<'scope, v8::Value>,
) -> Result<(v8::Local<'scope, v8::ArrayBuffer>, Range<usize>), v8::DataError> {
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
    let (offset, len) = (view.byte_offset(), view.byte_length());
    let buffer = view.buffer(scope).ok_or(v8::DataError::NoData {
      expected: "view to have a buffer",
    })?;
    let buffer = v8::Local::new(scope, buffer); // recreate handle to avoid lifetime issues
    return Ok((buffer, offset..offset + len));
  }
  let b: v8::Local<v8::ArrayBuffer> = value.try_into()?;
  let b = v8::Local::new(scope, b); // recreate handle to avoid lifetime issues
  Ok((b, 0..b.byte_length()))
}

impl<T> FromV8 for V8Slice<T>
where
  T: V8Sliceable,
{
  fn from_v8<'scope, 'i>(
    scope: &mut v8::PinScope<'scope, 'i>,
    value: v8::Local<'scope, v8::Value>,
  ) -> Result<Self, crate::Error> {
    match to_ranged_buffer(scope, value) {
      Ok((b, range)) => {
        let store = b.get_backing_store();
        if store.is_resizable_by_user_javascript() {
          Err(crate::Error::ResizableBackingStoreNotSupported)
        } else if store.is_shared() {
          Err(crate::Error::ExpectedBuffer(value.type_repr()))
        } else {
          // SAFETY: we got these parts from to_ranged_buffer
          Ok(unsafe { V8Slice::from_parts(store, range) })
        }
      }
      Err(_) => Err(crate::Error::ExpectedBuffer(value.type_repr())),
    }
  }
}

impl<T> Deref for V8Slice<T>
where
  T: V8Sliceable,
{
  type Target = [T];
  fn deref(&self) -> &[T] {
    self.as_slice()
  }
}

impl<T> DerefMut for V8Slice<T>
where
  T: V8Sliceable,
{
  fn deref_mut(&mut self) -> &mut [T] {
    self.as_slice_mut()
  }
}

impl<T> AsRef<[T]> for V8Slice<T>
where
  T: V8Sliceable,
{
  fn as_ref(&self) -> &[T] {
    self.as_slice()
  }
}

impl<T> AsMut<[T]> for V8Slice<T>
where
  T: V8Sliceable,
{
  fn as_mut(&mut self) -> &mut [T] {
    self.as_slice_mut()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_slice<T: V8Sliceable>(len: usize) -> V8Slice<T> {
    let slice = V8Slice::<T>::very_unsafe_new_only_for_test(
      len * std::mem::size_of::<T>(),
    );
    assert_eq!(slice.len(), len);
    slice
  }

  #[test]
  pub fn test_split_off() {
    test_split_off_generic::<u8>();
    test_split_off_generic::<u32>();
  }

  pub fn test_split_off_generic<T: V8Sliceable>() {
    let mut slice = make_slice::<T>(1024);
    let mut other = slice.split_off(16);
    assert_eq!(0..16, slice.range);
    assert_eq!(16..1024, other.range);
    let other2 = other.split_off(16);
    assert_eq!(16..32, other.range);
    assert_eq!(32..1024, other2.range);
  }

  #[test]
  pub fn test_split_to() {
    test_split_to_generic::<u8>();
    test_split_to_generic::<u32>();
  }

  pub fn test_split_to_generic<T: V8Sliceable>() {
    let mut slice = make_slice::<T>(1024);
    let other = slice.split_to(16);
    assert_eq!(16..1024, slice.range);
    assert_eq!(0..16, other.range);
    let other2 = slice.split_to(16);
    assert_eq!(32..1024, slice.range);
    assert_eq!(16..32, other2.range);
  }

  #[test]
  pub fn test_truncate() {
    test_truncate_generic::<u8>();
    test_truncate_generic::<u32>();
  }

  pub fn test_truncate_generic<T: V8Sliceable>() {
    let mut slice = make_slice::<T>(1024);
    slice.truncate(16);
    assert_eq!(0..16, slice.range);
  }

  #[test]
  fn test_truncate_after_split() {
    test_truncate_after_split_generic::<u8>();
    test_truncate_after_split_generic::<u32>();
  }

  pub fn test_truncate_after_split_generic<T: V8Sliceable>() {
    let mut slice = make_slice::<T>(1024);
    _ = slice.split_to(16);
    assert_eq!(16..1024, slice.range);
    slice.truncate(16);
    assert_eq!(16..32, slice.range);
  }
}
