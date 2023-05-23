use brotli::ffi::compressor::*;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ZeroCopyBuf;

fn encoder_mode(mode: u32) -> Result<BrotliEncoderMode, AnyError> {
  if mode > 6 {
    return Err(type_error("Invalid encoder mode"));
  }
  // SAFETY: mode is a valid discriminant for BrotliEncoderMode
  unsafe { Ok(std::mem::transmute::<u32, BrotliEncoderMode>(mode)) }
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
