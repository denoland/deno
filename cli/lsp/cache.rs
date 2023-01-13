// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::cache::CachedUrlMetadata;
use crate::cache::HttpCache;

use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

/// Calculate a version for for a given path.
pub fn calculate_fs_version(path: &Path) -> Option<String> {
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

#[derive(Debug, Default, Clone)]
pub struct CacheMetadata {
  cache: HttpCache,
  metadata: Arc<Mutex<HashMap<ModuleSpecifier, Metadata>>>,
}

impl CacheMetadata {
  pub fn new(location: &Path) -> Self {
    Self {
      cache: HttpCache::new(location),
      metadata: Default::default(),
    }
  }

  /// Return the meta data associated with the specifier. Unlike the `get()`
  /// method, redirects of the supplied specifier will not be followed.
  pub fn get(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<HashMap<MetadataKey, String>>> {
    if specifier.scheme() == "file" || specifier.scheme() == "npm" {
      return None;
    }
    let version = self
      .cache
      .get_cache_filename(specifier)
      .and_then(|ref path| calculate_fs_version(path));
    let metadata = self.metadata.lock().get(specifier).cloned();
    if metadata.as_ref().and_then(|m| m.version.clone()) != version {
      self.refresh(specifier).map(|m| m.values)
    } else {
      metadata.map(|m| m.values)
    }
  }

  fn refresh(&self, specifier: &ModuleSpecifier) -> Option<Metadata> {
    if specifier.scheme() == "file" || specifier.scheme() == "npm" {
      return None;
    }
    let cache_filename = self.cache.get_cache_filename(specifier)?;
    let specifier_metadata = CachedUrlMetadata::read(&cache_filename).ok()?;
    let values = Arc::new(parse_metadata(&specifier_metadata.headers));
    let version = calculate_fs_version(&cache_filename);
    let mut metadata_map = self.metadata.lock();
    let metadata = Metadata { values, version };
    metadata_map.insert(specifier.clone(), metadata.clone());
    Some(metadata)
  }

  pub fn set_location(&mut self, location: &Path) {
    self.cache = HttpCache::new(location);
    self.metadata.lock().clear();
  }
}
