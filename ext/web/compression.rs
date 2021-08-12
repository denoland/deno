use deno_core::{ResourceId, OpState, Resource, ZeroCopyBuf};
use deno_core::error::{AnyError, bad_resource_id};

use std::io::prelude::*;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::borrow::Cow;

pub struct GzipCompressorResource(GzEncoder<Vec<u8>>);
impl Resource for GzipCompressorResource {
  fn name(&self) -> Cow<str> {
    "gzipCompressor".into()
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
    },
    "deflate" => {
      // TODO
    },
    _ => unreachable!(),
  };

  Ok(rid) // TODO
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
    },
    "deflate" => {
      // TODO
    },
    _ => unreachable!(),
  };

  Ok(data) // TODO
}
