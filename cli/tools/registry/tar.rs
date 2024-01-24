// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use deno_config::glob::FilePatterns;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use sha2::Digest;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::Path;
use tar::Header;

use crate::tools::registry::paths::PackagePath;
use crate::util::import_map::ImportMapUnfurler;

use super::diagnostics::PublishDiagnostic;
use super::diagnostics::PublishDiagnosticsCollector;

#[derive(Debug, Clone, PartialEq)]
pub struct PublishableTarballFile {
  pub specifier: Url,
  pub size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishableTarball {
  pub files: Vec<PublishableTarballFile>,
  pub hash: String,
  pub bytes: Bytes,
}

pub fn create_gzipped_tarball(
  dir: &Path,
  source_cache: &dyn deno_graph::ParsedSourceStore,
  diagnostics_collector: &PublishDiagnosticsCollector,
  unfurler: &ImportMapUnfurler,
  file_patterns: Option<FilePatterns>,
) -> Result<PublishableTarball, AnyError> {
  let mut tar = TarGzArchive::new();
  let mut files = vec![];

  let mut paths = HashSet::new();

  let mut iterator = walkdir::WalkDir::new(dir).follow_links(false).into_iter();
  while let Some(entry) = iterator.next() {
    let entry = entry?;

    let path = entry.path();
    let file_type = entry.file_type();

    let matches_pattern = file_patterns
      .as_ref()
      .map(|p| p.matches_path(path))
      .unwrap_or(true);
    if !matches_pattern
      && !(path.file_name() == Some(OsStr::new(".git"))
        || path.file_name() == Some(OsStr::new("node_modules")))
    {
      if file_type.is_dir() {
        iterator.skip_current_dir();
      }
      continue;
    }

    let Ok(specifier) = Url::from_file_path(path) else {
      diagnostics_collector
        .to_owned()
        .push(PublishDiagnostic::InvalidPath {
          path: path.to_path_buf(),
          message: "unable to convert path to url".to_string(),
        });
      continue;
    };

    if file_type.is_file() {
      let Ok(relative_path) = path.strip_prefix(dir) else {
        diagnostics_collector
          .to_owned()
          .push(PublishDiagnostic::InvalidPath {
            path: path.to_path_buf(),
            message: "path is not in publish directory".to_string(),
          });
        continue;
      };

      let path_str = relative_path.components().fold(
        "".to_string(),
        |mut path, component| {
          path.push('/');
          match component {
            std::path::Component::Normal(normal) => {
              path.push_str(&normal.to_string_lossy())
            }
            std::path::Component::CurDir => path.push('.'),
            std::path::Component::ParentDir => path.push_str(".."),
            _ => unreachable!(),
          }
          path
        },
      );

      match PackagePath::new(path_str.clone()) {
        Ok(package_path) => {
          if !paths.insert(package_path) {
            diagnostics_collector.to_owned().push(
              PublishDiagnostic::DuplicatePath {
                path: path.to_path_buf(),
              },
            );
          }
        }
        Err(err) => {
          diagnostics_collector.to_owned().push(
            PublishDiagnostic::InvalidPath {
              path: path.to_path_buf(),
              message: err.to_string(),
            },
          );
        }
      }

      let data = std::fs::read(path).with_context(|| {
        format!("Unable to read file '{}'", entry.path().display())
      })?;
      files.push(PublishableTarballFile {
        specifier: specifier.clone(),
        size: data.len(),
      });
      let content = match source_cache.get_parsed_source(&specifier) {
        Some(parsed_source) => {
          let mut reporter = |diagnostic| {
            diagnostics_collector
              .push(PublishDiagnostic::ImportMapUnfurl(diagnostic));
          };
          let content =
            unfurler.unfurl(&specifier, &parsed_source, &mut reporter);
          content.into_bytes()
        }
        None => data,
      };
      tar
        .add_file(format!(".{}", path_str), &content)
        .with_context(|| {
          format!("Unable to add file to tarball '{}'", entry.path().display())
        })?;
    } else if !file_type.is_dir() {
      diagnostics_collector.push(PublishDiagnostic::UnsupportedFileType {
        specifier,
        kind: if file_type.is_symlink() {
          "symlink".to_owned()
        } else {
          format!("{file_type:?}")
        },
      });
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
