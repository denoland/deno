// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::cache::HttpCache;
use crate::util::path::specifier_to_file_path;

use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;
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

fn calculate_fs_version_in_cache(
  cache: &Arc<dyn HttpCache>,
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

#[derive(Debug, Clone)]
pub struct CacheMetadata {
  cache: Arc<dyn HttpCache>,
  metadata: Arc<Mutex<HashMap<ModuleSpecifier, Metadata>>>,
}

impl CacheMetadata {
  pub fn new(cache: Arc<dyn HttpCache>) -> Self {
    Self {
      cache,
      metadata: Default::default(),
    }
  }

  /// Return the meta data associated with the specifier. Unlike the `get()`
  /// method, redirects of the supplied specifier will not be followed.
  pub fn get(
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
    let headers = self.cache.read_headers(&cache_key).ok()??;
    let values = Arc::new(parse_metadata(&headers));
    let version = calculate_fs_version_in_cache(&self.cache, specifier);
    let mut metadata_map = self.metadata.lock();
    let metadata = Metadata { values, version };
    metadata_map.insert(specifier.clone(), metadata.clone());
    Some(metadata)
  }

  pub fn set_cache(&mut self, cache: Arc<dyn HttpCache>) {
    self.cache = cache;
    self.metadata.lock().clear();
  }
}
