// Copyright 2018-2025 the Deno authors. MIT license.

use std::alloc::Layout;
use std::cell::Cell;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;

use bit_set::BitSet;
use bit_vec::BitVec;

use super::alloc;
use super::alloc_layout;

/// In debug mode we use a signature to ensure that raw pointers are pointing to the correct
/// shape of arena object.
#[cfg(debug_assertions)]
const SIGNATURE: usize = 0x1234567812345678;

/// A very-`unsafe`, arena for raw pointers that falls back to raw allocation when full. This
/// should be used with great care, and ideally you should only be using the higher-level arenas
/// built on top of this.
///
/// # Safety
///
/// Items placed into the RawArena are dropped, but there is no check to ensure that an allocated
/// item is valid before dropping it. Use `recycle_without_drop` to return an item to the arena
/// without dropping it.
///
/// # Example
///
/// ```rust
/// # use deno_core::arena::RawArena;
/// // Create a RawArena with a capacity of 10 elements
/// let arena = RawArena::<usize>::with_capacity(10);
///
/// // Allocate elements in the arena
/// unsafe {
///   let mut elements = Vec::new();
///   for i in 0..10 {
///     let mut element_ptr = arena.allocate();
///     *element_ptr.as_mut() = i * 2;
///     elements.push(element_ptr);
///   }
///
///   // Recycle elements back into the arena
///   for &element_ptr in elements.iter() {
///     arena.recycle(element_ptr);
///   }
/// }
/// ```
pub struct RawArena<T> {
  #[cfg(debug_assertions)]
  signature: usize,
  alloc: NonNull<RawArenaEntry<T>>,
  past_alloc_end: NonNull<RawArenaEntry<T>>,
  max: Cell<NonNull<RawArenaEntry<T>>>,
  next: Cell<NonNull<RawArenaEntry<T>>>,
  allocated: Cell<usize>,
  capacity: usize,
}

/// The [`RawArena`] is [`Send`], but not [`Sync`].
unsafe impl<T> Send for RawArena<T> {}

static_assertions::assert_impl_one!(RawArena<()>: Send);
static_assertions::assert_not_impl_any!(RawArena<()>: Sync);

union RawArenaEntry<T> {
  /// If this is a vacant entry, points to the next entry.
  next: NonNull<RawArenaEntry<T>>,
  /// If this is a valid entry, contains the raw data.
  value: ManuallyDrop<T>,
}

impl<T> RawArenaEntry<T> {
  #[inline(always)]
  unsafe fn next(
    entry: NonNull<RawArenaEntry<T>>,
  ) -> NonNull<RawArenaEntry<T>> {
    unsafe { (*(entry.as_ptr())).next }
  }

  #[inline(always)]
  unsafe fn drop(entry: NonNull<RawArenaEntry<T>>) {
    unsafe {
      std::ptr::drop_in_place(
        std::ptr::addr_of_mut!((*entry.as_ptr()).value) as *mut T
      );
    }
  }
}

impl<T> RawArena<T> {
  /// Returns the constant overhead per allocation to assist with making allocations
  /// page-aligned.
  pub const fn overhead() -> usize {
    Self::allocation_size() - std::mem::size_of::<T>()
  }

  /// Returns the size of each allocation.
  pub const fn allocation_size() -> usize {
    std::mem::size_of::<RawArenaEntry<T>>()
  }

  /// Allocate an arena, completely initialized. This memory is not zeroed, and
  /// we use the high-water mark to keep track of what we've initialized so far.
  ///
  /// This is safe, because dropping the [`RawArena`] without doing anything to
  /// it is safe.
  pub fn with_capacity(capacity: usize) -> Self {
    let alloc = alloc_layout(Self::layout(capacity));
    Self {
      #[cfg(debug_assertions)]
      signature: SIGNATURE,
      alloc,
      past_alloc_end: unsafe {
        NonNull::new_unchecked(alloc.as_ptr().add(capacity))
      },
      max: alloc.into(),
      next: Cell::new(alloc),
      allocated: Default::default(),
      capacity,
    }
  }

  // TODO(mmastrac): const when https://github.com/rust-lang/rust/issues/67521 is fixed
  fn layout(capacity: usize) -> Layout {
    match Layout::array::<RawArenaEntry<T>>(capacity) {
      Ok(l) => l,
      _ => panic!("Zero-sized objects are not supported"),
    }
  }

  /// Helper method to transmute internal pointers.
  ///
  /// # Safety
  ///
  /// For internal use.
  #[inline(always)]
  unsafe fn entry_to_data(entry: NonNull<RawArenaEntry<T>>) -> NonNull<T> {
    // Transmute the union
    entry.cast()
  }

  /// Helper method to transmute internal pointers.
  ///
  /// # Safety
  ///
  /// For internal use.
  #[inline(always)]
  unsafe fn data_to_entry(data: NonNull<T>) -> NonNull<RawArenaEntry<T>> {
    // Transmute the union
    data.cast()
  }

  /// Gets the next free entry, allocating if necessary. This is `O(1)` if we have free space in
  /// the arena, `O(?)` if we need to allocate from the allocator (where `?` is defined by the
  /// system allocator).
  ///
  /// # Safety
  ///
  /// As the memory area is considered uninitialized and you must be careful to fully and validly
  /// initialize the underlying data, this method is marked as unsafe.
  ///
  /// This pointer will be invalidated when we drop the `RawArena`, so the allocator API is `unsafe`
  /// as there are no lifetimes here.
  ///
  /// **IMPORTANT:** Ensure all allocated entries are fully initialized before dropping `RawArena`,
  /// or use `recycle_without_drop` to manually handle recycling, as dropping the arena does not
  /// perform any validation or cleanup on the allocated items. Dropping `RawArena` will automatically
  /// trigger the drop of all items allocated within.
  pub unsafe fn allocate(&self) -> NonNull<T> {
    unsafe {
      #[cfg(debug_assertions)]
      debug_assert_eq!(self.signature, SIGNATURE);
      let next = self.next.get();
      let max = self.max.get();

      // Check to see if we have gone past our high-water mark, and we need to extend it. The high-water
      // mark allows us to leave the allocation uninitialized, and assume that the remaining part of the
      // next-free list is a trivial linked-list where each node points to the next one.
      if max == next {
        // Are we out of room?
        if max == self.past_alloc_end {
          // We filled the RawArena, so start allocating
          return Self::entry_to_data(alloc());
        }

        // Nope, we can extend by one
        let next = NonNull::new_unchecked(self.max.get().as_ptr().add(1));
        self.next.set(next);
        self.max.set(next);
      } else {
        // We haven't passed the high-water mark, so walk the internal next-free list
        // for our next allocation
        self.next.set(RawArenaEntry::next(next));
      }

      // Update accounting
      self.allocated.set(self.allocated.get() + 1);
      Self::entry_to_data(next)
    }
  }

  /// Gets the next free entry, returning null if full. This is `O(1)`.
  ///
  /// # Safety
  ///
  /// As the memory area is considered uninitialized and you must be careful to fully and validly
  /// initialize the underlying data, this method is marked as unsafe.
  ///
  /// This pointer will be invalidated when we drop the `RawArena`, so the allocator API is `unsafe`
  /// as there are no lifetimes here.
  ///
  /// **IMPORTANT:** Ensure all allocated entries are fully initialized before dropping `RawArena`,
  /// or use `recycle_without_drop` to manually handle recycling, as dropping the arena does not
  /// perform any validation or cleanup on the allocated items. Dropping `RawArena` will automatically
  /// trigger the drop of all items allocated within.
  pub unsafe fn allocate_if_space(&self) -> Option<NonNull<T>> {
    unsafe {
      #[cfg(debug_assertions)]
      debug_assert_eq!(self.signature, SIGNATURE);
      let next = self.next.get();
      let max = self.max.get();

      // Check to see if we have gone past our high-water mark, and we need to extend it. The high-water
      // mark allows us to leave the allocation uninitialized, and assume that the remaining part of the
      // next-free list is a trivial linked-list where each node points to the next one.
      if max == next {
        // Are we out of room?
        if max == self.past_alloc_end {
          // We filled the RawArena, so return None
          return None;
        }

        // Nope, we can extend by one
        let next = NonNull::new_unchecked(self.max.get().as_ptr().add(1));
        self.next.set(next);
        self.max.set(next);
      } else {
        // We haven't passed the high-water mark, so walk the internal next-free list
        // for our next allocation
        self.next.set((*(next.as_ptr())).next);
      }

      // Update accounting
      self.allocated.set(self.allocated.get() + 1);
      Some(Self::entry_to_data(next))
    }
  }

  /// Returns the remaining capacity of this [`RawArena`] that can be provided without allocation.
  pub fn remaining(&self) -> usize {
    self.capacity - self.allocated.get()
  }

  /// Returns the remaining capacity of this [`RawArena`] that can be provided without allocation.
  pub fn allocated(&self) -> usize {
    self.allocated.get()
  }

  /// Clear all internally-allocated entries, resetting the arena state to its original state. Any
  /// non-vacant entries are dropped.
  ///
  /// This operation must walk the vacant list and is worst-case `O(n)`, where `n` is the largest
  /// size of this arena since the last clear operation.
  ///
  /// # Safety
  ///
  /// Does not clear system-allocator entries. Pointers previously [`allocate`](Self::allocate)d may still be in use.
  pub unsafe fn clear_allocated(&self) {
    #[cfg(debug_assertions)]
    debug_assert_eq!(self.signature, SIGNATURE);

    // We need to drop the allocated pointers, but we don't know which ones they are. We only
    // know the vacant slots.
    if self.allocated.get() > 0 {
      unsafe {
        // How many entries are we possibly using?
        let max = self.max.get();

        // Compute the vacant set by walking the `next` pointers
        let count = max.as_ptr().offset_from(self.alloc.as_ptr()) as usize;
        let mut vacant = BitVec::with_capacity(count);
        vacant.grow(count, false);

        let mut next = self.next.get();
        while next != max {
          let i = next.as_ptr().offset_from(self.alloc.as_ptr()) as usize;
          vacant.set(i, true);
          next = RawArenaEntry::next(next);
        }

        vacant.negate();

        // Iterate over the inverse of the vacant set and free those items
        for alloc in BitSet::from_bit_vec(vacant).into_iter() {
          let entry = self.alloc.as_ptr().add(alloc);
          std::ptr::drop_in_place(
            std::ptr::addr_of_mut!((*entry).value) as *mut T
          );
        }
      }
    }

    self.max.set(self.alloc);
    self.next.set(self.alloc);
    self.allocated.set(0);
  }

  /// Recycle a used item, returning it to the next-free list. Drops the associated item
  /// in place before recycling.
  ///
  /// # Safety
  ///
  /// We assume this pointer is either internal to the arena (in which case we return it
  /// to the arena), or allocated via [`std::alloc::alloc`] in [`allocate`](Self::allocate).
  pub unsafe fn recycle(&self, data: NonNull<T>) -> bool {
    unsafe {
      #[cfg(debug_assertions)]
      debug_assert_eq!(self.signature, SIGNATURE);
      let mut entry = Self::data_to_entry(data);
      let mut emptied = false;
      RawArenaEntry::drop(entry);
      if entry >= self.alloc && entry < self.past_alloc_end {
        let next = self.next.get();
        let count = self.allocated.get() - 1;
        emptied = count == 0;
        self.allocated.set(count);
        entry.as_mut().next = next;
        self.next.set(entry);
      } else {
        std::alloc::dealloc(
          entry.as_ptr() as _,
          Layout::new::<RawArenaEntry<T>>(),
        );
      }
      emptied
    }
  }

  /// Recycle a used item, returning it to the next-free list.
  ///
  /// # Safety
  ///
  /// We assume this pointer is either internal to the arena (in which case we return it
  /// to the arena), or allocated via [`std::alloc::alloc`] in [`allocate`](Self::allocate).
  pub unsafe fn recycle_without_drop(&self, data: NonNull<T>) -> bool {
    unsafe {
      #[cfg(debug_assertions)]
      debug_assert_eq!(self.signature, SIGNATURE);
      let mut entry = Self::data_to_entry(data);
      let mut emptied = false;
      if entry >= self.alloc && entry < self.past_alloc_end {
        let next = self.next.get();
        let count = self.allocated.get() - 1;
        emptied = count == 0;
        self.allocated.set(count);
        entry.as_mut().next = next;
        self.next.set(entry);
      } else {
        std::alloc::dealloc(
          entry.as_ptr() as _,
          Layout::new::<RawArenaEntry<T>>(),
        );
      }
      emptied
    }
  }
}

impl<T> Drop for RawArena<T> {
  /// Drop the arena. All pointers are invalidated at this point, except for those
  /// allocated outside outside of the arena.
  ///
  /// The allocation APIs are unsafe because we don't track lifetimes here.
  fn drop(&mut self) {
    unsafe { self.clear_allocated() };

    #[cfg(debug_assertions)]
    {
      debug_assert_eq!(self.signature, SIGNATURE);
      self.signature = 0;
    }
    unsafe {
      std::alloc::dealloc(self.alloc.as_ptr() as _, Self::layout(self.capacity))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[must_use = "If you don't use this, it'll leak!"]
  unsafe fn allocate(arena: &RawArena<usize>, i: usize) -> NonNull<usize> {
    unsafe {
      let mut new = arena.allocate();
      *new.as_mut() = i;
      new
    }
  }

  #[test]
  fn test_add_remove_many() {
    let arena = RawArena::<usize>::with_capacity(1024);
    unsafe {
      for i in 0..2000 {
        let v = allocate(&arena, i);
        assert_eq!(arena.remaining(), 1023);
        assert_eq!(*v.as_ref(), i);
        arena.recycle(v);
        assert_eq!(arena.remaining(), 1024);
      }
    }
  }

  #[test]
  fn test_add_clear_many() {
    let arena = RawArena::<usize>::with_capacity(1024);
    unsafe {
      for i in 0..2000 {
        _ = allocate(&arena, i);
        assert_eq!(arena.remaining(), 1023);
        arena.clear_allocated();
        assert_eq!(arena.remaining(), 1024);
      }
    }
  }

  #[test]
  fn test_add_remove_many_separate() {
    let arena = RawArena::<usize>::with_capacity(1024);
    unsafe {
      let mut nodes = vec![];
      // This will spill over into memory allocations
      for i in 0..2000 {
        nodes.push(allocate(&arena, i));
      }
      assert_eq!(arena.remaining(), 0);
      for i in (0..2000).rev() {
        let node = nodes.pop().unwrap();
        assert_eq!(*node.as_ref(), i);
        arena.recycle(node);
      }
      assert_eq!(arena.remaining(), 1024);
    }
  }

  #[test]
  fn test_droppable() {
    // Make sure we correctly drop all the items in this arena if they are droppable
    let arena = RawArena::<_>::with_capacity(16);
    unsafe {
      let mut nodes = vec![];
      // This will spill over into memory allocations
      for i in 0..20 {
        let node = arena.allocate();
        std::ptr::write(
          node.as_ptr(),
          Box::new(std::future::ready(format!("iteration {i}"))),
        );
        nodes.push(node);
      }
      assert_eq!(arena.remaining(), 0);
      for node in nodes {
        arena.recycle(node);
      }
      assert_eq!(arena.remaining(), 16);
    }
  }

  #[test]
  fn test_no_drop() {
    let arena = RawArena::<String>::with_capacity(16);
    unsafe {
      arena.recycle_without_drop(arena.allocate());
      arena.clear_allocated();
    }
  }

  #[test]
  fn test_drops() {
    let arena = RawArena::<_>::with_capacity(16);
    unsafe {
      for i in 0..2 {
        let ptr = arena.allocate();
        std::ptr::write(ptr.as_ptr(), format!("iteration {i}"));
      }
      // Leave a space in the internal allocations
      let ptr = arena.allocate();
      std::ptr::write(ptr.as_ptr(), "deleted".to_owned());
      arena.recycle(ptr);
      arena.clear_allocated();
    }
  }

  #[test]
  fn test_drops_full() {
    #[allow(dead_code)]
    struct Droppable(String);

    let arena = RawArena::<_>::with_capacity(16);
    unsafe {
      for i in 0..2 {
        let ptr = arena.allocate();
        std::ptr::write(ptr.as_ptr(), Droppable(format!("iteration {i}")));
      }
      arena.clear_allocated();
    }
  }
}
