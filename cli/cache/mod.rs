// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::errors::get_error_class_name;
use crate::file_fetcher::FileFetcher;
use crate::npm;

use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_runtime::permissions::PermissionsContainer;
use std::sync::Arc;

mod check;
mod common;
mod deno_dir;
mod disk_cache;
mod emit;
mod http_cache;
mod incremental;
mod node;
mod parsed_source;

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
  root_permissions: PermissionsContainer,
}

impl FetchCacher {
  pub fn new(
    emit_cache: EmitCache,
    file_fetcher: Arc<FileFetcher>,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Self {
    Self {
      emit_cache,
      dynamic_permissions,
      file_fetcher,
      root_permissions,
    }
  }
}

impl Loader for FetchCacher {
  fn get_cache_info(&self, specifier: &ModuleSpecifier) -> Option<CacheInfo> {
    if specifier.scheme() == "npm" {
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
    if specifier.scheme() == "npm" {
      return Box::pin(futures::future::ready(
        match npm::NpmPackageReference::from_specifier(specifier) {
          Ok(_) => Ok(Some(deno_graph::source::LoadResponse::External {
            specifier: specifier.clone(),
          })),
          Err(err) => Err(err),
        },
      ));
    }

    let specifier = specifier.clone();
    let permissions = if is_dynamic {
      self.dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    let file_fetcher = self.file_fetcher.clone();

    async move {
      file_fetcher
        .fetch(&specifier, permissions)
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
