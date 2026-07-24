// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::io::Write;

use brotli::DecompressorWriter as BrotliDecoder;
use brotli::enc::encode::BrotliEncoderOperation;
use brotli::enc::encode::BrotliEncoderParameter;
use brotli::enc::encode::BrotliEncoderStateStruct;
use brotli::writer::StandardAlloc;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use flate2::Compression;
use flate2::write::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::write::GzDecoder;
use flate2::write::GzEncoder;
use flate2::write::ZlibDecoder;
use flate2::write::ZlibEncoder;

/// Like `write_all`, but stops gracefully when `write` returns `Ok(0)`.
/// This is needed for decoders: when the compressed stream ends (e.g.
/// zlib footer reached), `write()` returns `Ok(0)` for any remaining
/// input bytes. `write_all()` would treat this as a `WriteZero` error.
fn write_all_allowing_partial(
  w: &mut impl Write,
  mut buf: &[u8],
) -> std::io::Result<()> {
  while !buf.is_empty() {
    match w.write(buf) {
      Ok(0) => break,
      Ok(n) => buf = &buf[n..],
      Err(e) => return Err(e),
    }
  }
  Ok(())
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CompressionError {
  #[class(type)]
  #[error("Unsupported format")]
  UnsupportedFormat,
  #[class(type)]
  #[error("resource is closed")]
  ResourceClosed,
  #[class(type)]
  #[error(transparent)]
  IoTypeError(std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  Io(std::io::Error),
}

const BROTLI_COMPRESSION_QUALITY: u32 = 6;
const BROTLI_COMPRESSION_LGWIN: u32 = 22;

// Quality level 6 is based on google's nginx default value for on-the-fly
// compression:
// https://github.com/google/ngx_brotli#brotli_comp_level
// lgwin 22 is equivalent to brotli window size of (2**22)-16 bytes (~4MB).
fn new_brotli_encoder() -> BrotliEncoderStateStruct<StandardAlloc> {
  let mut stm = BrotliEncoderStateStruct::new(StandardAlloc::default());
  stm.set_parameter(
    BrotliEncoderParameter::BROTLI_PARAM_QUALITY,
    BROTLI_COMPRESSION_QUALITY,
  );
  stm.set_parameter(
    BrotliEncoderParameter::BROTLI_PARAM_LGWIN,
    BROTLI_COMPRESSION_LGWIN,
  );
  stm
}

fn max_brotli_compressed_size(input_size: usize) -> usize {
  if input_size == 0 {
    return 2;
  }

  // [window bits / empty metadata] + N * [uncompressed] + [last empty]
  let num_large_blocks = input_size >> 14;
  let overhead = 2 + (4 * num_large_blocks) + 3 + 1;
  let result = input_size + overhead;

  if result < input_size { 0 } else { result }
}

struct RawBrotliEncoder {
  stm: BrotliEncoderStateStruct<StandardAlloc>,
}

impl RawBrotliEncoder {
  fn new() -> Self {
    Self {
      stm: new_brotli_encoder(),
    }
  }

  fn compress(
    &mut self,
    input: &[u8],
    operation: BrotliEncoderOperation,
  ) -> Result<Vec<u8>, CompressionError> {
    let mut input_offset = 0;
    let mut available_in = input.len();
    let mut output = vec![0; max_brotli_compressed_size(input.len()).max(1024)];
    let mut output_offset = 0;
    let mut total_out = Some(0);

    loop {
      let mut available_out = output.len() - output_offset;
      let ok = self.stm.compress_stream(
        operation,
        &mut available_in,
        input,
        &mut input_offset,
        &mut available_out,
        &mut output,
        &mut output_offset,
        &mut total_out,
        &mut |_, _, _, _| (),
      );

      if !ok {
        return Err(CompressionError::IoTypeError(std::io::Error::new(
          std::io::ErrorKind::InvalidData,
          "brotli compression failed",
        )));
      }

      let done = match operation {
        BrotliEncoderOperation::BROTLI_OPERATION_FINISH => {
          self.stm.is_finished()
        }
        _ => available_in == 0 && !self.stm.has_more_output(),
      };

      if done {
        output.truncate(output_offset);
        return Ok(output);
      }

      if output_offset == output.len() {
        output.resize(output.len() + 1024, 0);
      }
    }
  }

  fn write(&mut self, input: &[u8]) -> Result<Vec<u8>, CompressionError> {
    self.compress(input, BrotliEncoderOperation::BROTLI_OPERATION_FLUSH)
  }

  fn finish(mut self) -> Result<Vec<u8>, CompressionError> {
    self.compress(&[], BrotliEncoderOperation::BROTLI_OPERATION_FINISH)
  }
}

#[derive(Debug)]
struct CompressionResource(RefCell<Option<Inner>>);

// SAFETY: we're sure `CompressionResource` can be GCed
unsafe impl deno_core::GarbageCollected for CompressionResource {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CompressionResource"
  }
}

/// https://wicg.github.io/compression/#supported-formats
enum Inner {
  DeflateDecoder(ZlibDecoder<Vec<u8>>),
  DeflateEncoder(ZlibEncoder<Vec<u8>>),
  DeflateRawDecoder(DeflateDecoder<Vec<u8>>),
  DeflateRawEncoder(DeflateEncoder<Vec<u8>>),
  GzDecoder(GzDecoder<Vec<u8>>),
  GzEncoder(GzEncoder<Vec<u8>>),
  BrotliDecoder(Box<BrotliDecoder<Vec<u8>>>),
  BrotliEncoder(Box<RawBrotliEncoder>),
}

impl std::fmt::Debug for Inner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Inner::DeflateDecoder(_) => write!(f, "DeflateDecoder"),
      Inner::DeflateEncoder(_) => write!(f, "DeflateEncoder"),
      Inner::DeflateRawDecoder(_) => write!(f, "DeflateRawDecoder"),
      Inner::DeflateRawEncoder(_) => write!(f, "DeflateRawEncoder"),
      Inner::GzDecoder(_) => write!(f, "GzDecoder"),
      Inner::GzEncoder(_) => write!(f, "GzEncoder"),
      Inner::BrotliDecoder(_) => write!(f, "BrotliDecoder"),
      Inner::BrotliEncoder(_) => write!(f, "BrotliEncoder"),
    }
  }
}

#[op2]
#[cppgc]
pub fn op_compression_new(
  #[string] format: &str,
  is_decoder: bool,
) -> Result<CompressionResource, CompressionError> {
  let w = Vec::new();
  let inner = match (format, is_decoder) {
    ("deflate", true) => Inner::DeflateDecoder(ZlibDecoder::new(w)),
    ("deflate", false) => {
      Inner::DeflateEncoder(ZlibEncoder::new(w, Compression::default()))
    }
    ("deflate-raw", true) => Inner::DeflateRawDecoder(DeflateDecoder::new(w)),
    ("deflate-raw", false) => {
      Inner::DeflateRawEncoder(DeflateEncoder::new(w, Compression::default()))
    }
    ("gzip", true) => Inner::GzDecoder(GzDecoder::new(w)),
    ("gzip", false) => {
      Inner::GzEncoder(GzEncoder::new(w, Compression::default()))
    }
    ("brotli", true) => {
      // 4096 is the default buffer size used by brotli crate
      Inner::BrotliDecoder(Box::new(BrotliDecoder::new(w, 4096)))
    }
    ("brotli", false) => {
      drop(w);
      Inner::BrotliEncoder(Box::new(RawBrotliEncoder::new()))
    }
    _ => return Err(CompressionError::UnsupportedFormat),
  };
  Ok(CompressionResource(RefCell::new(Some(inner))))
}

#[op2]
pub fn op_compression_write(
  #[cppgc] resource: &CompressionResource,
  #[anybuffer] input: &[u8],
) -> Result<Uint8Array, CompressionError> {
  let mut inner = resource.0.borrow_mut();
  let inner = inner.as_mut().ok_or(CompressionError::ResourceClosed)?;
  let out: Vec<u8> = match &mut *inner {
    // Decoders: use write_all_allowing_partial instead of write_all.
    // When the compressed stream ends (e.g. zlib footer reached) before
    // all input is consumed (trailing bytes), write() returns Ok(0).
    // write_all treats this as an error, but for decoders it's valid.
    Inner::DeflateDecoder(d) => {
      write_all_allowing_partial(d, input)
        .map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::DeflateEncoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::DeflateRawDecoder(d) => {
      write_all_allowing_partial(d, input)
        .map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::DeflateRawEncoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::GzDecoder(d) => {
      write_all_allowing_partial(d, input)
        .map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::GzEncoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::BrotliDecoder(d) => {
      write_all_allowing_partial(d, input)
        .map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::BrotliEncoder(d) => {
      return d.write(input).map(Into::into);
    }
  }
  .collect();
  Ok(out.into())
}

#[op2]
pub fn op_compression_finish(
  #[cppgc] resource: &CompressionResource,
  report_errors: bool,
) -> Result<Uint8Array, CompressionError> {
  let inner = resource
    .0
    .borrow_mut()
    .take()
    .ok_or(CompressionError::ResourceClosed)?;
  let out = match inner {
    Inner::DeflateDecoder(d) => {
      d.finish().map_err(CompressionError::IoTypeError)
    }
    Inner::DeflateEncoder(d) => {
      d.finish().map_err(CompressionError::IoTypeError)
    }
    Inner::DeflateRawDecoder(d) => {
      d.finish().map_err(CompressionError::IoTypeError)
    }
    Inner::DeflateRawEncoder(d) => {
      d.finish().map_err(CompressionError::IoTypeError)
    }
    Inner::GzDecoder(d) => d.finish().map_err(CompressionError::IoTypeError),
    Inner::GzEncoder(d) => d.finish().map_err(CompressionError::IoTypeError),
    Inner::BrotliDecoder(d) => d.into_inner().map_err(|_| {
      CompressionError::IoTypeError(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "brotli decompression failed",
      ))
    }),
    Inner::BrotliEncoder(d) => d.finish(),
  };
  match out {
    Err(err) => {
      if report_errors {
        Err(err)
      } else {
        Ok(Vec::with_capacity(0).into())
      }
    }
    Ok(out) => Ok(out.into()),
  }
}

#[cfg(test)]
mod tests {
  use std::io::Read;

  use super::*;

  fn decompress_brotli(input: &[u8]) -> Vec<u8> {
    let mut decoder = brotli::Decompressor::new(input, 4096);
    let mut output = vec![];
    decoder.read_to_end(&mut output).unwrap();
    output
  }

  #[test]
  fn raw_brotli_encoder_flushes_multiple_chunks() {
    let mut encoder = RawBrotliEncoder::new();
    let mut compressed = vec![];
    compressed.extend(encoder.write(b"hello ").unwrap());
    compressed.extend(encoder.write(b"world").unwrap());
    compressed.extend(encoder.finish().unwrap());

    assert_eq!(decompress_brotli(&compressed), b"hello world");
  }

  #[test]
  fn raw_brotli_encoder_handles_empty_input() {
    let mut encoder = RawBrotliEncoder::new();
    let mut compressed = encoder.write(&[]).unwrap();
    compressed.extend(encoder.finish().unwrap());

    assert_eq!(decompress_brotli(&compressed), b"");
  }
}
