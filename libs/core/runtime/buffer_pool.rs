// Copyright 2018-2026 the Deno authors. MIT license.

//! Pooled ArrayBuffer backing-store allocator.
//!
//! V8's default ArrayBuffer allocator malloc+zeroes every backing store and
//! frees it back to the system allocator when the buffer dies. For the
//! mid-size buffers that dominate streaming workloads (fetch body chunks,
//! file reads, encoder output; typically 16 KiB - 2 MiB), that cycle is
//! expensive on macOS and Linux alike: the allocator's medium-size magazines
//! madvise pages away on free, so the next allocation page-faults the memory
//! back in just to zero it again. A `new Uint8Array(64 * 1024)` costs ~5.5us
//! on an M1 Max; the equivalent allocation in JavaScriptCore (which hands out
//! lazily-zeroed pages) is under 1us.
//!
//! This allocator keeps freed backing stores of pooled size classes on
//! freelists instead of returning them to the system. A pooled allocation
//! pops a warm block and zeroes only the requested length: the pages are
//! still resident, so the zeroing runs at memset speed instead of
//! page-fault speed, and the malloc magazine churn disappears entirely.
//!
//! Size classes are powers of two from MIN_POOLED (16 KiB) to MAX_POOLED
//! (4 MiB). Requests outside that range, including all small allocations
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
/// Largest pooled block: 4 MiB. log2 = 22.
const MAX_POOLED_SHIFT: u32 = 22;
const NUM_CLASSES: usize = (MAX_POOLED_SHIFT - MIN_POOLED_SHIFT + 1) as usize;

/// Maximum bytes retained per size class (not per allocation site). With 9
/// classes this bounds total retention at 9 * 8 MiB = 72 MiB, reached only
/// by a workload that actively cycles buffers of every class.
const CLASS_CAP_BYTES: usize = 8 * 1024 * 1024;

struct SizeClass {
  blocks: Mutex<Vec<*mut u8>>,
}

// SAFETY: the raw pointers are owned, unaliased heap blocks; the Mutex
// serializes access.
unsafe impl Send for SizeClass {}
unsafe impl Sync for SizeClass {}

pub(crate) struct BufferPool {
  classes: [SizeClass; NUM_CLASSES],
}

/// Returns the class index for a pooled length, or None for the fall-through
/// path.
#[inline]
fn class_for(len: usize) -> Option<usize> {
  if len == 0 || len > (1 << MAX_POOLED_SHIFT) {
    return None;
  }
  let shift = usize::BITS - (len - 1).leading_zeros();
  let shift = shift.max(MIN_POOLED_SHIFT);
  // Small allocations are cheap through the system allocator and pooling
  // them would waste most of a 16 KiB block.
  if len <= (1 << MIN_POOLED_SHIFT) / 2 {
    return None;
  }
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
        blocks: Mutex::new(Vec::new()),
      }),
    }
  }

  /// Pop a warm block or allocate a fresh one. Returns an uninitialized
  /// block of at least `len` bytes.
  #[inline]
  fn acquire(&self, class: usize) -> *mut u8 {
    let popped = self.classes[class].blocks.lock().unwrap().pop();
    match popped {
      Some(ptr) => ptr,
      // SAFETY: class_layout is a valid non-zero layout.
      None => unsafe { alloc(class_layout(class)) },
    }
  }

  #[inline]
  fn release(&self, class: usize, ptr: *mut u8) {
    let mut blocks = self.classes[class].blocks.lock().unwrap();
    if (blocks.len() + 1) * class_size(class) <= CLASS_CAP_BYTES {
      blocks.push(ptr);
    } else {
      drop(blocks);
      // SAFETY: ptr was allocated with class_layout(class).
      unsafe { dealloc(ptr, class_layout(class)) };
    }
  }
}

unsafe extern "C" fn pool_allocate(pool: &BufferPool, len: usize) -> *mut c_void {
  match class_for(len) {
    Some(class) => {
      let ptr = pool.acquire(class);
      if !ptr.is_null() {
        // Zero only the requested length: V8 never exposes the block's
        // tail. The pages are resident for pooled hits, so this runs at
        // memset speed.
        // SAFETY: ptr is valid for class_size(class) >= len bytes.
        unsafe { ptr::write_bytes(ptr, 0, len) };
      }
      ptr as *mut c_void
    }
    // SAFETY: raw_layout is a valid non-zero layout. alloc_zeroed uses
    // calloc under the hood, which gets pre-zeroed pages from the OS for
    // large requests.
    None => unsafe { alloc_zeroed(raw_layout(len)) as *mut c_void },
  }
}

unsafe extern "C" fn pool_allocate_uninitialized(
  pool: &BufferPool,
  len: usize,
) -> *mut c_void {
  match class_for(len) {
    Some(class) => pool.acquire(class) as *mut c_void,
    // SAFETY: raw_layout is a valid non-zero layout.
    None => unsafe { alloc(raw_layout(len)) as *mut c_void },
  }
}

unsafe extern "C" fn pool_free(
  pool: &BufferPool,
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

unsafe extern "C" fn pool_drop(pool: *const BufferPool) {
  // SAFETY: the handle was created with Arc::into_raw in allocator().
  drop(unsafe { Arc::from_raw(pool) });
}

static VTABLE: v8::RustAllocatorVtable<BufferPool> =
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
pub(crate) fn array_buffer_allocator()
-> Option<v8::UniqueRef<v8::Allocator>> {
  let pool = POOL.get_or_init(|| {
    if std::env::var_os("DENO_DISABLE_BUFFER_POOL").is_some() {
      None
    } else {
      Some(Arc::new(BufferPool::new()))
    }
  });
  let pool = pool.as_ref()?;
  // SAFETY: the handle is a strong Arc reference released in pool_drop, and
  // VTABLE matches the BufferPool handle type.
  Some(unsafe { v8::new_rust_allocator(Arc::into_raw(pool.clone()), &VTABLE) })
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
    assert_eq!(class_for(4 * 1024 * 1024 + 1), None);
  }

  #[test]
  fn acquire_release_roundtrip() {
    let pool = BufferPool::new();
    let class = class_for(64 * 1024).unwrap();
    let a = pool.acquire(class);
    assert!(!a.is_null());
    pool.release(class, a);
    let b = pool.acquire(class);
    assert_eq!(a, b, "released block is reused");
    pool.release(class, b);
  }
}
