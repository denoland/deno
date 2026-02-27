// Copyright 2018-2025 the Deno authors. MIT license.

use std::marker::PhantomData;
use std::mem::ManuallyDrop;

/// Define an external type.
#[macro_export]
macro_rules! external {
  ($type:ty, $name:literal) => {
    impl $crate::Externalizable for $type {
      fn external_marker() -> ::core::primitive::usize {
        // Use the address of a static mut as a way to get around lack of usize-sized TypeId. Because it is mutable, the
        // compiler cannot collapse multiple definitions into one.
        static mut DEFINITION: $crate::ExternalDefinition =
          $crate::ExternalDefinition::new($name);
        // SAFETY: Wash the pointer through black_box so the compiler cannot see what we're going to do with it and needs
        // to assume it will be used for valid purposes. We are taking the address of a static item, but we avoid taking an
        // intermediate mutable reference to make this safe.
        let ptr = ::std::hint::black_box(::std::ptr::addr_of_mut!(DEFINITION));
        ptr as ::core::primitive::usize
      }

      fn external_name() -> &'static ::core::primitive::str {
        $name
      }
    }
  };
}

pub trait Externalizable {
  fn external_marker() -> usize;
  fn external_name() -> &'static str;
}

#[doc(hidden)]
pub struct ExternalDefinition {
  #[allow(unused)]
  pub name: &'static str,
}

impl ExternalDefinition {
  #[doc(hidden)]
  pub const fn new(name: &'static str) -> Self {
    Self { name }
  }
}

#[repr(C)]
struct ExternalWithMarker<T> {
  marker: usize,
  external: T,
}

/// A strongly-typed external pointer. As this is a shared pointer, it only provides immutable references to
/// the underlying data. To allow for interior mutation, use an interior-mutable container such as [`RefCell`].
#[repr(transparent)]
pub struct ExternalPointer<E: Externalizable> {
  ptr: *mut ManuallyDrop<ExternalWithMarker<E>>,
  _type: std::marker::PhantomData<E>,
}

impl<E: Externalizable> ExternalPointer<E> {
  pub fn new(external: E) -> Self {
    let marker = E::external_marker();
    let new =
      Box::new(ManuallyDrop::new(ExternalWithMarker { marker, external }));
    ExternalPointer {
      ptr: Box::into_raw(new),
      _type: PhantomData,
    }
  }

  pub fn into_raw(self) -> *const std::ffi::c_void {
    self.ptr as _
  }

  /// Create an [`ExternalPointer`] from a raw pointer. This does not validate the pointer at all.
  pub fn from_raw(ptr: *const std::ffi::c_void) -> Self {
    ExternalPointer {
      ptr: ptr as _,
      _type: PhantomData,
    }
  }

  /// Checks the alignment and marker of the pointer's data. If this is not a valid pointer for any reason,
  /// panics. If there is a mismatch here there is a serious programming error somewhere in either Rust or JavaScript
  /// and we cannot risk continuing.
  fn validate_pointer(&self) -> *mut ExternalWithMarker<E> {
    let expected_marker = E::external_marker();
    // SAFETY: we assume the pointer is valid. If it is not, we risk a crash but that's
    // unfortunately not something we can easily test.
    if self.ptr.is_null()
      || self.ptr.align_offset(std::mem::align_of::<usize>()) != 0
      || unsafe { std::ptr::read::<usize>(self.ptr as _) } != expected_marker
    {
      panic!(
        "Detected an invalid v8::External (expected {})",
        E::external_name()
      );
    }
    self.ptr as _
  }

  /// Unsafely retrieves the underlying object from this pointer after validating it.
  ///
  /// # Safety
  ///
  /// This method is inherently unsafe because we cannot know if the underlying memory has been deallocated at some point.
  ///
  /// The lifetime of the return value is tied to the pointer itself, however you must take care not to use methods that
  /// mutate the underlying pointer such as `unsafely_take` while this reference is alive.
  pub unsafe fn unsafely_deref(&self) -> &E {
    unsafe {
      let validated_ptr = self.validate_pointer();
      let external = std::ptr::addr_of!((*validated_ptr).external);
      &*external
    }
  }

  /// Unsafely takes the object from this external.
  ///
  /// # Safety
  ///
  /// This method is inherently unsafe because we cannot know if
  /// the underlying memory has been deallocated at some point.
  ///
  /// You must ensure that no other references to this object are alive at the time you call this method.
  pub unsafe fn unsafely_take(self) -> E {
    unsafe {
      let validated_ptr = self.validate_pointer();
      let marker = std::ptr::addr_of_mut!((*validated_ptr).marker);
      // Ensure that this object has not been taken
      assert_ne!(std::ptr::replace(marker, 0), 0);
      std::ptr::write(marker, 0);
      let external =
        std::ptr::read(std::ptr::addr_of!((*validated_ptr).external));
      // Deallocate without dropping
      _ = Box::<ManuallyDrop<ExternalWithMarker<E>>>::from_raw(self.ptr);
      external
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct External1(u32);
  external!(External1, "external 1");

  struct External2(());
  external!(External2, "external 2");

  // Use the same name as External 1
  struct External1b(());
  external!(External1b, "external 1");

  /// Use this to avoid leaking in miri tests
  struct DeallocOnPanic<E: Externalizable>(Option<ExternalPointer<E>>);

  impl<E: Externalizable> DeallocOnPanic<E> {
    pub fn new(external: &ExternalPointer<E>) -> Self {
      Self(Some(ExternalPointer {
        ptr: external.ptr,
        _type: PhantomData,
      }))
    }
  }

  impl<E: Externalizable> Drop for DeallocOnPanic<E> {
    fn drop(&mut self) {
      unsafe {
        self.0.take().unwrap().unsafely_take();
      }
    }
  }

  #[test]
  pub fn test_external() {
    let external = ExternalPointer::new(External1(1));
    assert_eq!(unsafe { external.unsafely_deref() }.0, 1);
    let ptr = external.into_raw();

    let external = ExternalPointer::<External1>::from_raw(ptr);
    assert_eq!(unsafe { external.unsafely_deref() }.0, 1);
    assert_eq!(unsafe { external.unsafely_take() }.0, 1);
  }

  // If this test ever fails then our "pseudo type ID" system is not working as expected. Each of these are considered
  // different "types" of externals and must have different markers.
  #[test]
  pub fn test_external_markers() {
    let m1 = External1::external_marker();
    let m2 = External2::external_marker();
    let m1b = External1b::external_marker();

    assert_ne!(m1, m2);
    assert_ne!(m1, m1b);
  }

  // If this test ever fails then our "pseudo type ID" system is not working as expected. Each of these are considered
  // different "types" of externals and must have different markers, and we must not be able to deref across these
  // different external types.
  #[test]
  #[should_panic]
  pub fn test_external_incompatible_same_name() {
    let external = ExternalPointer::new(External1(1));
    let _dealloc = DeallocOnPanic::new(&external);
    assert_eq!(unsafe { external.unsafely_deref() }.0, 1);
    let ptr = external.into_raw();

    let external = ExternalPointer::<External1b>::from_raw(ptr);
    unsafe {
      external.unsafely_deref();
    }
  }

  // This test fails on miri because it's actually doing bad things
  #[cfg(not(miri))]
  #[test]
  #[should_panic]
  pub fn test_external_deref_after_take() {
    let external = ExternalPointer::new(External1(1));
    let ptr = external.into_raw();

    // OK
    let external = ExternalPointer::<External1>::from_raw(ptr);
    unsafe {
      external.unsafely_take();
    }

    // Panic!
    let external = ExternalPointer::<External1>::from_raw(ptr);
    unsafe {
      external.unsafely_deref();
    }
  }

  #[test]
  #[should_panic]
  pub fn test_external_incompatible_deref() {
    let external = ExternalPointer::new(External1(1));
    let _dealloc = DeallocOnPanic::new(&external);
    assert_eq!(unsafe { external.unsafely_deref() }.0, 1);
    let ptr = external.into_raw();

    let external = ExternalPointer::<External2>::from_raw(ptr);
    unsafe {
      external.unsafely_deref();
    }
  }

  #[test]
  #[should_panic]
  pub fn test_external_incompatible_take() {
    let external = ExternalPointer::new(External1(1));
    let _dealloc = DeallocOnPanic::new(&external);
    assert_eq!(unsafe { external.unsafely_deref() }.0, 1);
    let ptr = external.into_raw();

    let external = ExternalPointer::<External2>::from_raw(ptr);
    unsafe {
      external.unsafely_take();
    }
  }
}
