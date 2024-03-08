// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::glob::FilePatterns;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use sha2::Digest;
use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::Path;
use tar::Header;

use crate::cache::LazyGraphSourceParser;
use crate::tools::registry::paths::PackagePath;
use crate::util::fs::FileCollector;

use super::diagnostics::PublishDiagnostic;
use super::diagnostics::PublishDiagnosticsCollector;
use super::unfurl::SpecifierUnfurler;

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
  dir: &Path,
  source_parser: LazyGraphSourceParser,
  diagnostics_collector: &PublishDiagnosticsCollector,
  unfurler: &SpecifierUnfurler,
  file_patterns: Option<FilePatterns>,
) -> Result<PublishableTarball, AnyError> {
  let file_patterns = file_patterns
    .unwrap_or_else(|| FilePatterns::new_with_base(dir.to_path_buf()));
  let mut tar = TarGzArchive::new();
  let mut files = vec![];

  let iter_paths = FileCollector::new(|e| {
    if !e.file_type.is_file() {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(e.path) {
        diagnostics_collector.push(PublishDiagnostic::UnsupportedFileType {
          specifier,
          kind: if e.file_type.is_symlink() {
            "symlink".to_owned()
          } else {
            format!("{:?}", e.file_type)
          },
        });
      }
      return false;
    }
    e.path
      .file_name()
      .map(|s| s != ".DS_Store" && s != ".gitignore")
      .unwrap_or(true)
  })
  .ignore_git_folder()
  .ignore_node_modules()
  .ignore_vendor_folder()
  .use_gitignore()
  .collect_file_patterns(file_patterns)?;

  let mut paths = HashSet::with_capacity(iter_paths.len());

  for path in iter_paths {
    let Ok(specifier) = Url::from_file_path(&path) else {
      diagnostics_collector
        .to_owned()
        .push(PublishDiagnostic::InvalidPath {
          path: path.to_path_buf(),
          message: "unable to convert path to url".to_string(),
        });
      continue;
    };

    let Ok(relative_path) = path.strip_prefix(dir) else {
      diagnostics_collector
        .to_owned()
        .push(PublishDiagnostic::InvalidPath {
          path: path.to_path_buf(),
          message: "path is not in publish directory".to_string(),
        });
      continue;
    };

    let path_str =
      relative_path
        .components()
        .fold("".to_string(), |mut path, component| {
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
        });

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
        diagnostics_collector
          .to_owned()
          .push(PublishDiagnostic::InvalidPath {
            path: path.to_path_buf(),
            message: err.to_string(),
          });
      }
    }

    let content = resolve_content_maybe_unfurling(
      &path,
      &specifier,
      unfurler,
      source_parser,
      diagnostics_collector,
    )?;

    let media_type = MediaType::from_specifier(&specifier);
    if matches!(media_type, MediaType::Jsx | MediaType::Tsx) {
      diagnostics_collector.push(PublishDiagnostic::UnsupportedJsxTsx {
        specifier: specifier.clone(),
      });
    }

    files.push(PublishableTarballFile {
      path_str: path_str.clone(),
      specifier: specifier.clone(),
      // This hash string matches the checksum computed by registry
      hash: format!("sha256-{:x}", sha2::Sha256::digest(&content)),
      size: content.len(),
    });
    tar
      .add_file(format!(".{}", path_str), &content)
      .with_context(|| {
        format!("Unable to add file to tarball '{}'", path.display())
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

fn resolve_content_maybe_unfurling(
  path: &Path,
  specifier: &Url,
  unfurler: &SpecifierUnfurler,
  source_parser: LazyGraphSourceParser,
  diagnostics_collector: &PublishDiagnosticsCollector,
) -> Result<Vec<u8>, AnyError> {
  let parsed_source = match source_parser.get_or_parse_source(specifier)? {
    Some(parsed_source) => parsed_source,
    None => {
      let data = std::fs::read(path)
        .with_context(|| format!("Unable to read file '{}'", path.display()))?;
      let media_type = MediaType::from_specifier(specifier);

      match media_type {
        MediaType::JavaScript
        | MediaType::Jsx
        | MediaType::Mjs
        | MediaType::Cjs
        | MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Tsx => {
          // continue
        }
        MediaType::SourceMap
        | MediaType::Unknown
        | MediaType::Json
        | MediaType::Wasm
        | MediaType::TsBuildInfo => {
          // not unfurlable data
          return Ok(data);
        }
      }

      let text = String::from_utf8(data)?;
      deno_ast::parse_module(deno_ast::ParseParams {
        specifier: specifier.clone(),
        text_info: deno_ast::SourceTextInfo::from_string(text),
        media_type,
        capture_tokens: false,
        maybe_syntax: None,
        scope_analysis: false,
      })?
    }
  };

  log::debug!("Unfurling {}", specifier);
  let mut reporter = |diagnostic| {
    diagnostics_collector.push(PublishDiagnostic::SpecifierUnfurl(diagnostic));
  };
  let content = unfurler.unfurl(specifier, &parsed_source, &mut reporter);
  Ok(content.into_bytes())
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
