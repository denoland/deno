// Copyright 2018-2026 the Deno authors. MIT license.

//! Pooled ArrayBuffer backing-store allocator with background pre-zeroing.
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
//! This allocator keeps freed backing stores of pooled size classes on
//! per-class freelists instead of returning them to the system, split into
//! two tiers:
//!
//! - `clean`: fully zeroed blocks. `allocate` pops one and returns it with
//!   no zeroing at all on the hot path.
//! - `dirty`: blocks as freed by V8. A lazily spawned background thread
//!   drains the dirty tier, zeroing whole blocks and promoting them to
//!   `clean`. If `allocate` finds no clean block it falls back to zeroing a
//!   dirty block inline (still resident pages, memset speed), and finally
//!   to `alloc_zeroed` (which obtains pre-zeroed pages from the OS for
//!   these sizes). `allocate_uninitialized` prefers dirty blocks so clean
//!   ones are saved for zeroed allocations.
//!
//! Size classes are powers of two from MIN_POOLED (16 KiB) to MAX_POOLED
//! (4 MiB). Requests outside that range, including all small allocations
//! (cheap already) and giant buffers (retention risk), fall through to the
//! plain global allocator. Each class retains at most CLASS_CAP_BYTES
//! across both tiers; blocks freed beyond the cap go straight back to the
//! system.
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
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::Once;
use std::sync::OnceLock;

/// Backing stores are at most 8-byte aligned as far as V8 is concerned, but
/// the default allocator provides malloc alignment; match it.
const ALIGN: usize = 16;

/// Smallest pooled block: 16 KiB. log2 = 14.
const MIN_POOLED_SHIFT: u32 = 14;
/// Largest pooled block: 4 MiB. log2 = 22.
const MAX_POOLED_SHIFT: u32 = 22;
const NUM_CLASSES: usize = (MAX_POOLED_SHIFT - MIN_POOLED_SHIFT + 1) as usize;

/// Maximum bytes retained per size class across both tiers. With 9 classes
/// this bounds total retention at 9 * 8 MiB = 72 MiB, reached only by a
/// workload that actively cycles buffers of every class.
const CLASS_CAP_BYTES: usize = 8 * 1024 * 1024;

/// Raw block pointers stored in the freelists.
struct Blocks(Vec<*mut u8>);

// SAFETY: the raw pointers are owned, unaliased heap blocks; the Mutex
// around each tier serializes access.
unsafe impl Send for Blocks {}

struct SizeClass {
  clean: Mutex<Blocks>,
  dirty: Mutex<Blocks>,
}

pub(crate) struct BufferPool {
  classes: [SizeClass; NUM_CLASSES],
  /// Signals the zeroer thread that dirty blocks are waiting.
  zeroer_wakeup: Condvar,
  zeroer_mutex: Mutex<bool>,
  zeroer_spawn: Once,
}

// SAFETY: all interior state is behind Mutexes/Condvar.
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
        clean: Mutex::new(Blocks(Vec::new())),
        dirty: Mutex::new(Blocks(Vec::new())),
      }),
      zeroer_wakeup: Condvar::new(),
      zeroer_mutex: Mutex::new(false),
      zeroer_spawn: Once::new(),
    }
  }

  /// Pop a fully-zeroed block, if one is available.
  #[inline]
  fn pop_clean(&self, class: usize) -> Option<*mut u8> {
    self.classes[class].clean.lock().unwrap().0.pop()
  }

  /// Pop a freed, not-yet-zeroed block, if one is available.
  #[inline]
  fn pop_dirty(&self, class: usize) -> Option<*mut u8> {
    self.classes[class].dirty.lock().unwrap().0.pop()
  }

  fn retained_bytes(&self, class: usize) -> usize {
    let clean = self.classes[class].clean.lock().unwrap().0.len();
    let dirty = self.classes[class].dirty.lock().unwrap().0.len();
    (clean + dirty) * class_size(class)
  }

  fn release(self: &Arc<Self>, class: usize, ptr: *mut u8) {
    if self.retained_bytes(class) + class_size(class) > CLASS_CAP_BYTES {
      // SAFETY: ptr was allocated with class_layout(class).
      unsafe { dealloc(ptr, class_layout(class)) };
      return;
    }
    self.classes[class].dirty.lock().unwrap().0.push(ptr);
    self.ensure_zeroer();
    let mut pending = self.zeroer_mutex.lock().unwrap();
    *pending = true;
    drop(pending);
    self.zeroer_wakeup.notify_one();
  }

  /// Spawns the background zeroing thread on first use. The thread parks on
  /// the condvar and wakes when blocks land in a dirty tier; it zeroes whole
  /// blocks outside any lock and promotes them to the clean tier.
  fn ensure_zeroer(self: &Arc<Self>) {
    if self.zeroer_spawn.is_completed() {
      return;
    }
    let pool = self.clone();
    self.zeroer_spawn.call_once(move || {
      std::thread::Builder::new()
        .name("deno-buffer-zeroer".into())
        .spawn(move || {
          loop {
            {
              let mut pending = pool.zeroer_mutex.lock().unwrap();
              while !*pending {
                pending = pool.zeroer_wakeup.wait(pending).unwrap();
              }
              *pending = false;
            }
            // Drain all dirty tiers. New blocks freed while we work set
            // `pending` again, so nothing is lost between passes.
            for class in 0..NUM_CLASSES {
              loop {
                let Some(ptr) = pool.pop_dirty(class) else { break };
                // SAFETY: ptr is a valid block of class_size(class) bytes.
                unsafe { ptr::write_bytes(ptr, 0, class_size(class)) };
                pool.classes[class].clean.lock().unwrap().0.push(ptr);
              }
            }
          }
        })
        .ok();
    });
  }
}

unsafe extern "C" fn pool_allocate(
  pool: &Arc<BufferPool>,
  len: usize,
) -> *mut c_void {
  match class_for(len) {
    Some(class) => {
      // Fast path: a pre-zeroed block, no zeroing at all.
      if let Some(ptr) = pool.pop_clean(class) {
        return ptr as *mut c_void;
      }
      // Warm fallback: zero a resident dirty block at memset speed.
      if let Some(ptr) = pool.pop_dirty(class) {
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
      // Prefer dirty blocks so clean ones are saved for zeroed allocations.
      if let Some(ptr) = pool.pop_dirty(class) {
        return ptr as *mut c_void;
      }
      if let Some(ptr) = pool.pop_clean(class) {
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
    assert_eq!(class_for(4 * 1024 * 1024 + 1), None);
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
      // The block sits in the dirty tier (or clean, if the zeroer ran);
      // either way the next allocation must observe zeroed memory again.
      let b = pool_allocate(&pool, len);
      assert!(!b.is_null());
      let bytes = std::slice::from_raw_parts(b as *const u8, len);
      assert!(bytes.iter().all(|&x| x == 0));
      pool_free(&pool, b, len);
    }
  }

  #[test]
  fn uninitialized_prefers_dirty() {
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
