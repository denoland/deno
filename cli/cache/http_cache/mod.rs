// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::url::Url;
use std::sync::Arc;
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

// DO NOT make the path public. The fact that this is stored in a file
// is an implementation detail.
pub struct MaybeHttpCacheItem {
  url: Url,
  cache: Arc<dyn HttpCache>,
}

impl MaybeHttpCacheItem {
  pub fn new(cache: Arc<dyn HttpCache>, url: Url) -> Self {
    Self { url, cache }
  }

  #[cfg(test)]
  pub fn read_to_string(&self) -> Result<Option<String>, AnyError> {
    let Some(bytes) = self.read_to_bytes()? else {
      return Ok(None);
    };
    Ok(Some(String::from_utf8(bytes)?))
  }

  pub fn read_to_bytes(&self) -> Result<Option<Vec<u8>>, AnyError> {
    self.cache.read_file_bytes(&self.url)
  }

  pub fn read_metadata(&self) -> Result<Option<CachedUrlMetadata>, AnyError> {
    self.cache.read_metadata(&self.url)
  }
}

pub trait HttpCache: Send + Sync + std::fmt::Debug {
  fn contains(&self, url: &Url) -> bool;
  fn get_modified_time(
    &self,
    url: &Url,
  ) -> Result<Option<SystemTime>, AnyError>;
  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> Result<(), AnyError>;
  fn read_file_bytes(&self, url: &Url) -> Result<Option<Vec<u8>>, AnyError>;
  fn read_metadata(
    &self,
    url: &Url,
  ) -> Result<Option<CachedUrlMetadata>, AnyError>;
  fn write_metadata(
    &self,
    url: &Url,
    meta_data: &CachedUrlMetadata,
  ) -> Result<(), AnyError>;
}
