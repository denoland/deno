// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CacheSetting;
use crate::errors::get_error_class_name;
use crate::file_fetcher::FetchOptions;
use crate::file_fetcher::FileFetcher;
use crate::util::fs::atomic_write_file;

use deno_ast::MediaType;
use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_runtime::permissions::PermissionsContainer;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

mod cache_db;
mod caches;
mod check;
mod common;
mod deno_dir;
mod disk_cache;
mod emit;
mod incremental;
mod node;
mod parsed_source;

pub use caches::Caches;
pub use check::TypeCheckCache;
pub use common::FastInsecureHasher;
pub use deno_dir::DenoDir;
pub use deno_dir::DenoDirProvider;
pub use disk_cache::DiskCache;
pub use emit::EmitCache;
pub use incremental::IncrementalCache;
pub use node::NodeAnalysisCache;
pub use parsed_source::ParsedSourceCache;

/// Permissions used to save a file in the disk caches.
pub const CACHE_PERM: u32 = 0o644;

#[derive(Debug, Clone)]
pub struct RealDenoCacheEnv;

impl deno_cache_dir::DenoCacheEnv for RealDenoCacheEnv {
  fn read_file_bytes(&self, path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
      Ok(s) => Ok(Some(s)),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(err) => Err(err),
    }
  }

  fn atomic_write_file(
    &self,
    path: &Path,
    bytes: &[u8],
  ) -> std::io::Result<()> {
    atomic_write_file(path, bytes, CACHE_PERM)
  }

  fn modified(&self, path: &Path) -> std::io::Result<Option<SystemTime>> {
    match std::fs::metadata(path) {
      Ok(metadata) => Ok(Some(
        metadata.modified().unwrap_or_else(|_| SystemTime::now()),
      )),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(err) => Err(err),
    }
  }

  fn is_file(&self, path: &Path) -> bool {
    path.is_file()
  }

  fn time_now(&self) -> SystemTime {
    SystemTime::now()
  }
}

pub type GlobalHttpCache = deno_cache_dir::GlobalHttpCache<RealDenoCacheEnv>;
pub type LocalHttpCache = deno_cache_dir::LocalHttpCache<RealDenoCacheEnv>;
pub type LocalLspHttpCache =
  deno_cache_dir::LocalLspHttpCache<RealDenoCacheEnv>;
pub use deno_cache_dir::CachedUrlMetadata;
pub use deno_cache_dir::HttpCache;

/// A "wrapper" for the FileFetcher and DiskCache for the Deno CLI that provides
/// a concise interface to the DENO_DIR when building module graphs.
pub struct FetchCacher {
  emit_cache: EmitCache,
  file_fetcher: Arc<FileFetcher>,
  file_header_overrides: HashMap<ModuleSpecifier, HashMap<String, String>>,
  global_http_cache: Arc<GlobalHttpCache>,
  parsed_source_cache: Arc<ParsedSourceCache>,
  permissions: PermissionsContainer,
  cache_info_enabled: bool,
  maybe_local_node_modules_url: Option<ModuleSpecifier>,
}

impl FetchCacher {
  pub fn new(
    emit_cache: EmitCache,
    file_fetcher: Arc<FileFetcher>,
    file_header_overrides: HashMap<ModuleSpecifier, HashMap<String, String>>,
    global_http_cache: Arc<GlobalHttpCache>,
    parsed_source_cache: Arc<ParsedSourceCache>,
    permissions: PermissionsContainer,
    maybe_local_node_modules_url: Option<ModuleSpecifier>,
  ) -> Self {
    Self {
      emit_cache,
      file_fetcher,
      file_header_overrides,
      global_http_cache,
      parsed_source_cache,
      permissions,
      cache_info_enabled: false,
      maybe_local_node_modules_url,
    }
  }

  /// The cache information takes a bit of time to fetch and it's
  /// not always necessary. It should only be enabled for deno info.
  pub fn enable_loading_cache_info(&mut self) {
    self.cache_info_enabled = true;
  }

  // DEPRECATED: Where the file is stored and how it's stored should be an implementation
  // detail of the cache.
  //
  // todo(dsheret): remove once implementing
  //  * https://github.com/denoland/deno/issues/17707
  //  * https://github.com/denoland/deno/issues/17703
  #[deprecated(
    note = "There should not be a way to do this because the file may not be cached at a local path in the future."
  )]
  fn get_local_path(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    // TODO(@kitsonk) fix when deno_graph does not query cache for synthetic
    // modules
    if specifier.scheme() == "flags" {
      None
    } else if specifier.scheme() == "file" {
      specifier.to_file_path().ok()
    } else {
      #[allow(deprecated)]
      self
        .global_http_cache
        .get_global_cache_filepath(specifier)
        .ok()
    }
  }
}

static DENO_REGISTRY_URL: Lazy<Url> = Lazy::new(|| {
  let env_var_name = "DENO_REGISTRY_URL";
  if let Ok(registry_url) = std::env::var(env_var_name) {
    // ensure there is a trailing slash for the directory
    let registry_url = format!("{}/", registry_url.trim_end_matches('/'));
    match Url::parse(&registry_url) {
      Ok(url) => {
        return url;
      }
      Err(err) => {
        log::debug!("Invalid {} environment variable: {:#}", env_var_name, err,);
      }
    }
  }

  deno_graph::source::DEFAULT_DENO_REGISTRY_URL.clone()
});

impl Loader for FetchCacher {
  fn registry_url(&self) -> &Url {
    &DENO_REGISTRY_URL
  }

  fn get_cache_info(&self, specifier: &ModuleSpecifier) -> Option<CacheInfo> {
    if !self.cache_info_enabled {
      return None;
    }

    #[allow(deprecated)]
    let local = self.get_local_path(specifier)?;
    if local.is_file() {
      let emit = self
        .emit_cache
        .get_emit_filepath(specifier)
        .filter(|p| p.is_file());
      Some(CacheInfo {
        local: Some(local),
        emit,
        map: None,
      })
    } else {
      None
    }
  }

  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    _is_dynamic: bool,
    cache_setting: deno_graph::source::CacheSetting,
  ) -> LoadFuture {
    use deno_graph::source::CacheSetting as LoaderCacheSetting;

    if let Some(node_modules_url) = self.maybe_local_node_modules_url.as_ref() {
      // The specifier might be in a completely different symlinked tree than
      // what the resolved node_modules_url is in (ex. `/my-project-1/node_modules`
      // symlinked to `/my-project-2/node_modules`), so first check if the path
      // is in a node_modules dir to avoid needlessly canonicalizing, then compare
      // against the canonicalized specifier.
      if specifier.path().contains("/node_modules/") {
        let specifier =
          crate::node::resolve_specifier_into_node_modules(specifier);
        if specifier.as_str().starts_with(node_modules_url.as_str()) {
          return Box::pin(futures::future::ready(Ok(Some(
            LoadResponse::External { specifier },
          ))));
        }
      }
    }

    let file_fetcher = self.file_fetcher.clone();
    let file_header_overrides = self.file_header_overrides.clone();
    let permissions = self.permissions.clone();
    let specifier = specifier.clone();

    async move {
      let maybe_cache_setting = match cache_setting {
        LoaderCacheSetting::Use => None,
        // todo: error if cache setting is currently `Only`
        LoaderCacheSetting::Reload => Some(CacheSetting::ReloadAll),
        LoaderCacheSetting::Only => Some(CacheSetting::Only),
      };
      file_fetcher
        .fetch_with_options(FetchOptions {
          specifier: &specifier,
          permissions,
          maybe_accept: None,
          maybe_cache_setting: maybe_cache_setting.as_ref(),
        })
        .await
        .map(|file| {
          let maybe_headers =
            match (file.maybe_headers, file_header_overrides.get(&specifier)) {
              (Some(headers), Some(overrides)) => {
                Some(headers.into_iter().chain(overrides.clone()).collect())
              }
              (Some(headers), None) => Some(headers),
              (None, Some(overrides)) => Some(overrides.clone()),
              (None, None) => None,
            };
          Ok(Some(LoadResponse::Module {
            specifier: file.specifier,
            maybe_headers,
            content: file.source,
          }))
        })
        .unwrap_or_else(|err| {
          if let Some(err) = err.downcast_ref::<std::io::Error>() {
            if err.kind() == std::io::ErrorKind::NotFound {
              return Ok(None);
            }
          } else if get_error_class_name(&err) == "NotFound" {
            return Ok(None);
          }
          Err(err)
        })
    }
    .boxed()
  }

  fn cache_module_info(
    &mut self,
    specifier: &ModuleSpecifier,
    source: &str,
    module_info: &deno_graph::ModuleInfo,
  ) {
    let result = self.parsed_source_cache.cache_module_info(
      specifier,
      MediaType::from_specifier(specifier),
      source,
      module_info,
    );
    if let Err(err) = result {
      log::debug!(
        "Error saving module cache info for {}. {:#}",
        specifier,
        err
      );
    }
  }
}
