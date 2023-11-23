// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::url::Url;
use hyper::body::Bytes;
use std::io::Write;
use std::path::PathBuf;
use tar::Header;

use crate::util::import_map::ImportMapUnfurler;

pub fn create_gzipped_tarball(
  dir: PathBuf,
  // TODO(bartlomieju): this is too specific, factor it out into a callback that
  // returns data
  unfurler: ImportMapUnfurler,
) -> Result<Bytes, AnyError> {
  let mut tar = TarGzArchive::new();
  let dir_url = Url::from_directory_path(&dir).unwrap();

  for entry in walkdir::WalkDir::new(dir).follow_links(false) {
    let entry = entry?;

    if entry.file_type().is_file() {
      let url = Url::from_file_path(entry.path())
        .map_err(|_| anyhow::anyhow!("Invalid file path {:?}", entry.path()))?;
      let relative_path = dir_url
        .make_relative(&url)
        .expect("children can be relative to parent");
      let data = std::fs::read(entry.path())
        .with_context(|| format!("Unable to read file {:?}", entry.path()))?;
      let content = unfurler
        .unfurl(url.to_string(), data)
        .with_context(|| format!("Unable to unfurl file {:?}", entry.path()))?;
      tar.add_file(relative_path, &content).with_context(|| {
        format!("Unable to add file to tarball {:?}", entry.path())
      })?;
    } else if entry.file_type().is_dir() {
      // skip
    } else {
      bail!("Unsupported file type at path {:?}", entry.path());
    }
  }

  let v = tar.finish().context("Unable to finish tarball")?;
  Ok(Bytes::from(v))
}

struct TarGzArchive {
  builder: tar::Builder<Vec<u8>>,
}

impl TarGzArchive {
  pub fn new() -> Self {
    Self {
      builder: tar::Builder::new(Vec::new()),
    }
  }

  pub fn add_file(
    &mut self,
    path: String,
    data: &[u8],
  ) -> Result<(), AnyError> {
    let mut header = Header::new_gnu();
    header.set_size(data.len() as u64);
    self.builder.append_data(&mut header, &path, data)?;
    Ok(())
  }

  fn finish(mut self) -> Result<Vec<u8>, AnyError> {
    self.builder.finish()?;
    let bytes = self.builder.into_inner()?;
    let mut gz_bytes = Vec::new();
    let mut encoder = flate2::write::GzEncoder::new(
      &mut gz_bytes,
      flate2::Compression::default(),
    );
    encoder.write_all(&bytes)?;
    encoder.finish()?;
    Ok(gz_bytes)
  }
}
