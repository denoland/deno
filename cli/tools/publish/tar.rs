// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt::Write as FmtWrite;
use std::io::Write;

use bytes::Bytes;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::ModuleGraph;
use sha2::Digest;
use tar::Header;

use super::diagnostics::PublishDiagnosticsCollector;
use super::module_content::ModuleContentProvider;
use super::paths::CollectedPublishPath;

#[derive(Debug, Clone, PartialEq)]
pub struct PublishableTarballFile {
  pub path_str: String,
  pub specifier: Url,
  pub hash: String,
  pub size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishableTarball {
  pub files: Vec<PublishableTarballFile>,
  pub hash: String,
  pub bytes: Bytes,
}

pub fn create_gzipped_tarball(
  module_content_provider: &ModuleContentProvider,
  graph: &ModuleGraph,
  diagnostics_collector: &PublishDiagnosticsCollector,
  publish_paths: Vec<CollectedPublishPath>,
) -> Result<PublishableTarball, AnyError> {
  let mut tar = TarGzArchive::new();
  let mut files = vec![];

  for path in publish_paths {
    let path_str = &path.relative_path;
    let specifier = &path.specifier;

    let content = match path.maybe_content {
      Some(content) => content.clone(),
      None => module_content_provider.resolve_content_maybe_unfurling(
        graph,
        diagnostics_collector,
        &path.path,
        specifier,
      )?,
    };

    files.push(PublishableTarballFile {
      path_str: path_str.clone(),
      specifier: specifier.clone(),
      // This hash string matches the checksum computed by registry
      hash: format!("sha256-{:x}", sha2::Sha256::digest(&content)),
      size: content.len(),
    });
    assert!(path_str.starts_with('/'));
    tar
      .add_file(format!(".{}", path_str), &content)
      .with_context(|| {
        format!("Unable to add file to tarball '{}'", path.path.display())
      })?;
  }

  let v = tar.finish().context("Unable to finish tarball")?;
  let hash_bytes: Vec<u8> = sha2::Sha256::digest(&v).iter().cloned().collect();
  let mut hash = "sha256-".to_string();
  for byte in hash_bytes {
    write!(&mut hash, "{:02x}", byte).unwrap();
  }

  files.sort_by(|a, b| a.specifier.cmp(&b.specifier));

  Ok(PublishableTarball {
    files,
    hash,
    bytes: Bytes::from(v),
  })
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
