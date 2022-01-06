use deno_core::error::AnyError;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use async_compression::tokio::write::DeflateDecoder;
use async_compression::tokio::write::DeflateEncoder;
use async_compression::tokio::write::GzipDecoder;
use async_compression::tokio::write::GzipEncoder;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::DuplexStream;

pub struct GzipCompressorResource {
  encoder: RefCell<GzipEncoder<DuplexStream>>,
  tx: RefCell<DuplexStream>,
}
impl Resource for GzipCompressorResource {
  fn name(&self) -> Cow<str> {
    "gzipCompressor".into()
  }
}

pub struct GzipDecompressorResource {
  decoder: RefCell<GzipDecoder<DuplexStream>>,
  tx: RefCell<DuplexStream>,
}
impl Resource for GzipDecompressorResource {
  fn name(&self) -> Cow<str> {
    "gzipDecompressor".into()
  }
}

pub struct DeflateCompressorResource {
  encoder: RefCell<DeflateEncoder<DuplexStream>>,
  tx: RefCell<DuplexStream>,
}
impl Resource for DeflateCompressorResource {
  fn name(&self) -> Cow<str> {
    "deflateCompressor".into()
  }
}

pub struct DeflateDecompressorResource {
  decoder: RefCell<DeflateDecoder<DuplexStream>>,
  tx: RefCell<DuplexStream>,
}
impl Resource for DeflateDecompressorResource {
  fn name(&self) -> Cow<str> {
    "deflateDecompressor".into()
  }
}

pub fn op_compression_create_compressor(
  state: &mut OpState,
  format: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  let rid = match format.as_str() {
    "gzip" => {
      let (rx, tx) = tokio::io::duplex(65536);
      let encoder =
        GzipEncoder::with_quality(rx, async_compression::Level::Precise(8));
      state.resource_table.add(GzipCompressorResource {
        encoder: RefCell::new(encoder),
        tx: RefCell::new(tx),
      })
    }
    "deflate" => {
      let (rx, tx) = tokio::io::duplex(65536);
      let encoder =
        DeflateEncoder::with_quality(rx, async_compression::Level::Precise(8));
      state.resource_table.add(DeflateCompressorResource {
        encoder: RefCell::new(encoder),
        tx: RefCell::new(tx),
      })
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

pub async fn op_compression_compress(
  state: Rc<RefCell<OpState>>,
  args: CompressArgs,
  _: (),
) -> Result<ZeroCopyBuf, AnyError> {
  let data = match args.format.as_str() {
    "gzip" => {
      let compressor = state
        .borrow()
        .resource_table
        .get::<GzipCompressorResource>(args.rid)?;
      compressor
        .encoder
        .borrow_mut()
        .write_all(args.data.as_ref())
        .await?;
      let mut data = vec![];
      let n = compressor.tx.borrow_mut().read(data.as_mut_slice()).await?;
      data[0..n].to_vec()
    }
    "deflate" => {
      let compressor = state
        .borrow()
        .resource_table
        .get::<DeflateCompressorResource>(args.rid)?;
      compressor
        .encoder
        .borrow_mut()
        .write_all(args.data.as_ref())
        .await?;
      let mut data = vec![];
      let n = compressor.tx.borrow_mut().read(data.as_mut_slice()).await?;
      data[0..n].to_vec()
    }
    _ => unreachable!(),
  };

  Ok(data.into())
}

pub async fn op_compression_compress_finalize(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  format: String,
) -> Result<ZeroCopyBuf, AnyError> {
  let data = match format.as_str() {
    "gzip" => {
      let compressor = state
        .borrow()
        .resource_table
        .get::<GzipCompressorResource>(rid)?;
      compressor.encoder.borrow_mut().shutdown().await?;
      let mut data = vec![];
      let n = compressor.tx.borrow_mut().read_to_end(&mut data).await?;
      data[0..n].to_vec()
    }
    "deflate" => {
      let compressor = state
        .borrow()
        .resource_table
        .get::<DeflateCompressorResource>(rid)?;
      compressor.encoder.borrow_mut().shutdown().await?;
      let mut data = vec![];
      let n = compressor.tx.borrow_mut().read_to_end(&mut data).await?;
      data[0..n].to_vec()
    }
    _ => unreachable!(),
  };

  Ok(data.into())
}

pub fn op_compression_create_decompressor(
  state: &mut OpState,
  format: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  let rid = match format.as_str() {
    "gzip" => {
      let (rx, tx) = tokio::io::duplex(65536);
      let decoder = GzipDecoder::new(rx);
      state.resource_table.add(GzipDecompressorResource {
        decoder: RefCell::new(decoder),
        tx: RefCell::new(tx),
      })
    }
    "deflate" => {
      let (rx, tx) = tokio::io::duplex(65536);
      let decoder = DeflateDecoder::new(rx);
      state.resource_table.add(DeflateDecompressorResource {
        decoder: RefCell::new(decoder),
        tx: RefCell::new(tx),
      })
    }
    _ => unreachable!(),
  };

  Ok(rid)
}

pub async fn op_compression_decompress(
  state: Rc<RefCell<OpState>>,
  args: CompressArgs,
  _: (),
) -> Result<ZeroCopyBuf, AnyError> {
  let data = match args.format.as_str() {
    "gzip" => {
      let compressor = state
        .borrow()
        .resource_table
        .get::<GzipDecompressorResource>(args.rid)?;
      compressor
        .decoder
        .borrow_mut()
        .write_all(args.data.as_ref())
        .await?;
      let mut data = vec![];
      let n = compressor.tx.borrow_mut().read(data.as_mut_slice()).await?;
      data[0..n].to_vec()
    }
    "deflate" => {
      let compressor = state
        .borrow()
        .resource_table
        .get::<DeflateDecompressorResource>(args.rid)?;
      compressor
        .decoder
        .borrow_mut()
        .write_all(args.data.as_ref())
        .await?;
      let mut data = vec![];
      let n = compressor.tx.borrow_mut().read(data.as_mut_slice()).await?;
      data[0..n].to_vec()
    }
    _ => unreachable!(),
  };

  Ok(data.into())
}

pub async fn op_compression_decompress_finalize(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  format: String,
) -> Result<ZeroCopyBuf, AnyError> {
  let data = match format.as_str() {
    "gzip" => {
      let compressor = state
        .borrow()
        .resource_table
        .get::<GzipDecompressorResource>(rid)?;
      compressor.decoder.borrow_mut().shutdown().await?;
      let mut data = vec![];
      let n = compressor.tx.borrow_mut().read_to_end(&mut data).await?;
      data[0..n].to_vec()
    }
    "deflate" => {
      let compressor = state
        .borrow()
        .resource_table
        .get::<DeflateDecompressorResource>(rid)?;
      compressor.decoder.borrow_mut().shutdown().await?;
      let mut data = vec![];
      let n = compressor.tx.borrow_mut().read_to_end(&mut data).await?;
      data[0..n].to_vec()
    }
    _ => unreachable!(),
  };

  Ok(data.into())
}
