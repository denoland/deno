// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use thiserror::Error;

use crate::cache::CACHE_PERM;
use crate::http_util::HeadersMap;
use crate::util;
use crate::util::fs::atomic_write_file;

use super::common::base_url_to_filename_parts;
use super::common::read_file_bytes;
use super::CachedUrlMetadata;
use super::HttpCache;
use super::HttpCacheItemKey;

#[derive(Debug, Error)]
#[error("Can't convert url (\"{}\") to filename.", .url)]
pub struct UrlToFilenameConversionError {
  pub(super) url: String,
}

/// Turn provided `url` into a hashed filename.
/// URLs can contain a lot of characters that cannot be used
/// in filenames (like "?", "#", ":"), so in order to cache
/// them properly they are deterministically hashed into ASCII
/// strings.
pub fn url_to_filename(
  url: &Url,
) -> Result<PathBuf, UrlToFilenameConversionError> {
  let Some(mut cache_filename) = base_url_to_filename(url) else {
    return Err(UrlToFilenameConversionError { url: url.to_string() });
  };

  let mut rest_str = url.path().to_string();
  if let Some(query) = url.query() {
    rest_str.push('?');
    rest_str.push_str(query);
  }
  // NOTE: fragment is omitted on purpose - it's not taken into
  // account when caching - it denotes parts of webpage, which
  // in case of static resources doesn't make much sense
  let hashed_filename = util::checksum::gen(&[rest_str.as_bytes()]);
  cache_filename.push(hashed_filename);
  Ok(cache_filename)
}

// Turn base of url (scheme, hostname, port) into a valid filename.
/// This method replaces port part with a special string token (because
/// ":" cannot be used in filename on some platforms).
/// Ex: $DENO_DIR/deps/https/deno.land/
fn base_url_to_filename(url: &Url) -> Option<PathBuf> {
  base_url_to_filename_parts(url, "_PORT").map(|parts| {
    let mut out = PathBuf::new();
    for part in parts {
      out.push(part);
    }
    out
  })
}

#[derive(Debug)]
pub struct GlobalHttpCache(PathBuf);

impl GlobalHttpCache {
  pub fn new(path: PathBuf) -> Self {
    assert!(path.is_absolute());
    Self(path)
  }

  // Deprecated to discourage using this as where the file is stored and
  // how it's stored should be an implementation detail of the cache.
  #[deprecated(note = "Should only be used for deno info.")]
  pub fn get_global_cache_location(&self) -> &PathBuf {
    &self.0
  }

  // DEPRECATED: Where the file is stored and how it's stored should be an implementation
  // detail of the cache.
  #[deprecated(note = "Do not assume the cache will be stored at a file path.")]
  pub fn get_global_cache_filepath(
    &self,
    url: &Url,
  ) -> Result<PathBuf, AnyError> {
    Ok(self.0.join(url_to_filename(url)?))
  }

  fn get_cache_filepath(&self, url: &Url) -> Result<PathBuf, AnyError> {
    Ok(self.0.join(url_to_filename(url)?))
  }

  #[inline]
  fn key_file_path<'a>(&self, key: &'a HttpCacheItemKey) -> &'a PathBuf {
    // The key file path is always set for the global cache because
    // the file will always exist, unlike the local cache, which won't
    // have this for redirects.
    key.file_path.as_ref().unwrap()
  }
}

impl HttpCache for GlobalHttpCache {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> Result<HttpCacheItemKey<'a>, AnyError> {
    Ok(HttpCacheItemKey {
      #[cfg(debug_assertions)]
      is_local_key: false,
      url,
      file_path: Some(self.get_cache_filepath(url)?),
    })
  }

  fn contains(&self, url: &Url) -> bool {
    let Ok(cache_filepath) = self.get_cache_filepath(url) else {
      return false
    };
    cache_filepath.is_file()
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<SystemTime>, AnyError> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    match std::fs::metadata(self.key_file_path(key)) {
      Ok(metadata) => Ok(Some(metadata.modified()?)),
      Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(err) => Err(err.into()),
    }
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> Result<(), AnyError> {
    let cache_filepath = self.get_cache_filepath(url)?;
    // Cache content
    atomic_write_file(&cache_filepath, content, CACHE_PERM)?;

    let metadata = CachedUrlMetadata {
      time: SystemTime::now(),
      url: url.to_string(),
      headers,
    };
    write_metadata(&cache_filepath, &metadata)?;

    Ok(())
  }

  fn read_file_bytes(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<Vec<u8>>, AnyError> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    Ok(read_file_bytes(self.key_file_path(key))?)
  }

  fn read_metadata(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<CachedUrlMetadata>, AnyError> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    match read_metadata(self.key_file_path(key))? {
      Some(metadata) => Ok(Some(metadata)),
      None => Ok(None),
    }
  }
}

fn read_metadata(path: &Path) -> Result<Option<CachedUrlMetadata>, AnyError> {
  let path = path.with_extension("metadata.json");
  match read_file_bytes(&path)? {
    Some(metadata) => Ok(Some(serde_json::from_slice(&metadata)?)),
    None => Ok(None),
  }
}

fn write_metadata(
  path: &Path,
  meta_data: &CachedUrlMetadata,
) -> Result<(), AnyError> {
  let path = path.with_extension("metadata.json");
  let json = serde_json::to_string_pretty(meta_data)?;
  atomic_write_file(&path, json, CACHE_PERM)?;
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;
  use std::collections::HashMap;
  use test_util::TempDir;

  #[test]
  fn test_url_to_filename() {
    let test_cases = [
      ("https://deno.land/x/foo.ts", "https/deno.land/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8"),
      (
        "https://deno.land:8080/x/foo.ts",
        "https/deno.land_PORT8080/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8",
      ),
      ("https://deno.land/", "https/deno.land/8a5edab282632443219e051e4ade2d1d5bbc671c781051bf1437897cbdfea0f1"),
      (
        "https://deno.land/?asdf=qwer",
        "https/deno.land/e4edd1f433165141015db6a823094e6bd8f24dd16fe33f2abd99d34a0a21a3c0",
      ),
      // should be the same as case above, fragment (#qwer) is ignored
      // when hashing
      (
        "https://deno.land/?asdf=qwer#qwer",
        "https/deno.land/e4edd1f433165141015db6a823094e6bd8f24dd16fe33f2abd99d34a0a21a3c0",
      ),
      (
        "data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=",
        "data/c21c7fc382b2b0553dc0864aa81a3acacfb7b3d1285ab5ae76da6abec213fb37",
      ),
      (
        "data:text/plain,Hello%2C%20Deno!",
        "data/967374e3561d6741234131e342bf5c6848b70b13758adfe23ee1a813a8131818",
      )
    ];

    for (url, expected) in test_cases.iter() {
      let u = Url::parse(url).unwrap();
      let p = url_to_filename(&u).unwrap();
      assert_eq!(p, PathBuf::from(expected));
    }
  }

  #[test]
  fn test_create_cache() {
    let dir = TempDir::new();
    let cache_path = dir.path().join("foobar");
    // HttpCache should be created lazily on first use:
    // when zipping up a local project with no external dependencies
    // "$DENO_DIR/deps" is empty. When unzipping such project
    // "$DENO_DIR/deps" might not get restored and in situation
    // when directory is owned by root we might not be able
    // to create that directory. However if it's not needed it
    // doesn't make sense to return error in such specific scenarios.
    // For more details check issue:
    // https://github.com/denoland/deno/issues/5688
    let cache = GlobalHttpCache::new(cache_path.to_path_buf());
    assert!(!cache.0.exists());
    let url = Url::parse("http://example.com/foo/bar.js").unwrap();
    cache
      .set(&url, HeadersMap::new(), b"hello world")
      .expect("Failed to add to cache");
    assert!(cache_path.is_dir());
    assert!(cache.get_cache_filepath(&url).unwrap().is_file());
  }

  #[test]
  fn test_get_set() {
    let dir = TempDir::new();
    let cache = GlobalHttpCache::new(dir.path().to_path_buf());
    let url = Url::parse("https://deno.land/x/welcome.ts").unwrap();
    let mut headers = HashMap::new();
    headers.insert(
      "content-type".to_string(),
      "application/javascript".to_string(),
    );
    headers.insert("etag".to_string(), "as5625rqdsfb".to_string());
    let content = b"Hello world";
    let r = cache.set(&url, headers, content);
    eprintln!("result {r:?}");
    assert!(r.is_ok());
    let key = cache.cache_item_key(&url).unwrap();
    let content =
      String::from_utf8(cache.read_file_bytes(&key).unwrap().unwrap()).unwrap();
    let headers = cache.read_metadata(&key).unwrap().unwrap().headers;
    assert_eq!(content, "Hello world");
    assert_eq!(
      headers.get("content-type").unwrap(),
      "application/javascript"
    );
    assert_eq!(headers.get("etag").unwrap(), "as5625rqdsfb");
    assert_eq!(headers.get("foobar"), None);
  }
}
