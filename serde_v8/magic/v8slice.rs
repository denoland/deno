// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::marker::PhantomData;
use std::ops::Range;

use super::transl8::FromV8;

pub type PhantomUnsync = PhantomData<std::cell::Cell<()>>;
pub type PhantomUnsend = PhantomData<std::sync::MutexGuard<'static, ()>>;

/// [V8Slice] encapsulates a borrowed byte slice from V8, in the form of a
/// [v8::BackingStore]. The allocation backing the [v8::BackingStore] is safe
/// from garbage collection until the [V8Slice] is collected.
///
/// If the underlying [v8::BackingStore] comes from a [v8::ArrayBuffer] wrapped
/// in a [v8::ArrayBufferView], the current start and end range of the view is
/// captured upon creation of the [V8Slice]. The [V8Slice] only exposes the data
/// contained within this range. The [v8::BackingStore] must not come from a
/// [v8::SharedArrayBuffer].
///
/// To access the data backing a [V8Slice], one can call [V8Slice::to_vec] to
/// fully copy the data into a [Vec<u8>], or [V8Slice::open] with a synchronous
/// callback to get access to a `&mut [u8]` representing the data.
///
/// ### Cloning
///
/// Cloning a V8Slice does not clone the contents of the underlying backing
/// store. Rather it clones the underlying smart-pointer.
///
/// To actually clone the contents of the buffer, use [V8Slice::to_vec].
///
/// ### Growing and shrinking ArrayBuffers
///
/// Since V8 11.2, ArrayBuffer is both growable and shrinkable. Both ArrayBuffer
/// growth and shrinkage are implemented in V8 without re-alloc. The maximum
/// length of the buffer must be specifed up-front and is reserved in virtual
/// address space by [v8::BackingStore]. When the [v8::BackingStore] is grown,
/// the underlying buffer is grown to the specified size by allocating physical
/// pages for the relevant exisiting virtual address space. When a buffer is
/// shrunk, the physical pages storing the excess bytes are de-allocated. In
/// both cases the length of the reserved virtual address space stays fixed.
///
/// [V8Slice] can safely handle resizable buffers (safety is explained below).
/// When the underlying [v8::BackingStore] is shrunk below the `range` of this
/// [V8Slice], the length of any exposed byte slices is truncated to fit within
/// the new bounds.
///
/// ### Safety
///
/// To make [V8Slice] fit within Rust's safety guaruantees, the following two
/// constraints must always be upheld (especially in light of buffer resizing):
///
/// - There MUST never exist a mutable reference and a read-only reference to
///   a byte slice at the same time (this is Rust's memory model).
/// - While a `&[u8]` or `&mut [u8]` pointing to an underlying allocation exists
///   that allocation MUST NEVER be deallocated (doing so may result in a
///   use-after-free).
///
/// JavaScript execution has the ability to get a `&mut [u8]` for the underlying
/// bytes at any time. JavaScript execution can also resize the allocation at
/// any time. As such, it is never safe to expose a `&[u8]` or `&mut [u8]` while
/// JavaScript is executing, as this would violate the above constraints.
///
/// To ensure that these constraints can not be violated, this type never
/// exposes `&[u8]` or `&mut [u8]` pointing to the underlying bytes while
/// JavaScript is executing. This is done through two mechanisms:
///
/// - [V8Slice] is not [Send] or [Sync]: it can not be sent to a different
///   thread. This means that no `&[u8]` or `&mut [u8]` can be created from a
///   different thread, out of the purview of the JavaScript executing thread
///   which would possibly cause a constraint violation.
/// - [V8Slice] never exposes a `&[u8]` or `&mut [u8]` that can be held
///   asynchronously across a point causing JavaScript execution. This is
///   enforced through the API design for asynchronous Rust. Users MUST take
///   care to not execute JavaScript within a [V8Slice::open] or
///   [V8Slice::open_mut] callback.
#[derive(Clone)]
pub struct V8Slice {
  pub(crate) store: v8::SharedRef<v8::BackingStore>,
  pub(crate) range: Option<Range<usize>>,
  _no_sync: PhantomUnsync,
  _no_send: PhantomUnsend,
}

impl V8Slice {
  pub fn from_array_buffer(
    buffer: v8::Local<v8::ArrayBuffer>,
    range: Option<Range<usize>>,
  ) -> Result<Self, v8::DataError> {
    let store = buffer.get_backing_store();
    if store.is_shared() {
      return Err(v8::DataError::BadType {
        actual: "shared ArrayBufferView",
        expected: "non-shared ArrayBufferView",
      });
    }
    Ok(Self {
      store,
      range,
      _no_send: Default::default(),
      _no_sync: Default::default(),
    })
  }

  pub fn from_array_buffer_view(
    scope: &mut v8::HandleScope,
    view: v8::Local<v8::ArrayBufferView>,
  ) -> Result<Self, v8::DataError> {
    let (buffer, range) = array_buffer_view_to_array_buffer(scope, view)?;
    Self::from_array_buffer(buffer, range)
  }

  pub fn from_value(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, v8::DataError> {
    let (buffer, range) = value_to_array_buffer(scope, value)?;
    Self::from_array_buffer(buffer, range)
  }

  // The truncated range of the backing store.
  fn truncated_range(&self) -> Range<usize> {
    let actual_length = self.store.byte_length();
    if let Some(range) = &self.range {
      let start = range.start.min(actual_length);
      let end = range.end.min(actual_length);
      start..end
    } else {
      0..actual_length
    }
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
    let range = self.truncated_range();
    let bytes_celled = &self.store[range];
    // SAFETY: v8::BackingStore points to a fixed continous slice of bytes on
    // the heap. Constraints on the V8Slice type ensure that the bytes
    // represented by the v8::BackingStore can not be deallocated or modified
    // for the duration of the callback `cb`'s execution. The reasoning for this
    // is elaborated on the safety comment for V8Slice.
    let bytes: &[u8] = unsafe { &*(bytes_celled as *const _ as *mut [u8]) };
    cb(bytes)
  }

  /// Access a mutable slice the contents of the underlying byte slice.
  ///
  /// ### Safety
  ///
  /// V8 must never be invoked or the underlying [v8::BackingStore] accessed or
  /// resized for the duration of the callback `cb`'s execution.
  pub fn open_mut<'s, F, R>(&'s mut self, cb: F) -> R
  where
    F: FnOnce(&mut [u8]) -> R,
  {
    let range = self.truncated_range();
    let bytes_celled = &self.store[range];
    // SAFETY: v8::BackingStore points to a fixed continous slice of bytes on
    // the heap. Constraints on the V8Slice type ensure that the bytes
    // represented by the v8::BackingStore can not be deallocated or modified
    // for the duration of the callback `cb`'s execution. The reasoning for this
    // is elaborated on the safety comment for V8Slice.
    let bytes: &mut [u8] =
      unsafe { &mut *(bytes_celled as *const _ as *mut [u8]) };
    cb(bytes)
  }

  /// Copy the contents of the underlying byte slice into a new [Vec].
  pub fn to_vec(&self) -> Vec<u8> {
    self.open(|bytes| bytes.to_vec())
  }
}

impl FromV8 for V8Slice {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    Self::from_value(scope, value).map_err(|_| crate::Error::ExpectedBuffer)
  }
}

pub(crate) fn value_to_array_buffer<'a>(
  scope: &mut v8::HandleScope<'a>,
  value: v8::Local<v8::Value>,
) -> Result<(v8::Local<'a, v8::ArrayBuffer>, Option<Range<usize>>), v8::DataError>
{
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
    array_buffer_view_to_array_buffer(scope, view)
  } else if let Ok(buffer) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
    Ok((v8::Local::new(scope, buffer), None))
  } else {
    Err(v8::DataError::BadType {
      actual: "non-ArrayBuffer and non-ArrayBufferView",
      expected: "ArrayBuffer or ArrayBufferView",
    })
  }
}

pub(crate) fn array_buffer_view_to_array_buffer<'a>(
  scope: &mut v8::HandleScope<'a>,
  view: v8::Local<v8::ArrayBufferView>,
) -> Result<(v8::Local<'a, v8::ArrayBuffer>, Option<Range<usize>>), v8::DataError>
{
  let range = view.byte_offset()..view.byte_length();
  let buffer = view.buffer(scope).ok_or(v8::DataError::NoData {
    expected: "view to have a buffer",
  })?;
  Ok((buffer, Some(range)))
}

// // Implement V8Slice -> bytes::Bytes
// impl V8Slice {
//   fn rc_into_byte_parts(self: Rc<Self>) -> (*const u8, usize, *mut V8Slice) {
//     let (ptr, len) = {
//       let slice = self.as_ref();
//       (slice.as_ptr(), slice.len())
//     };
//     let rc_raw = Rc::into_raw(self);
//     let data = rc_raw as *mut V8Slice;
//     (ptr, len, data)
//   }
// }

// impl From<V8Slice> for bytes::Bytes {
//   fn from(v8slice: V8Slice) -> Self {
//     let (ptr, len, data) = Rc::new(v8slice).rc_into_byte_parts();
//     rawbytes::RawBytes::new_raw(ptr, len, data.cast(), &V8SLICE_VTABLE)
//   }
// }

// // NOTE: in the limit we could avoid extra-indirection and use the C++ shared_ptr
// // but we can't store both the underlying data ptr & ctrl ptr ... so instead we
// // use a shared rust ptr (Rc/Arc) that itself controls the C++ shared_ptr
// const V8SLICE_VTABLE: rawbytes::Vtable = rawbytes::Vtable {
//   clone: v8slice_clone,
//   drop: v8slice_drop,
//   to_vec: v8slice_to_vec,
// };

// unsafe fn v8slice_clone(
//   data: &rawbytes::AtomicPtr<()>,
//   ptr: *const u8,
//   len: usize,
// ) -> bytes::Bytes {
//   let rc = Rc::from_raw(*data as *const V8Slice);
//   let (_, _, data) = rc.clone().rc_into_byte_parts();
//   std::mem::forget(rc);
//   // NOTE: `bytes::Bytes` does bounds checking so we trust its ptr, len inputs
//   // and must use them to allow cloning Bytes it has sliced
//   rawbytes::RawBytes::new_raw(ptr, len, data.cast(), &V8SLICE_VTABLE)
// }

// unsafe fn v8slice_to_vec(
//   data: &rawbytes::AtomicPtr<()>,
//   ptr: *const u8,
//   len: usize,
// ) -> Vec<u8> {
//   let rc = Rc::from_raw(*data as *const V8Slice);
//   std::mem::forget(rc);
//   // NOTE: `bytes::Bytes` does bounds checking so we trust its ptr, len inputs
//   // and must use them to allow cloning Bytes it has sliced
//   Vec::from_raw_parts(ptr as _, len, len)
// }

// unsafe fn v8slice_drop(
//   data: &mut rawbytes::AtomicPtr<()>,
//   _: *const u8,
//   _: usize,
// ) {
//   drop(Rc::from_raw(*data as *const V8Slice))
// }
