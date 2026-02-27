// Copyright 2018-2025 the Deno authors. MIT license.

mod raw_arena;
mod shared_arena;
mod shared_atomic_arena;
mod unique_arena;

use std::alloc::Layout;
use std::alloc::handle_alloc_error;
use std::ptr::NonNull;

pub use raw_arena::*;
pub use shared_arena::*;
pub use shared_atomic_arena::*;
pub use unique_arena::*;

const unsafe fn ptr_byte_add<T, U>(
  ptr: NonNull<T>,
  offset: usize,
) -> NonNull<U> {
  unsafe { NonNull::new_unchecked((ptr.as_ptr() as *mut u8).add(offset) as _) }
}

const unsafe fn ptr_byte_sub<T, U>(
  ptr: NonNull<T>,
  offset: usize,
) -> NonNull<U> {
  unsafe { NonNull::new_unchecked((ptr.as_ptr() as *mut u8).sub(offset) as _) }
}

#[inline(always)]
fn alloc_layout<T>(layout: Layout) -> NonNull<T> {
  // Layout of size zero is UB
  assert!(std::mem::size_of::<T>() > 0);
  let alloc = unsafe { std::alloc::alloc(layout) } as *mut _;
  let Some(alloc) = NonNull::new(alloc) else {
    handle_alloc_error(layout);
  };
  alloc
}

#[inline(always)]
fn alloc<T>() -> NonNull<T> {
  // Layout of size zero is UB
  assert!(std::mem::size_of::<T>() > 0);
  let alloc = unsafe { std::alloc::alloc(Layout::new::<T>()) } as *mut _;
  let Some(alloc) = NonNull::new(alloc) else {
    handle_alloc_error(Layout::new::<T>());
  };
  alloc
}
