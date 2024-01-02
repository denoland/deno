// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use std::io::Write;
use std::path::Path;
use tar::Header;

use crate::util::import_map::ImportMapUnfurler;

pub fn create_gzipped_tarball(
  dir: &Path,
  // TODO(bartlomieju): this is too specific, factor it out into a callback that
  // returns data
  unfurler: ImportMapUnfurler,
) -> Result<(Bytes, Vec<String>), AnyError> {
  let mut tar = TarGzArchive::new();
  let dir = dir
    .canonicalize()
    .map_err(|_| anyhow::anyhow!("Unable to canonicalize path {:?}", dir))?;
  let mut diagnostics = vec![];

  for entry in walkdir::WalkDir::new(&dir).follow_links(false) {
    let entry = entry?;

    if entry.file_type().is_file() {
      let url = Url::from_file_path(entry.path())
        .map_err(|_| anyhow::anyhow!("Unable to convert path to url"))?;
      let relative_path = entry
        .path()
        .strip_prefix(&dir)
        .map_err(|err| anyhow::anyhow!("Unable to strip prefix: {err}"))?;
      let relative_path = relative_path.to_str().ok_or_else(|| {
        anyhow::anyhow!("Unable to convert path to string {:?}", relative_path)
      })?;
      let data = std::fs::read(entry.path())
        .with_context(|| format!("Unable to read file {:?}", entry.path()))?;
      let (content, unfurl_diagnostics) = unfurler
        .unfurl(&url, data)
        .with_context(|| format!("Unable to unfurl file {:?}", entry.path()))?;

      diagnostics.extend_from_slice(&unfurl_diagnostics);
      tar
        .add_file(relative_path.to_string(), &content)
        .with_context(|| {
          format!("Unable to add file to tarball {:?}", entry.path())
        })?;
    } else if entry.file_type().is_dir() {
      // skip
    } else {
      diagnostics
        .push(format!("Unsupported file type at path {:?}", entry.path()));
    }
  }

  let v = tar.finish().context("Unable to finish tarball")?;
  Ok((Bytes::from(v), diagnostics))
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
