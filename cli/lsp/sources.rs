// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::text::LineIndex;
use super::tsc;
use super::urls::INVALID_SPECIFIER;

use crate::config_file::ConfigFile;
use crate::file_fetcher::get_source_from_bytes;
use crate::file_fetcher::map_content_type;
use crate::file_fetcher::SUPPORTED_SCHEMES;
use crate::flags::Flags;
use crate::http_cache;
use crate::http_cache::HttpCache;
use crate::import_map::ImportMap;
use crate::media_type::MediaType;
use crate::module_graph::GraphBuilder;
use crate::program_state::ProgramState;
use crate::specifier_handler::FetchHandler;
use crate::text_encoding;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tsc::NavigationTree;

pub async fn cache(
  specifier: &ModuleSpecifier,
  maybe_import_map: &Option<ImportMap>,
  maybe_config_file: &Option<ConfigFile>,
  maybe_cache_path: &Option<PathBuf>,
) -> Result<(), AnyError> {
  let program_state = Arc::new(
    ProgramState::build(Flags {
      cache_path: maybe_cache_path.clone(),
      ..Default::default()
    })
    .await?,
  );
  let handler = Arc::new(Mutex::new(FetchHandler::new(
    &program_state,
    Permissions::allow_all(),
    Permissions::allow_all(),
  )?));
  let mut builder = GraphBuilder::new(handler, maybe_import_map.clone(), None);
  builder.analyze_config_file(maybe_config_file).await?;
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
  let cache_filename = http_cache.get_cache_filename(specifier)?;
  if redirect_limit >= 0 && cache_filename.is_file() {
    let headers = get_remote_headers(&cache_filename)?;
    if let Some(location) = headers.get("location") {
      let redirect =
        deno_core::resolve_import(location, specifier.as_str()).ok()?;
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
  let scheme = specifier.scheme();
  if !SUPPORTED_SCHEMES.contains(&scheme) {
    return None;
  }

  if scheme == "data" {
    Some(specifier.clone())
  } else if scheme == "file" {
    let path = specifier.to_file_path().ok()?;
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

#[derive(Debug, Clone)]
struct Metadata {
  dependencies: Option<HashMap<String, analysis::Dependency>>,
  length_utf16: usize,
  line_index: LineIndex,
  maybe_navigation_tree: Option<tsc::NavigationTree>,
  maybe_types: Option<analysis::ResolvedDependency>,
  maybe_warning: Option<String>,
  media_type: MediaType,
  source: String,
  specifier: ModuleSpecifier,
  version: String,
}

impl Default for Metadata {
  fn default() -> Self {
    Self {
      dependencies: None,
      length_utf16: 0,
      line_index: LineIndex::default(),
      maybe_navigation_tree: None,
      maybe_types: None,
      maybe_warning: None,
      media_type: MediaType::default(),
      source: String::default(),
      specifier: INVALID_SPECIFIER.clone(),
      version: String::default(),
    }
  }
}

impl Metadata {
  fn new(
    specifier: &ModuleSpecifier,
    source: &str,
    version: &str,
    media_type: &MediaType,
    maybe_warning: Option<String>,
    maybe_import_map: &Option<ImportMap>,
  ) -> Self {
    let (dependencies, maybe_types) = if let Ok(parsed_module) =
      analysis::parse_module(specifier, source, media_type)
    {
      let (deps, maybe_types) = analysis::analyze_dependencies(
        specifier,
        media_type,
        &parsed_module,
        maybe_import_map,
      );
      (Some(deps), maybe_types)
    } else {
      (None, None)
    };
    let line_index = LineIndex::new(source);

    Self {
      dependencies,
      length_utf16: source.encode_utf16().count(),
      line_index,
      maybe_navigation_tree: None,
      maybe_types,
      maybe_warning,
      media_type: media_type.to_owned(),
      source: source.to_string(),
      specifier: specifier.clone(),
      version: version.to_string(),
    }
  }

  fn refresh(&mut self, maybe_import_map: &Option<ImportMap>) {
    let (dependencies, maybe_types) = if let Ok(parsed_module) =
      analysis::parse_module(&self.specifier, &self.source, &self.media_type)
    {
      let (deps, maybe_types) = analysis::analyze_dependencies(
        &self.specifier,
        &self.media_type,
        &parsed_module,
        maybe_import_map,
      );
      (Some(deps), maybe_types)
    } else {
      (None, None)
    };
    self.dependencies = dependencies;
    self.maybe_types = maybe_types;
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
    self.0.lock().contains_key(specifier)
  }

  pub fn get_line_index(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<LineIndex> {
    self.0.lock().get_line_index(specifier)
  }

  pub fn get_maybe_types(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<analysis::ResolvedDependency> {
    self.0.lock().get_maybe_types(specifier)
  }

  pub fn get_maybe_warning(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    self.0.lock().get_maybe_warning(specifier)
  }

  pub fn get_media_type(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<MediaType> {
    self.0.lock().get_media_type(specifier)
  }

  pub fn get_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<tsc::NavigationTree> {
    self.0.lock().get_navigation_tree(specifier)
  }

  pub fn get_script_version(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    self.0.lock().get_script_version(specifier)
  }

  pub fn get_source(&self, specifier: &ModuleSpecifier) -> Option<String> {
    self.0.lock().get_source(specifier)
  }

  pub fn len(&self) -> usize {
    self.0.lock().metadata.len()
  }

  pub fn resolve_import(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    self.0.lock().resolve_import(specifier, referrer)
  }

  pub fn specifiers(&self) -> Vec<ModuleSpecifier> {
    self.0.lock().metadata.keys().cloned().collect()
  }

  pub fn set_import_map(&self, maybe_import_map: Option<ImportMap>) {
    self.0.lock().set_import_map(maybe_import_map)
  }

  pub fn set_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
    navigation_tree: tsc::NavigationTree,
  ) -> Result<(), AnyError> {
    self
      .0
      .lock()
      .set_navigation_tree(specifier, navigation_tree)
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

  fn get_maybe_warning(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    let metadata = self.get_metadata(specifier)?;
    metadata.maybe_warning
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
    let scheme = specifier.scheme();
    let (source, media_type, maybe_types, maybe_warning) = if scheme == "file" {
      let maybe_charset =
        Some(text_encoding::detect_charset(&bytes).to_string());
      let source = get_source_from_bytes(bytes, maybe_charset).ok()?;
      (source, MediaType::from(specifier), None, None)
    } else {
      let cache_filename = self.http_cache.get_cache_filename(specifier)?;
      let headers = get_remote_headers(&cache_filename)?;
      let maybe_content_type = headers.get("content-type").cloned();
      let (media_type, maybe_charset) =
        map_content_type(specifier, maybe_content_type);
      let source = get_source_from_bytes(bytes, maybe_charset).ok()?;
      let maybe_types = headers.get("x-typescript-types").map(|s| {
        analysis::resolve_import(s, specifier, &self.maybe_import_map)
      });
      let maybe_warning = headers.get("x-deno-warning").cloned();
      (source, media_type, maybe_types, maybe_warning)
    };
    let mut metadata = Metadata::new(
      specifier,
      &source,
      &version,
      &media_type,
      maybe_warning,
      &self.maybe_import_map,
    );
    if maybe_types.is_some() {
      metadata.maybe_types = maybe_types;
    }
    self.metadata.insert(specifier.clone(), metadata.clone());
    Some(metadata)
  }

  fn get_navigation_tree(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<tsc::NavigationTree> {
    let specifier =
      resolve_specifier(specifier, &mut self.redirects, &self.http_cache)?;
    let metadata = self.get_metadata(&specifier)?;
    metadata.maybe_navigation_tree
  }

  fn get_path(&mut self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    if specifier.scheme() == "file" {
      specifier.to_file_path().ok()
    } else if let Some(path) = self.remotes.get(specifier) {
      Some(path.clone())
    } else {
      let path = self.http_cache.get_cache_filename(specifier)?;
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
        // even if we have a module in the maybe_types slot, it doesn't mean
        // that it is the actual module we should be using based on headers,
        // so we check here and update properly.
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

  fn set_import_map(&mut self, maybe_import_map: Option<ImportMap>) {
    for (_, metadata) in self.metadata.iter_mut() {
      metadata.refresh(&maybe_import_map);
    }
    self.maybe_import_map = maybe_import_map;
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

  fn set_navigation_tree(
    &mut self,
    specifier: &ModuleSpecifier,
    navigation_tree: NavigationTree,
  ) -> Result<(), AnyError> {
    let mut metadata = self
      .metadata
      .get_mut(specifier)
      .ok_or_else(|| anyhow!("Specifier not found {}"))?;
    metadata.maybe_navigation_tree = Some(navigation_tree);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_path;
  use deno_core::resolve_url;
  use deno_core::serde_json::json;
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
    let tests = test_util::testdata_path();
    let specifier =
      resolve_path(&tests.join("001_hello.js").to_string_lossy()).unwrap();
    let actual = sources.get_script_version(&specifier);
    assert!(actual.is_some());
  }

  #[test]
  fn test_sources_get_text() {
    let (sources, _) = setup();
    let tests = test_util::testdata_path();
    let specifier =
      resolve_path(&tests.join("001_hello.js").to_string_lossy()).unwrap();
    let actual = sources.get_source(&specifier);
    assert!(actual.is_some());
    let actual = actual.unwrap();
    assert_eq!(actual, "console.log(\"Hello World\");\n");
  }

  #[test]
  fn test_resolve_dependency_types() {
    let (sources, location) = setup();
    let cache = HttpCache::new(&location);
    let specifier_dep = resolve_url("https://deno.land/x/mod.ts").unwrap();
    cache
      .set(
        &specifier_dep,
        Default::default(),
        b"export * from \"https://deno.land/x/lib.js\";",
      )
      .unwrap();
    let specifier_code = resolve_url("https://deno.land/x/lib.js").unwrap();
    let mut headers_code = HashMap::new();
    headers_code
      .insert("x-typescript-types".to_string(), "./lib.d.ts".to_string());
    cache
      .set(&specifier_code, headers_code, b"export const a = 1;")
      .unwrap();
    let specifier_type = resolve_url("https://deno.land/x/lib.d.ts").unwrap();
    cache
      .set(
        &specifier_type,
        Default::default(),
        b"export const a: number;",
      )
      .unwrap();
    let actual =
      sources.resolve_import("https://deno.land/x/lib.js", &specifier_dep);
    assert_eq!(actual, Some((specifier_type, MediaType::Dts)))
  }

  #[test]
  /// This is a regression test for https://github.com/denoland/deno/issues/10031
  fn test_resolve_dependency_import_types() {
    let (sources, location) = setup();
    let cache = HttpCache::new(&location);
    let specifier_dep = resolve_url("https://deno.land/x/mod.ts").unwrap();
    cache
      .set(
        &specifier_dep,
        Default::default(),
        b"import type { A } from \"https://deno.land/x/lib.js\";\nconst a: A = { a: \"a\" };",
      )
      .unwrap();
    let specifier_code = resolve_url("https://deno.land/x/lib.js").unwrap();
    let mut headers_code = HashMap::new();
    headers_code
      .insert("x-typescript-types".to_string(), "./lib.d.ts".to_string());
    cache
      .set(&specifier_code, headers_code, b"export const a = 1;")
      .unwrap();
    let specifier_type = resolve_url("https://deno.land/x/lib.d.ts").unwrap();
    cache
      .set(
        &specifier_type,
        Default::default(),
        b"export const a: number;\nexport interface A { a: number; }\n",
      )
      .unwrap();
    let actual =
      sources.resolve_import("https://deno.land/x/lib.js", &specifier_dep);
    assert_eq!(actual, Some((specifier_type, MediaType::Dts)))
  }

  #[test]
  fn test_warning_header() {
    let (sources, location) = setup();
    let cache = HttpCache::new(&location);
    let specifier = resolve_url("https://deno.land/x/lib.js").unwrap();
    let mut headers = HashMap::new();
    headers.insert(
      "x-deno-warning".to_string(),
      "this is a warning".to_string(),
    );
    cache
      .set(&specifier, headers, b"export const a = 1;")
      .unwrap();
    let actual = sources.get_maybe_warning(&specifier);
    assert_eq!(actual, Some("this is a warning".to_string()));
  }

  #[test]
  fn test_resolve_dependency_evil_redirect() {
    let (sources, location) = setup();
    let cache = HttpCache::new(&location);
    let evil_specifier = resolve_url("https://deno.land/x/evil.ts").unwrap();
    let mut evil_headers = HashMap::new();
    evil_headers
      .insert("location".to_string(), "file:///etc/passwd".to_string());
    cache.set(&evil_specifier, evil_headers, b"").unwrap();
    let remote_specifier = resolve_url("https://deno.land/x/mod.ts").unwrap();
    cache
      .set(
        &remote_specifier,
        Default::default(),
        b"export * from \"./evil.ts\";",
      )
      .unwrap();
    let actual = sources.resolve_import("./evil.ts", &remote_specifier);
    assert_eq!(actual, None);
  }

  #[test]
  fn test_resolve_with_import_map() {
    let (sources, location) = setup();
    let import_map_json = json!({
      "imports": {
        "mylib": "https://deno.land/x/myLib/index.js"
      }
    });
    let import_map = ImportMap::from_json(
      "https://deno.land/x/",
      &import_map_json.to_string(),
    )
    .unwrap();
    sources.set_import_map(Some(import_map));
    let cache = HttpCache::new(&location);
    let mylib_specifier =
      resolve_url("https://deno.land/x/myLib/index.js").unwrap();
    let mut mylib_headers_map = HashMap::new();
    mylib_headers_map.insert(
      "content-type".to_string(),
      "application/javascript".to_string(),
    );
    cache
      .set(
        &mylib_specifier,
        mylib_headers_map,
        b"export const a = \"a\";\n",
      )
      .unwrap();
    let referrer = resolve_url("https://deno.land/x/mod.ts").unwrap();
    cache
      .set(
        &referrer,
        Default::default(),
        b"export { a } from \"mylib\";",
      )
      .unwrap();
    let actual = sources.resolve_import("mylib", &referrer);
    assert_eq!(actual, Some((mylib_specifier, MediaType::JavaScript)));
  }

  #[test]
  fn test_update_import_map() {
    let (sources, location) = setup();
    let import_map_json = json!({
      "imports": {
        "otherlib": "https://deno.land/x/otherlib/index.js"
      }
    });
    let import_map = ImportMap::from_json(
      "https://deno.land/x/",
      &import_map_json.to_string(),
    )
    .unwrap();
    sources.set_import_map(Some(import_map));
    let cache = HttpCache::new(&location);
    let mylib_specifier =
      resolve_url("https://deno.land/x/myLib/index.js").unwrap();
    let mut mylib_headers_map = HashMap::new();
    mylib_headers_map.insert(
      "content-type".to_string(),
      "application/javascript".to_string(),
    );
    cache
      .set(
        &mylib_specifier,
        mylib_headers_map,
        b"export const a = \"a\";\n",
      )
      .unwrap();
    let referrer = resolve_url("https://deno.land/x/mod.ts").unwrap();
    cache
      .set(
        &referrer,
        Default::default(),
        b"export { a } from \"mylib\";",
      )
      .unwrap();
    let actual = sources.resolve_import("mylib", &referrer);
    assert_eq!(actual, None);
    let import_map_json = json!({
      "imports": {
        "otherlib": "https://deno.land/x/otherlib/index.js",
        "mylib": "https://deno.land/x/myLib/index.js"
      }
    });
    let import_map = ImportMap::from_json(
      "https://deno.land/x/",
      &import_map_json.to_string(),
    )
    .unwrap();
    sources.set_import_map(Some(import_map));
    let actual = sources.resolve_import("mylib", &referrer);
    assert_eq!(actual, Some((mylib_specifier, MediaType::JavaScript)));
  }

  #[test]
  fn test_sources_resolve_specifier_non_supported_schema() {
    let (sources, _) = setup();
    let specifier =
      resolve_url("foo://a/b/c.ts").expect("could not create specifier");
    let sources = sources.0.lock();
    let mut redirects = sources.redirects.clone();
    let http_cache = sources.http_cache.clone();
    let actual = resolve_specifier(&specifier, &mut redirects, &http_cache);
    assert!(actual.is_none());
  }
}
