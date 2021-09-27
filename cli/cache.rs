// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::disk_cache::DiskCache;
use crate::file_fetcher::FileFetcher;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
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
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize)]
pub struct EmitMetadata {
  pub version_hash: String,
}

pub(crate) trait Cacher {
  fn get_emit(&self, specifier: &ModuleSpecifier) -> Option<String>;
  fn get_map(&self, specifier: &ModuleSpecifier) -> Option<String>;
  fn get_tsbuildinfo(&self, specifier: &ModuleSpecifier) -> Option<String>;
  fn get_version(&self, specifier: &ModuleSpecifier) -> Option<String>;
  fn set_emit(
    &mut self,
    specifier: &ModuleSpecifier,
    emit: String,
  ) -> Result<(), AnyError>;
  fn set_map(
    &mut self,
    specifier: &ModuleSpecifier,
    map: String,
  ) -> Result<(), AnyError>;
  fn set_tsbuildinfo(
    &mut self,
    specifier: &ModuleSpecifier,
    info: String,
  ) -> Result<(), AnyError>;
  fn set_version(
    &mut self,
    specifier: &ModuleSpecifier,
    hash: String,
  ) -> Result<(), AnyError>;
}

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
      let emit = match self
        .disk_cache
        .get_cache_filename_with_extension(specifier, "js")
        .map(|p| location.join(p))
      {
        Some(path_buf) => {
          if path_buf.is_file() {
            Some(path_buf)
          } else {
            None
          }
        }
        _ => None,
      };
      let map = match self
        .disk_cache
        .get_cache_filename_with_extension(specifier, "js.map")
        .map(|p| location.join(p))
      {
        Some(path_buf) => {
          if path_buf.is_file() {
            Some(path_buf)
          } else {
            None
          }
        }
        _ => None,
      };
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
      let load_result = file_fetcher
        .fetch(&specifier, &mut permissions)
        .await
        .map_or_else(
          |err| {
            if let Some(err) = err.downcast_ref::<std::io::Error>() {
              if err.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
              } else {
                Err(anyhow!("{}", err))
              }
            } else {
              Err(anyhow!("{}", err))
            }
          },
          |file| {
            Ok(Some(LoadResponse {
              specifier: file.specifier,
              maybe_headers: file.maybe_headers,
              content: file.source,
            }))
          },
        );

      (specifier, load_result)
    }
    .boxed()
  }
}

impl Cacher for FetchCacher {
  fn get_emit(&self, specifier: &ModuleSpecifier) -> Option<String> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "js")?;
    self
      .disk_cache
      .get(&filename)
      .ok()
      .map(|b| String::from_utf8(b).ok())
      .flatten()
  }

  fn get_map(&self, specifier: &ModuleSpecifier) -> Option<String> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "js.map")?;
    self
      .disk_cache
      .get(&filename)
      .ok()
      .map(|b| String::from_utf8(b).ok())
      .flatten()
  }

  fn get_tsbuildinfo(&self, specifier: &ModuleSpecifier) -> Option<String> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "buildinfo")?;
    self
      .disk_cache
      .get(&filename)
      .ok()
      .map(|b| String::from_utf8(b).ok())
      .flatten()
  }

  fn get_version(&self, specifier: &ModuleSpecifier) -> Option<String> {
    self.get_emit_metadata(specifier).map(|d| d.version_hash)
  }

  fn set_emit(
    &mut self,
    specifier: &ModuleSpecifier,
    emit: String,
  ) -> Result<(), AnyError> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "js")
      .unwrap();
    self
      .disk_cache
      .set(&filename, emit.as_bytes())
      .map_err(|e| e.into())
  }

  fn set_map(
    &mut self,
    specifier: &ModuleSpecifier,
    map: String,
  ) -> Result<(), AnyError> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "js.map")
      .unwrap();
    self
      .disk_cache
      .set(&filename, map.as_bytes())
      .map_err(|e| e.into())
  }

  fn set_tsbuildinfo(
    &mut self,
    specifier: &ModuleSpecifier,
    info: String,
  ) -> Result<(), AnyError> {
    let filename = self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "buildinfo")
      .unwrap();
    self
      .disk_cache
      .set(&filename, info.as_bytes())
      .map_err(|e| e.into())
  }

  fn set_version(
    &mut self,
    specifier: &ModuleSpecifier,
    version_hash: String,
  ) -> Result<(), AnyError> {
    let data = if let Some(mut data) = self.get_emit_metadata(specifier) {
      data.version_hash = version_hash;
      data
    } else {
      EmitMetadata { version_hash }
    };
    self.set_emit_metadata(specifier, data)
  }
}
