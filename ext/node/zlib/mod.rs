use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use libz_sys::*;

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

#[op]
pub fn op_zlib_deflate_sync(
  buffer: &[u8],
  level: i32,
  window_bits: i32,
  mem_level: i32,
  strategy: i32,
) -> Result<(), AnyError> {
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
    let mut stream = std::mem::MaybeUninit::<z_stream>::uninit();

    let ret = deflateInit2_(
      stream.as_mut_ptr(),
      level,
      Z_DEFLATED,
      window_bits,
      mem_level,
      strategy,
      zlibVersion(),
      std::mem::size_of::<z_stream>() as i32,
    );

    let mut stream = stream.assume_init();

    check(ret == Z_OK, "deflateInit2 failed")?;

    let mut out = Vec::new();
    let mut out_buffer = [0u8; 1024];

    loop {
      stream.next_out = out_buffer.as_mut_ptr() as *mut _;
      stream.avail_out = out_buffer.len() as u32;

      let ret = deflate(&mut stream, Z_FINISH);

      if ret == Z_STREAM_END {
        break;
      }

      check(ret == Z_OK, "deflate failed")?;

      out.extend_from_slice(
        &out_buffer[..(out_buffer.len() - stream.avail_out as usize)],
      );
    }

    deflateEnd(&mut stream);
  }

  Ok(())
}
