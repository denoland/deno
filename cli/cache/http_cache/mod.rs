// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::url::Url;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::http_util::HeadersMap;

mod common;
mod global;
mod local;

pub use global::url_to_filename;
pub use global::GlobalHttpCache;
pub use local::LocalHttpCache;

/// Cached metadata about a url.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedUrlMetadata {
  pub headers: HeadersMap,
  pub url: String,
  #[serde(default = "SystemTime::now", rename = "now")]
  pub time: SystemTime,
}

pub struct MaybeHttpCacheItem<'a, 'b> {
  cache: &'a dyn HttpCache,
  key: HttpCacheItemKey<'b>,
}

impl<'a, 'b> MaybeHttpCacheItem<'a, 'b> {
  #[cfg(test)]
  pub fn read_to_string(&self) -> Result<Option<String>, AnyError> {
    let Some(bytes) = self.read_to_bytes()? else {
      return Ok(None);
    };
    Ok(Some(String::from_utf8(bytes)?))
  }

  pub fn read_to_bytes(&self) -> Result<Option<Vec<u8>>, AnyError> {
    self.cache.read_file_bytes(&self.key)
  }

  pub fn read_metadata(&self) -> Result<Option<CachedUrlMetadata>, AnyError> {
    self.cache.read_metadata(&self.key)
  }
}

/// Computed cache key, which can help reduce the work of computing the cache key multiple times.
pub struct HttpCacheItemKey<'a> {
  // The key is specific to the implementation of HttpCache,
  // so keep these private to the module. For example, the
  // fact that these may be stored in a file is an implementation
  // detail.
  #[cfg(debug_assertions)]
  pub(super) is_local_key: bool,
  pub(super) url: &'a Url,
  /// This will be set all the time for the global cache, but it
  /// may not be set for the local cache if it's a redirect.
  ///
  /// That/ said, even redirects might have this set if they haven't
  /// previously been cached, so the local cache code should never assume
  /// that it being set means it's not a redirect, but it can assume
  /// that it not being set means it's a redirect.
  pub(super) file_path: Option<PathBuf>,
}

pub trait HttpCache: Send + Sync + std::fmt::Debug {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> Result<HttpCacheItemKey<'a>, AnyError>;

  fn contains(&self, url: &Url) -> bool;
  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> Result<(), AnyError>;
  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<SystemTime>, AnyError>;
  fn read_file_bytes(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<Vec<u8>>, AnyError>;
  fn read_metadata(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<CachedUrlMetadata>, AnyError>;
}

pub trait HttpCacheExtensions {
  fn get<'a, 'b>(
    &'a self,
    url: &'b Url,
  ) -> Result<MaybeHttpCacheItem<'a, 'b>, AnyError>;
}

impl HttpCacheExtensions for dyn HttpCache {
  fn get<'a, 'b>(
    &'a self,
    url: &'b Url,
  ) -> Result<MaybeHttpCacheItem<'a, 'b>, AnyError> {
    let cache_item_key = self.cache_item_key(url)?;
    Ok(MaybeHttpCacheItem {
      key: cache_item_key,
      // this requires a dyn HttpCache and so we can't include
      // this method directly in the HttpCache trait
      cache: &*self,
    })
  }
}
