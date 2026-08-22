// Copyright 2018-2026 the Deno authors. MIT license.

use std::fmt::Write as FmtWrite;
use std::io::Write;

use bytes::Bytes;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::ModuleGraph;
use sha2::Digest;
use tar::Header;

use super::diagnostics::PublishDiagnostic;
use super::diagnostics::PublishDiagnosticsCollector;
use super::module_content::ModuleContentProvider;
use super::paths::CollectedPublishPath;

/// Maximum size of a single file in a package, as enforced by the registry when
/// it unpacks the tarball. Kept in sync with `MAX_FILE_SIZE` in the registry's
/// `api/src/tarball.rs` so that a dry run fails on the same packages a publish
/// would.
const MAX_FILE_SIZE: u64 = 20 * 1024 * 1024;
/// Maximum total size of all (uncompressed) files in a package, as enforced by
/// the registry. Kept in sync with `MAX_TOTAL_FILE_SIZE` in the registry's
/// `api/src/tarball.rs`.
const MAX_TOTAL_FILE_SIZE: u64 = 20 * 1024 * 1024;

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
  let mut total_file_size: u64 = 0;
  let mut has_file_too_large = false;
  let mut reported_package_too_large = false;

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

    // mirror the size limits the registry enforces while unpacking the
    // tarball, so that they surface during a dry run instead of only once the
    // package has been uploaded
    let size = content.len() as u64;
    if size > MAX_FILE_SIZE {
      has_file_too_large = true;
      diagnostics_collector.push(PublishDiagnostic::FileTooLarge {
        specifier: specifier.clone(),
        size,
        max_size: MAX_FILE_SIZE,
      });
    }
    total_file_size += size;
    // report the total only once, on the file that pushed the package over the
    // limit, which is what the registry reports too. Once a single file has
    // been found to be too large the total is not worth reporting: the
    // registry rejects that file before it ever sums the sizes, and the file
    // is almost certainly what put the package over the limit anyway.
    if total_file_size > MAX_TOTAL_FILE_SIZE
      && !has_file_too_large
      && !reported_package_too_large
    {
      reported_package_too_large = true;
      diagnostics_collector.push(PublishDiagnostic::PackageTooLarge {
        specifier: specifier.clone(),
        size: total_file_size,
        max_size: MAX_TOTAL_FILE_SIZE,
      });
    }

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
