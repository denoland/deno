// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use flate2::write::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::write::GzDecoder;
use flate2::write::GzEncoder;
use flate2::write::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

#[derive(Debug)]
struct CompressionResource(RefCell<Inner>);

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

impl Resource for CompressionResource {
  fn name(&self) -> Cow<str> {
    "compression".into()
  }
}

#[op(fast)]
pub fn op_compression_new(
  state: &mut OpState,
  format: u32,
  is_decoder: bool,
) -> Result<u32, AnyError> {
  let w = Vec::new();
  let inner = match (format, is_decoder) {
    (0, true) => Inner::DeflateDecoder(ZlibDecoder::new(w)),
    (0, false) => {
      Inner::DeflateEncoder(ZlibEncoder::new(w, Compression::default()))
    }
    (1, true) => Inner::DeflateRawDecoder(DeflateDecoder::new(w)),
    (1, false) => {
      Inner::DeflateRawEncoder(DeflateEncoder::new(w, Compression::default()))
    }
    (2, true) => Inner::GzDecoder(GzDecoder::new(w)),
    (2, false) => Inner::GzEncoder(GzEncoder::new(w, Compression::default())),
    _ => return Err(type_error("Invalid compression format")),
  };
  let resource = CompressionResource(RefCell::new(inner));
  Ok(state.resource_table.add(resource))
}

#[op]
pub fn op_compression_write(
  state: &mut OpState,
  rid: ResourceId,
  input: &[u8],
) -> Result<ZeroCopyBuf, AnyError> {
  let resource = state.resource_table.get::<CompressionResource>(rid)?;
  let mut inner = resource.0.borrow_mut();
  let out: Vec<u8> = match &mut *inner {
    Inner::DeflateDecoder(d) => {
      d.write_all(input)?;
      d.flush()?;
      d.get_mut().drain(..)
    }
    Inner::DeflateEncoder(d) => {
      d.write_all(input)?;
      d.flush()?;
      d.get_mut().drain(..)
    }
    Inner::DeflateRawDecoder(d) => {
      d.write_all(input)?;
      d.flush()?;
      d.get_mut().drain(..)
    }
    Inner::DeflateRawEncoder(d) => {
      d.write_all(input)?;
      d.flush()?;
      d.get_mut().drain(..)
    }
    Inner::GzDecoder(d) => {
      d.write_all(input)?;
      d.flush()?;
      d.get_mut().drain(..)
    }
    Inner::GzEncoder(d) => {
      d.write_all(input)?;
      d.flush()?;
      d.get_mut().drain(..)
    }
  }
  .collect();
  Ok(out.into())
}

#[op]
pub fn op_compression_finish(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<ZeroCopyBuf, AnyError> {
  let resource = state.resource_table.take::<CompressionResource>(rid)?;
  let resource = Rc::try_unwrap(resource).unwrap();
  let inner = resource.0.into_inner();
  let out: Vec<u8> = match inner {
    Inner::DeflateDecoder(d) => d.finish()?,
    Inner::DeflateEncoder(d) => d.finish()?,
    Inner::DeflateRawDecoder(d) => d.finish()?,
    Inner::DeflateRawEncoder(d) => d.finish()?,
    Inner::GzDecoder(d) => d.finish()?,
    Inner::GzEncoder(d) => d.finish()?,
  };
  Ok(out.into())
}
