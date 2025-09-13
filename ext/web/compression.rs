// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::io::Write;

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
#[derive(Debug)]
enum Inner {
  DeflateDecoder(ZlibDecoder<Vec<u8>>),
  DeflateEncoder(ZlibEncoder<Vec<u8>>),
  DeflateRawDecoder(DeflateDecoder<Vec<u8>>),
  DeflateRawEncoder(DeflateEncoder<Vec<u8>>),
  GzDecoder(GzDecoder<Vec<u8>>),
  GzEncoder(GzEncoder<Vec<u8>>),
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
    _ => return Err(CompressionError::UnsupportedFormat),
  };
  Ok(CompressionResource(RefCell::new(Some(inner))))
}

#[op2]
#[buffer]
pub fn op_compression_write(
  #[cppgc] resource: &CompressionResource,
  #[anybuffer] input: &[u8],
) -> Result<Vec<u8>, CompressionError> {
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
  }
  .collect();
  Ok(out)
}

#[op2]
#[buffer]
pub fn op_compression_finish(
  #[cppgc] resource: &CompressionResource,
  report_errors: bool,
) -> Result<Vec<u8>, CompressionError> {
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
  };
  match out {
    Err(err) => {
      if report_errors {
        Err(err)
      } else {
        Ok(Vec::with_capacity(0))
      }
    }
    Ok(out) => Ok(out),
  }
}
