use brotli::ffi::compressor::*;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ZeroCopyBuf;
use std::ffi::c_void;
use std::alloc;

fn encoder_mode(mode: u32) -> Result<BrotliEncoderMode, AnyError> {
  if mode > 6 {
    return Err(type_error("Invalid encoder mode"));
  }
  // SAFETY: mode is a valid discriminant for BrotliEncoderMode
  unsafe { Ok(std::mem::transmute::<u32, BrotliEncoderMode>(mode)) }
}

// convert back and forth from brotli's c_void and std::ffi::c_void
fn cvt(ptr: *mut brotli::ffi::broccoli::c_void) -> *mut c_void {
  unsafe { std::mem::transmute(ptr) }
}

#[op]
pub fn op_brotli_compress(
  buffer: &[u8],
  out: &mut [u8],
  quality: i32,
  lgwin: i32,
  mode: u32,
) -> Result<usize, AnyError> {
  let in_buffer = buffer.as_ptr();
  let in_size = buffer.len();
  let out_buffer = out.as_mut_ptr();
  let mut out_size = out.len();

  if unsafe {
    BrotliEncoderCompress(
      quality,
      lgwin,
      encoder_mode(mode)?,
      in_size,
      in_buffer,
      &mut out_size as *mut usize,
      out_buffer,
    )
  } != 1
  {
    return Err(type_error("Failed to compress"));
  }

  Ok(out_size)
}

fn max_compressed_size(input_size: usize) -> usize {
  if input_size == 0 {
    return 2;
  }

  // [window bits / empty metadata] + N * [uncompressed] + [last empty]
  let num_large_blocks = input_size >> 14;
  let overhead = 2 + (4 * num_large_blocks) + 3 + 1;
  let result = input_size + overhead;

  if result < input_size {
    0
  } else {
    result
  }
}

#[op]
pub async fn op_brotli_compress_async(
  input: ZeroCopyBuf,
  quality: i32,
  lgwin: i32,
  mode: u32,
) -> Result<ZeroCopyBuf, AnyError> {
  tokio::task::spawn_blocking(move || {
    let in_buffer = input.as_ptr();
    let in_size = input.len();

    let mut out = vec![0u8; max_compressed_size(in_size)];
    let out_buffer = out.as_mut_ptr();
    let mut out_size = out.len();

    if unsafe {
      BrotliEncoderCompress(
        quality,
        lgwin,
        encoder_mode(mode)?,
        in_size,
        in_buffer,
        &mut out_size as *mut usize,
        out_buffer,
      )
    } != 1
    {
      return Err(type_error("Failed to compress"));
    }

    out.truncate(out_size);
    Ok(out.into())
  })
  .await?
}

struct BrotliCompressCtx {
  inst: *mut BrotliEncoderState,
}

impl Drop for BrotliCompressCtx {
  fn drop(&mut self) {
    unsafe { BrotliEncoderDestroyInstance(self.inst) };
  }
}

const ALIGN: usize = std::mem::align_of::<usize>();

fn align_up(size: usize, align: usize) -> usize {
  (size + align - 1) & !(align - 1)
}

extern "C" fn brotli_alloc(
  _: *mut brotli::ffi::broccoli::c_void,
  size: usize,
) -> *mut brotli::ffi::broccoli::c_void {
  fn cvt(ptr: *mut c_void) -> *mut brotli::ffi::broccoli::c_void {
    // SAFETY: brotli's c_void is repr(u8)
    unsafe { std::mem::transmute(ptr) }
  }

  let size =
    match align_up(size, ALIGN).checked_add(std::mem::size_of::<usize>()) {
      Some(size) => size,
      None => return cvt(std::ptr::null_mut()),
    };

  let layout = match std::alloc::Layout::from_size_align(size, ALIGN) {
    Ok(layout) => layout,
    Err(_) => return cvt(std::ptr::null_mut()),
  };

  // SAFETY: `layout` has non-zero size, guaranteed to be a sentinel address
  // or a null pointer.
  unsafe {
    // Allocate the data, and if successful store the size we allocated
    // at the beginning and then return an offset pointer.
    let ptr = alloc::alloc(layout) as *mut usize;
    if ptr.is_null() {
      return cvt(ptr as *mut c_void);
    }
    *ptr = size;
    cvt(ptr.add(1) as *mut c_void)
  }
}

extern "C" fn brotli_free(
  _: *mut brotli::ffi::broccoli::c_void,
  ptr: *mut brotli::ffi::broccoli::c_void,
) {
  // SAFETY: `ptr` is a valid pointer to a size, which we can read and then
  // deallocate.
  unsafe {
    if ptr.is_null() {
      return;
    }
    let ptr: *mut usize = std::mem::transmute(ptr);
    let size = *ptr;
    let layout = std::alloc::Layout::from_size_align_unchecked(size, ALIGN);
    alloc::dealloc(ptr as *mut u8, layout);
  }
}

#[op]
pub fn op_create_brotli_compress() {
  let inst = unsafe {
    BrotliEncoderCreateInstance(
      Some(brotli_alloc),
      Some(brotli_free),
      std::ptr::null_mut(),
    )
  };
}
