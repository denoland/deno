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
  specifier: &ModuleSpecifier,
  maybe_import_map: &Option<ImportMap>,
) -> Result<(), AnyError> {
  let program_state = Arc::new(ProgramState::new(Default::default())?);
  let handler = Arc::new(Mutex::new(FetchHandler::new(
    &program_state,
    Permissions::allow_all(),
  )?));
  let mut builder = GraphBuilder::new(handler, maybe_import_map.clone(), None);
  builder.add(specifier, false).await
}

fn get_remote_headers(
  cache_filename: &Path,
) -> Option<HashMap<String, String>> {
  let metadata_path = http_cache::Metadata::filename(cache_filename);
  let metadata_str = fs::read_to_string(metadata_path).ok()?;
  let metadata: http_cache::Metadata =
    serde_json::from_str(&metadata_str).ok()?;
  Some(metadata.headers)
}

fn resolve_remote_specifier(
  specifier: &ModuleSpecifier,
  http_cache: &HttpCache,
  redirect_limit: isize,
) -> Option<ModuleSpecifier> {
  let cache_filename = http_cache.get_cache_filename(specifier.as_url());
  if redirect_limit >= 0 && cache_filename.is_file() {
    let headers = get_remote_headers(&cache_filename)?;
    if let Some(location) = headers.get("location") {
      let redirect =
        ModuleSpecifier::resolve_import(location, specifier.as_str()).ok()?;
      resolve_remote_specifier(&redirect, http_cache, redirect_limit - 1)
    } else {
      Some(specifier.clone())
    }
  } else {
    None
  }
}

fn resolve_specifier(
  specifier: &ModuleSpecifier,
  redirects: &mut HashMap<ModuleSpecifier, ModuleSpecifier>,
  http_cache: &HttpCache,
) -> Option<ModuleSpecifier> {
  let scheme = specifier.as_url().scheme();
  if !SUPPORTED_SCHEMES.contains(&scheme) {
    return None;
  }

  if scheme == "data" {
    Some(specifier.clone())
  } else if scheme == "file" {
    let path = specifier.as_url().to_file_path().ok()?;
    if path.is_file() {
      Some(specifier.clone())
    } else {
      None
    }
  } else if let Some(specifier) = redirects.get(specifier) {
    Some(specifier.clone())
  } else {
    let redirect = resolve_remote_specifier(specifier, http_cache, 10)?;
    redirects.insert(specifier.clone(), redirect.clone());
    Some(redirect)
  }
}

#[derive(Debug, Clone, Default)]
struct Metadata {
  dependencies: Option<HashMap<String, analysis::Dependency>>,
  length_utf16: usize,
  line_index: LineIndex,
  maybe_types: Option<analysis::ResolvedDependency>,
  media_type: MediaType,
  source: String,
  version: String,
}

impl Metadata {
  fn new(
    specifier: &ModuleSpecifier,
    source: &str,
    version: &str,
    media_type: &MediaType,
    maybe_import_map: &Option<ImportMap>,
  ) -> Self {
    let (dependencies, maybe_types) = if let Some((dependencies, maybe_types)) =
      analysis::analyze_dependencies(
        specifier,
        source,
        media_type,
        maybe_import_map,
      ) {
      (Some(dependencies), maybe_types)
    } else {
      (None, None)
    };
    let line_index = LineIndex::new(source);

    Self {
      dependencies,
      length_utf16: source.encode_utf16().count(),
      line_index,
      maybe_types,
      media_type: media_type.to_owned(),
      source: source.to_string(),
      version: version.to_string(),
    }
  }
}

#[derive(Debug, Clone, Default)]
struct Inner {
  http_cache: HttpCache,
  maybe_import_map: Option<ImportMap>,
  metadata: HashMap<ModuleSpecifier, Metadata>,
  redirects: HashMap<ModuleSpecifier, ModuleSpecifier>,
  remotes: HashMap<ModuleSpecifier, PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct Sources(Arc<Mutex<Inner>>);

impl Sources {
  pub fn new(location: &Path) -> Self {
    Self(Arc::new(Mutex::new(Inner::new(location))))
  }

  pub fn contains_key(&self, specifier: &ModuleSpecifier) -> bool {
    self.0.lock().unwrap().contains_key(specifier)
  }

  /// Provides the length of the source content, calculated in a way that should
  /// match the behavior of JavaScript, where strings are stored effectively as
  /// `&[u16]` and when counting "chars" we need to represent the string as a
  /// UTF-16 string in Rust.
  pub fn get_length_utf16(&self, specifier: &ModuleSpecifier) -> Option<usize> {
    self.0.lock().unwrap().get_length_utf16(specifier)
  }

  pub fn get_line_index(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<LineIndex> {
    self.0.lock().unwrap().get_line_index(specifier)
  }

  pub fn get_maybe_types(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<analysis::ResolvedDependency> {
    self.0.lock().unwrap().get_maybe_types(specifier)
  }

  pub fn get_media_type(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<MediaType> {
    self.0.lock().unwrap().get_media_type(specifier)
  }

  pub fn get_script_version(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    self.0.lock().unwrap().get_script_version(specifier)
  }

  pub fn get_source(&self, specifier: &ModuleSpecifier) -> Option<String> {
    self.0.lock().unwrap().get_source(specifier)
  }

  pub fn resolve_import(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    self.0.lock().unwrap().resolve_import(specifier, referrer)
  }
}

impl Inner {
  fn new(location: &Path) -> Self {
    Self {
      http_cache: HttpCache::new(location),
      ..Default::default()
    }
  }

  fn calculate_script_version(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    let path = self.get_path(specifier)?;
    let metadata = fs::metadata(path).ok()?;
    if let Ok(modified) = metadata.modified() {
      if let Ok(n) = modified.duration_since(SystemTime::UNIX_EPOCH) {
        Some(format!("{}", n.as_millis()))
      } else {
        Some("1".to_string())
      }
    } else {
      Some("1".to_string())
    }
  }

  fn contains_key(&mut self, specifier: &ModuleSpecifier) -> bool {
    if let Some(specifier) =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)
    {
      if self.get_metadata(&specifier).is_some() {
        return true;
      }
    }
    false
  }

  fn get_length_utf16(&mut self, specifier: &ModuleSpecifier) -> Option<usize> {
    let specifier =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.length_utf16)
  }

  fn get_line_index(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<LineIndex> {
    let specifier =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.line_index)
  }

  fn get_maybe_types(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<analysis::ResolvedDependency> {
    let specifier =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)?;
    let metadata = self.get_metadata(&specifier)?;
    metadata.maybe_types
  }

  fn get_media_type(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<MediaType> {
    let specifier =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.media_type)
  }

  fn get_metadata(&mut self, specifier: &ModuleSpecifier) -> Option<Metadata> {
    if let Some(metadata) = self.metadata.get(specifier).cloned() {
      if metadata.version == self.calculate_script_version(specifier)? {
        return Some(metadata);
      }
    }

    let version = self.calculate_script_version(specifier)?;
    let path = self.get_path(specifier)?;
    let bytes = fs::read(path).ok()?;
    let scheme = specifier.as_url().scheme();
    let (source, media_type, maybe_types) = if scheme == "file" {
      let maybe_charset =
        Some(text_encoding::detect_charset(&bytes).to_string());
      let source = get_source_from_bytes(bytes, maybe_charset).ok()?;
      (source, MediaType::from(specifier), None)
    } else {
      let cache_filename =
        self.http_cache.get_cache_filename(specifier.as_url());
      let headers = get_remote_headers(&cache_filename)?;
      let maybe_content_type = headers.get("content-type").cloned();
      let (media_type, maybe_charset) =
        map_content_type(specifier, maybe_content_type);
      let source = get_source_from_bytes(bytes, maybe_charset).ok()?;
      let maybe_types = headers.get("x-typescript-types").map(|s| {
        analysis::resolve_import(s, &specifier, &self.maybe_import_map)
      });
      (source, media_type, maybe_types)
    };
    let mut metadata = Metadata::new(
      specifier,
      &source,
      &version,
      &media_type,
      &self.maybe_import_map,
    );
    if metadata.maybe_types.is_none() {
      metadata.maybe_types = maybe_types;
    }
    self.metadata.insert(specifier.clone(), metadata.clone());
    Some(metadata)
  }

  fn get_path(&mut self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    if specifier.as_url().scheme() == "file" {
      specifier.as_url().to_file_path().ok()
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

  fn get_script_version(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    let specifier =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.version)
  }

  fn get_source(&mut self, specifier: &ModuleSpecifier) -> Option<String> {
    let specifier =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)?;
    let metadata = self.get_metadata(&specifier)?;
    Some(metadata.source)
  }

  fn resolution_result(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let specifier =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)?;
    let media_type = if let Some(metadata) = self.metadata.get(&specifier) {
      metadata.media_type
    } else {
      MediaType::from(&specifier)
    };
    Some((specifier, media_type))
  }

  fn resolve_import(
    &mut self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let referrer =
      resolve_specifier(referrer, &mut self.redirects, &self.http_cache)?;
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
        if let Some(type_dependency) = self.get_maybe_types(resolved_specifier)
        {
          self.set_maybe_type(specifier, &referrer, &type_dependency);
          if let analysis::ResolvedDependency::Resolved(type_specifier) =
            type_dependency
          {
            self.resolution_result(&type_specifier)
          } else {
            self.resolution_result(resolved_specifier)
          }
        } else {
          self.resolution_result(resolved_specifier)
        }
      } else {
        None
      }
    }
  }

  fn set_maybe_type(
    &mut self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    dependency: &analysis::ResolvedDependency,
  ) {
    if let Some(metadata) = self.metadata.get_mut(referrer) {
      if let Some(dependencies) = &mut metadata.dependencies {
        if let Some(dep) = dependencies.get_mut(specifier) {
          dep.maybe_type = Some(dependency.clone());
        }
      }
    }
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
    let (sources, _) = setup();
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
    let (sources, _) = setup();
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let tests = c.join("tests");
    let specifier = ModuleSpecifier::resolve_path(
      &tests.join("001_hello.js").to_string_lossy(),
    )
    .unwrap();
    let actual = sources.get_source(&specifier);
    assert!(actual.is_some());
    let actual = actual.unwrap();
    assert_eq!(actual, "console.log(\"Hello World\");\n");
  }

  #[test]
  fn test_sources_get_length_utf16() {
    let (sources, _) = setup();
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
  fn test_resolve_dependency_types() {
    let (sources, location) = setup();
    let cache = HttpCache::new(&location);
    let specifier_dep = ModuleSpecifier::resolve_url("https://deno.land/x/mod.ts").unwrap();
    cache.set(specifier_dep.as_url(), Default::default(), b"export * from \"https://deno.land/x/lib.js\";").unwrap();
    let specifier_code = ModuleSpecifier::resolve_url("https://deno.land/x/lib.js").unwrap();
    let mut headers_code = HashMap::new();
    headers_code.insert("x-typescript-types".to_string(), "./lib.d.ts".to_string());
    cache.set(specifier_code.as_url(), headers_code, b"export const a = 1;").unwrap();
    let specifier_type = ModuleSpecifier::resolve_url("https://deno.land/x/lib.d.ts").unwrap();
    cache.set(specifier_type.as_url(), Default::default(), b"export const a: number;").unwrap();
    let actual = sources.resolve_import("https://deno.land/x/lib.js", &specifier_dep);
    assert_eq!(actual, Some((specifier_type, MediaType::Dts)))
  }

  #[test]
  fn test_sources_resolve_specifier_non_supported_schema() {
    let (sources, _) = setup();
    let specifier = ModuleSpecifier::resolve_url("foo://a/b/c.ts")
      .expect("could not create specifier");
    let sources = sources.0.lock().unwrap();
    let mut redirects = sources.redirects.clone();
    let http_cache = sources.http_cache.clone();
    let actual = resolve_specifier(&specifier, &mut redirects, &http_cache);
    assert!(actual.is_none());
  }
}
