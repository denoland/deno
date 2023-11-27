// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::cache::LocalLspHttpCache;
use crate::util::path::specifier_to_file_path;

use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

pub fn calculate_fs_version(
  cache: &Arc<dyn HttpCache>,
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

fn calculate_fs_version_in_cache<T: HttpCache + ?Sized>(
  cache: &Arc<T>,
  specifier: &ModuleSpecifier,
) -> Option<String> {
  let Ok(cache_key) = cache.cache_item_key(specifier) else {
    return Some("1".to_string());
  };
  match cache.read_modified_time(&cache_key) {
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

/// Populate the metadata map based on the supplied headers
fn parse_metadata(
  headers: &HashMap<String, String>,
) -> HashMap<MetadataKey, String> {
  let mut metadata = HashMap::new();
  if let Some(warning) = headers.get("x-deno-warning").cloned() {
    metadata.insert(MetadataKey::Warning, warning);
  }
  metadata
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum MetadataKey {
  /// Represent the `x-deno-warning` header associated with the document
  Warning,
}

#[derive(Debug, Clone)]
struct Metadata {
  values: Arc<HashMap<MetadataKey, String>>,
  version: Option<String>,
}

#[derive(Debug)]
pub struct CacheWithMetadata<T: HttpCache> {
  cache: Arc<T>,
  metadata: Arc<Mutex<HashMap<ModuleSpecifier, Metadata>>>,
}

impl<T: HttpCache> CacheWithMetadata<T> {
  pub fn new(cache: Arc<T>) -> Self {
    Self {
      cache,
      metadata: Default::default(),
    }
  }

  /// Return the meta data associated with the specifier. Unlike the `get()`
  /// method, redirects of the supplied specifier will not be followed.
  pub fn metadata_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<HashMap<MetadataKey, String>>> {
    if matches!(
      specifier.scheme(),
      "file" | "npm" | "node" | "data" | "blob"
    ) {
      return None;
    }
    let version = calculate_fs_version_in_cache(&self.cache, specifier);
    let metadata = self.metadata.lock().get(specifier).cloned();
    if metadata.as_ref().and_then(|m| m.version.clone()) != version {
      self.refresh(specifier).map(|m| m.values)
    } else {
      metadata.map(|m| m.values)
    }
  }

  fn refresh(&self, specifier: &ModuleSpecifier) -> Option<Metadata> {
    if matches!(
      specifier.scheme(),
      "file" | "npm" | "node" | "data" | "blob"
    ) {
      return None;
    }
    let cache_key = self.cache.cache_item_key(specifier).ok()?;
    let specifier_metadata = self.cache.read_metadata(&cache_key).ok()??;
    let values = Arc::new(parse_metadata(&specifier_metadata.headers));
    let version = calculate_fs_version_in_cache(&self.cache, specifier);
    let mut metadata_map = self.metadata.lock();
    let metadata = Metadata { values, version };
    metadata_map.insert(specifier.clone(), metadata.clone());
    Some(metadata)
  }
}

// Needs to be implemented manually because the derived impl needlessly requires
// the type parameter to be Clone, even though it's in an Arc.
impl<T: HttpCache> Clone for CacheWithMetadata<T> {
  fn clone(&self) -> Self {
    Self {
      cache: self.cache.clone(),
      metadata: self.metadata.clone(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct LspCache {
  global_cache: CacheWithMetadata<GlobalHttpCache>,
  vendor_caches_by_scope:
    BTreeMap<ModuleSpecifier, CacheWithMetadata<LocalLspHttpCache>>,
}

impl LspCache {
  pub fn new(
    global_deps_dir: &Path,
    vendor_dirs_by_scope: &BTreeMap<ModuleSpecifier, PathBuf>,
  ) -> Self {
    let global_cache = Arc::new(GlobalHttpCache::new(
      global_deps_dir.to_path_buf(),
      crate::cache::RealDenoCacheEnv,
    ));
    Self {
      global_cache: CacheWithMetadata::new(global_cache.clone()),
      vendor_caches_by_scope: vendor_dirs_by_scope
        .iter()
        .map(|(specifier, vendor_dir)| {
          (
            specifier.clone(),
            CacheWithMetadata::new(Arc::new(LocalLspHttpCache::new(
              vendor_dir.clone(),
              global_cache.clone(),
            ))),
          )
        })
        .collect(),
    }
  }

  pub fn for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Arc<dyn HttpCache> {
    self
      .vendor_caches_by_scope
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .map(|(_, v)| v.cache.clone() as _)
      .unwrap_or_else(|| self.global_cache.cache.clone() as _)
  }

  pub fn global(&self) -> &Arc<GlobalHttpCache> {
    &self.global_cache.cache
  }

  // TODO(nayeemrmn): This should take a `scope` argument.
  pub fn vendored_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    if !matches!(specifier.scheme(), "http" | "https") {
      return None;
    }
    self
      .vendor_caches_by_scope
      .values()
      .find_map(|v| v.cache.get_file_url(specifier))
  }

  pub fn unvendored_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let Ok(path) = specifier_to_file_path(specifier) else {
      return None;
    };
    self
      .vendor_caches_by_scope
      .values()
      .find_map(|v| v.cache.get_remote_url(&path))
  }

  pub fn metadata(
    &self,
    specifier: &ModuleSpecifier,
    scope: Option<&ModuleSpecifier>,
  ) -> Option<Arc<HashMap<MetadataKey, String>>> {
    if let Some(scope) = scope {
      if let Some(vendor_cache) = self.vendor_caches_by_scope.get(scope) {
        return vendor_cache.metadata_for_specifier(specifier);
      }
    }
    self.global_cache.metadata_for_specifier(specifier)
  }
}
