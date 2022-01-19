use async_compression::tokio::write::DeflateDecoder;
use async_compression::tokio::write::DeflateEncoder;
use async_compression::tokio::write::GzipDecoder;
use async_compression::tokio::write::GzipEncoder;
use deno_core::error::AnyError;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::DuplexStream;

pub struct CompressResource {
  data: RefCell<flate2::Compress>,
}

impl Resource for CompressResource {
  fn name(&self) -> Cow<str> {
    "compress".into()
  }
}

pub struct DecompressResource {
  data: RefCell<flate2::Decompress>,
}

impl Resource for DecompressResource {
  fn name(&self) -> Cow<str> {
    "decompress".into()
  }
}

pub fn op_compression_compress_new(
  state: &mut OpState,
  _format: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  let level = flate2::Compression::fast();
  let data = RefCell::new(flate2::Compress::new(level, true));
  Ok(state.resource_table.add(CompressResource { data }))
}

pub fn op_compression_decompress_new(
  state: &mut OpState,
  format: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  let zlib_header = match format.as_str() {
    "gzip" => false,
    "deflate" => true,
    _ => unreachable!(),
  };
  let data = RefCell::new(flate2::Decompress::new(zlib_header));
  Ok(state.resource_table.add(DecompressResource { data }))
}

pub fn op_compression_compress_total_in_out(
  state: &mut OpState,
  rid: ResourceId,
  _: (),
) -> Result<(u64, u64), AnyError> {
  let resource = state.resource_table.get::<CompressResource>(rid)?;
  let d = resource.data.borrow();
  Ok((d.total_in(), d.total_out()))
}

pub fn op_compression_decompress_total_in_out(
  state: &mut OpState,
  rid: ResourceId,
  _: (),
) -> Result<(u64, u64), AnyError> {
  let resource = state.resource_table.get::<DecompressResource>(rid)?;
  let d = resource.data.borrow();
  Ok((d.total_in(), d.total_out()))
}

pub fn op_compression_compress(
  state: &mut OpState,
  rid: ResourceId,
  input_output_flush: (ZeroCopyBuf, ZeroCopyBuf, i32),
) -> Result<i32, AnyError> {
  let (input, mut output, flush) = input_output_flush;

  let resource = state.resource_table.get::<CompressResource>(rid)?;
  let mut data = resource.data.borrow_mut();

  let flush = match flush {
    0 => flate2::FlushCompress::None,
    1 => flate2::FlushCompress::Sync,
    2 => flate2::FlushCompress::Partial,
    3 => flate2::FlushCompress::Full,
    4 => flate2::FlushCompress::Finish,
    _ => unreachable!(),
  };

  let r = data.compress(&input, &mut output, flush)?;

  Ok(match r {
    flate2::Status::Ok => 0,
    flate2::Status::BufError => 1,
    flate2::Status::StreamEnd => 2,
  })
}

pub fn op_compression_decompress(
  state: &mut OpState,
  rid: ResourceId,
  input_output_flush: (ZeroCopyBuf, ZeroCopyBuf, i32),
) -> Result<i32, AnyError> {
  let (input, mut output, flush) = input_output_flush;

  let resource = state.resource_table.get::<DecompressResource>(rid)?;
  let mut data = resource.data.borrow_mut();

  let flush = match flush {
    0 => flate2::FlushDecompress::None,
    1 => flate2::FlushDecompress::Sync,
    2 => flate2::FlushDecompress::Finish,
    _ => unreachable!(),
  };

  let r = data.decompress(&input, &mut output, flush)?;

  Ok(match r {
    flate2::Status::Ok => 0,
    flate2::Status::BufError => 1,
    flate2::Status::StreamEnd => 2,
  })
}

pub struct GzipCompressorResource {
  encoder: AsyncRefCell<GzipEncoder<DuplexStream>>,
  tx: AsyncRefCell<DuplexStream>,
}
impl Resource for GzipCompressorResource {
  fn name(&self) -> Cow<str> {
    "gzipCompressor".into()
  }

  fn read(self: Rc<Self>, mut buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.tx);
      let mut tx = resource.borrow_mut().await;
      let n = tx.read(&mut buf).await?;
      Ok(n)
    })
  }

  fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.encoder);
      let mut encoder = resource.borrow_mut().await;
      let n = encoder.write(&buf).await?;
      Ok(n)
    })
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.encoder);
      let mut encoder = resource.borrow_mut().await;
      encoder.shutdown().await?;
      Ok(())
    })
  }
}

pub struct GzipDecompressorResource {
  decoder: AsyncRefCell<GzipDecoder<DuplexStream>>,
  tx: AsyncRefCell<DuplexStream>,
}
impl Resource for GzipDecompressorResource {
  fn name(&self) -> Cow<str> {
    "gzipDecompressor".into()
  }

  fn read(self: Rc<Self>, mut buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.tx);
      let mut tx = resource.borrow_mut().await;
      let n = tx.read(&mut buf).await?;
      Ok(n)
    })
  }

  fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.decoder);
      let mut encoder = resource.borrow_mut().await;
      let n = encoder.write(&buf).await?;
      Ok(n)
    })
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.decoder);
      let mut encoder = resource.borrow_mut().await;
      encoder.shutdown().await?;
      Ok(())
    })
  }
}

pub struct DeflateCompressorResource {
  encoder: AsyncRefCell<DeflateEncoder<DuplexStream>>,
  tx: AsyncRefCell<DuplexStream>,
}
impl Resource for DeflateCompressorResource {
  fn name(&self) -> Cow<str> {
    "deflateCompressor".into()
  }

  fn read(self: Rc<Self>, mut buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.tx);
      let mut tx = resource.borrow_mut().await;
      let n = tx.read(&mut buf).await?;
      Ok(n)
    })
  }

  fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.encoder);
      let mut encoder = resource.borrow_mut().await;
      let n = encoder.write(&buf).await?;
      Ok(n)
    })
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.encoder);
      let mut encoder = resource.borrow_mut().await;
      encoder.shutdown().await?;
      Ok(())
    })
  }
}

pub struct DeflateDecompressorResource {
  decoder: AsyncRefCell<DeflateDecoder<DuplexStream>>,
  tx: AsyncRefCell<DuplexStream>,
}
impl Resource for DeflateDecompressorResource {
  fn name(&self) -> Cow<str> {
    "deflateDecompressor".into()
  }

  fn read(self: Rc<Self>, mut buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.tx);
      let mut tx = resource.borrow_mut().await;
      let n = tx.read(&mut buf).await?;
      Ok(n)
    })
  }

  fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.decoder);
      let mut encoder = resource.borrow_mut().await;
      let n = encoder.write(&buf).await?;
      Ok(n)
    })
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(async move {
      let resource = deno_core::RcRef::map(self, |r| &r.decoder);
      let mut encoder = resource.borrow_mut().await;
      encoder.shutdown().await?;
      Ok(())
    })
  }
}

pub fn op_compression_compressor_create(
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
        encoder: AsyncRefCell::new(encoder),
        tx: AsyncRefCell::new(tx),
      })
    }
    "deflate" => {
      let (rx, tx) = tokio::io::duplex(65536);
      let encoder =
        DeflateEncoder::with_quality(rx, async_compression::Level::Precise(8));
      state.resource_table.add(DeflateCompressorResource {
        encoder: AsyncRefCell::new(encoder),
        tx: AsyncRefCell::new(tx),
      })
    }
    _ => unreachable!(),
  };

  Ok(rid)
}

pub fn op_compression_decompressor_create(
  state: &mut OpState,
  format: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  let rid = match format.as_str() {
    "gzip" => {
      let (rx, tx) = tokio::io::duplex(65536);
      let decoder = GzipDecoder::new(rx);
      state.resource_table.add(GzipDecompressorResource {
        decoder: AsyncRefCell::new(decoder),
        tx: AsyncRefCell::new(tx),
      })
    }
    "deflate" => {
      let (rx, tx) = tokio::io::duplex(65536);
      let decoder = DeflateDecoder::new(rx);
      state.resource_table.add(DeflateDecompressorResource {
        decoder: AsyncRefCell::new(decoder),
        tx: AsyncRefCell::new(tx),
      })
    }
    _ => unreachable!(),
  };

  Ok(rid)
}
