use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use libz_sys::*;
use std::borrow::Cow;

#[inline]
fn check(condition: bool, msg: &str) -> Result<(), AnyError> {
  if condition {
    Ok(())
  } else {
    Err(type_error(msg.to_string()))
  }
}

const Z_MIN_CHUNK: i32 = 64;
const Z_MAX_CHUNK: i32 = std::i32::MAX;
const Z_DEFAULT_CHUNK: i32 = 16 * 1024;
const Z_MIN_MEMLEVEL: i32 = 1;
const Z_MAX_MEMLEVEL: i32 = 9;
const Z_DEFAULT_MEMLEVEL: i32 = 8;
const Z_MIN_LEVEL: i32 = -1;
const Z_MAX_LEVEL: i32 = 9;
const Z_DEFAULT_LEVEL: i32 = Z_DEFAULT_COMPRESSION;
const Z_MIN_WINDOWBITS: i32 = 8;
const Z_MAX_WINDOWBITS: i32 = 15;
const Z_DEFAULT_WINDOWBITS: i32 = 15;

struct ZlibStream {
  stream: z_stream,
}

impl deno_core::Resource for ZlibStream {
  fn name(&self) -> Cow<str> {
    "zlibStream".into()
  }
}

#[op]
pub fn op_zlib_create_deflate(
  state: &mut OpState,
  level: i32,
  window_bits: i32,
  mem_level: i32,
  strategy: i32, 
) -> Result<u32, AnyError> {
  check(
    level >= Z_MIN_LEVEL && level <= Z_MAX_LEVEL,
    "Invalid compression level",
  )?;

  check(
    mem_level >= Z_MIN_MEMLEVEL && mem_level <= Z_MAX_MEMLEVEL,
    "Invalid memLevel",
  )?;

  check(
    strategy == Z_FILTERED
      || strategy == Z_HUFFMAN_ONLY
      || strategy == Z_RLE
      || strategy == Z_FIXED
      || strategy == Z_DEFAULT_STRATEGY,
    "Invalid strategy",
  )?;
 
  unsafe {
    let mut stream: libz_sys::z_stream = libz_sys::z_stream {
      next_in: std::ptr::null_mut(),
      avail_in: 0,
      total_in: 0,
      next_out: std::ptr::null_mut(),
      avail_out: 0,
      total_out: 0,
      msg: std::ptr::null_mut(),
      state: std::ptr::null_mut(),
      zalloc: flate2_libz_helpers::zalloc,
      zfree: flate2_libz_helpers::zfree,
      opaque: 0 as libz_sys::voidpf,
      data_type: 0,
      adler: 0,
      reserved: 0,
  };

    let ret = deflateInit2_(
      &mut stream,
      level,
      Z_DEFLATED,
      window_bits,
      mem_level,
      strategy,
      zlibVersion(),
      std::mem::size_of::<z_stream>() as i32,
    );

    check(ret == Z_OK, "deflateInit2 failed")?;

    Ok(state
      .resource_table
      .add(ZlibStream { stream }))
  }
}

#[op]
pub fn op_zlib_deflate_sync(
  buffer: &[u8],
  level: i32,
  window_bits: i32,
  mem_level: i32,
  strategy: i32,
) -> Result<ZeroCopyBuf, AnyError> {
  check(
    level >= Z_MIN_LEVEL && level <= Z_MAX_LEVEL,
    "Invalid compression level",
  )?;

  check(
    mem_level >= Z_MIN_MEMLEVEL && mem_level <= Z_MAX_MEMLEVEL,
    "Invalid memLevel",
  )?;

  check(
    strategy == Z_FILTERED
      || strategy == Z_HUFFMAN_ONLY
      || strategy == Z_RLE
      || strategy == Z_FIXED
      || strategy == Z_DEFAULT_STRATEGY,
    "Invalid strategy",
  )?;

  unsafe {
    let mut stream: libz_sys::z_stream = libz_sys::z_stream {
      next_in: std::ptr::null_mut(),
      avail_in: 0,
      total_in: 0,
      next_out: std::ptr::null_mut(),
      avail_out: 0,
      total_out: 0,
      msg: std::ptr::null_mut(),
      state: std::ptr::null_mut(),
      zalloc: flate2_libz_helpers::zalloc,
      zfree: flate2_libz_helpers::zfree,
      opaque: 0 as libz_sys::voidpf,
      data_type: 0,
      adler: 0,
      reserved: 0,
  };

    let ret = deflateInit2_(
      &mut stream,
      level,
      Z_DEFLATED,
      window_bits,
      mem_level,
      strategy,
      zlibVersion(),
      std::mem::size_of::<z_stream>() as i32,
    );

    check(ret == Z_OK, "deflateInit2 failed")?;

    let mut out = Vec::with_capacity(buffer.len());
    let mut out_buffer = [0; 1024];

    loop {
      stream.next_out = out_buffer.as_mut_ptr() as *mut _;
      stream.avail_out = out_buffer.len() as u32;

      stream.next_in = buffer.as_ptr() as *mut _;
      stream.avail_in = buffer.len() as u32;

      let ret = deflate(&mut stream, Z_FINISH);
      if ret == Z_STREAM_END {
        out.extend_from_slice(
          &out_buffer[..(out_buffer.len() - stream.avail_out as usize)],
        );
        break;
      }
      
      check(ret == Z_OK, "deflate failed")?;
      out.extend_from_slice(
        &out_buffer[..(out_buffer.len() - stream.avail_out as usize)],
      );
    }

    Ok(out.into())
  }
}

mod flate2_libz_helpers {
  // Workaround for https://github.com/rust-lang/libz-sys/issues/55
  // See https://github.com/rust-lang/flate2-rs/blob/31fb07820345691352aaa64f367c1e482ad9cfdc/src/ffi/c.rs#L60
  use std::os::raw::c_void;
  use std::{
    alloc::{self, Layout},
    ptr,
  };

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
    unsafe {
      // Move our address being free'd back one pointer, read the size we
      // stored in `zalloc`, and then free it using the standard Rust
      // allocator.
      let ptr = (address as *mut usize).offset(-1);
      let size = *ptr;
      let layout = Layout::from_size_align_unchecked(size, ALIGN);
      alloc::dealloc(ptr as *mut u8, layout)
    }
  }
}