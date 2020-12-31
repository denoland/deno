// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast::Location;
use crate::deno_dir::DenoDir;
use crate::disk_cache::DiskCache;
use crate::file_fetcher::FileFetcher;
use crate::media_type::MediaType;
use crate::program_state::ProgramState;
use deno_runtime::permissions::Permissions;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::Future;
use deno_core::futures::FutureExt;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

pub type DependencyMap = HashMap<String, Dependency>;
pub type FetchFuture = Pin<
  Box<
    (dyn Future<Output = Result<CachedModule, (ModuleSpecifier, AnyError)>>
       + 'static
       + Send),
  >,
>;

/// A group of errors that represent errors that can occur with an
/// an implementation of `SpecifierHandler`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HandlerError {
  /// A fetch error, where we have a location associated with it.
  FetchErrorWithLocation(String, Location),
}

impl fmt::Display for HandlerError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      HandlerError::FetchErrorWithLocation(ref err, ref location) => {
        write!(f, "{}\n    at {}", err, location)
      }
    }
  }
}

impl std::error::Error for HandlerError {}

#[derive(Debug, Clone)]
pub struct CachedModule {
  pub is_remote: bool,
  pub maybe_dependencies: Option<DependencyMap>,
  pub maybe_emit: Option<Emit>,
  pub maybe_emit_path: Option<(PathBuf, Option<PathBuf>)>,
  pub maybe_types: Option<String>,
  pub maybe_version: Option<String>,
  pub media_type: MediaType,
  pub requested_specifier: ModuleSpecifier,
  pub source: String,
  pub source_path: PathBuf,
  pub specifier: ModuleSpecifier,
}

impl Default for CachedModule {
  fn default() -> Self {
    let specifier = ModuleSpecifier::resolve_url("file:///example.js").unwrap();
    CachedModule {
      is_remote: false,
      maybe_dependencies: None,
      maybe_emit: None,
      maybe_emit_path: None,
      maybe_types: None,
      maybe_version: None,
      media_type: MediaType::Unknown,
      requested_specifier: specifier.clone(),
      source: "".to_string(),
      source_path: PathBuf::new(),
      specifier,
    }
  }
}

/// An enum to own the a specific emit.
///
/// Currently there is only one type of emit that is cacheable, but this has
/// been added to future proof the ability for the specifier handler
/// implementations to be able to handle other types of emits, like form a
/// runtime API which might have a different configuration.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Emit {
  /// Code that was emitted for use by the CLI
  Cli((String, Option<String>)),
}

impl Default for Emit {
  fn default() -> Self {
    Emit::Cli(("".to_string(), None))
  }
}

#[derive(Debug, Clone)]
pub struct Dependency {
  /// Flags if the dependency is a dynamic import or not.
  pub is_dynamic: bool,
  /// The location in the source code where the dependency statement occurred.
  pub location: Location,
  /// The module specifier that resolves to the runtime code dependency for the
  /// module.
  pub maybe_code: Option<ModuleSpecifier>,
  /// The module specifier that resolves to the type only dependency for the
  /// module.
  pub maybe_type: Option<ModuleSpecifier>,
}

impl Dependency {
  pub fn new(location: Location) -> Self {
    Dependency {
      is_dynamic: false,
      location,
      maybe_code: None,
      maybe_type: None,
    }
  }
}

pub trait SpecifierHandler: Sync + Send {
  /// Instructs the handler to fetch a specifier or retrieve its value from the
  /// cache.
  fn fetch(
    &mut self,
    specifier: ModuleSpecifier,
    maybe_location: Option<Location>,
    is_dynamic: bool,
  ) -> FetchFuture;

  /// Get the optional build info from the cache for a given module specifier.
  /// Because build infos are only associated with the "root" modules, they are
  /// not expected to be cached for each module, but are "lazily" checked when
  /// a root module is identified.  The `emit_type` also indicates what form
  /// of the module the build info is valid for.
  fn get_tsbuildinfo(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<String>, AnyError>;

  /// Set the emit for the module specifier.
  fn set_cache(
    &mut self,
    specifier: &ModuleSpecifier,
    emit: &Emit,
  ) -> Result<(), AnyError>;

  /// When parsed out of a JavaScript module source, the triple slash reference
  /// to the types should be stored in the cache.
  fn set_types(
    &mut self,
    specifier: &ModuleSpecifier,
    types: String,
  ) -> Result<(), AnyError>;

  /// Set the build info for a module specifier, also providing the cache type.
  fn set_tsbuildinfo(
    &mut self,
    specifier: &ModuleSpecifier,
    tsbuildinfo: String,
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
  /// An instance of disk where generated (emitted) files are stored.
  disk_cache: DiskCache,
  /// The set of current runtime permissions which need to be applied to
  /// dynamic imports.
  runtime_permissions: Permissions,
  /// A clone of the `program_state` file fetcher.
  file_fetcher: FileFetcher,
}

impl FetchHandler {
  pub fn new(
    program_state: &Arc<ProgramState>,
    runtime_permissions: Permissions,
  ) -> Result<Self, AnyError> {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();
    let deno_dir = DenoDir::new(custom_root)?;
    let disk_cache = deno_dir.gen_cache;
    let file_fetcher = program_state.file_fetcher.clone();

    Ok(FetchHandler {
      disk_cache,
      runtime_permissions,
      file_fetcher,
    })
  }
}

impl SpecifierHandler for FetchHandler {
  fn fetch(
    &mut self,
    requested_specifier: ModuleSpecifier,
    maybe_location: Option<Location>,
    is_dynamic: bool,
  ) -> FetchFuture {
    // When the module graph fetches dynamic modules, the set of dynamic
    // permissions need to be applied.  Other static imports have all
    // permissions.
    let permissions = if is_dynamic {
      self.runtime_permissions.clone()
    } else {
      Permissions::allow_all()
    };
    let file_fetcher = self.file_fetcher.clone();
    let disk_cache = self.disk_cache.clone();

    async move {
      let source_file = file_fetcher
        .fetch(&requested_specifier, &permissions)
        .await
        .map_err(|err| {
          let err = if let Some(e) = err.downcast_ref::<std::io::Error>() {
            if e.kind() == std::io::ErrorKind::NotFound {
              let message = if let Some(location) = &maybe_location {
                format!(
                  "Cannot resolve module \"{}\" from \"{}\".",
                  requested_specifier, location.filename
                )
              } else {
                format!("Cannot resolve module \"{}\".", requested_specifier)
              };
              custom_error("NotFound", message)
            } else {
              err
            }
          } else {
            err
          };
          if let Some(location) = maybe_location {
            // Injected modules (like test and eval) come with locations, but
            // they are confusing to the user to print out the location because
            // they cannot actually get to the source code that is quoted, as
            // it only exists in the runtime memory of Deno.
            if !location.filename.contains("$deno$") {
              (
                requested_specifier.clone(),
                HandlerError::FetchErrorWithLocation(err.to_string(), location)
                  .into(),
              )
            } else {
              (requested_specifier.clone(), err)
            }
          } else {
            (requested_specifier.clone(), err)
          }
        })?;
      let url = source_file.specifier.as_url();
      let is_remote = url.scheme() != "file";
      let filename = disk_cache.get_cache_filename_with_extension(url, "meta");
      let maybe_version = if let Some(filename) = filename {
        if let Ok(bytes) = disk_cache.get(&filename) {
          if let Ok(compiled_file_metadata) =
            CompiledFileMetadata::from_bytes(&bytes)
          {
            Some(compiled_file_metadata.version_hash)
          } else {
            None
          }
        } else {
          None
        }
      } else {
        None
      };

      let mut maybe_map_path = None;
      let map_path =
        disk_cache.get_cache_filename_with_extension(&url, "js.map");
      let maybe_map = if let Some(map_path) = map_path {
        if let Ok(map) = disk_cache.get(&map_path) {
          maybe_map_path = Some(disk_cache.location.join(map_path));
          Some(String::from_utf8(map).unwrap())
        } else {
          None
        }
      } else {
        None
      };
      let mut maybe_emit = None;
      let mut maybe_emit_path = None;
      let emit_path = disk_cache.get_cache_filename_with_extension(&url, "js");
      if let Some(emit_path) = emit_path {
        if let Ok(code) = disk_cache.get(&emit_path) {
          maybe_emit =
            Some(Emit::Cli((String::from_utf8(code).unwrap(), maybe_map)));
          maybe_emit_path =
            Some((disk_cache.location.join(emit_path), maybe_map_path));
        }
      };

      Ok(CachedModule {
        is_remote,
        maybe_dependencies: None,
        maybe_emit,
        maybe_emit_path,
        maybe_types: source_file.maybe_types,
        maybe_version,
        media_type: source_file.media_type,
        requested_specifier,
        source: source_file.source,
        source_path: source_file.local,
        specifier: source_file.specifier,
      })
    }
    .boxed()
  }

  fn get_tsbuildinfo(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<String>, AnyError> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier.as_url(), "buildinfo");
    if let Some(filename) = filename {
      if let Ok(tsbuildinfo) = self.disk_cache.get(&filename) {
        Ok(Some(String::from_utf8(tsbuildinfo)?))
      } else {
        Ok(None)
      }
    } else {
      Ok(None)
    }
  }

  fn set_tsbuildinfo(
    &mut self,
    specifier: &ModuleSpecifier,
    tsbuildinfo: String,
  ) -> Result<(), AnyError> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier.as_url(), "buildinfo")
      .unwrap();
    debug!("set_tsbuildinfo - filename {:?}", filename);
    self
      .disk_cache
      .set(&filename, tsbuildinfo.as_bytes())
      .map_err(|e| e.into())
  }

  fn set_cache(
    &mut self,
    specifier: &ModuleSpecifier,
    emit: &Emit,
  ) -> Result<(), AnyError> {
    match emit {
      Emit::Cli((code, maybe_map)) => {
        let url = specifier.as_url();
        let filename = self
          .disk_cache
          .get_cache_filename_with_extension(url, "js")
          .unwrap();
        self.disk_cache.set(&filename, code.as_bytes())?;

        if let Some(map) = maybe_map {
          let filename = self
            .disk_cache
            .get_cache_filename_with_extension(url, "js.map")
            .unwrap();
          self.disk_cache.set(&filename, map.as_bytes())?;
        }
      }
    };

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
      .get_cache_filename_with_extension(specifier.as_url(), "meta")
      .unwrap();

    self
      .disk_cache
      .set(
        &filename,
        compiled_file_metadata.to_json_string()?.as_bytes(),
      )
      .map_err(|e| e.into())
  }
}

pub struct MemoryHandler {
  sources: HashMap<String, String>,
}

impl MemoryHandler {
  pub fn new(sources: HashMap<String, String>) -> Self {
    Self { sources }
  }
}

impl SpecifierHandler for MemoryHandler {
  fn fetch(
    &mut self,
    specifier: ModuleSpecifier,
    _maybe_referrer: Option<Location>,
    _is_dynamic: bool,
  ) -> FetchFuture {
    let mut specifier_text = specifier.to_string();
    if !self.sources.contains_key(&specifier_text) {
      specifier_text = specifier_text.replace("file:///", "/");
      if !self.sources.contains_key(&specifier_text) {
        // Convert `C:/a/path/file.ts` to `/a/path/file.ts`
        specifier_text = specifier_text[3..].to_string()
      }
    }
    let result = if let Some(source) = self.sources.get(&specifier_text) {
      let media_type = MediaType::from(&specifier);
      let is_remote = specifier.as_url().scheme() != "file";

      Ok(CachedModule {
        source: source.to_string(),
        requested_specifier: specifier.clone(),
        specifier,
        media_type,
        is_remote,
        ..Default::default()
      })
    } else {
      Err((
        specifier.clone(),
        custom_error(
          "NotFound",
          format!("Unable to find specifier in sources: {}", specifier),
        ),
      ))
    };

    Box::pin(future::ready(result))
  }

  fn get_tsbuildinfo(
    &self,
    _specifier: &ModuleSpecifier,
  ) -> Result<Option<String>, AnyError> {
    Ok(None)
  }

  fn set_cache(
    &mut self,
    _specifier: &ModuleSpecifier,
    _emit: &Emit,
  ) -> Result<(), AnyError> {
    Ok(())
  }

  fn set_types(
    &mut self,
    _specifier: &ModuleSpecifier,
    _types: String,
  ) -> Result<(), AnyError> {
    Ok(())
  }

  fn set_tsbuildinfo(
    &mut self,
    _specifier: &ModuleSpecifier,
    _tsbuildinfo: String,
  ) -> Result<(), AnyError> {
    Ok(())
  }

  fn set_deps(
    &mut self,
    _specifier: &ModuleSpecifier,
    _dependencies: DependencyMap,
  ) -> Result<(), AnyError> {
    Ok(())
  }

  fn set_version(
    &mut self,
    _specifier: &ModuleSpecifier,
    _version: String,
  ) -> Result<(), AnyError> {
    Ok(())
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::file_fetcher::CacheSetting;
  use crate::http_cache::HttpCache;
  use tempfile::TempDir;

  macro_rules! map (
    { $($key:expr => $value:expr),+ } => {
      {
        let mut m = ::std::collections::HashMap::new();
        $(
          m.insert($key, $value);
        )+
        m
      }
    };
  );

  fn setup() -> (TempDir, FetchHandler) {
    let temp_dir = TempDir::new().expect("could not setup");
    let deno_dir = DenoDir::new(Some(temp_dir.path().to_path_buf()))
      .expect("could not setup");

    let file_fetcher = FileFetcher::new(
      HttpCache::new(&temp_dir.path().to_path_buf().join("deps")),
      CacheSetting::Use,
      true,
      None,
    )
    .expect("could not setup");
    let disk_cache = deno_dir.gen_cache;

    let fetch_handler = FetchHandler {
      disk_cache,
      runtime_permissions: Permissions::default(),
      file_fetcher,
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
    let cached_module: CachedModule = file_fetcher
      .fetch(specifier.clone(), None, false)
      .await
      .unwrap();
    assert!(cached_module.maybe_emit.is_none());
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
    let cached_module: CachedModule = file_fetcher
      .fetch(specifier.clone(), None, false)
      .await
      .unwrap();
    assert!(cached_module.maybe_emit.is_none());
    let code = String::from("some code");
    file_fetcher
      .set_cache(&specifier, &Emit::Cli((code, None)))
      .expect("could not set cache");
    let cached_module: CachedModule = file_fetcher
      .fetch(specifier.clone(), None, false)
      .await
      .unwrap();
    assert_eq!(
      cached_module.maybe_emit,
      Some(Emit::Cli(("some code".to_string(), None)))
    );
  }

  #[tokio::test]
  async fn test_fetch_handler_is_remote() {
    let _http_server_guard = test_util::http_server();
    let (_, mut file_fetcher) = setup();
    let specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/subdir/mod2.ts",
    )
    .unwrap();
    let cached_module: CachedModule =
      file_fetcher.fetch(specifier, None, false).await.unwrap();
    assert_eq!(cached_module.is_remote, true);
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let specifier = ModuleSpecifier::resolve_url_or_path(
      c.join("tests/subdir/mod1.ts").as_os_str().to_str().unwrap(),
    )
    .unwrap();
    let cached_module: CachedModule =
      file_fetcher.fetch(specifier, None, false).await.unwrap();
    assert_eq!(cached_module.is_remote, false);
  }

  #[tokio::test]
  async fn test_memory_handler_fetch() {
    let a_src = r#"
      import * as b from "./b.ts";
      console.log(b);
    "#;
    let b_src = r#"
      export const b = "b";
    "#;
    let c_src = r#"
      export const c = "c";
    "#;
    let d_src = r#"
      export const d: string;
    "#;
    let sources = map!(
      "/a.ts" => a_src,
      "/b.ts" => b_src,
      "https://deno.land/x/c.js" => c_src,
      "https://deno.land/x/d.d.ts" => d_src
    );
    let sources: HashMap<String, String> = sources
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect();
    let mut handler = MemoryHandler::new(sources);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///a.ts").unwrap();
    let actual: CachedModule = handler
      .fetch(specifier.clone(), None, false)
      .await
      .expect("could not fetch module");
    assert_eq!(actual.source, a_src.to_string());
    assert_eq!(actual.requested_specifier, specifier);
    assert_eq!(actual.specifier, specifier);
    assert_eq!(actual.media_type, MediaType::TypeScript);
    assert_eq!(actual.is_remote, false);

    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///b.ts").unwrap();
    let actual: CachedModule = handler
      .fetch(specifier.clone(), None, false)
      .await
      .expect("could not fetch module");
    assert_eq!(actual.source, b_src.to_string());
    assert_eq!(actual.requested_specifier, specifier);
    assert_eq!(actual.specifier, specifier);
    assert_eq!(actual.media_type, MediaType::TypeScript);
    assert_eq!(actual.is_remote, false);

    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/c.js").unwrap();
    let actual: CachedModule = handler
      .fetch(specifier.clone(), None, false)
      .await
      .expect("could not fetch module");
    assert_eq!(actual.source, c_src.to_string());
    assert_eq!(actual.requested_specifier, specifier);
    assert_eq!(actual.specifier, specifier);
    assert_eq!(actual.media_type, MediaType::JavaScript);
    assert_eq!(actual.is_remote, true);

    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/d.d.ts")
        .unwrap();
    let actual: CachedModule = handler
      .fetch(specifier.clone(), None, false)
      .await
      .expect("could not fetch module");
    assert_eq!(actual.source, d_src.to_string());
    assert_eq!(actual.requested_specifier, specifier);
    assert_eq!(actual.specifier, specifier);
    assert_eq!(actual.media_type, MediaType::Dts);
    assert_eq!(actual.is_remote, true);

    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/missing.ts")
        .unwrap();
    handler
      .fetch(specifier.clone(), None, false)
      .await
      .expect_err("should have errored");

    let specifier = ModuleSpecifier::resolve_url_or_path("/a.ts").unwrap();
    let actual: CachedModule = handler
      .fetch(specifier.clone(), None, false)
      .await
      .expect("could not fetch module");
    assert_eq!(actual.source, a_src.to_string());
    assert_eq!(actual.requested_specifier, specifier);
    assert_eq!(actual.specifier, specifier);
    assert_eq!(actual.media_type, MediaType::TypeScript);
    assert_eq!(actual.is_remote, false);

    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///C:/a.ts").unwrap();
    let actual: CachedModule = handler
      .fetch(specifier.clone(), None, false)
      .await
      .expect("could not fetch module");
    assert_eq!(actual.source, a_src.to_string());
    assert_eq!(actual.requested_specifier, specifier);
    assert_eq!(actual.specifier, specifier);
    assert_eq!(actual.media_type, MediaType::TypeScript);
    assert_eq!(actual.is_remote, false);
  }
}
