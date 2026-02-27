// Copyright 2018-2025 the Deno authors. MIT license.

use std::alloc::Layout;
use std::mem::offset_of;
use std::ptr::NonNull;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use parking_lot::lock_api::RawMutex;

use crate::arena::raw_arena::RawArena;

use super::alloc;
use super::ptr_byte_add;
use super::ptr_byte_sub;

/// In debug mode we use a signature to ensure that raw pointers are pointing to the correct
/// shape of arena object.
#[cfg(debug_assertions)]
const SIGNATURE: usize = 0x1133224455667788;

pub struct ArenaSharedAtomicReservation<T>(NonNull<ArenaArcData<T>>);

impl<T> Drop for ArenaSharedAtomicReservation<T> {
  fn drop(&mut self) {
    panic!("A reservation must be completed or forgotten")
  }
}

/// Represents an atomic reference-counted pointer into an arena-allocated object.
pub struct ArenaArc<T> {
  ptr: NonNull<ArenaArcData<T>>,
}

impl<T> ArenaArc<T> {
  /// Offset of the `ptr` field within the `ArenaArc` struct.
  const PTR_OFFSET: usize = offset_of!(ArenaArc<T>, ptr);

  /// Converts a raw pointer to the data into a `NonNull` pointer to `ArenaArcData`.
  ///
  /// # Safety
  ///
  /// This function assumes that the input `ptr` points to the data within an `ArenaArc` object.
  /// Improper usage may result in undefined behavior.
  #[inline(always)]
  unsafe fn data_from_ptr(ptr: NonNull<T>) -> NonNull<ArenaArcData<T>> {
    unsafe { ptr_byte_sub(ptr, Self::PTR_OFFSET) }
  }

  /// Converts a `NonNull` pointer to `ArenaArcData` into a raw pointer to the data.
  ///
  /// # Safety
  ///
  /// This function assumes that the input `ptr` is a valid `NonNull` pointer to `ArenaArcData`.
  /// Improper usage may result in undefined behavior.
  #[inline(always)]
  unsafe fn ptr_from_data(ptr: NonNull<ArenaArcData<T>>) -> NonNull<T> {
    unsafe { ptr_byte_add(ptr, Self::PTR_OFFSET) }
  }

  /// Consumes the `ArenaArc`, forgetting it, and returns a raw pointer to the contained data.
  ///
  /// # Safety
  ///
  /// This function returns a raw pointer without managing the memory, potentially leading to
  /// memory leaks if the pointer is not properly handled or deallocated.
  #[inline(always)]
  pub fn into_raw(arc: ArenaArc<T>) -> NonNull<T> {
    let ptr = arc.ptr;
    std::mem::forget(arc);
    unsafe { Self::ptr_from_data(ptr) }
  }

  /// Clones the `ArenaArc` reference, increments its reference count, and returns a raw pointer to the contained data.
  ///
  /// This function increments the reference count of the `ArenaArc`.
  #[inline(always)]
  pub fn clone_into_raw(arc: &ArenaArc<T>) -> NonNull<T> {
    unsafe {
      arc.ptr.as_ref().ref_count.fetch_add(1, Ordering::Relaxed);
      Self::ptr_from_data(arc.ptr)
    }
  }

  /// Constructs an `ArenaArc` from a raw pointer to the contained data.
  ///
  /// This function safely constructs an `ArenaArc` from a raw pointer, assuming the pointer is
  /// valid, properly aligned, and was originally created by `into_raw` or `clone_into_raw`.
  ///
  /// # Safety
  ///
  /// This function assumes the provided `ptr` is a valid raw pointer to the data within an `ArenaArc`
  /// object. Misuse may lead to undefined behavior, memory unsafety, or data corruption.
  #[inline(always)]
  pub unsafe fn from_raw(ptr: NonNull<T>) -> ArenaArc<T> {
    unsafe {
      let ptr = Self::data_from_ptr(ptr);

      #[cfg(debug_assertions)]
      debug_assert_eq!(ptr.as_ref().signature, SIGNATURE);
      ArenaArc { ptr }
    }
  }

  /// Clones an `ArenaArc` reference from a raw pointer and increments its reference count.
  ///
  /// This method increments the reference count of the `ArenaArc` instance
  /// associated with the provided raw pointer, allowing multiple references
  /// to the same allocated data.
  ///
  /// # Safety
  ///
  /// This function assumes that the provided `ptr` is a valid raw pointer
  /// to the data within an `ArenaArc` object. Improper usage may lead
  /// to memory unsafety or data corruption.
  #[inline(always)]
  pub unsafe fn clone_from_raw(ptr: NonNull<T>) -> ArenaArc<T> {
    unsafe {
      let ptr = Self::data_from_ptr(ptr);
      let this = ptr.as_ref();
      this.ref_count.fetch_add(1, Ordering::Relaxed);
      ArenaArc { ptr }
    }
  }

  /// Increments the reference count associated with the raw pointer to an `ArenaArc`-managed data.
  ///
  /// This method manually increases the reference count of the `ArenaArc` instance
  /// associated with the provided raw pointer. It allows incrementing the reference count
  /// without constructing a full `ArenaArc` instance, ideal for scenarios where direct
  /// manipulation of raw pointers is required.
  ///
  /// # Safety
  ///
  /// This method bypasses some safety checks enforced by the `ArenaArc` type. Incorrect usage
  /// or mishandling of raw pointers might lead to memory unsafety or data corruption.
  /// Use with caution and ensure proper handling of associated data.
  #[inline(always)]
  pub unsafe fn clone_raw_from_raw(ptr: NonNull<T>) {
    unsafe {
      let this = Self::data_from_ptr(ptr).as_ref();
      this.ref_count.fetch_add(1, Ordering::Relaxed);
    }
  }

  /// Drops the `ArenaArc` reference pointed to by the raw pointer.
  ///
  /// If the reference count drops to zero, the associated data is returned to the arena.
  ///
  /// # Safety
  ///
  /// This function assumes that the provided `ptr` is a valid raw pointer
  /// to the data within an `ArenaArc` object. Improper usage may lead
  /// to memory unsafety or data corruption.
  #[inline(always)]
  pub unsafe fn drop_from_raw(ptr: NonNull<T>) {
    unsafe {
      let ptr = Self::data_from_ptr(ptr);
      let this = ptr.as_ref();
      if this.ref_count.fetch_sub(1, Ordering::Relaxed) == 0 {
        ArenaSharedAtomic::delete(ptr);
      }
    }
  }
}

unsafe impl<T: Send + Sync> Send for ArenaArc<T> {}
unsafe impl<T: Send + Sync> Sync for ArenaArc<T> {}

// T is Send + Sync, so ArenaArc is too
static_assertions::assert_impl_all!(ArenaArc<()>: Send, Sync);
// T is !Send & !Sync, so ArenaArc is too
static_assertions::assert_not_impl_any!(ArenaArc<*mut ()>: Send, Sync);

impl<T> ArenaArc<T> {}

impl<T> Drop for ArenaArc<T> {
  fn drop(&mut self) {
    unsafe {
      let this = self.ptr.as_ref();
      if this.ref_count.fetch_sub(1, Ordering::Relaxed) == 0 {
        ArenaSharedAtomic::delete(self.ptr);
      }
    }
  }
}

impl<T> std::ops::Deref for ArenaArc<T> {
  type Target = T;
  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    unsafe { &self.ptr.as_ref().data }
  }
}

impl<T> std::convert::AsRef<T> for ArenaArc<T> {
  #[inline(always)]
  fn as_ref(&self) -> &T {
    unsafe { &self.ptr.as_ref().data }
  }
}

/// Data structure containing metadata and the actual data within the `ArenaArc`.
struct ArenaArcData<T> {
  #[cfg(debug_assertions)]
  signature: usize,
  ref_count: AtomicUsize,
  arena_data: NonNull<ArenaSharedAtomicData<T>>,
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
/// The `ArenaSharedAtomic` allows multiple threads to allocate, share,
/// and deallocate objects within the arena, ensuring safety and atomicity
/// during these operations.
pub struct ArenaSharedAtomic<T> {
  ptr: NonNull<ArenaSharedAtomicData<T>>,
}

unsafe impl<T> Send for ArenaSharedAtomic<T> {}

// The arena itself may not be shared so that we can guarantee all [`RawArena`]
// access happens on the owning thread.
static_assertions::assert_impl_any!(ArenaSharedAtomic<()>: Send);
static_assertions::assert_not_impl_any!(ArenaSharedAtomic<()>: Sync);

/// Data structure containing a mutex and the `RawArena` for atomic access in the `ArenaSharedAtomic`.
struct ArenaSharedAtomicData<T> {
  /// A mutex ensuring thread-safe access to the internal raw arena and refcount.
  mutex: parking_lot::RawMutex,
  protected: ArenaSharedAtomicDataProtected<T>,
}

struct ArenaSharedAtomicDataProtected<T> {
  raw_arena: RawArena<ArenaArcData<T>>,
  ref_count: usize,
}

impl<T> ArenaSharedAtomic<T> {
  /// Returns the constant overhead per allocation to assist with making allocations
  /// page-aligned.
  pub const fn overhead() -> usize {
    Self::allocation_size() - std::mem::size_of::<T>()
  }

  /// Returns the size of each allocation.
  pub const fn allocation_size() -> usize {
    RawArena::<ArenaArcData<T>>::allocation_size()
  }

  pub fn with_capacity(capacity: usize) -> Self {
    unsafe {
      let ptr = alloc();
      std::ptr::write(
        ptr.as_ptr(),
        ArenaSharedAtomicData {
          mutex: parking_lot::RawMutex::INIT,
          protected: ArenaSharedAtomicDataProtected {
            raw_arena: RawArena::with_capacity(capacity),
            ref_count: 0,
          },
        },
      );
      Self { ptr }
    }
  }

  #[inline(always)]
  unsafe fn lock<'s>(
    arena: NonNull<ArenaSharedAtomicData<T>>,
  ) -> &'s mut ArenaSharedAtomicDataProtected<T> {
    unsafe {
      let mutex = &*std::ptr::addr_of!((*arena.as_ptr()).mutex);
      while !mutex.try_lock() {
        std::thread::yield_now();
      }
      &mut *std::ptr::addr_of_mut!((*arena.as_ptr()).protected)
    }
  }

  #[inline(always)]
  unsafe fn unlock(arena: NonNull<ArenaSharedAtomicData<T>>) {
    unsafe {
      let mutex = &*std::ptr::addr_of!((*arena.as_ptr()).mutex);
      mutex.unlock()
    }
  }

  #[cold]
  #[inline(never)]
  unsafe fn drop_data(arena: NonNull<ArenaSharedAtomicData<T>>) {
    unsafe {
      let arena = arena.as_ptr();
      std::ptr::drop_in_place(arena);
      std::alloc::dealloc(
        arena as _,
        Layout::new::<ArenaSharedAtomicData<T>>(),
      );
    }
  }

  #[inline(always)]
  unsafe fn delete(data: NonNull<ArenaArcData<T>>) {
    unsafe {
      let ptr = (*data.as_ptr()).arena_data;
      // We cannot materialize a reference to arena_data until we have the lock
      let this = Self::lock(ptr);
      this.raw_arena.recycle(data);
      if this.ref_count == 0 {
        Self::drop_data(ptr);
      } else {
        this.ref_count -= 1;
        Self::unlock(ptr);
      }
    }
  }

  /// Allocates a new object in the arena and returns an `ArenaArc` pointing to it.
  ///
  /// This method creates a new instance of type `T` within the `RawArena`. The provided `data`
  /// is initialized within the arena, and an `ArenaArc` is returned to manage this allocated data.
  /// The `ArenaArc` serves as an atomic, reference-counted pointer to the allocated data within
  /// the arena, ensuring safe concurrent access across multiple threads while maintaining the
  /// reference count for memory management.
  ///
  /// The allocation process employs a mutex to ensure thread-safe access to the arena, allowing
  /// only one thread at a time to modify the internal state, including allocating and deallocating memory.
  ///
  /// # Safety
  ///
  /// The provided `data` is allocated within the arena and managed by the `ArenaArc`. Improper handling
  /// or misuse of the returned `ArenaArc` pointer may lead to memory leaks or memory unsafety.
  ///
  /// # Example
  ///
  /// ```rust
  /// # use deno_core::arena::ArenaSharedAtomic;
  ///
  /// // Define a struct that will be allocated within the arena
  /// struct MyStruct {
  ///     data: usize,
  /// }
  ///
  /// // Create a new instance of ArenaSharedAtomic with a specified base capacity
  /// let arena: ArenaSharedAtomic<MyStruct> = ArenaSharedAtomic::with_capacity(16);
  ///
  /// // Allocate a new MyStruct instance within the arena
  /// let data_instance = MyStruct { data: 42 };
  /// let allocated_arc = arena.allocate(data_instance);
  ///
  /// // Now, allocated_arc can be used as a managed reference to the allocated data
  /// assert_eq!(allocated_arc.data, 42); // Validate the data stored in the allocated arc
  /// ```
  pub fn allocate(&self, data: T) -> ArenaArc<T> {
    let ptr = unsafe {
      let this = Self::lock(self.ptr);
      let ptr = this.raw_arena.allocate();
      this.ref_count += 1;
      std::ptr::write(
        ptr.as_ptr(),
        ArenaArcData {
          #[cfg(debug_assertions)]
          signature: SIGNATURE,
          arena_data: self.ptr,
          ref_count: AtomicUsize::default(),
          data,
        },
      );
      Self::unlock(self.ptr);
      ptr
    };

    ArenaArc { ptr }
  }

  /// Allocates a new object in the arena and returns an `ArenaArc` pointing to it. If no space
  /// is available, returns the original object.
  ///
  /// This method creates a new instance of type `T` within the `RawArena`. The provided `data`
  /// is initialized within the arena, and an `ArenaArc` is returned to manage this allocated data.
  /// The `ArenaArc` serves as an atomic, reference-counted pointer to the allocated data within
  /// the arena, ensuring safe concurrent access across multiple threads while maintaining the
  /// reference count for memory management.
  ///
  /// The allocation process employs a mutex to ensure thread-safe access to the arena, allowing
  /// only one thread at a time to modify the internal state, including allocating and deallocating memory.
  pub fn allocate_if_space(&self, data: T) -> Result<ArenaArc<T>, T> {
    let ptr = unsafe {
      let this = Self::lock(self.ptr);
      let Some(ptr) = this.raw_arena.allocate_if_space() else {
        return Err(data);
      };
      this.ref_count += 1;
      Self::unlock(self.ptr);

      std::ptr::write(
        ptr.as_ptr(),
        ArenaArcData {
          #[cfg(debug_assertions)]
          signature: SIGNATURE,
          arena_data: self.ptr,
          ref_count: AtomicUsize::default(),
          data,
        },
      );
      ptr
    };

    Ok(ArenaArc { ptr })
  }

  /// Attempt to reserve space in this arena.
  ///
  /// # Safety
  ///
  /// Reservations must be either completed or forgotten, and must be provided to the same
  /// arena that created them.
  #[inline(always)]
  pub unsafe fn reserve_space(
    &self,
  ) -> Option<ArenaSharedAtomicReservation<T>> {
    unsafe {
      let this = Self::lock(self.ptr);
      let ptr = this.raw_arena.allocate_if_space()?;
      this.ref_count += 1;
      Self::unlock(self.ptr);
      Some(ArenaSharedAtomicReservation(ptr))
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
    reservation: ArenaSharedAtomicReservation<T>,
  ) {
    unsafe {
      let ptr = reservation.0;
      std::mem::forget(reservation);
      let this = Self::lock(self.ptr);
      this.ref_count -= 1;
      this.raw_arena.recycle_without_drop(ptr);
      Self::unlock(self.ptr);
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
    reservation: ArenaSharedAtomicReservation<T>,
    data: T,
  ) -> ArenaArc<T> {
    unsafe {
      let ptr = reservation.0;
      std::mem::forget(reservation);
      let ptr = {
        std::ptr::write(
          ptr.as_ptr(),
          ArenaArcData {
            #[cfg(debug_assertions)]
            signature: SIGNATURE,
            arena_data: self.ptr,
            ref_count: AtomicUsize::default(),
            data,
          },
        );
        ptr
      };
      ArenaArc { ptr }
    }
  }
}

impl<T> Drop for ArenaSharedAtomic<T> {
  fn drop(&mut self) {
    unsafe {
      let this = Self::lock(self.ptr);
      if this.ref_count == 0 {
        Self::drop_data(self.ptr);
      } else {
        this.ref_count -= 1;
        Self::unlock(self.ptr);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::cell::RefCell;
  use std::sync::Arc;
  use std::sync::Mutex;

  #[test]
  fn test_raw() {
    let arena: ArenaSharedAtomic<RefCell<usize>> =
      ArenaSharedAtomic::with_capacity(16);
    let arc = arena.allocate(Default::default());
    let raw = ArenaArc::into_raw(arc);
    _ = unsafe { ArenaArc::from_raw(raw) };
  }

  #[test]
  fn test_clone_into_raw() {
    let arena: ArenaSharedAtomic<RefCell<usize>> =
      ArenaSharedAtomic::with_capacity(16);
    let arc = arena.allocate(Default::default());
    let raw = ArenaArc::clone_into_raw(&arc);
    _ = unsafe { ArenaArc::from_raw(raw) };
  }

  #[test]
  fn test_allocate_drop_arc_first() {
    let arena: ArenaSharedAtomic<RefCell<usize>> =
      ArenaSharedAtomic::with_capacity(16);
    let arc = arena.allocate(Default::default());
    *arc.borrow_mut() += 1;
    drop(arc);
    drop(arena);
  }

  #[test]
  fn test_allocate_drop_arena_first() {
    let arena: ArenaSharedAtomic<RefCell<usize>> =
      ArenaSharedAtomic::with_capacity(16);
    let arc = arena.allocate(Default::default());
    *arc.borrow_mut() += 1;
    drop(arena);
    drop(arc);
  }

  #[test]
  fn test_threaded() {
    let arena: Arc<Mutex<ArenaSharedAtomic<RefCell<usize>>>> =
      Arc::new(Mutex::new(ArenaSharedAtomic::with_capacity(2000)));
    const THREADS: usize = 20;
    const ITERATIONS: usize = 100;

    let mut handles = Vec::new();
    let barrier = Arc::new(std::sync::Barrier::new(THREADS));

    for _ in 0..THREADS {
      let arena = Arc::clone(&arena);
      let barrier = Arc::clone(&barrier);

      let handle = std::thread::spawn(move || {
        barrier.wait();

        for _ in 0..ITERATIONS {
          let arc = arena.lock().unwrap().allocate(RefCell::new(0));
          *arc.borrow_mut() += 1;
          drop(arc); // Ensure the Arc is dropped at the end of the iteration
        }
      });

      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }
  }
}
