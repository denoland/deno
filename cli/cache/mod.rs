// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::errors::get_error_class_name;
use crate::file_fetcher::FileFetcher;

use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_runtime::permissions::PermissionsContainer;
use std::collections::HashMap;
use std::sync::Arc;

mod cache_db;
mod caches;
mod check;
mod common;
mod deno_dir;
mod disk_cache;
mod emit;
mod http_cache;
mod incremental;
mod node;
mod parsed_source;

pub use caches::Caches;
pub use check::TypeCheckCache;
pub use common::FastInsecureHasher;
pub use deno_dir::DenoDir;
pub use disk_cache::DiskCache;
pub use emit::EmitCache;
pub use http_cache::CachedUrlMetadata;
pub use http_cache::HttpCache;
pub use incremental::IncrementalCache;
pub use node::NodeAnalysisCache;
pub use parsed_source::ParsedSourceCache;

/// Permissions used to save a file in the disk caches.
pub const CACHE_PERM: u32 = 0o644;

/// A "wrapper" for the FileFetcher and DiskCache for the Deno CLI that provides
/// a concise interface to the DENO_DIR when building module graphs.
pub struct FetchCacher {
  emit_cache: EmitCache,
  dynamic_permissions: PermissionsContainer,
  file_fetcher: Arc<FileFetcher>,
  file_header_overrides: HashMap<ModuleSpecifier, HashMap<String, String>>,
  root_permissions: PermissionsContainer,
  cache_info_enabled: bool,
  maybe_local_node_modules_url: Option<ModuleSpecifier>,
}

impl FetchCacher {
  pub fn new(
    emit_cache: EmitCache,
    file_fetcher: Arc<FileFetcher>,
    file_header_overrides: HashMap<ModuleSpecifier, HashMap<String, String>>,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
    maybe_local_node_modules_url: Option<ModuleSpecifier>,
  ) -> Self {
    Self {
      emit_cache,
      dynamic_permissions,
      file_fetcher,
      file_header_overrides,
      root_permissions,
      cache_info_enabled: false,
      maybe_local_node_modules_url,
    }
  }

  /// The cache information takes a bit of time to fetch and it's
  /// not always necessary. It should only be enabled for deno info.
  pub fn enable_loading_cache_info(&mut self) {
    self.cache_info_enabled = true;
  }
}

impl Loader for FetchCacher {
  fn get_cache_info(&self, specifier: &ModuleSpecifier) -> Option<CacheInfo> {
    if !self.cache_info_enabled {
      return None;
    }

    let local = self.file_fetcher.get_local_path(specifier)?;
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
    is_dynamic: bool,
  ) -> LoadFuture {
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

    let permissions = if is_dynamic {
      self.dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    let file_fetcher = self.file_fetcher.clone();
    let file_header_overrides = self.file_header_overrides.clone();
    let specifier = specifier.clone();

    async move {
      file_fetcher
        .fetch(&specifier, permissions)
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
}
