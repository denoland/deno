// Copyright 2018-2025 the Deno authors. MIT license.

use std::alloc::Layout;
use std::future::Future;
use std::mem::offset_of;
use std::pin::Pin;
use std::ptr::NonNull;

use crate::arena::raw_arena::RawArena;

use super::alloc;
use super::ptr_byte_add;
use super::ptr_byte_sub;

/// In debug mode we use a signature to ensure that raw pointers are pointing to the correct
/// shape of arena object.
#[cfg(debug_assertions)]
const SIGNATURE: usize = 0x8877665544332211;

pub struct ArenaUniqueReservation<T>(NonNull<ArenaBoxData<T>>);

impl<T> Drop for ArenaUniqueReservation<T> {
  fn drop(&mut self) {
    panic!("A reservation must be completed or forgotten")
  }
}

pub struct ArenaBox<T: 'static> {
  ptr: NonNull<ArenaBoxData<T>>,
}

impl<T> Unpin for ArenaBox<T> {}

struct ArenaBoxData<T> {
  #[cfg(debug_assertions)]
  signature: usize,
  arena_data: NonNull<ArenaUniqueData<T>>,
  data: T,
}

impl<T: 'static> ArenaBox<T> {
  /// Offset of the `ptr` field within the `ArenaBox` struct.
  const PTR_OFFSET: usize = offset_of!(ArenaBox<T>, ptr);

  /// Constructs a `NonNull` reference to `ArenaBoxData` from a raw pointer to `T`.
  #[inline(always)]
  unsafe fn data_from_ptr(ptr: NonNull<T>) -> NonNull<ArenaBoxData<T>> {
    unsafe { ptr_byte_sub(ptr, Self::PTR_OFFSET) }
  }

  /// Obtains a raw pointer to `T` from a `NonNull` reference to `ArenaBoxData`.  
  #[inline(always)]
  unsafe fn ptr_from_data(ptr: NonNull<ArenaBoxData<T>>) -> NonNull<T> {
    unsafe { ptr_byte_add(ptr, Self::PTR_OFFSET) }
  }

  /// Transforms an `ArenaBox` into a raw pointer to `T` and forgets it.
  ///
  /// # Safety
  ///
  /// This function returns a raw pointer without managing the memory, potentially leading to
  /// memory leaks if the pointer is not properly handled or deallocated.
  #[inline(always)]
  pub fn into_raw(alloc: ArenaBox<T>) -> NonNull<T> {
    let ptr: NonNull<ArenaBoxData<T>> = alloc.ptr;
    std::mem::forget(alloc);
    unsafe { Self::ptr_from_data(ptr) }
  }

  /// Constructs an `ArenaBox` from a raw pointer to the contained data.
  ///
  /// # Safety
  ///
  /// This function safely constructs an `ArenaBox` from a raw pointer, assuming the pointer is
  /// valid and properly aligned. Misuse may lead to undefined behavior, memory unsafety, or data corruption.
  #[inline(always)]
  pub unsafe fn from_raw(ptr: NonNull<T>) -> ArenaBox<T> {
    unsafe {
      let ptr = Self::data_from_ptr(ptr);

      #[cfg(debug_assertions)]
      debug_assert_eq!(ptr.as_ref().signature, SIGNATURE);
      ArenaBox { ptr }
    }
  }
}

// This Box cannot be sent between threads
static_assertions::assert_not_impl_any!(ArenaBox<()>: Send, Sync);

impl<T> ArenaBox<T> {
  #[inline(always)]
  fn data(&self) -> &ArenaBoxData<T> {
    unsafe { self.ptr.as_ref() }
  }

  #[inline(always)]
  pub(crate) fn deref_data(&self) -> NonNull<T> {
    unsafe {
      NonNull::new_unchecked(std::ptr::addr_of_mut!((*self.ptr.as_ptr()).data))
    }
  }
}

impl<T> Drop for ArenaBox<T> {
  #[inline(always)]
  fn drop(&mut self) {
    unsafe {
      ArenaUnique::delete(self.ptr);
    }
  }
}

impl<T> std::ops::Deref for ArenaBox<T> {
  type Target = T;
  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.data().data
  }
}

impl<T> std::convert::AsRef<T> for ArenaBox<T> {
  #[inline(always)]
  fn as_ref(&self) -> &T {
    &self.data().data
  }
}

impl<F, R> std::future::Future for ArenaBox<F>
where
  F: Future<Output = R>,
{
  type Output = R;

  #[inline(always)]
  fn poll(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    unsafe { F::poll(Pin::new_unchecked(self.deref_data().as_mut()), cx) }
  }
}

/// An arena-based unique ownership container allowing allocation
/// and deallocation of objects with exclusive ownership semantics.
///
/// `ArenaUnique` provides exclusive ownership semantics similar to
/// a `Box`. It utilizes a `RawArena` for allocation and
/// deallocation of objects, maintaining the sole ownership of the
/// allocated data and enabling safe cleanup when the `ArenaUnique`
/// instance is dropped.
///
/// This container guarantees exclusive access to the allocated data
/// within the arena, allowing single-threaded operations while
/// efficiently managing memory and ensuring cleanup on drop.
pub struct ArenaUnique<T> {
  ptr: NonNull<ArenaUniqueData<T>>,
}

// The arena itself may not be shared so that we can guarantee all [`RawArena`]
// access happens on the owning thread.
static_assertions::assert_not_impl_any!(ArenaUnique<()>: Send, Sync);

struct ArenaUniqueData<T> {
  raw_arena: RawArena<ArenaBoxData<T>>,
  alive: bool,
}

impl<T> ArenaUnique<T> {
  /// Returns the constant overhead per allocation to assist with making allocations
  /// page-aligned.
  pub const fn overhead() -> usize {
    Self::allocation_size() - std::mem::size_of::<T>()
  }

  /// Returns the size of each allocation.
  pub const fn allocation_size() -> usize {
    RawArena::<ArenaBoxData<T>>::allocation_size()
  }

  pub fn with_capacity(capacity: usize) -> Self {
    unsafe {
      let ptr = alloc();
      std::ptr::write(
        ptr.as_ptr(),
        ArenaUniqueData {
          raw_arena: RawArena::with_capacity(capacity),
          alive: true,
        },
      );
      Self { ptr }
    }
  }

  #[cold]
  #[inline(never)]
  unsafe fn drop_data(data: NonNull<ArenaUniqueData<T>>) {
    unsafe {
      let data = data.as_ptr();
      std::ptr::drop_in_place(data);
      std::alloc::dealloc(data as _, Layout::new::<ArenaUniqueData<T>>());
    }
  }

  /// Deletes the data associated with an `ArenaBox` from the arena. If this is the last
  /// allocation for the arena and the arena has been dropped, de-allocate everything.
  #[inline(always)]
  unsafe fn delete(data: NonNull<ArenaBoxData<T>>) {
    unsafe {
      let arena_data = data.as_ref().arena_data;
      let arena = arena_data.as_ref();
      if arena.raw_arena.recycle(data) && !arena.alive {
        Self::drop_data(arena_data)
      }
    }
  }

  /// Allocates a new data instance of type `T` within the arena, encapsulating it within an `ArenaBox`.
  ///
  /// This method creates a new instance of type `T` within the `RawArena`. The provided `data`
  /// is initialized within the arena, and an `ArenaBox` is returned to manage this allocated data.
  /// The `ArenaBox` serves as a reference to the allocated data within the arena, providing safe access
  /// and management of the stored value.
  ///
  /// # Safety
  ///
  /// The provided `data` is allocated within the arena and managed by the `ArenaBox`. Improper handling
  /// or misuse of the returned `ArenaBox` pointer may lead to memory leaks or memory unsafety.
  ///
  /// # Example
  ///
  /// ```rust
  /// # use deno_core::arena::ArenaUnique;
  ///
  /// // Define a struct that will be allocated within the arena
  /// struct MyStruct {
  ///   data: usize,
  /// }
  ///
  /// // Create a new instance of ArenaUnique with a specified base capacity
  /// let arena: ArenaUnique<MyStruct> = ArenaUnique::with_capacity(16);
  ///
  /// // Allocate a new MyStruct instance within the arena
  /// let data_instance = MyStruct { data: 42 };
  /// let allocated_box = arena.allocate(data_instance);
  ///
  /// // Now, allocated_box can be used as a managed reference to the allocated data
  /// assert_eq!(allocated_box.data, 42); // Validate the data stored in the allocated box
  /// ```
  pub fn allocate(&self, data: T) -> ArenaBox<T> {
    let ptr = unsafe {
      let this = self.ptr.as_ptr();
      let ptr = (*this).raw_arena.allocate();
      std::ptr::write(
        ptr.as_ptr(),
        ArenaBoxData {
          #[cfg(debug_assertions)]
          signature: SIGNATURE,
          arena_data: self.ptr,
          data,
        },
      );
      ptr
    };

    ArenaBox { ptr }
  }

  /// Attempt to reserve space in this arena.
  ///
  /// # Safety
  ///
  /// Reservations must be either completed or forgotten, and must be provided to the same
  /// arena that created them.
  #[inline(always)]
  pub unsafe fn reserve_space(&self) -> Option<ArenaUniqueReservation<T>> {
    unsafe {
      let this = &mut *self.ptr.as_ptr();
      let ptr = this.raw_arena.allocate_if_space()?;
      Some(ArenaUniqueReservation(ptr))
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
    reservation: ArenaUniqueReservation<T>,
  ) {
    unsafe {
      let ptr = reservation.0;
      std::mem::forget(reservation);
      let this = self.ptr.as_ptr();
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
    reservation: ArenaUniqueReservation<T>,
    data: T,
  ) -> ArenaBox<T> {
    unsafe {
      let ptr = reservation.0;
      std::mem::forget(reservation);
      let ptr = {
        std::ptr::write(
          ptr.as_ptr(),
          ArenaBoxData {
            #[cfg(debug_assertions)]
            signature: SIGNATURE,
            arena_data: self.ptr,
            data,
          },
        );
        ptr
      };

      ArenaBox { ptr }
    }
  }
}

impl<T> Drop for ArenaUnique<T> {
  fn drop(&mut self) {
    unsafe {
      let this = self.ptr.as_mut();
      if this.raw_arena.allocated() == 0 {
        Self::drop_data(self.ptr);
      } else {
        this.alive = false;
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
    let arena: ArenaUnique<RefCell<usize>> = ArenaUnique::with_capacity(16);
    let arc = arena.allocate(Default::default());
    let raw = ArenaBox::into_raw(arc);
    _ = unsafe { ArenaBox::from_raw(raw) };
  }

  #[test]
  fn test_allocate_drop_box_first() {
    let arena: ArenaUnique<RefCell<usize>> = ArenaUnique::with_capacity(16);
    let alloc = arena.allocate(Default::default());
    *alloc.borrow_mut() += 1;
    drop(alloc);
    drop(arena);
  }

  #[test]
  fn test_allocate_drop_arena_first() {
    let arena: ArenaUnique<RefCell<usize>> = ArenaUnique::with_capacity(16);
    let alloc = arena.allocate(Default::default());
    *alloc.borrow_mut() += 1;
    drop(arena);
    drop(alloc);
  }
}
