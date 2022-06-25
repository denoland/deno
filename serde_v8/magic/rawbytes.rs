// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
pub(crate) type AtomicPtr<T> = *mut T;
#[allow(unused)]
pub(crate) struct RawBytes {
  ptr: *const u8,
  len: usize,
  // inlined "trait object"
  data: AtomicPtr<()>,
  vtable: &'static Vtable,
}

impl RawBytes {
  pub fn new_raw(
    ptr: *const u8,
    len: usize,
    data: AtomicPtr<()>,
    vtable: &'static Vtable,
  ) -> bytes::Bytes {
    RawBytes {
      ptr,
      len,
      data,
      vtable,
    }
    .into()
  }
}

#[allow(unused)]
pub(crate) struct Vtable {
  /// fn(data, ptr, len)
  pub clone: unsafe fn(&AtomicPtr<()>, *const u8, usize) -> bytes::Bytes,
  /// fn(data, ptr, len)
  pub drop: unsafe fn(&mut AtomicPtr<()>, *const u8, usize),
}

impl From<RawBytes> for bytes::Bytes {
  fn from(b: RawBytes) -> Self {
    // SAFETY: RawBytes has the same layout as bytes::Bytes
    // this is tested below, both are composed of usize-d ptrs/values
    // thus aren't currently subject to rust's field re-ordering to minimize padding
    unsafe { std::mem::transmute(b) }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::mem;

  const HELLO: &str = "hello";

  // ===== impl StaticVtable =====

  const STATIC_VTABLE: Vtable = Vtable {
    clone: static_clone,
    drop: static_drop,
  };

  unsafe fn static_clone(
    _: &AtomicPtr<()>,
    ptr: *const u8,
    len: usize,
  ) -> bytes::Bytes {
    from_static(std::slice::from_raw_parts(ptr, len)).into()
  }

  unsafe fn static_drop(_: &mut AtomicPtr<()>, _: *const u8, _: usize) {
    // nothing to drop for &'static [u8]
  }

  fn from_static(bytes: &'static [u8]) -> RawBytes {
    RawBytes {
      ptr: bytes.as_ptr(),
      len: bytes.len(),
      data: std::ptr::null_mut(),
      vtable: &STATIC_VTABLE,
    }
  }

  #[test]
  fn bytes_identity() {
    let b1: bytes::Bytes = from_static(HELLO.as_bytes()).into();
    let b2 = bytes::Bytes::from_static(HELLO.as_bytes());
    assert_eq!(b1, b2); // Values are equal
  }

  #[test]
  fn bytes_layout() {
    // SAFETY: ensuring layout is the same
    let u1: [usize; 4] =
      unsafe { mem::transmute(from_static(HELLO.as_bytes())) };
    // SAFETY: ensuring layout is the same
    let u2: [usize; 4] =
      unsafe { mem::transmute(bytes::Bytes::from_static(HELLO.as_bytes())) };
    assert_eq!(u1[..3], u2[..3]); // Struct bytes are equal besides Vtables
  }
}
