// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::url::Url;
use hyper::body::Bytes;
use std::io::Write;
use std::path::PathBuf;
use tar::Header;

use crate::util::import_map::ImportMapUnfurler;

pub fn create_tarball(
  dir: PathBuf,
  // TODO(bartlomieju): this is too specific, factor it out into a callback that
  // returns data
  unfurler: ImportMapUnfurler,
) -> Result<Bytes, AnyError> {
  let mut tar = TarArchive::new();
  let dir_url = Url::from_directory_path(&dir).unwrap();

  // TODO(bartlomieju): this should be helper function and it should also
  // exclude test/bench files when publishing.
  for file in walkdir::WalkDir::new(dir).follow_links(false) {
    let file = file?;

    if file.file_type().is_dir() {
      continue;
    }

    let path = file.path();

    let url = Url::from_file_path(path).unwrap();
    // TODO(bartlomieju): use the same functionality as in `deno test`/
    // `deno bench` to match these
    if url.as_str().contains("_test") || url.as_str().contains("_bench") {
      continue;
    }

    let relative_path = dir_url.make_relative(&url).unwrap();
    let data = std::fs::read(path)?;
    let content = unfurler.unfurl(url.to_string(), data)?;
    tar.add_file(relative_path, &content)?;
  }

  let v = tar.finish()?;
  Ok(Bytes::from(v))
}

struct TarArchive {
  builder: tar::Builder<Vec<u8>>,
}

impl TarArchive {
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
