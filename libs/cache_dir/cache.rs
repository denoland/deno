// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::SystemTime;

use deno_error::JsError;
use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use url::Url;

use crate::GlobalHttpCacheRc;
use crate::LocalHttpCacheRc;
use crate::common::HeadersMap;
use crate::common::base_url_to_filename_parts;
use crate::common::checksum;
use crate::global::GlobalHttpCacheSys;
use crate::local::LocalHttpCacheSys;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GlobalToLocalCopy {
  /// When using a local cache (vendor folder), allow the cache to
  /// copy from the global cache into the local one.
  Allow,
  /// Disallow copying from the global to the local cache. This is
  /// useful for the LSP because we want to ensure that checksums
  /// are evaluated for JSR dependencies, which is difficult to do
  /// in the LSP. This could be improved in the future to not require
  /// this
  Disallow,
}

impl GlobalToLocalCopy {
  pub fn is_true(&self) -> bool {
    matches!(self, GlobalToLocalCopy::Allow)
  }
}

#[derive(Debug, Error, JsError)]
#[class(type)]
#[error("Integrity check failed for {}\n\nActual: {}\nExpected: {}", .url, .actual, .expected)]
pub struct ChecksumIntegrityError {
  pub url: Url,
  pub actual: String,
  pub expected: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Checksum<'a>(&'a str);

impl<'a> Checksum<'a> {
  pub fn new(checksum: &'a str) -> Self {
    Self(checksum)
  }

  pub fn as_str(&self) -> &str {
    self.0
  }

  pub fn check(
    &self,
    url: &Url,
    content: &[u8],
  ) -> Result<(), Box<ChecksumIntegrityError>> {
    let actual = checksum(content);
    if self.as_str() != actual {
      Err(Box::new(ChecksumIntegrityError {
        url: url.clone(),
        expected: self.as_str().to_string(),
        actual,
      }))
    } else {
      Ok(())
    }
  }
}

/// Turn provided `url` into a hashed filename.
/// URLs can contain a lot of characters that cannot be used
/// in filenames (like "?", "#", ":"), so in order to cache
/// them properly they are deterministically hashed into ASCII
/// strings.
pub fn url_to_filename(url: &Url) -> std::io::Result<PathBuf> {
  // Replaces port part with a special string token (because
  // ":" cannot be used in filename on some platforms).
  // Ex: $DENO_DIR/remote/https/deno.land/
  let Some(cache_parts) = base_url_to_filename_parts(url, "_PORT") else {
    return Err(std::io::Error::new(
      ErrorKind::InvalidInput,
      format!("Can't convert url (\"{}\") to filename.", url),
    ));
  };

  let rest_str = if let Some(query) = url.query() {
    let mut rest_str =
      String::with_capacity(url.path().len() + 1 + query.len());
    rest_str.push_str(url.path());
    rest_str.push('?');
    rest_str.push_str(query);
    Cow::Owned(rest_str)
  } else {
    Cow::Borrowed(url.path())
  };

  // NOTE: fragment is omitted on purpose - it's not taken into
  // account when caching - it denotes parts of webpage, which
  // in case of static resources doesn't make much sense
  let hashed_filename = checksum(rest_str.as_bytes());
  let capacity = cache_parts.iter().map(|s| s.len() + 1).sum::<usize>()
    + 1
    + hashed_filename.len();
  let mut cache_filename = PathBuf::with_capacity(capacity);
  cache_filename.extend(cache_parts.iter().map(|s| s.as_ref()));
  cache_filename.push(hashed_filename);
  debug_assert_eq!(cache_filename.capacity(), capacity);
  Ok(cache_filename)
}

#[derive(Debug, Error, JsError)]
pub enum CacheReadFileError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  ChecksumIntegrity(Box<ChecksumIntegrityError>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializedCachedUrlMetadata {
  pub headers: HeadersMap,
  pub url: String,
  /// Number of seconds since the UNIX epoch.
  #[serde(default)]
  pub time: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
  pub metadata: SerializedCachedUrlMetadata,
  pub content: Cow<'static, [u8]>,
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
  /// won't ever be set for the local cache because that also needs
  /// header information to determine the final path.
  pub(super) file_path: Option<PathBuf>,
}

#[allow(clippy::disallowed_types, reason = "arc wrapper type")]
pub type HttpCacheRc = deno_maybe_sync::MaybeArc<dyn HttpCache>;

pub trait HttpCache: MaybeSend + MaybeSync + std::fmt::Debug {
  /// A pre-computed key for looking up items in the cache.
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> std::io::Result<HttpCacheItemKey<'a>>;

  fn contains(&self, url: &Url) -> bool;
  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> std::io::Result<()>;
  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<CacheEntry>, CacheReadFileError>;
  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>>;
  /// Reads the headers for the cache item.
  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>>;
  /// Reads the time the item was downloaded to the cache.
  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>>;
}

#[derive(Debug, Clone)]
pub enum GlobalOrLocalHttpCache<Sys: GlobalHttpCacheSys + LocalHttpCacheSys> {
  Global(GlobalHttpCacheRc<Sys>),
  Local(LocalHttpCacheRc<Sys>),
}

impl<Sys: GlobalHttpCacheSys + LocalHttpCacheSys> From<GlobalHttpCacheRc<Sys>>
  for GlobalOrLocalHttpCache<Sys>
{
  fn from(global: GlobalHttpCacheRc<Sys>) -> Self {
    Self::Global(global)
  }
}

impl<Sys: GlobalHttpCacheSys + LocalHttpCacheSys> From<LocalHttpCacheRc<Sys>>
  for GlobalOrLocalHttpCache<Sys>
{
  fn from(local: LocalHttpCacheRc<Sys>) -> Self {
    Self::Local(local)
  }
}

impl<Sys: GlobalHttpCacheSys + LocalHttpCacheSys> HttpCache
  for GlobalOrLocalHttpCache<Sys>
{
  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>> {
    match self {
      GlobalOrLocalHttpCache::Global(global) => global.read_headers(key),
      GlobalOrLocalHttpCache::Local(local) => local.read_headers(key),
    }
  }

  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> std::io::Result<HttpCacheItemKey<'a>> {
    match self {
      GlobalOrLocalHttpCache::Global(global) => global.cache_item_key(url),
      GlobalOrLocalHttpCache::Local(local) => local.cache_item_key(url),
    }
  }

  fn contains(&self, url: &Url) -> bool {
    match self {
      GlobalOrLocalHttpCache::Global(global) => global.contains(url),
      GlobalOrLocalHttpCache::Local(local) => local.contains(url),
    }
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> std::io::Result<()> {
    match self {
      GlobalOrLocalHttpCache::Global(global) => {
        global.set(url, headers, content)
      }
      GlobalOrLocalHttpCache::Local(local) => local.set(url, headers, content),
    }
  }

  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<CacheEntry>, CacheReadFileError> {
    match self {
      GlobalOrLocalHttpCache::Global(global) => global.get(key, maybe_checksum),
      GlobalOrLocalHttpCache::Local(local) => local.get(key, maybe_checksum),
    }
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    match self {
      GlobalOrLocalHttpCache::Global(global) => global.read_modified_time(key),
      GlobalOrLocalHttpCache::Local(local) => local.read_modified_time(key),
    }
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    match self {
      GlobalOrLocalHttpCache::Global(global) => global.read_download_time(key),
      GlobalOrLocalHttpCache::Local(local) => local.read_download_time(key),
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn deserialized_no_time() {
    let json = r#"{
      "headers": {
        "content-type": "application/javascript"
      },
      "url": "https://deno.land/std/http/file_server.ts"
    }"#;
    let data: SerializedCachedUrlMetadata = serde_json::from_str(json).unwrap();
    assert_eq!(
      data,
      SerializedCachedUrlMetadata {
        headers: HeadersMap::from([(
          "content-type".to_string(),
          "application/javascript".to_string()
        )]),
        time: None,
        url: "https://deno.land/std/http/file_server.ts".to_string(),
      }
    );
  }

  #[test]
  fn serialize_deserialize_time() {
    let json = r#"{
      "headers": {
        "content-type": "application/javascript"
      },
      "url": "https://deno.land/std/http/file_server.ts",
      "time": 123456789
    }"#;
    let data: SerializedCachedUrlMetadata = serde_json::from_str(json).unwrap();
    let expected = SerializedCachedUrlMetadata {
      headers: HeadersMap::from([(
        "content-type".to_string(),
        "application/javascript".to_string(),
      )]),
      time: Some(123456789),
      url: "https://deno.land/std/http/file_server.ts".to_string(),
    };
    assert_eq!(data, expected);
  }
}
