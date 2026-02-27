// Copyright 2018-2025 the Deno authors. MIT license.

use std::alloc::Layout;
use std::cell::Cell;
use std::mem::offset_of;
use std::ptr::NonNull;

use crate::arena::raw_arena::RawArena;

use super::alloc;
use super::ptr_byte_add;
use super::ptr_byte_sub;

/// In debug mode we use a signature to ensure that raw pointers are pointing to the correct
/// shape of arena object.
#[cfg(debug_assertions)]
const SIGNATURE: usize = 0x1122334455667788;

pub struct ArenaSharedReservation<T>(NonNull<ArenaRcData<T>>);

impl<T> Drop for ArenaSharedReservation<T> {
  fn drop(&mut self) {
    panic!("A reservation must be completed or forgotten")
  }
}

/// Represents an atomic reference-counted pointer into an arena-allocated object.
pub struct ArenaRc<T> {
  ptr: NonNull<ArenaRcData<T>>,
}

static_assertions::assert_not_impl_any!(ArenaRc<()>: Send, Sync);

impl<T> ArenaRc<T> {
  /// Offset of the `ptr` field within the `ArenaRc` struct.
  const PTR_OFFSET: usize = offset_of!(ArenaRc<T>, ptr);

  /// Converts a raw pointer to the data into a `NonNull` pointer to `ArenaRcData`.
  ///
  /// # Safety
  ///
  /// This function assumes that the input `ptr` points to the data within an `ArenaRc` object.
  /// Improper usage may result in undefined behavior.
  #[inline(always)]
  unsafe fn data_from_ptr(ptr: NonNull<T>) -> NonNull<ArenaRcData<T>> {
    unsafe { ptr_byte_sub(ptr, Self::PTR_OFFSET) }
  }

  /// Converts a `NonNull` pointer to `ArenaRcData` into a raw pointer to the data.
  ///
  /// # Safety
  ///
  /// This function assumes that the input `ptr` is a valid `NonNull` pointer to `ArenaRcData`.
  /// Improper usage may result in undefined behavior.
  #[inline(always)]
  unsafe fn ptr_from_data(ptr: NonNull<ArenaRcData<T>>) -> NonNull<T> {
    unsafe { ptr_byte_add(ptr, Self::PTR_OFFSET) }
  }

  /// Consumes the `ArenaRc`, forgetting it, and returns a raw pointer to the contained data.
  ///
  /// # Safety
  ///
  /// This function returns a raw pointer without managing the memory, potentially leading to
  /// memory leaks if the pointer is not properly handled or deallocated.
  #[inline(always)]
  pub fn into_raw(arc: ArenaRc<T>) -> NonNull<T> {
    let ptr = arc.ptr;
    std::mem::forget(arc);
    unsafe { Self::ptr_from_data(ptr) }
  }

  /// Clones the `ArenaRc` reference, increments its reference count, and returns a raw pointer to the contained data.
  ///
  /// This function increments the reference count of the `ArenaRc`.
  #[inline(always)]
  pub fn clone_into_raw(arc: &ArenaRc<T>) -> NonNull<T> {
    unsafe {
      let ptr = arc.ptr;
      ptr.as_ref().ref_count.set(ptr.as_ref().ref_count.get() + 1);
      Self::ptr_from_data(arc.ptr)
    }
  }

  /// Constructs an `ArenaRc` from a raw pointer to the contained data.
  ///
  /// This function safely constructs an `ArenaRc` from a raw pointer, assuming the pointer is
  /// valid, properly aligned, and was originally created by `into_raw` or `clone_into_raw`.
  ///
  /// # Safety
  ///
  /// This function assumes the provided `ptr` is a valid raw pointer to the data within an `ArenaRc`
  /// object. Misuse may lead to undefined behavior, memory unsafety, or data corruption.
  #[inline(always)]
  pub unsafe fn from_raw(ptr: NonNull<T>) -> ArenaRc<T> {
    unsafe {
      let ptr = Self::data_from_ptr(ptr);

      #[cfg(debug_assertions)]
      debug_assert_eq!(ptr.as_ref().signature, SIGNATURE);
      ArenaRc { ptr }
    }
  }

  /// Clones an `ArenaRc` reference from a raw pointer and increments its reference count.
  ///
  /// This method increments the reference count of the `ArenaRc` instance
  /// associated with the provided raw pointer, allowing multiple references
  /// to the same allocated data.
  ///
  /// # Safety
  ///
  /// This function assumes that the provided `ptr` is a valid raw pointer
  /// to the data within an `ArenaRc` object. Improper usage may lead
  /// to memory unsafety or data corruption.
  #[inline(always)]
  pub unsafe fn clone_from_raw(ptr: NonNull<T>) -> ArenaRc<T> {
    unsafe {
      let ptr = Self::data_from_ptr(ptr);
      ptr.as_ref().ref_count.set(ptr.as_ref().ref_count.get() + 1);
      ArenaRc { ptr }
    }
  }

  /// Increments the reference count associated with the raw pointer to an `ArenaRc`-managed data.
  ///
  /// This method manually increases the reference count of the `ArenaRc` instance
  /// associated with the provided raw pointer. It allows incrementing the reference count
  /// without constructing a full `ArenaRc` instance, ideal for scenarios where direct
  /// manipulation of raw pointers is required.
  ///
  /// # Safety
  ///
  /// This method bypasses some safety checks enforced by the `ArenaRc` type. Incorrect usage
  /// or mishandling of raw pointers might lead to memory unsafety or data corruption.
  /// Use with caution and ensure proper handling of associated data.
  #[inline(always)]
  pub unsafe fn clone_raw_from_raw(ptr: NonNull<T>) {
    unsafe {
      let ptr = Self::data_from_ptr(ptr);
      ptr.as_ref().ref_count.set(ptr.as_ref().ref_count.get() + 1);
    }
  }

  /// Drops the `ArenaRc` reference pointed to by the raw pointer.
  ///
  /// If the reference count drops to zero, the associated data is returned to the arena.
  ///
  /// # Safety
  ///
  /// This function assumes that the provided `ptr` is a valid raw pointer
  /// to the data within an `ArenaRc` object. Improper usage may lead
  /// to memory unsafety or data corruption.
  #[inline(always)]
  pub unsafe fn drop_from_raw(ptr: NonNull<T>) {
    unsafe {
      let ptr = Self::data_from_ptr(ptr);
      let ref_count = ptr.as_ref().ref_count.get();
      if ref_count == 0 {
        let this = ptr.as_ref();
        ArenaShared::delete(this.arena_data, ptr);
      } else {
        ptr.as_ref().ref_count.set(ref_count - 1);
      }
    }
  }
}

impl<T> ArenaRc<T> {}

impl<T> Drop for ArenaRc<T> {
  fn drop(&mut self) {
    unsafe {
      let ref_count = self.ptr.as_ref().ref_count.get();
      if ref_count == 0 {
        let this = self.ptr.as_ref();
        ArenaShared::delete(this.arena_data, self.ptr);
      } else {
        self.ptr.as_ref().ref_count.set(ref_count - 1);
      }
    }
  }
}

impl<T> std::ops::Deref for ArenaRc<T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    unsafe { &self.ptr.as_ref().data }
  }
}

impl<T> std::convert::AsRef<T> for ArenaRc<T> {
  fn as_ref(&self) -> &T {
    unsafe { &self.ptr.as_ref().data }
  }
}

/// Data structure containing metadata and the actual data within the `ArenaRc`.
struct ArenaRcData<T> {
  #[cfg(debug_assertions)]
  signature: usize,
  ref_count: Cell<usize>,
  arena_data: NonNull<ArenaSharedData<T>>,
  data: T,
}

/// An atomic reference-counted pointer into an arena-allocated object
/// with thread-safe allocation and deallocation capabilities.
///
/// This structure ensures atomic access and safe sharing of allocated
/// data across multiple threads while maintaining reference counting
/// to manage the memory deallocation when no longer needed.
///
/// It combines a thread-safe `RawArena` for allocation and deallocation
/// and provides a mutex to guarantee exclusive access to the internal
/// data for safe multi-threaded operation.
///
/// The `ArenaShared` allows multiple threads to allocate, share,
/// and deallocate objects within the arena, ensuring safety and atomicity
/// during these operations.
pub struct ArenaShared<T> {
  ptr: NonNull<ArenaSharedData<T>>,
}

static_assertions::assert_not_impl_any!(ArenaShared<()>: Send, Sync);

/// Data structure containing a mutex and the `RawArena` for atomic access in the `ArenaShared`.
struct ArenaSharedData<T> {
  raw_arena: RawArena<ArenaRcData<T>>,
  ref_count: usize,
}

impl<T> ArenaShared<T> {
  /// Returns the constant overhead per allocation to assist with making allocations
  /// page-aligned.
  pub const fn overhead() -> usize {
    Self::allocation_size() - std::mem::size_of::<T>()
  }

  /// Returns the size of each allocation.
  pub const fn allocation_size() -> usize {
    RawArena::<ArenaRcData<T>>::allocation_size()
  }

  pub fn with_capacity(capacity: usize) -> Self {
    unsafe {
      let ptr = alloc();
      std::ptr::write(
        ptr.as_ptr(),
        ArenaSharedData {
          raw_arena: RawArena::with_capacity(capacity),
          ref_count: 0,
        },
      );
      Self { ptr }
    }
  }

  #[cold]
  #[inline(never)]
  unsafe fn drop_data(data: NonNull<ArenaSharedData<T>>) {
    unsafe {
      let data = data.as_ptr();
      std::ptr::drop_in_place(data);
      std::alloc::dealloc(data as _, Layout::new::<ArenaSharedData<T>>());
    }
  }

  #[inline(always)]
  unsafe fn delete(
    mut arena_ptr: NonNull<ArenaSharedData<T>>,
    data: NonNull<ArenaRcData<T>>,
  ) {
    unsafe {
      let arena = arena_ptr.as_mut();
      arena.raw_arena.recycle(data as _);
      if arena.ref_count == 0 {
        Self::drop_data(arena_ptr);
      } else {
        arena.ref_count -= 1;
      }
    }
  }

  /// Allocates a new object in the arena and returns an `ArenaRc` pointing to it.
  ///
  /// This method creates a new instance of type `T` within the `RawArena`. The provided `data`
  /// is initialized within the arena, and an `ArenaRc` is returned to manage this allocated data.
  /// The `ArenaRc` serves as an atomic, reference-counted pointer to the allocated data within
  /// the arena, ensuring safe concurrent access across multiple threads while maintaining the
  /// reference count for memory management.
  ///
  /// The allocation process employs a mutex to ensure thread-safe access to the arena, allowing
  /// only one thread at a time to modify the internal state, including allocating and deallocating memory.
  ///
  /// # Safety
  ///
  /// The provided `data` is allocated within the arena and managed by the `ArenaRc`. Improper handling
  /// or misuse of the returned `ArenaRc` pointer may lead to memory leaks or memory unsafety.
  ///
  /// # Example
  ///
  /// ```rust
  /// # use deno_core::arena::ArenaShared;
  ///
  /// // Define a struct that will be allocated within the arena
  /// struct MyStruct {
  ///     data: usize,
  /// }
  ///
  /// // Create a new instance of ArenaShared with a specified base capacity
  /// let arena: ArenaShared<MyStruct> = ArenaShared::with_capacity(16);
  ///
  /// // Allocate a new MyStruct instance within the arena
  /// let data_instance = MyStruct { data: 42 };
  /// let allocated_arc = arena.allocate(data_instance);
  ///
  /// // Now, allocated_arc can be used as a managed reference to the allocated data
  /// assert_eq!(allocated_arc.data, 42); // Validate the data stored in the allocated arc
  /// ```
  pub fn allocate(&self, data: T) -> ArenaRc<T> {
    let ptr = unsafe {
      let this = self.ptr.as_ptr();
      let ptr = (*this).raw_arena.allocate();
      (*this).ref_count += 1;

      std::ptr::write(
        ptr.as_ptr(),
        ArenaRcData {
          #[cfg(debug_assertions)]
          signature: SIGNATURE,
          arena_data: self.ptr,
          ref_count: Default::default(),
          data,
        },
      );
      ptr
    };

    ArenaRc { ptr }
  }

  /// Allocates a new object in the arena and returns an `ArenaRc` pointing to it. If no space
  /// is available, returns the original object.
  ///
  /// This method creates a new instance of type `T` within the `RawArena`. The provided `data`
  /// is initialized within the arena, and an `ArenaRc` is returned to manage this allocated data.
  /// The `ArenaRc` serves as an atomic, reference-counted pointer to the allocated data within
  /// the arena, ensuring safe concurrent access across multiple threads while maintaining the
  /// reference count for memory management.
  ///
  /// The allocation process employs a mutex to ensure thread-safe access to the arena, allowing
  /// only one thread at a time to modify the internal state, including allocating and deallocating memory.
  pub fn allocate_if_space(&self, data: T) -> Result<ArenaRc<T>, T> {
    let ptr = unsafe {
      let this = &mut *self.ptr.as_ptr();
      let Some(ptr) = this.raw_arena.allocate_if_space() else {
        return Err(data);
      };
      this.ref_count += 1;

      std::ptr::write(
        ptr.as_ptr(),
        ArenaRcData {
          #[cfg(debug_assertions)]
          signature: SIGNATURE,
          arena_data: self.ptr,
          ref_count: Default::default(),
          data,
        },
      );
      ptr
    };

    Ok(ArenaRc { ptr })
  }

  /// Attempt to reserve space in this arena.
  ///
  /// # Safety
  ///
  /// Reservations must be either completed or forgotten, and must be provided to the same
  /// arena that created them.
  #[inline(always)]
  pub unsafe fn reserve_space(&self) -> Option<ArenaSharedReservation<T>> {
    unsafe {
      let this = &mut *self.ptr.as_ptr();
      let ptr = this.raw_arena.allocate_if_space()?;
      this.ref_count += 1;
      Some(ArenaSharedReservation(ptr))
    }
  }

  /// Forget a reservation.
  ///
  /// # Safety
  ///
  /// Reservations must be either completed or forgotten, and must be provided to the same
  /// arena that created them.
  pub unsafe fn forget_reservation(
    &self,
    reservation: ArenaSharedReservation<T>,
  ) {
    unsafe {
      let ptr = reservation.0;
      std::mem::forget(reservation);
      let this = self.ptr.as_ptr();
      (*this).ref_count -= 1;
      (*this).raw_arena.recycle_without_drop(ptr);
    }
  }

  /// Complete a reservation.
  ///
  /// # Safety
  ///
  /// Reservations must be either completed or forgotten, and must be provided to the same
  /// arena that created them.
  #[inline(always)]
  pub unsafe fn complete_reservation(
    &self,
    reservation: ArenaSharedReservation<T>,
    data: T,
  ) -> ArenaRc<T> {
    unsafe {
      let ptr = reservation.0;
      std::mem::forget(reservation);
      let ptr = {
        std::ptr::write(
          ptr.as_ptr(),
          ArenaRcData {
            #[cfg(debug_assertions)]
            signature: SIGNATURE,
            arena_data: self.ptr,
            ref_count: Default::default(),
            data,
          },
        );
        ptr
      };

      ArenaRc { ptr }
    }
  }
}

impl<T> Drop for ArenaShared<T> {
  fn drop(&mut self) {
    unsafe {
      let this = self.ptr.as_mut();
      if this.ref_count == 0 {
        Self::drop_data(self.ptr);
      } else {
        this.ref_count -= 1;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::cell::RefCell;

  #[test]
  fn test_raw() {
    let arena: ArenaShared<RefCell<usize>> = ArenaShared::with_capacity(16);
    let arc = arena.allocate(Default::default());
    let raw = ArenaRc::into_raw(arc);
    _ = unsafe { ArenaRc::from_raw(raw) };
  }

  #[test]
  fn test_clone_into_raw() {
    let arena: ArenaShared<RefCell<usize>> = ArenaShared::with_capacity(16);
    let arc = arena.allocate(Default::default());
    let raw = ArenaRc::clone_into_raw(&arc);
    _ = unsafe { ArenaRc::from_raw(raw) };
  }

  #[test]
  fn test_allocate_drop_arc_first() {
    let arena: ArenaShared<RefCell<usize>> = ArenaShared::with_capacity(16);
    let arc = arena.allocate(Default::default());
    *arc.borrow_mut() += 1;
    drop(arc);
    drop(arena);
  }

  #[test]
  fn test_allocate_drop_arena_first() {
    let arena: ArenaShared<RefCell<usize>> = ArenaShared::with_capacity(16);
    let arc = arena.allocate(Default::default());
    *arc.borrow_mut() += 1;
    drop(arena);
    drop(arc);
  }
}
