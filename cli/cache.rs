// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::disk_cache::DiskCache;
use crate::errors::get_error_class_name;
use crate::file_fetcher::FileFetcher;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::ModuleSpecifier;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_runtime::permissions::Permissions;
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize)]
pub struct EmitMetadata {
  pub version_hash: String,
}

pub enum CacheType {
  Emit,
  SourceMap,
  TypeScriptBuildInfo,
  Version,
}

/// A trait which provides a concise implementation to getting and setting
/// values in a cache.
pub trait Cacher {
  /// Get a value from the cache.
  fn get(
    &self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
  ) -> Option<String>;
  /// Set a value in the cache.
  fn set(
    &self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
    value: String,
  ) -> Result<(), AnyError>;
}

/// A "wrapper" for the FileFetcher and DiskCache for the Deno CLI that provides
/// a concise interface to the DENO_DIR when building module graphs.
pub struct FetchCacher {
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
