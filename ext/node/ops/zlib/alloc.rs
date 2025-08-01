// Copyright 2018-2025 the Deno authors. MIT license.

// Workaround for https://github.com/rust-lang/libz-sys/issues/55
// See https://github.com/rust-lang/flate2-rs/blob/31fb07820345691352aaa64f367c1e482ad9cfdc/src/ffi/c.rs#L60
use std::alloc::Layout;
use std::alloc::{self};
use std::os::raw::c_void;
use std::ptr;

const ALIGN: usize = std::mem::align_of::<usize>();

fn align_up(size: usize, align: usize) -> usize {
  (size + align - 1) & !(align - 1)
}

pub extern "C" fn zalloc(
  _ptr: *mut c_void,
  items: u32,
  item_size: u32,
) -> *mut c_void {
  // We need to multiply `items` and `item_size` to get the actual desired
  // allocation size. Since `zfree` doesn't receive a size argument we
  // also need to allocate space for a `usize` as a header so we can store
  // how large the allocation is to deallocate later.
  let size = match (items as usize)
    .checked_mul(item_size as usize)
    .map(|size| align_up(size, ALIGN))
    .and_then(|i| i.checked_add(std::mem::size_of::<usize>()))
  {
    Some(i) => i,
    None => return ptr::null_mut(),
  };

  // Make sure the `size` isn't too big to fail `Layout`'s restrictions
  let layout = match Layout::from_size_align(size, ALIGN) {
    Ok(layout) => layout,
    Err(_) => return ptr::null_mut(),
  };

  // SAFETY: `layout` has non-zero size, guaranteed to be a sentinel address
  // or a null pointer.
  unsafe {
    // Allocate the data, and if successful store the size we allocated
    // at the beginning and then return an offset pointer.
    let ptr = alloc::alloc(layout) as *mut usize;
    if ptr.is_null() {
      return ptr as *mut c_void;
    }
    *ptr = size;
    ptr.add(1) as *mut c_void
  }
}

pub extern "C" fn zfree(_ptr: *mut c_void, address: *mut c_void) {
  // SAFETY: Move our address being free'd back one pointer, read the size we
  // stored in `zalloc`, and then free it using the standard Rust
  // allocator.
  unsafe {
    let ptr = (address as *mut usize).offset(-1);
    let size = *ptr;
    let layout = Layout::from_size_align_unchecked(size, ALIGN);
    alloc::dealloc(ptr as *mut u8, layout)
  }
}

pub extern "C" fn brotli_alloc(
  _opaque: *mut brotli::ffi::broccoli::c_void,
  size: usize,
) -> *mut brotli::ffi::broccoli::c_void {
  // Allocate space for a `usize` header to store the allocation size
  let total_size = match size
    .checked_add(std::mem::size_of::<usize>())
    .map(|size| align_up(size, ALIGN))
  {
    Some(i) => i,
    None => return ptr::null_mut(),
  };

  let layout = match Layout::from_size_align(total_size, ALIGN) {
    Ok(layout) => layout,
    Err(_) => return ptr::null_mut(),
  };

  // SAFETY: `layout` has non-zero size
  unsafe {
    let ptr = alloc::alloc(layout) as *mut usize;
    if ptr.is_null() {
      return ptr as *mut brotli::ffi::broccoli::c_void;
    }
    *ptr = total_size;
    ptr.add(1) as *mut brotli::ffi::broccoli::c_void
  }
}

pub extern "C" fn brotli_free(
  _opaque: *mut brotli::ffi::broccoli::c_void,
  address: *mut brotli::ffi::broccoli::c_void,
) {
  if address.is_null() {
    return;
  }

  // SAFETY: Move back one pointer to read the size we stored in `brotli_alloc`
  unsafe {
    let ptr = (address as *mut usize).offset(-1);
    let size = *ptr;
    let layout = Layout::from_size_align_unchecked(size, ALIGN);
    alloc::dealloc(ptr as *mut u8, layout)
  }
}
