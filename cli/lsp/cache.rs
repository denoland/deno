// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::cache::DenoDir;
use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::cache::LocalLspHttpCache;
use crate::lsp::config::Config;
use crate::lsp::logging::lsp_log;
use crate::lsp::logging::lsp_warn;
use deno_runtime::fs_util::specifier_to_file_path;

use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

/// In the LSP, we disallow the cache from automatically copying from
/// the global cache to the local cache for technical reasons.
///
/// 1. We need to verify the checksums from the lockfile are correct when
///    moving from the global to the local cache.
/// 2. We need to verify the checksums for JSR https specifiers match what
///    is found in the package's manifest.
pub const LSP_DISALLOW_GLOBAL_TO_LOCAL_COPY: deno_cache_dir::GlobalToLocalCopy =
  deno_cache_dir::GlobalToLocalCopy::Disallow;

pub fn calculate_fs_version(
  cache: &LspCache,
  specifier: &ModuleSpecifier,
) -> Option<String> {
  match specifier.scheme() {
    "npm" | "node" | "data" | "blob" => None,
    "file" => specifier_to_file_path(specifier)
      .ok()
      .and_then(|path| calculate_fs_version_at_path(&path)),
    _ => calculate_fs_version_in_cache(cache, specifier),
  }
}

/// Calculate a version for for a given path.
pub fn calculate_fs_version_at_path(path: &Path) -> Option<String> {
  let metadata = fs::metadata(path).ok()?;
  if let Ok(modified) = metadata.modified() {
    if let Ok(n) = modified.duration_since(SystemTime::UNIX_EPOCH) {
      Some(n.as_millis().to_string())
    } else {
      Some("1".to_string())
    }
  } else {
    Some("1".to_string())
  }
}

fn calculate_fs_version_in_cache(
  cache: &LspCache,
  specifier: &ModuleSpecifier,
) -> Option<String> {
  let http_cache = cache.root_vendor_or_global();
  let Ok(cache_key) = http_cache.cache_item_key(specifier) else {
    return Some("1".to_string());
  };
  match http_cache.read_modified_time(&cache_key) {
    Ok(Some(modified)) => {
      match modified.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => Some(n.as_millis().to_string()),
        Err(_) => Some("1".to_string()),
      }
    }
    Ok(None) => None,
    Err(_) => Some("1".to_string()),
  }
}

#[derive(Debug, Clone)]
pub struct LspCache {
  deno_dir: DenoDir,
  global: Arc<GlobalHttpCache>,
  root_vendor: Option<Arc<LocalLspHttpCache>>,
}

impl Default for LspCache {
  fn default() -> Self {
    Self::new(None)
  }
}

impl LspCache {
  pub fn new(global_cache_url: Option<Url>) -> Self {
    let global_cache_path = global_cache_url.and_then(|s| {
      specifier_to_file_path(&s)
        .inspect(|p| {
          lsp_log!("Resolved global cache path: \"{}\"", p.to_string_lossy());
        })
        .inspect_err(|err| {
          lsp_warn!("Failed to resolve custom cache path: {err}");
        })
        .ok()
    });
    let deno_dir = DenoDir::new(global_cache_path)
      .expect("should be infallible with absolute custom root");
    let global = Arc::new(GlobalHttpCache::new(
      deno_dir.deps_folder_path(),
      crate::cache::RealDenoCacheEnv,
    ));
    Self {
      deno_dir,
      global,
      root_vendor: None,
    }
  }

  pub fn update_config(&mut self, config: &Config) {
    self.root_vendor = config.tree.root_data().and_then(|data| {
      let vendor_dir = data.vendor_dir.as_ref()?;
      Some(Arc::new(LocalLspHttpCache::new(
        vendor_dir.clone(),
        self.global.clone(),
      )))
    });
  }

  pub fn deno_dir(&self) -> &DenoDir {
    &self.deno_dir
  }

  pub fn global(&self) -> &Arc<GlobalHttpCache> {
    &self.global
  }

  pub fn root_vendor(&self) -> Option<&Arc<LocalLspHttpCache>> {
    self.root_vendor.as_ref()
  }

  pub fn root_vendor_or_global(&self) -> Arc<dyn HttpCache> {
    self
      .root_vendor
      .as_ref()
      .map(|v| v.clone() as _)
      .unwrap_or(self.global.clone() as _)
  }
}
