// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::text::LineIndex;

use crate::file_fetcher::get_source_from_bytes;
use crate::file_fetcher::map_content_type;
use crate::file_fetcher::SUPPORTED_SCHEMES;
use crate::http_cache;
use crate::http_cache::HttpCache;
use crate::import_map::ImportMap;
use crate::media_type::MediaType;
use crate::module_graph::GraphBuilder;
use crate::program_state::ProgramState;
use crate::specifier_handler::FetchHandler;
use crate::text_encoding;
use deno_runtime::permissions::Permissions;

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

pub async fn cache(
  specifier: ModuleSpecifier,
  maybe_import_map: Option<ImportMap>,
) -> Result<(), AnyError> {
  let program_state = Arc::new(ProgramState::new(Default::default())?);
  let handler = Arc::new(Mutex::new(FetchHandler::new(
    &program_state,
    Permissions::allow_all(),
  )?));
  let mut builder = GraphBuilder::new(handler, maybe_import_map, None);
  builder.add(&specifier, false).await
}

#[derive(Debug, Clone, Default)]
struct Metadata {
  dependencies: Option<HashMap<String, analysis::Dependency>>,
  line_index: LineIndex,
  maybe_types: Option<analysis::ResolvedDependency>,
  media_type: MediaType,
  source: String,
  version: String,
}

#[derive(Debug, Clone, Default)]
pub struct Sources {
  http_cache: HttpCache,
  maybe_import_map: Option<ImportMap>,
  metadata: HashMap<ModuleSpecifier, Metadata>,
  redirects: HashMap<ModuleSpecifier, ModuleSpecifier>,
  remotes: HashMap<ModuleSpecifier, PathBuf>,
}

impl Sources {
  pub fn new(location: &Path) -> Self {
    Self {
      http_cache: HttpCache::new(location),
      ..Default::default()
    }
  }

  pub fn contains(&mut self, specifier: &ModuleSpecifier) -> bool {
    if let Some(specifier) = self.resolve_specifier(specifier) {
      if self.get_metadata(&specifier).is_some() {
        return true;
      }
    }
    false
  }

  /// Provides the length of the source content, calculated in a way that should
  /// match the behavior of JavaScript, where strings are stored effectively as
  /// `&[u16]` and when counting "chars" we need to represent the string as a
  /// UTF-16 string in Rust.
  pub fn get_length_utf16(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<usize> {
    let specifier = self.resolve_specifier(specifier)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.source.encode_utf16().count())
  }

  pub fn get_line_index(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<LineIndex> {
    let specifier = self.resolve_specifier(specifier)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.line_index)
  }

  pub fn get_maybe_types(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<analysis::ResolvedDependency> {
    let metadata = self.get_metadata(specifier)?;
    metadata.maybe_types
  }

  pub fn get_media_type(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<MediaType> {
    let specifier = self.resolve_specifier(specifier)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.media_type)
  }

  fn get_metadata(&mut self, specifier: &ModuleSpecifier) -> Option<Metadata> {
    if let Some(metadata) = self.metadata.get(specifier).cloned() {
      if let Some(current_version) = self.get_script_version(specifier) {
        if metadata.version == current_version {
          return Some(metadata);
        }
      }
    }
    // TODO(@kitsonk) this needs to be refactored, lots of duplicate logic and
    // is really difficult to follow.
    let version = self.get_script_version(specifier)?;
    let path = self.get_path(specifier)?;
    if let Ok(bytes) = fs::read(path) {
      if specifier.as_url().scheme() == "file" {
        let charset = text_encoding::detect_charset(&bytes).to_string();
        if let Ok(source) = get_source_from_bytes(bytes, Some(charset)) {
          let media_type = MediaType::from(specifier);
          let mut maybe_types = None;
          let maybe_import_map = self.maybe_import_map.clone();
          let dependencies = if let Some((dependencies, mt)) =
            analysis::analyze_dependencies(
              &specifier,
              &source,
              &media_type,
              &maybe_import_map,
              &mut |s| self.get_maybe_types(s),
            ) {
            maybe_types = mt;
            Some(dependencies)
          } else {
            None
          };
          let line_index = LineIndex::new(&source);
          let metadata = Metadata {
            dependencies,
            line_index,
            maybe_types,
            media_type,
            source,
            version,
          };
          self.metadata.insert(specifier.clone(), metadata.clone());
          Some(metadata)
        } else {
          None
        }
      } else {
        let headers = self.get_remote_headers(specifier)?;
        let maybe_content_type = headers.get("content-type").cloned();
        let (media_type, maybe_charset) =
          map_content_type(specifier, maybe_content_type);
        if let Ok(source) = get_source_from_bytes(bytes, maybe_charset) {
          let mut maybe_types =
            if let Some(types) = headers.get("x-typescript-types") {
              Some(analysis::resolve_import(
                types,
                &specifier,
                &self.maybe_import_map,
              ))
            } else {
              None
            };
          let maybe_import_map = self.maybe_import_map.clone();
          let dependencies = if let Some((dependencies, mt)) =
            analysis::analyze_dependencies(
              &specifier,
              &source,
              &media_type,
              &maybe_import_map,
              &mut |s| self.get_maybe_types(s),
            ) {
            if maybe_types.is_none() {
              maybe_types = mt;
            }
            Some(dependencies)
          } else {
            None
          };
          let line_index = LineIndex::new(&source);
          let metadata = Metadata {
            dependencies,
            line_index,
            maybe_types,
            media_type,
            source,
            version,
          };
          self.metadata.insert(specifier.clone(), metadata.clone());
          Some(metadata)
        } else {
          None
        }
      }
    } else {
      None
    }
  }

  fn get_path(&mut self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    let specifier = self.resolve_specifier(specifier)?;
    if specifier.as_url().scheme() == "file" {
      if let Ok(path) = specifier.as_url().to_file_path() {
        Some(path)
      } else {
        None
      }
    } else if let Some(path) = self.remotes.get(&specifier) {
      Some(path.clone())
    } else {
      let path = self.http_cache.get_cache_filename(&specifier.as_url());
      if path.is_file() {
        self.remotes.insert(specifier.clone(), path.clone());
        Some(path)
      } else {
        None
      }
    }
  }

  fn get_remote_headers(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<HashMap<String, String>> {
    let cache_filename = self.http_cache.get_cache_filename(specifier.as_url());
    let metadata_path = http_cache::Metadata::filename(&cache_filename);
    if let Ok(metadata) = fs::read_to_string(metadata_path) {
      if let Ok(metadata) =
        serde_json::from_str::<'_, http_cache::Metadata>(&metadata)
      {
        return Some(metadata.headers);
      }
    }
    None
  }

  pub fn get_script_version(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    if let Some(path) = self.get_path(specifier) {
      if let Ok(metadata) = fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
          return if let Ok(n) = modified.duration_since(SystemTime::UNIX_EPOCH)
          {
            Some(format!("{}", n.as_millis()))
          } else {
            Some("1".to_string())
          };
        } else {
          return Some("1".to_string());
        }
      }
    }
    None
  }

  pub fn get_text(&mut self, specifier: &ModuleSpecifier) -> Option<String> {
    let specifier = self.resolve_specifier(specifier)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.source)
  }

  fn resolution_result(
    &mut self,
    resolved_specifier: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let resolved_specifier = self.resolve_specifier(resolved_specifier)?;
    let media_type =
      if let Some(metadata) = self.metadata.get(&resolved_specifier) {
        metadata.media_type
      } else {
        MediaType::from(&resolved_specifier)
      };
    Some((resolved_specifier, media_type))
  }

  pub fn resolve_import(
    &mut self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let referrer = self.resolve_specifier(referrer)?;
    let metadata = self.get_metadata(&referrer)?;
    let dependencies = &metadata.dependencies?;
    let dependency = dependencies.get(specifier)?;
    if let Some(type_dependency) = &dependency.maybe_type {
      if let analysis::ResolvedDependency::Resolved(resolved_specifier) =
        type_dependency
      {
        self.resolution_result(resolved_specifier)
      } else {
        None
      }
    } else {
      let code_dependency = &dependency.maybe_code.clone()?;
      if let analysis::ResolvedDependency::Resolved(resolved_specifier) =
        code_dependency
      {
        self.resolution_result(resolved_specifier)
      } else {
        None
      }
    }
  }

  fn resolve_specifier(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let scheme = specifier.as_url().scheme();
    if !SUPPORTED_SCHEMES.contains(&scheme) {
      return None;
    }

    if scheme == "file" {
      if let Ok(path) = specifier.as_url().to_file_path() {
        if path.is_file() {
          return Some(specifier.clone());
        }
      }
    } else {
      if let Some(specifier) = self.redirects.get(specifier) {
        return Some(specifier.clone());
      }
      if let Some(redirect) = self.resolve_remote_specifier(specifier, 10) {
        self.redirects.insert(specifier.clone(), redirect.clone());
        return Some(redirect);
      }
    }
    None
  }

  fn resolve_remote_specifier(
    &self,
    specifier: &ModuleSpecifier,
    redirect_limit: isize,
  ) -> Option<ModuleSpecifier> {
    let cached_filename =
      self.http_cache.get_cache_filename(specifier.as_url());
    if redirect_limit >= 0 && cached_filename.is_file() {
      if let Some(headers) = self.get_remote_headers(specifier) {
        if let Some(redirect_to) = headers.get("location") {
          if let Ok(redirect) =
            ModuleSpecifier::resolve_import(redirect_to, specifier.as_str())
          {
            return self
              .resolve_remote_specifier(&redirect, redirect_limit - 1);
          }
        } else {
          return Some(specifier.clone());
        }
      }
    }
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::env;
  use tempfile::TempDir;

  fn setup() -> (Sources, PathBuf) {
    let temp_dir = TempDir::new().expect("could not create temp dir");
    let location = temp_dir.path().join("deps");
    let sources = Sources::new(&location);
    (sources, location)
  }

  #[test]
  fn test_sources_get_script_version() {
    let (mut sources, _) = setup();
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let tests = c.join("tests");
    let specifier = ModuleSpecifier::resolve_path(
      &tests.join("001_hello.js").to_string_lossy(),
    )
    .unwrap();
    let actual = sources.get_script_version(&specifier);
    assert!(actual.is_some());
  }

  #[test]
  fn test_sources_get_text() {
    let (mut sources, _) = setup();
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let tests = c.join("tests");
    let specifier = ModuleSpecifier::resolve_path(
      &tests.join("001_hello.js").to_string_lossy(),
    )
    .unwrap();
    let actual = sources.get_text(&specifier);
    assert!(actual.is_some());
    let actual = actual.unwrap();
    assert_eq!(actual, "console.log(\"Hello World\");\n");
  }

  #[test]
  fn test_sources_get_length_utf16() {
    let (mut sources, _) = setup();
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let tests = c.join("tests");
    let specifier = ModuleSpecifier::resolve_path(
      &tests.join("001_hello.js").to_string_lossy(),
    )
    .unwrap();
    let actual = sources.get_length_utf16(&specifier);
    assert!(actual.is_some());
    let actual = actual.unwrap();
    assert_eq!(actual, 28);
  }

  #[test]
  fn test_sources_resolve_specifier_non_supported_schema() {
    let (mut sources, _) = setup();
    let specifier = ModuleSpecifier::resolve_url("foo://a/b/c.ts")
      .expect("could not create specifier");
    let actual = sources.resolve_specifier(&specifier);
    assert!(actual.is_none());
  }
}
