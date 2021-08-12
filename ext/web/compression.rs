use deno_core::error::{bad_resource_id, AnyError};
use deno_core::{OpState, Resource, ResourceId, ZeroCopyBuf};

use flate2::write::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::borrow::Cow;
use std::io::prelude::*;

pub struct GzipCompressorResource(GzEncoder<Vec<u8>>);
impl Resource for GzipCompressorResource {
  fn name(&self) -> Cow<str> {
    "gzipCompressor".into()
  }
}

pub struct GzipDecompressorResource(GzDecoder<Vec<u8>>);
impl Resource for GzipDecompressorResource {
  fn name(&self) -> Cow<str> {
    "gzipDecompressor".into()
  }
}

fn op_compression_create_compressor(
  state: &mut OpState,
  format: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  let rid = match format.as_str() {
    "gzip" => {
      let comp = GzEncoder::new(Vec::new(), Compression::new(8)); // TODO: The only valid value of the CM (Compression Method) field is 8.
      state.resource_table.add(GzipCompressorResource(comp))
    }
    "deflate" => {
      // TODO
    }
    _ => unreachable!(),
  };

  Ok(rid)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressArgs {
  format: String,
  rid: ResourceId,
  data: ZeroCopyBuf,
}

fn op_compression_compress(
  state: &mut OpState,
  args: CompressArgs,
  _: (),
) -> Result<ZeroCopyBuf, AnyError> {
  let data = match args.format.as_str() {
    "gzip" => {
      let compressor = state
        .resource_table
        .get::<GzipCompressorResource>(args.rid)
        .ok_or_else(bad_resource_id)?;
      compressor.0.write_all(args.data.as_ref())?;
    }
    "deflate" => {
      // TODO
    }
    _ => unreachable!(),
  };

  Ok(data) // TODO
}

fn op_compression_compress_finalize(
  state: &mut OpState,
  rid: ResourceId,
  format: String,
) -> Result<ZeroCopyBuf, AnyError> {
  let data = match format.as_str() {
    "gzip" => {
      let compressor = state
        .resource_table
        .get::<GzipCompressorResource>(rid)
        .ok_or_else(bad_resource_id)?;
      compressor.0.finish()?
    }
    "deflate" => {
      // TODO
    }
    _ => unreachable!(),
  };

  Ok(data.into())
}

fn op_compression_create_decompressor(
  state: &mut OpState,
  format: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  let rid = match format.as_str() {
    "gzip" => {
      let comp = GzDecoder::new(Vec::new());
      state.resource_table.add(GzipDecompressorResource(comp))
    }
    "deflate" => {
      // TODO
    }
    _ => unreachable!(),
  };

  Ok(rid)
}

fn op_compression_decompress(
  state: &mut OpState,
  args: CompressArgs,
  _: (),
) -> Result<ZeroCopyBuf, AnyError> {
  let data = match args.format.as_str() {
    "gzip" => {
      let compressor = state
        .resource_table
        .get::<GzipDecompressorResource>(args.rid)
        .ok_or_else(bad_resource_id)?;
      compressor.0.write_all(args.data.as_ref())?;
      // TODO
    }
    "deflate" => {
      // TODO
    }
    _ => unreachable!(),
  };

  Ok(data) // TODO
}

fn op_compression_decompress_finalize(
  state: &mut OpState,
  rid: ResourceId,
  format: String,
) -> Result<ZeroCopyBuf, AnyError> {
  let data = match format.as_str() {
    "gzip" => {
      let compressor = state
        .resource_table
        .get::<GzipDecompressorResource>(rid)
        .ok_or_else(bad_resource_id)?;
      compressor.0.finish()?
    }
    "deflate" => {
      // TODO
    }
    _ => unreachable!(),
  };

  Ok(data.into())
}
