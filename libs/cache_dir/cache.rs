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

/// Synthetic header stored on cache entries that record a 404 response
/// from the remote server. This allows treating "not found" as cached
/// information instead of re-requesting the URL on every process start.
/// Similar to how redirects are stored as body-less entries distinguished
/// by their "location" header.
pub const NOT_FOUND_CACHE_HEADER: &str = "x-deno-not-found";

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

  /// Verify a remote module against a lockfile hash.
  ///
  /// `deno_graph` writes remote lockfile hashes from the charset-decoded
  /// source text (see `ModuleSourceAndInfo::source_bytes`), while the HTTP
  /// cache stores the original response bytes. Hash the same decoded bytes
  /// here so a Latin-1 (or other non-UTF-8) `Content-Type` charset does not
  /// cause a false integrity failure on a subsequent `--frozen` load.
  pub fn check_remote(
    &self,
    url: &Url,
    maybe_content_type: Option<&str>,
    content: &[u8],
  ) -> Result<(), Box<ChecksumIntegrityError>> {
    let bytes = remote_lock_integrity_bytes(url, maybe_content_type, content);
    self.check(url, bytes.as_ref())
  }
}

/// Bytes that `deno_graph` hashes when writing a remote lockfile entry.
fn remote_lock_integrity_bytes<'a>(
  url: &Url,
  maybe_content_type: Option<&str>,
  content: &'a [u8],
) -> Cow<'a, [u8]> {
  use deno_media_type::MediaType;
  use deno_media_type::encoding::DecodedArcSourceDetailKind;
  use deno_media_type::encoding::decode_arc_source_detail;
  use deno_media_type::encoding::detect_charset;
  use deno_media_type::resolve_media_type_and_charset_from_content_type;
  use std::sync::Arc;

  let (media_type, maybe_charset) =
    resolve_media_type_and_charset_from_content_type(url, maybe_content_type);

  // Only the media types that deno_graph runs through `new_source_with_text`
  // are charset-decoded before the lockfile hash is taken.
  let is_text = matches!(
    media_type,
    MediaType::JavaScript
      | MediaType::Mjs
      | MediaType::Jsx
      | MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Tsx
      | MediaType::Cjs
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Json
  );

  if !is_text {
    return Cow::Borrowed(content);
  }

  let charset = maybe_charset.unwrap_or_else(|| detect_charset(url, content));
  match decode_arc_source_detail(charset, Arc::<[u8]>::from(content)) {
    Ok(detail) => match detail.kind {
      DecodedArcSourceDetailKind::Unchanged => Cow::Borrowed(content),
      DecodedArcSourceDetailKind::Changed
      | DecodedArcSourceDetailKind::OnlyUtf8Bom => {
        Cow::Owned(detail.text.as_bytes().to_vec())
      }
    },
    // If decoding fails, fall back to the raw bytes (previous behaviour).
    Err(_) => Cow::Borrowed(content),
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
  let Some(cache_parts) = base_url_to_filename_parts(url) else {
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn remote_lock_checksum_matches_decoded_latin1() {
    // "export default \"café\";\n" as Latin-1 bytes (é = 0xE9)
    let source = "export default \"café\";\n";
    let latin1: Vec<u8> = source.chars().map(|c| c as u8).collect();
    let utf8 = source.as_bytes();
    assert_ne!(checksum(&latin1), checksum(utf8));

    let url = Url::parse("http://127.0.0.1/mod.js").unwrap();
    let expected_hash = checksum(utf8);
    let expected = Checksum::new(&expected_hash);

    // Raw bytes must fail (old behaviour).
    assert!(expected.check(&url, &latin1).is_err());

    // Charset-aware check matches the lockfile write path.
    assert!(
      expected
        .check_remote(
          &url,
          Some("application/javascript; charset=iso-8859-1"),
          &latin1,
        )
        .is_ok()
    );
  }

  #[test]
  fn remote_lock_checksum_utf8_unchanged() {
    let content = b"export default 1;\n";
    let url = Url::parse("http://example.com/mod.js").unwrap();
    let expected_hash = checksum(content);
    let expected = Checksum::new(&expected_hash);
    assert!(
      expected
        .check_remote(
          &url,
          Some("application/javascript; charset=utf-8"),
          content
        )
        .is_ok()
    );
  }
}
