use brotli::enc::encode::BrotliEncoderParameter;
use brotli::ffi::compressor::*;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::Resource;
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

// TODO(@littledivy): Not complete and usused currently.
struct BrotliCompressCtx {
  inst: *mut BrotliEncoderState,
}

impl Resource for BrotliCompressCtx {}

impl Drop for BrotliCompressCtx {
  fn drop(&mut self) {
    unsafe { BrotliEncoderDestroyInstance(self.inst) };
  }
}

#[op]
pub fn op_create_brotli_compress(
  state: &mut OpState,
  params: Vec<(u8, i32)>,
) -> u32 {
  let inst =
    unsafe { BrotliEncoderCreateInstance(None, None, std::ptr::null_mut()) };

  for (key, value) in params {
    unsafe {
      BrotliEncoderSetParameter(inst, encoder_param(key), value as u32);
    }
  }

  state.resource_table.add(BrotliCompressCtx { inst })
}

fn encoder_param(param: u8) -> BrotliEncoderParameter {
  // SAFETY: BrotliEncoderParam is valid for 0-255
  unsafe { std::mem::transmute(param as u32) }
}

#[op]
pub fn op_brotli_compress_stream(
  state: &mut OpState,
  rid: u32,
  input: &[u8],
  output: &mut [u8],
) -> Result<usize, AnyError> {
  let ctx = state.resource_table.get::<BrotliCompressCtx>(rid)?;

  unsafe {
    let mut available_in = input.len();
    let mut next_in = input.as_ptr();
    let mut available_out = output.len();
    let mut next_out = output.as_mut_ptr();
    let mut total_out = 0;

    if BrotliEncoderCompressStream(
      ctx.inst,
      BrotliEncoderOperation::BROTLI_OPERATION_PROCESS,
      &mut available_in,
      &mut next_in,
      &mut available_out,
      &mut next_out,
      &mut total_out,
    ) != 1
    {
      return Err(type_error("Failed to compress"));
    }

    // On progress, next_out is advanced and available_out is reduced.
    Ok(output.len() - available_out)
  }
}
