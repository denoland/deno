// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::cache::DenoDir;
use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::cache::LocalLspHttpCache;
use crate::lsp::config::Config;
use crate::lsp::logging::lsp_log;
use crate::lsp::logging::lsp_warn;

use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_path_util::url_to_file_path;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

pub fn calculate_fs_version(
  cache: &LspCache,
  specifier: &ModuleSpecifier,
  file_referrer: Option<&ModuleSpecifier>,
) -> Option<String> {
  match specifier.scheme() {
    "npm" | "node" | "data" | "blob" => None,
    "file" => url_to_file_path(specifier)
      .ok()
      .and_then(|path| calculate_fs_version_at_path(&path)),
    _ => calculate_fs_version_in_cache(cache, specifier, file_referrer),
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
  file_referrer: Option<&ModuleSpecifier>,
) -> Option<String> {
  let http_cache = cache.for_specifier(file_referrer);
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
  vendors_by_scope: BTreeMap<ModuleSpecifier, Option<Arc<LocalLspHttpCache>>>,
}

impl Default for LspCache {
  fn default() -> Self {
    Self::new(None)
  }
}

impl LspCache {
  pub fn new(global_cache_url: Option<Url>) -> Self {
    let global_cache_path = global_cache_url.and_then(|s| {
      url_to_file_path(&s)
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
      deno_dir.remote_folder_path(),
      crate::cache::RealDenoCacheEnv,
    ));
    Self {
      deno_dir,
      global,
      vendors_by_scope: Default::default(),
    }
  }

  pub fn update_config(&mut self, config: &Config) {
    self.vendors_by_scope = config
      .tree
      .data_by_scope()
      .iter()
      .map(|(scope, config_data)| {
        (
          scope.clone(),
          config_data.vendor_dir.as_ref().map(|v| {
            Arc::new(LocalLspHttpCache::new(v.clone(), self.global.clone()))
          }),
        )
      })
      .collect();
  }

  pub fn deno_dir(&self) -> &DenoDir {
    &self.deno_dir
  }

  pub fn global(&self) -> &Arc<GlobalHttpCache> {
    &self.global
  }

  pub fn for_specifier(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Arc<dyn HttpCache> {
    let Some(file_referrer) = file_referrer else {
      return self.global.clone();
    };
    self
      .vendors_by_scope
      .iter()
      .rfind(|(s, _)| file_referrer.as_str().starts_with(s.as_str()))
      .and_then(|(_, v)| v.clone().map(|v| v as _))
      .unwrap_or(self.global.clone() as _)
  }

  pub fn vendored_specifier(
    &self,
    specifier: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<ModuleSpecifier> {
    let file_referrer = file_referrer?;
    if !matches!(specifier.scheme(), "http" | "https") {
      return None;
    }
    let vendor = self
      .vendors_by_scope
      .iter()
      .rfind(|(s, _)| file_referrer.as_str().starts_with(s.as_str()))?
      .1
      .as_ref()?;
    vendor.get_file_url(specifier)
  }

  pub fn unvendored_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let path = url_to_file_path(specifier).ok()?;
    let vendor = self
      .vendors_by_scope
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))?
      .1
      .as_ref()?;
    vendor.get_remote_url(&path)
  }

  pub fn is_valid_file_referrer(&self, specifier: &ModuleSpecifier) -> bool {
    if let Ok(path) = url_to_file_path(specifier) {
      if !path.starts_with(&self.deno_dir().root) {
        return true;
      }
    }
    false
  }
}
