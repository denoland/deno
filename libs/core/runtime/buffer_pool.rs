// Copyright 2018-2026 the Deno authors. MIT license.

//! Pooled ArrayBuffer backing-store allocator.
//!
//! V8's default ArrayBuffer allocator malloc+zeroes every backing store and
//! frees it back to the system allocator when the buffer dies. For the
//! mid-size buffers that dominate streaming workloads (fetch body chunks,
//! file reads, encoder output; typically 16 KiB - 2 MiB), that cycle is
//! expensive: the allocator's medium-size magazines madvise pages away on
//! free, so the next allocation page-faults the memory back in just to zero
//! it again. A `new Uint8Array(64 * 1024)` costs ~5.5us on an M1 Max; the
//! equivalent allocation in JavaScriptCore (which hands out lazily-zeroed
//! pages) is under 1us.
//!
//! This allocator keeps freed backing stores of pooled size classes on a
//! per-class freelist instead of returning them to the system. `allocate`
//! pops a freed block and zeroes it inline before handing it back; because the
//! block is still resident (its pages were never madvised away), that zeroing
//! runs at memset speed rather than paying the page-fault-then-zero cost of a
//! fresh allocation. A miss falls back to `alloc_zeroed`, which obtains
//! pre-zeroed pages from the OS for these sizes. `allocate_uninitialized` skips
//! the zeroing entirely.
//!
//! Size classes are powers of two from MIN_POOLED (16 KiB) to MAX_POOLED
//! (16 MiB). Requests outside that range, including all small allocations
//! (cheap already) and giant buffers (retention risk), fall through to the
//! plain global allocator. Each class retains at most CLASS_CAP_BYTES; blocks
//! freed beyond the cap go straight back to the system.
//!
//! The pool is process-global and shared by all isolates: allocator
//! callbacks can fire on GC and background threads, and buffers regularly
//! outlive the isolate that allocated them (transfers), so per-isolate
//! pools would need the same synchronization anyway.
//!
//! Set DENO_DISABLE_BUFFER_POOL=1 to fall back to V8's default allocator
//! (useful for benchmarking and as an escape hatch).

use std::alloc::Layout;
use std::alloc::alloc;
use std::alloc::alloc_zeroed;
use std::alloc::dealloc;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Backing stores are at most 8-byte aligned as far as V8 is concerned, but
/// the default allocator provides malloc alignment; match it.
const ALIGN: usize = 16;

/// Smallest pooled block: 16 KiB. log2 = 14.
const MIN_POOLED_SHIFT: u32 = 14;
/// Largest pooled block: 16 MiB. log2 = 24. Covers whole-body assembly
/// buffers (Body consumers collect into one allocation), which benefit
/// from pooled blocks just like streaming chunks do.
const MAX_POOLED_SHIFT: u32 = 24;
const NUM_CLASSES: usize = (MAX_POOLED_SHIFT - MIN_POOLED_SHIFT + 1) as usize;

/// Maximum bytes retained per size class; classes at least this big retain a
/// single block. Bounds total retention at 9 * 8 MiB + 8 MiB + 16 MiB = 96 MiB,
/// reached only by a workload that actively cycles buffers of every class.
const CLASS_CAP_BYTES: usize = 8 * 1024 * 1024;

#[inline]
fn class_cap_bytes(class: usize) -> usize {
  CLASS_CAP_BYTES.max(class_size(class))
}

/// Raw block pointers stored in the freelist.
struct Blocks(Vec<*mut u8>);

// SAFETY: the raw pointers are owned, unaliased heap blocks; the Mutex
// around the freelist serializes access.
unsafe impl Send for Blocks {}

struct SizeClass {
  free: Mutex<Blocks>,
}

pub(crate) struct BufferPool {
  classes: [SizeClass; NUM_CLASSES],
}

// SAFETY: all interior state is behind Mutexes.
unsafe impl Send for BufferPool {}
unsafe impl Sync for BufferPool {}

/// Returns the class index for a pooled length, or None for the fall-through
/// path.
#[inline]
fn class_for(len: usize) -> Option<usize> {
  if len == 0 || len > (1 << MAX_POOLED_SHIFT) {
    return None;
  }
  // Small allocations are cheap through the system allocator and pooling
  // them would waste most of a 16 KiB block.
  if len <= (1 << MIN_POOLED_SHIFT) / 2 {
    return None;
  }
  let shift = usize::BITS - (len - 1).leading_zeros();
  let shift = shift.max(MIN_POOLED_SHIFT);
  Some((shift - MIN_POOLED_SHIFT) as usize)
}

#[inline]
fn class_size(class: usize) -> usize {
  1 << (class as u32 + MIN_POOLED_SHIFT)
}

#[inline]
fn class_layout(class: usize) -> Layout {
  // SAFETY: size is a power of two >= ALIGN, ALIGN is a power of two.
  unsafe { Layout::from_size_align_unchecked(class_size(class), ALIGN) }
}

#[inline]
fn raw_layout(len: usize) -> Layout {
  // SAFETY: ALIGN is a power of two; len was accepted by V8 so it does not
  // overflow when rounded up to alignment.
  unsafe { Layout::from_size_align_unchecked(len.max(1), ALIGN) }
}

impl BufferPool {
  fn new() -> Self {
    Self {
      classes: std::array::from_fn(|_| SizeClass {
        free: Mutex::new(Blocks(Vec::new())),
      }),
    }
  }

  /// Pop a freed block off the class freelist, if one is available.
  #[inline]
  fn pop(&self, class: usize) -> Option<*mut u8> {
    self.classes[class].free.lock().unwrap().0.pop()
  }

  fn release(&self, class: usize, ptr: *mut u8) {
    let mut free = self.classes[class].free.lock().unwrap();
    if (free.0.len() + 1) * class_size(class) > class_cap_bytes(class) {
      drop(free);
      // SAFETY: ptr was allocated with class_layout(class).
      unsafe { dealloc(ptr, class_layout(class)) };
      return;
    }
    free.0.push(ptr);
  }
}

impl Drop for BufferPool {
  fn drop(&mut self) {
    // Free every block still retained on the freelists. In production the pool
    // is an immortal process-global singleton, so this never runs there; it
    // only fires for the short-lived pools in tests / under Miri, keeping the
    // leak checker happy.
    for class in 0..NUM_CLASSES {
      for ptr in self.classes[class].free.lock().unwrap().0.drain(..) {
        // SAFETY: ptr was allocated with class_layout(class) and is owned
        // exclusively by this freelist.
        unsafe { dealloc(ptr, class_layout(class)) };
      }
    }
  }
}

unsafe extern "C" fn pool_allocate(
  pool: &Arc<BufferPool>,
  len: usize,
) -> *mut c_void {
  match class_for(len) {
    Some(class) => {
      // Reuse a freed block, zeroing it inline. The block is still resident,
      // so this is a memset rather than a page-fault-then-zero.
      if let Some(ptr) = pool.pop(class) {
        // SAFETY: ptr is valid for class_size(class) >= len bytes.
        unsafe { ptr::write_bytes(ptr, 0, len) };
        return ptr as *mut c_void;
      }
      // Miss: alloc_zeroed obtains pre-zeroed pages from the OS for these
      // sizes (calloc), avoiding an explicit memset of cold pages.
      // SAFETY: class_layout is a valid non-zero layout.
      unsafe { alloc_zeroed(class_layout(class)) as *mut c_void }
    }
    // SAFETY: raw_layout is a valid non-zero layout.
    None => unsafe { alloc_zeroed(raw_layout(len)) as *mut c_void },
  }
}

unsafe extern "C" fn pool_allocate_uninitialized(
  pool: &Arc<BufferPool>,
  len: usize,
) -> *mut c_void {
  match class_for(len) {
    Some(class) => {
      if let Some(ptr) = pool.pop(class) {
        return ptr as *mut c_void;
      }
      // SAFETY: class_layout is a valid non-zero layout.
      unsafe { alloc(class_layout(class)) as *mut c_void }
    }
    // SAFETY: raw_layout is a valid non-zero layout.
    None => unsafe { alloc(raw_layout(len)) as *mut c_void },
  }
}

unsafe extern "C" fn pool_free(
  pool: &Arc<BufferPool>,
  data: *mut c_void,
  len: usize,
) {
  if data.is_null() {
    return;
  }
  match class_for(len) {
    Some(class) => pool.release(class, data as *mut u8),
    // SAFETY: data was allocated with raw_layout(len) by the functions
    // above.
    None => unsafe { dealloc(data as *mut u8, raw_layout(len)) },
  }
}

unsafe extern "C" fn pool_drop(pool: *const Arc<BufferPool>) {
  // SAFETY: the handle was created with Box::into_raw in
  // array_buffer_allocator().
  drop(unsafe { Box::from_raw(pool as *mut Arc<BufferPool>) });
}

static VTABLE: v8::RustAllocatorVtable<Arc<BufferPool>> =
  v8::RustAllocatorVtable {
    allocate: pool_allocate,
    allocate_uninitialized: pool_allocate_uninitialized,
    free: pool_free,
    drop: pool_drop,
  };

static POOL: OnceLock<Option<Arc<BufferPool>>> = OnceLock::new();

/// Returns a pooled ArrayBuffer allocator, or None when disabled via
/// DENO_DISABLE_BUFFER_POOL (callers then fall back to V8's default
/// allocator).
pub(crate) fn array_buffer_allocator() -> Option<v8::UniqueRef<v8::Allocator>> {
  let pool = POOL.get_or_init(|| {
    // The pool is a process-global allocator installed on the V8 CreateParams
    // before any `sys_traits` environment is available, so read the opt-out
    // escape hatch directly here.
    #[allow(
      clippy::disallowed_methods,
      reason = "process-global allocator init runs before sys_traits is set up"
    )]
    let disabled = std::env::var_os("DENO_DISABLE_BUFFER_POOL").is_some();
    if disabled {
      None
    } else {
      Some(Arc::new(BufferPool::new()))
    }
  });
  let pool = pool.as_ref()?;
  let handle = Box::into_raw(Box::new(pool.clone()));
  // SAFETY: the handle is a heap-allocated strong Arc reference released in
  // pool_drop, and VTABLE matches the Arc<BufferPool> handle type.
  Some(unsafe { v8::new_rust_allocator(handle, &VTABLE) })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn class_selection() {
    assert_eq!(class_for(0), None);
    assert_eq!(class_for(1), None);
    assert_eq!(class_for(8 * 1024), None);
    assert_eq!(class_for(8 * 1024 + 1), Some(0));
    assert_eq!(class_for(16 * 1024), Some(0));
    assert_eq!(class_for(16 * 1024 + 1), Some(1));
    assert_eq!(class_for(64 * 1024), Some(2));
    assert_eq!(class_for(4 * 1024 * 1024), Some(8));
    assert_eq!(class_for(4 * 1024 * 1024 + 1), Some(9));
    assert_eq!(class_for(16 * 1024 * 1024), Some(10));
    assert_eq!(class_for(16 * 1024 * 1024 + 1), None);
  }

  #[test]
  fn allocate_free_roundtrip_reuses_block() {
    let pool = Arc::new(BufferPool::new());
    let len = 64 * 1024;
    // SAFETY: exercising the allocator callbacks directly.
    unsafe {
      let a = pool_allocate(&pool, len);
      assert!(!a.is_null());
      // Returned memory must be zeroed.
      assert_eq!(*(a as *const u8), 0);
      ptr::write_bytes(a as *mut u8, 0xab, len);
      pool_free(&pool, a, len);
      // The freed block is now on the freelist; the next allocation reuses it
      // and must observe zeroed memory again.
      let b = pool_allocate(&pool, len);
      assert!(!b.is_null());
      let bytes = std::slice::from_raw_parts(b as *const u8, len);
      assert!(bytes.iter().all(|&x| x == 0));
      pool_free(&pool, b, len);
    }
  }

  #[test]
  fn uninitialized_roundtrip_reuses_block() {
    let pool = Arc::new(BufferPool::new());
    let len = 32 * 1024;
    // SAFETY: exercising the allocator callbacks directly.
    unsafe {
      let a = pool_allocate_uninitialized(&pool, len);
      assert!(!a.is_null());
      pool_free(&pool, a, len);
      let b = pool_allocate_uninitialized(&pool, len);
      assert!(!b.is_null());
      pool_free(&pool, b, len);
    }
  }
}
