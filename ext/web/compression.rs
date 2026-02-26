// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::io::Write;

use brotli::CompressorWriter as BrotliEncoder;
use brotli::DecompressorWriter as BrotliDecoder;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use flate2::Compression;
use flate2::write::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::write::GzDecoder;
use flate2::write::GzEncoder;
use flate2::write::ZlibDecoder;
use flate2::write::ZlibEncoder;

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
  BrotliEncoder(Box<BrotliEncoder<Vec<u8>>>),
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
      // quality level 6 and lgwin 22 are based on google's nginx default values
      // https://github.com/google/ngx_brotli#brotli_comp_level
      // 4096 is the default buffer size used by brotli crate
      Inner::BrotliEncoder(Box::new(BrotliEncoder::new(w, 4096, 6, 22)))
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
    Inner::DeflateDecoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::DeflateEncoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::DeflateRawDecoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::DeflateRawEncoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::GzDecoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::GzEncoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::BrotliDecoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
    }
    Inner::BrotliEncoder(d) => {
      d.write_all(input).map_err(CompressionError::IoTypeError)?;
      d.flush().map_err(CompressionError::Io)?;
      d.get_mut().drain(..)
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
    Inner::BrotliEncoder(d) => Ok(d.into_inner()),
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
