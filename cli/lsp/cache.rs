// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_path_util::url_to_file_path;

use crate::cache::DenoDir;
use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::cache::LocalLspHttpCache;
use crate::lsp::config::Config;
use crate::lsp::logging::lsp_log;
use crate::lsp::logging::lsp_warn;
use crate::sys::CliSys;

/// Calculate a version for for a given path.
pub fn calculate_fs_version_at_path(path: impl AsRef<Path>) -> Option<String> {
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

#[derive(Debug, Clone)]
pub struct LspCache {
  deno_dir: DenoDir,
  global: Arc<GlobalHttpCache>,
  vendors_by_scope: BTreeMap<Arc<Url>, Option<Arc<LocalLspHttpCache>>>,
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
    let sys = CliSys::default();
    let deno_dir_root =
      deno_cache_dir::resolve_deno_dir(&sys, global_cache_path)
        .expect("should be infallible with absolute custom root");
    let deno_dir = DenoDir::new(sys.clone(), deno_dir_root);
    let global =
      Arc::new(GlobalHttpCache::new(sys, deno_dir.remote_folder_path()));
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

  pub fn in_cache_directory(&self, specifier: &Url) -> bool {
    let Ok(path) = url_to_file_path(specifier) else {
      return false;
    };
    if path.starts_with(&self.deno_dir().root) {
      return true;
    }
    let Some(vendor) = self
      .vendors_by_scope
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .and_then(|(_, c)| c.as_ref())
    else {
      return false;
    };
    vendor.get_remote_url(&path).is_some()
  }

  pub fn in_global_cache_directory(&self, specifier: &Url) -> bool {
    let Ok(path) = url_to_file_path(specifier) else {
      return false;
    };
    if path.starts_with(&self.deno_dir().root) {
      return true;
    }
    false
  }
}
