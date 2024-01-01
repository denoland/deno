// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use brotli::enc::encode::BrotliEncoderParameter;
use brotli::ffi::compressor::*;
use brotli::ffi::decompressor::ffi::interface::BrotliDecoderResult;
use brotli::ffi::decompressor::ffi::BrotliDecoderState;
use brotli::ffi::decompressor::*;
use brotli::Decompressor;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ToJsBuffer;
use std::io::Read;

fn encoder_mode(mode: u32) -> Result<BrotliEncoderMode, AnyError> {
  if mode > 6 {
    return Err(type_error("Invalid encoder mode"));
  }
  // SAFETY: mode is a valid discriminant for BrotliEncoderMode
  unsafe { Ok(std::mem::transmute::<u32, BrotliEncoderMode>(mode)) }
}

#[op2(fast)]
#[number]
pub fn op_brotli_compress(
  #[buffer] buffer: &[u8],
  #[buffer] out: &mut [u8],
  #[smi] quality: i32,
  #[smi] lgwin: i32,
  #[smi] mode: u32,
) -> Result<usize, AnyError> {
  let in_buffer = buffer.as_ptr();
  let in_size = buffer.len();
  let out_buffer = out.as_mut_ptr();
  let mut out_size = out.len();

  // SAFETY: in_size and in_buffer, out_size and out_buffer are valid for this call.
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

#[op2(async)]
#[serde]
pub async fn op_brotli_compress_async(
  #[buffer] input: JsBuffer,
  #[smi] quality: i32,
  #[smi] lgwin: i32,
  #[smi] mode: u32,
) -> Result<ToJsBuffer, AnyError> {
  tokio::task::spawn_blocking(move || {
    let in_buffer = input.as_ptr();
    let in_size = input.len();

    let mut out = vec![0u8; max_compressed_size(in_size)];
    let out_buffer = out.as_mut_ptr();
    let mut out_size = out.len();

    // SAFETY: in_size and in_buffer, out_size and out_buffer
    // are valid for this call.
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

impl Resource for BrotliCompressCtx {}

impl Drop for BrotliCompressCtx {
  fn drop(&mut self) {
    // SAFETY: `self.inst` is the current brotli encoder instance.
    // It is not used after the following call.
    unsafe { BrotliEncoderDestroyInstance(self.inst) };
  }
}

#[op2]
#[smi]
pub fn op_create_brotli_compress(
  state: &mut OpState,
  #[serde] params: Vec<(u8, i32)>,
) -> u32 {
  let inst =
    // SAFETY: Creates a brotli encoder instance for default allocators.
    unsafe { BrotliEncoderCreateInstance(None, None, std::ptr::null_mut()) };

  for (key, value) in params {
    // SAFETY: `key` can range from 0-255.
    // Any valid u32 can be used for the `value`.
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

#[op2(fast)]
#[number]
pub fn op_brotli_compress_stream(
  state: &mut OpState,
  #[smi] rid: u32,
  #[buffer] input: &[u8],
  #[buffer] output: &mut [u8],
) -> Result<usize, AnyError> {
  let ctx = state.resource_table.get::<BrotliCompressCtx>(rid)?;

  // SAFETY: TODO(littledivy)
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

#[op2(fast)]
#[number]
pub fn op_brotli_compress_stream_end(
  state: &mut OpState,
  #[smi] rid: u32,
  #[buffer] output: &mut [u8],
) -> Result<usize, AnyError> {
  let ctx = state.resource_table.take::<BrotliCompressCtx>(rid)?;

  // SAFETY: TODO(littledivy)
  unsafe {
    let mut available_out = output.len();
    let mut next_out = output.as_mut_ptr();
    let mut total_out = 0;

    if BrotliEncoderCompressStream(
      ctx.inst,
      BrotliEncoderOperation::BROTLI_OPERATION_FINISH,
      &mut 0,
      std::ptr::null_mut(),
      &mut available_out,
      &mut next_out,
      &mut total_out,
    ) != 1
    {
      return Err(type_error("Failed to compress"));
    }

    // On finish, next_out is advanced and available_out is reduced.
    Ok(output.len() - available_out)
  }
}

fn brotli_decompress(buffer: &[u8]) -> Result<ToJsBuffer, AnyError> {
  let mut output = Vec::with_capacity(4096);
  let mut decompressor = Decompressor::new(buffer, buffer.len());
  decompressor.read_to_end(&mut output)?;
  Ok(output.into())
}

#[op2]
#[serde]
pub fn op_brotli_decompress(
  #[buffer] buffer: &[u8],
) -> Result<ToJsBuffer, AnyError> {
  brotli_decompress(buffer)
}

#[op2(async)]
#[serde]
pub async fn op_brotli_decompress_async(
  #[buffer] buffer: JsBuffer,
) -> Result<ToJsBuffer, AnyError> {
  tokio::task::spawn_blocking(move || brotli_decompress(&buffer)).await?
}

struct BrotliDecompressCtx {
  inst: *mut BrotliDecoderState,
}

impl Resource for BrotliDecompressCtx {}

impl Drop for BrotliDecompressCtx {
  fn drop(&mut self) {
    // SAFETY: TODO(littledivy)
    unsafe { CBrotliDecoderDestroyInstance(self.inst) };
  }
}

#[op2(fast)]
#[smi]
pub fn op_create_brotli_decompress(state: &mut OpState) -> u32 {
  let inst =
    // SAFETY: TODO(littledivy)
    unsafe { CBrotliDecoderCreateInstance(None, None, std::ptr::null_mut()) };
  state.resource_table.add(BrotliDecompressCtx { inst })
}

#[op2(fast)]
#[number]
pub fn op_brotli_decompress_stream(
  state: &mut OpState,
  #[smi] rid: u32,
  #[buffer] input: &[u8],
  #[buffer] output: &mut [u8],
) -> Result<usize, AnyError> {
  let ctx = state.resource_table.get::<BrotliDecompressCtx>(rid)?;

  // SAFETY: TODO(littledivy)
  unsafe {
    let mut available_in = input.len();
    let mut next_in = input.as_ptr();
    let mut available_out = output.len();
    let mut next_out = output.as_mut_ptr();
    let mut total_out = 0;

    if matches!(
      CBrotliDecoderDecompressStream(
        ctx.inst,
        &mut available_in,
        &mut next_in,
        &mut available_out,
        &mut next_out,
        &mut total_out,
      ),
      BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR
    ) {
      return Err(type_error("Failed to decompress"));
    }

    // On progress, next_out is advanced and available_out is reduced.
    Ok(output.len() - available_out)
  }
}

#[op2(fast)]
#[number]
pub fn op_brotli_decompress_stream_end(
  state: &mut OpState,
  #[smi] rid: u32,
  #[buffer] output: &mut [u8],
) -> Result<usize, AnyError> {
  let ctx = state.resource_table.get::<BrotliDecompressCtx>(rid)?;

  // SAFETY: TODO(littledivy)
  unsafe {
    let mut available_out = output.len();
    let mut next_out = output.as_mut_ptr();
    let mut total_out = 0;

    if matches!(
      CBrotliDecoderDecompressStream(
        ctx.inst,
        &mut 0,
        std::ptr::null_mut(),
        &mut available_out,
        &mut next_out,
        &mut total_out,
      ),
      BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR
    ) {
      return Err(type_error("Failed to decompress"));
    }

    // On finish, next_out is advanced and available_out is reduced.
    Ok(output.len() - available_out)
  }
}
