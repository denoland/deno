// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use deno_ast::MediaType;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPattern;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use sha2::Digest;
use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::Path;
use tar::Header;

use crate::cache::LazyGraphSourceParser;
use crate::tools::registry::paths::PackagePath;

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
  maybe_jsx_config_pragmas: Option<String>,
) -> Result<PublishableTarball, AnyError> {
  let mut tar = TarGzArchive::new();
  let mut files = vec![];

  let mut paths = HashSet::new();

  let mut ob = OverrideBuilder::new(dir);
  ob.add("!.git")?.add("!node_modules")?.add("!.DS_Store")?;

  for pattern in file_patterns.as_ref().iter().flat_map(|p| p.include.iter()) {
    for path_or_pat in pattern.inner() {
      match path_or_pat {
        PathOrPattern::Path(p) => ob.add(p.to_str().unwrap())?,
        PathOrPattern::Pattern(p) => ob.add(p.as_str())?,
        PathOrPattern::RemoteUrl(_) => continue,
      };
    }
  }

  let overrides = ob.build()?;

  let iterator = WalkBuilder::new(dir)
    .follow_links(false)
    .require_git(false)
    .git_ignore(true)
    .git_global(true)
    .git_exclude(true)
    .overrides(overrides)
    .filter_entry(move |entry| {
      let matches_pattern = file_patterns
        .as_ref()
        .map(|p| p.matches_path(entry.path()))
        .unwrap_or(true);
      matches_pattern
    })
    .build();

  for entry in iterator {
    let entry = entry?;

    let path = entry.path();
    let Some(file_type) = entry.file_type() else {
      // entry doesn’t have a file type if it corresponds to stdin.
      continue;
    };

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

      let content = resolve_content_maybe_unfurling(
        path,
        &specifier,
        unfurler,
        source_parser,
        diagnostics_collector,
        maybe_jsx_config_pragmas.clone(),
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
  maybe_jsx_config_pragmas: Option<String>,
) -> Result<Vec<u8>, AnyError> {
  let media_type = MediaType::from_specifier(specifier);

  let parsed_source = match source_parser.get_or_parse_source(specifier)? {
    Some(parsed_source) => parsed_source,
    None => {
      let data = std::fs::read(path)
        .with_context(|| format!("Unable to read file '{}'", path.display()))?;

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
  let mut content = unfurler.unfurl(specifier, &parsed_source, &mut reporter);

  if matches!(media_type, MediaType::Jsx | MediaType::Tsx) {
    // Emit JSX configuration pragamas to the top of the file.
    if let Some(jsx_config_pragmas) = maybe_jsx_config_pragmas {
      content = format!("{}{}", jsx_config_pragmas, content);
    }
  }

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
