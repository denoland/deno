// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::disk_cache::DiskCache;
use crate::errors::get_error_class_name;
use crate::file_fetcher::FileFetcher;

use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::FutureExt;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_runtime::permissions::Permissions;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize)]
pub struct EmitMetadata {
  pub version_hash: String,
}

pub(crate) enum CacheType {
  Declaration,
  Emit,
  SourceMap,
  TypeScriptBuildInfo,
  Version,
}

/// A trait which provides a concise implementation to getting and setting
/// values in a cache.
pub(crate) trait Cacher {
  /// Get a value from the cache.
  fn get(
    &self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
  ) -> Option<String>;
  /// Set a value in the cache.
  fn set(
    &mut self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
    value: String,
  ) -> Result<(), AnyError>;
}

/// Combines the cacher trait along with the deno_graph Loader trait to provide
/// a single interface to be able to load and cache modules when building a
/// graph.
pub(crate) trait CacherLoader: Cacher + Loader {
  fn as_cacher(&self) -> &dyn Cacher;
  fn as_mut_loader(&mut self) -> &mut dyn Loader;
  fn as_mut_cacher(&mut self) -> &mut dyn Cacher;
}

/// A "wrapper" for the FileFetcher and DiskCache for the Deno CLI that provides
/// a concise interface to the DENO_DIR when building module graphs.
pub(crate) struct FetchCacher {
  disk_cache: DiskCache,
  dynamic_permissions: Permissions,
  file_fetcher: Arc<FileFetcher>,
  root_permissions: Permissions,
}

impl FetchCacher {
  pub fn new(
    disk_cache: DiskCache,
    file_fetcher: FileFetcher,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
  ) -> Self {
    let file_fetcher = Arc::new(file_fetcher);

    Self {
      disk_cache,
      dynamic_permissions,
      file_fetcher,
      root_permissions,
    }
  }

  fn get_emit_metadata(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<EmitMetadata> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "meta")?;
    let bytes = self.disk_cache.get(&filename).ok()?;
    serde_json::from_slice(&bytes).ok()
  }

  fn set_emit_metadata(
    &self,
    specifier: &ModuleSpecifier,
    data: EmitMetadata,
  ) -> Result<(), AnyError> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "meta")
      .unwrap();
    let bytes = serde_json::to_vec(&data)?;
    self.disk_cache.set(&filename, &bytes).map_err(|e| e.into())
  }
}

impl Loader for FetchCacher {
  fn get_cache_info(&self, specifier: &ModuleSpecifier) -> Option<CacheInfo> {
    let local = self.file_fetcher.get_local_path(specifier)?;
    if local.is_file() {
      let location = &self.disk_cache.location;
      let emit = self
        .disk_cache
        .get_cache_filename_with_extension(specifier, "js")
        .map(|p| location.join(p))
        .filter(|p| p.is_file());
      let map = self
        .disk_cache
        .get_cache_filename_with_extension(specifier, "js.map")
        .map(|p| location.join(p))
        .filter(|p| p.is_file());
      Some(CacheInfo {
        local: Some(local),
        emit,
        map,
      })
    } else {
      None
    }
  }

  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    is_dynamic: bool,
  ) -> LoadFuture {
    let specifier = specifier.clone();
    let mut permissions = if is_dynamic {
      self.dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    let file_fetcher = self.file_fetcher.clone();

    async move {
      file_fetcher
        .fetch(&specifier, &mut permissions)
        .await
        .map_or_else(
          |err| {
            if let Some(err) = err.downcast_ref::<std::io::Error>() {
              if err.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
              }
            } else if get_error_class_name(&err) == "NotFound" {
              return Ok(None);
            }
            Err(err)
          },
          |file| {
            Ok(Some(LoadResponse::Module {
              specifier: file.specifier,
              maybe_headers: file.maybe_headers,
              content: file.source,
            }))
          },
        )
    }
    .boxed()
  }
}

impl Cacher for FetchCacher {
  fn get(
    &self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    let extension = match cache_type {
      CacheType::Declaration => "d.ts",
      CacheType::Emit => "js",
      CacheType::SourceMap => "js.map",
      CacheType::TypeScriptBuildInfo => "buildinfo",
      CacheType::Version => {
        return self.get_emit_metadata(specifier).map(|d| d.version_hash)
      }
    };
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, extension)?;
    self
      .disk_cache
      .get(&filename)
      .ok()
      .and_then(|b| String::from_utf8(b).ok())
  }

  fn set(
    &mut self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
    value: String,
  ) -> Result<(), AnyError> {
    let extension = match cache_type {
      CacheType::Declaration => "d.ts",
      CacheType::Emit => "js",
      CacheType::SourceMap => "js.map",
      CacheType::TypeScriptBuildInfo => "buildinfo",
      CacheType::Version => {
        let data = if let Some(mut data) = self.get_emit_metadata(specifier) {
          data.version_hash = value;
          data
        } else {
          EmitMetadata {
            version_hash: value,
          }
        };
        return self.set_emit_metadata(specifier, data);
      }
    };
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, extension)
      .unwrap();
    self
      .disk_cache
      .set(&filename, value.as_bytes())
      .map_err(|e| e.into())
  }
}

impl CacherLoader for FetchCacher {
  fn as_cacher(&self) -> &dyn Cacher {
    self
  }

  fn as_mut_loader(&mut self) -> &mut dyn Loader {
    self
  }

  fn as_mut_cacher(&mut self) -> &mut dyn Cacher {
    self
  }
}

/// An in memory cache that is used by the runtime `Deno.emit()` API to provide
/// the same behavior as the disk cache when sources are provided.
#[derive(Debug)]
pub(crate) struct MemoryCacher {
  sources: HashMap<String, Arc<String>>,
  declarations: HashMap<ModuleSpecifier, String>,
  emits: HashMap<ModuleSpecifier, String>,
  maps: HashMap<ModuleSpecifier, String>,
  build_infos: HashMap<ModuleSpecifier, String>,
  versions: HashMap<ModuleSpecifier, String>,
}

impl MemoryCacher {
  pub fn new(sources: HashMap<String, Arc<String>>) -> Self {
    Self {
      sources,
      declarations: HashMap::default(),
      emits: HashMap::default(),
      maps: HashMap::default(),
      build_infos: HashMap::default(),
      versions: HashMap::default(),
    }
  }
}

impl Loader for MemoryCacher {
  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    _is_dynamic: bool,
  ) -> LoadFuture {
    let mut specifier_str = specifier.to_string();
    if !self.sources.contains_key(&specifier_str) {
      specifier_str = specifier_str.replace("file:///", "/");
      if !self.sources.contains_key(&specifier_str) {
        specifier_str = specifier_str[3..].to_string();
      }
    }
    let response =
      self
        .sources
        .get(&specifier_str)
        .map(|c| LoadResponse::Module {
          specifier: specifier.clone(),
          maybe_headers: None,
          content: c.to_owned(),
        });
    Box::pin(future::ready(Ok(response)))
  }
}

impl Cacher for MemoryCacher {
  fn get(
    &self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    match cache_type {
      CacheType::Declaration => self.declarations.get(specifier).cloned(),
      CacheType::Emit => self.emits.get(specifier).cloned(),
      CacheType::SourceMap => self.maps.get(specifier).cloned(),
      CacheType::TypeScriptBuildInfo => {
        self.build_infos.get(specifier).cloned()
      }
      CacheType::Version => self.versions.get(specifier).cloned(),
    }
  }

  fn set(
    &mut self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
    value: String,
  ) -> Result<(), AnyError> {
    match cache_type {
      CacheType::Declaration => {
        self.declarations.insert(specifier.clone(), value)
      }
      CacheType::Emit => self.emits.insert(specifier.clone(), value),
      CacheType::SourceMap => self.maps.insert(specifier.clone(), value),
      CacheType::TypeScriptBuildInfo => {
        self.build_infos.insert(specifier.clone(), value)
      }
      CacheType::Version => self.versions.insert(specifier.clone(), value),
    };
    Ok(())
  }
}

impl CacherLoader for MemoryCacher {
  fn as_cacher(&self) -> &dyn Cacher {
    self
  }

  fn as_mut_loader(&mut self) -> &mut dyn Loader {
    self
  }

  fn as_mut_cacher(&mut self) -> &mut dyn Cacher {
    self
  }
}
