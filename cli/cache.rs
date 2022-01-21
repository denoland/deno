// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::errors::get_error_class_name;
use crate::file_fetcher::FileFetcher;

use deno_core::futures::future;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_runtime::permissions::Permissions;
use std::collections::HashMap;
use std::sync::Arc;

/// A "wrapper" for the FileFetcher for the Deno CLI that provides
/// a concise interface to the DENO_DIR when building module graphs.
pub(crate) struct FetchCacher {
  dynamic_permissions: Permissions,
  file_fetcher: Arc<FileFetcher>,
  root_permissions: Permissions,
}

impl FetchCacher {
  pub fn new(
    file_fetcher: FileFetcher,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
  ) -> Self {
    let file_fetcher = Arc::new(file_fetcher);

    Self {
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
      Some(CacheInfo {
        local: Some(local),
        emit: None,
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
            Ok(Some(LoadResponse {
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

/// An in memory cache that is used by the runtime `Deno.emit()` API.
#[derive(Debug)]
pub(crate) struct MemoryCacher {
  sources: HashMap<String, Arc<String>>,
}

impl MemoryCacher {
  pub fn new(sources: HashMap<String, Arc<String>>) -> Self {
    Self { sources }
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
    let response = self.sources.get(&specifier_str).map(|c| LoadResponse {
      specifier: specifier.clone(),
      maybe_headers: None,
      content: c.to_owned(),
    });
    Box::pin(future::ready(Ok(response)))
  }
}
