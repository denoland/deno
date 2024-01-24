// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use sha2::Digest;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tar::Header;

use crate::util::import_map::ImportMapUnfurler;
use deno_config::glob::PathOrPatternSet;

use super::diagnostics::PublishDiagnostic;
use super::diagnostics::PublishDiagnosticsCollector;

#[derive(Debug, Clone, PartialEq)]
pub struct PublishableTarballFile {
  pub path: PathBuf,
  pub size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishableTarball {
  pub files: Vec<PublishableTarballFile>,
  pub diagnostics: Vec<String>,
  pub hash: String,
  pub bytes: Bytes,
}

pub fn create_gzipped_tarball(
  dir: &Path,
  source_cache: &dyn deno_graph::ParsedSourceStore,
  diagnostics_collector: &PublishDiagnosticsCollector,
  unfurler: &ImportMapUnfurler,
  exclude_patterns: &PathOrPatternSet,
) -> Result<PublishableTarball, AnyError> {
  let mut tar = TarGzArchive::new();
  let mut diagnostics = vec![];
  let mut files = vec![];

  let mut iterator = walkdir::WalkDir::new(dir).follow_links(false).into_iter();
  while let Some(entry) = iterator.next() {
    let entry = entry?;

    if exclude_patterns.matches_path(entry.path()) {
      if entry.file_type().is_dir() {
        iterator.skip_current_dir();
      }
      continue;
    }

    if entry.file_type().is_file() {
      let url = Url::from_file_path(entry.path())
        .map_err(|_| anyhow::anyhow!("Unable to convert path to url"))?;
      let relative_path = entry
        .path()
        .strip_prefix(dir)
        .map_err(|err| anyhow::anyhow!("Unable to strip prefix: {err:#}"))?;
      let relative_path_str = relative_path.to_str().ok_or_else(|| {
        anyhow::anyhow!(
          "Unable to convert path to string '{}'",
          relative_path.display()
        )
      })?;
      let data = std::fs::read(entry.path()).with_context(|| {
        format!("Unable to read file '{}'", entry.path().display())
      })?;
      files.push(PublishableTarballFile {
        path: relative_path.to_path_buf(),
        size: data.len(),
      });
      let content = match source_cache.get_parsed_source(&url) {
        Some(parsed_source) => {
          let mut reporter = |diagnostic| {
            diagnostics_collector
              .push(PublishDiagnostic::ImportMapUnfurl(diagnostic));
          };
          let content = unfurler.unfurl(&url, &parsed_source, &mut reporter);
          content.into_bytes()
        }
        None => data,
      };
      tar
        .add_file(relative_path_str.to_string(), &content)
        .with_context(|| {
          format!("Unable to add file to tarball '{}'", entry.path().display())
        })?;
    } else if entry.file_type().is_dir() {
      if entry.file_name() == ".git" || entry.file_name() == "node_modules" {
        iterator.skip_current_dir();
      }
    } else {
      diagnostics.push(format!(
        "Unsupported file type at path '{}'",
        entry.path().display()
      ));
    }
  }

  let v = tar.finish().context("Unable to finish tarball")?;
  let hash_bytes: Vec<u8> = sha2::Sha256::digest(&v).iter().cloned().collect();
  let mut hash = "sha256-".to_string();
  for byte in hash_bytes {
    write!(&mut hash, "{:02x}", byte).unwrap();
  }

  Ok(PublishableTarball {
    files,
    diagnostics,
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
