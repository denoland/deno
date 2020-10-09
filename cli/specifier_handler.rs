// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::deno_dir::DenoDir;
use crate::disk_cache::DiskCache;
use crate::file_fetcher::SourceFileFetcher;
use crate::global_state::GlobalState;
use crate::media_type::MediaType;
use crate::permissions::Permissions;

use deno_core::error::AnyError;
use deno_core::futures::Future;
use deno_core::futures::FutureExt;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

pub type DependencyMap = HashMap<String, Dependency>;
pub type EmitMap = HashMap<EmitType, (String, Option<String>)>;
pub type FetchFuture =
  Pin<Box<(dyn Future<Output = Result<CachedModule, AnyError>> + 'static)>>;

#[derive(Debug, Clone)]
pub struct CachedModule {
  pub emits: EmitMap,
  pub maybe_dependencies: Option<DependencyMap>,
  pub maybe_types: Option<String>,
  pub maybe_version: Option<String>,
  pub media_type: MediaType,
  pub source: String,
  pub specifier: ModuleSpecifier,
}

#[cfg(test)]
impl Default for CachedModule {
  fn default() -> Self {
    CachedModule {
      emits: HashMap::new(),
      maybe_dependencies: None,
      maybe_types: None,
      maybe_version: None,
      media_type: MediaType::Unknown,
      source: "".to_string(),
      specifier: ModuleSpecifier::resolve_url("https://deno.land/x/mod.ts")
        .unwrap(),
    }
  }
}

/// An enum that represents the different types of emitted code that can be
/// cached.  Different types can utilise different configurations which can
/// change the validity of the emitted code.
#[allow(unused)]
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum EmitType {
  /// Code that was emitted for use by the CLI
  Cli,
  /// Code that was emitted for bundling purposes
  Bundle,
  /// Code that was emitted based on a request to the runtime APIs
  Runtime,
}

impl Default for EmitType {
  fn default() -> Self {
    EmitType::Cli
  }
}

#[derive(Debug, Clone, Default)]
pub struct Dependency {
  /// The module specifier that resolves to the runtime code dependency for the
  /// module.
  pub maybe_code: Option<ModuleSpecifier>,
  /// The module specifier that resolves to the type only dependency for the
  /// module.
  pub maybe_type: Option<ModuleSpecifier>,
}

pub trait SpecifierHandler {
  /// Instructs the handler to fetch a specifier or retrieve its value from the
  /// cache.
  fn fetch(&mut self, specifier: ModuleSpecifier) -> FetchFuture;

  /// Get the optional build info from the cache for a given module specifier.
  /// Because build infos are only associated with the "root" modules, they are
  /// not expected to be cached for each module, but are "lazily" checked when
  /// a root module is identified.  The `emit_type` also indicates what form
  /// of the module the build info is valid for.
  fn get_build_info(
    &self,
    specifier: &ModuleSpecifier,
    emit_type: &EmitType,
  ) -> Result<Option<String>, AnyError>;

  /// Set the emitted code (and maybe map) for a given module specifier.  The
  /// cache type indicates what form the emit is related to.
  fn set_cache(
    &mut self,
    specifier: &ModuleSpecifier,
    emit_type: &EmitType,
    code: String,
    maybe_map: Option<String>,
  ) -> Result<(), AnyError>;

  /// When parsed out of a JavaScript module source, the triple slash reference
  /// to the types should be stored in the cache.
  fn set_types(
    &mut self,
    specifier: &ModuleSpecifier,
    types: String,
  ) -> Result<(), AnyError>;

  /// Set the build info for a module specifier, also providing the cache type.
  fn set_build_info(
    &mut self,
    specifier: &ModuleSpecifier,
    emit_type: &EmitType,
    build_info: String,
  ) -> Result<(), AnyError>;

  /// Set the graph dependencies for a given module specifier.
  fn set_deps(
    &mut self,
    specifier: &ModuleSpecifier,
    dependencies: DependencyMap,
  ) -> Result<(), AnyError>;

  /// Set the version of the source for a given module, which is used to help
  /// determine if a module needs to be re-emitted.
  fn set_version(
    &mut self,
    specifier: &ModuleSpecifier,
    version: String,
  ) -> Result<(), AnyError>;
}

impl fmt::Debug for dyn SpecifierHandler {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "SpecifierHandler {{ }}")
  }
}

/// Errors that could be raised by a `SpecifierHandler` implementation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SpecifierHandlerError {
  /// An error representing an error the `EmitType` that was supplied to a
  /// method of an implementor of the `SpecifierHandler` trait.
  UnsupportedEmitType(EmitType),
}
use SpecifierHandlerError::*;

impl fmt::Display for SpecifierHandlerError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      UnsupportedEmitType(ref emit_type) => write!(
        f,
        "The emit type of \"{:?}\" is unsupported for this operation.",
        emit_type
      ),
    }
  }
}

impl Error for SpecifierHandlerError {}

/// A representation of meta data for a compiled file.
///
/// *Note* this is currently just a copy of what is located in `tsc.rs` but will
/// be refactored to be able to store dependencies and type information in the
/// future.
#[derive(Deserialize, Serialize)]
pub struct CompiledFileMetadata {
  pub version_hash: String,
}

impl CompiledFileMetadata {
  pub fn from_bytes(bytes: &[u8]) -> Result<Self, AnyError> {
    let metadata_string = std::str::from_utf8(bytes)?;
    serde_json::from_str::<Self>(metadata_string).map_err(|e| e.into())
  }

  pub fn to_json_string(&self) -> Result<String, AnyError> {
    serde_json::to_string(self).map_err(|e| e.into())
  }
}

/// An implementation of the `SpecifierHandler` trait that integrates with the
/// existing `file_fetcher` interface, which will eventually be refactored to
/// align it more to the `SpecifierHandler` trait.
pub struct FetchHandler {
  disk_cache: DiskCache,
  file_fetcher: SourceFileFetcher,
  permissions: Permissions,
}

impl FetchHandler {
  pub fn new(
    global_state: &Arc<GlobalState>,
    permissions: Permissions,
  ) -> Result<Self, AnyError> {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();
    let deno_dir = DenoDir::new(custom_root)?;
    let disk_cache = deno_dir.gen_cache;
    let file_fetcher = global_state.file_fetcher.clone();

    Ok(FetchHandler {
      disk_cache,
      file_fetcher,
      permissions,
    })
  }
}

impl SpecifierHandler for FetchHandler {
  fn fetch(&mut self, specifier: ModuleSpecifier) -> FetchFuture {
    let permissions = self.permissions.clone();
    let file_fetcher = self.file_fetcher.clone();
    let disk_cache = self.disk_cache.clone();

    async move {
      let source_file = file_fetcher
        .fetch_source_file(&specifier, None, permissions)
        .await?;
      let url = source_file.url;
      let filename = disk_cache.get_cache_filename_with_extension(&url, "meta");
      let maybe_version = if let Ok(bytes) = disk_cache.get(&filename) {
        if let Ok(compiled_file_metadata) =
          CompiledFileMetadata::from_bytes(&bytes)
        {
          Some(compiled_file_metadata.version_hash)
        } else {
          None
        }
      } else {
        None
      };

      let filename =
        disk_cache.get_cache_filename_with_extension(&url, "js.map");
      let maybe_map: Option<String> = if let Ok(map) = disk_cache.get(&filename)
      {
        Some(String::from_utf8(map)?)
      } else {
        None
      };
      let mut emits = HashMap::new();
      let filename = disk_cache.get_cache_filename_with_extension(&url, "js");
      if let Ok(code) = disk_cache.get(&filename) {
        emits.insert(EmitType::Cli, (String::from_utf8(code)?, maybe_map));
      };

      Ok(CachedModule {
        emits,
        maybe_dependencies: None,
        maybe_types: source_file.types_header,
        maybe_version,
        media_type: source_file.media_type,
        source: source_file.source_code,
        specifier,
      })
    }
    .boxed_local()
  }

  fn get_build_info(
    &self,
    specifier: &ModuleSpecifier,
    emit_type: &EmitType,
  ) -> Result<Option<String>, AnyError> {
    if emit_type != &EmitType::Cli {
      return Err(UnsupportedEmitType(emit_type.clone()).into());
    }
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier.as_url(), "buildinfo");
    if let Ok(build_info) = self.disk_cache.get(&filename) {
      return Ok(Some(String::from_utf8(build_info)?));
    }

    Ok(None)
  }

  fn set_build_info(
    &mut self,
    specifier: &ModuleSpecifier,
    emit_type: &EmitType,
    build_info: String,
  ) -> Result<(), AnyError> {
    if emit_type != &EmitType::Cli {
      return Err(UnsupportedEmitType(emit_type.clone()).into());
    }
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier.as_url(), "buildinfo");
    self
      .disk_cache
      .set(&filename, build_info.as_bytes())
      .map_err(|e| e.into())
  }

  fn set_cache(
    &mut self,
    specifier: &ModuleSpecifier,
    emit_type: &EmitType,
    code: String,
    maybe_map: Option<String>,
  ) -> Result<(), AnyError> {
    if emit_type != &EmitType::Cli {
      return Err(UnsupportedEmitType(emit_type.clone()).into());
    }
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier.as_url(), "js");
    self.disk_cache.set(&filename, code.as_bytes())?;

    if let Some(map) = maybe_map {
      let filename = self
        .disk_cache
        .get_cache_filename_with_extension(specifier.as_url(), "js.map");
      self.disk_cache.set(&filename, map.as_bytes())?;
    }

    Ok(())
  }

  fn set_deps(
    &mut self,
    _specifier: &ModuleSpecifier,
    _dependencies: DependencyMap,
  ) -> Result<(), AnyError> {
    // file_fetcher doesn't have the concept of caching dependencies
    Ok(())
  }

  fn set_types(
    &mut self,
    _specifier: &ModuleSpecifier,
    _types: String,
  ) -> Result<(), AnyError> {
    // file_fetcher doesn't have the concept of caching of the types
    Ok(())
  }

  fn set_version(
    &mut self,
    specifier: &ModuleSpecifier,
    version_hash: String,
  ) -> Result<(), AnyError> {
    let compiled_file_metadata = CompiledFileMetadata { version_hash };
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier.as_url(), "meta");

    self
      .disk_cache
      .set(
        &filename,
        compiled_file_metadata.to_json_string()?.as_bytes(),
      )
      .map_err(|e| e.into())
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::http_cache::HttpCache;
  use tempfile::TempDir;

  fn setup() -> (TempDir, FetchHandler) {
    let temp_dir = TempDir::new().expect("could not setup");
    let deno_dir = DenoDir::new(Some(temp_dir.path().to_path_buf()))
      .expect("could not setup");

    let file_fetcher = SourceFileFetcher::new(
      HttpCache::new(&temp_dir.path().to_path_buf().join("deps")),
      true,
      Vec::new(),
      false,
      false,
      None,
    )
    .expect("could not setup");
    let disk_cache = deno_dir.gen_cache;

    let fetch_handler = FetchHandler {
      disk_cache,
      file_fetcher,
      permissions: Permissions::allow_all(),
    };

    (temp_dir, fetch_handler)
  }

  #[tokio::test]
  async fn test_fetch_handler_fetch() {
    let _http_server_guard = test_util::http_server();
    let (_, mut file_fetcher) = setup();
    let specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/subdir/mod2.ts",
    )
    .unwrap();
    let cached_module: CachedModule =
      file_fetcher.fetch(specifier.clone()).await.unwrap();
    assert_eq!(cached_module.emits.len(), 0);
    assert!(cached_module.maybe_dependencies.is_none());
    assert_eq!(cached_module.media_type, MediaType::TypeScript);
    assert_eq!(
      cached_module.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    assert_eq!(cached_module.specifier, specifier);
  }

  #[tokio::test]
  async fn test_fetch_handler_set_cache() {
    let _http_server_guard = test_util::http_server();
    let (_, mut file_fetcher) = setup();
    let specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/subdir/mod2.ts",
    )
    .unwrap();
    let cached_module: CachedModule =
      file_fetcher.fetch(specifier.clone()).await.unwrap();
    assert_eq!(cached_module.emits.len(), 0);
    let code = String::from("some code");
    file_fetcher
      .set_cache(&specifier, &EmitType::Cli, code, None)
      .expect("could not set cache");
    let cached_module: CachedModule =
      file_fetcher.fetch(specifier.clone()).await.unwrap();
    assert_eq!(cached_module.emits.len(), 1);
    let actual_emit = cached_module.emits.get(&EmitType::Cli).unwrap();
    assert_eq!(actual_emit.0, "some code");
    assert_eq!(actual_emit.1, None);
  }
}
