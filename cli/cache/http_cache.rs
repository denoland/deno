// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
//! This module is meant to eventually implement HTTP cache
//! as defined in RFC 7234 (<https://tools.ietf.org/html/rfc7234>).
//! Currently it's a very simplified version to fulfill Deno needs
//! at hand.
use crate::http_util::HeadersMap;
use crate::util;
use crate::util::fs::atomic_write_file;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::url::Url;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use thiserror::Error;

use super::CACHE_PERM;

/// Turn base of url (scheme, hostname, port) into a valid filename.
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

fn base_url_to_filename_parts(
  url: &Url,
  port_separator: &str,
) -> Option<Vec<String>> {
  let mut out = Vec::with_capacity(2);

  let scheme = url.scheme();
  out.push(scheme.to_string());

  match scheme {
    "http" | "https" => {
      let host = url.host_str().unwrap();
      let host_port = match url.port() {
        // underscores are not allowed in domains, so adding one here is fine
        Some(port) => format!("{host}{port_separator}{port}"),
        None => host.to_string(),
      };
      out.push(host_port);
    }
    "data" | "blob" => (),
    scheme => {
      log::debug!("Don't know how to create cache name for scheme: {}", scheme);
      return None;
    }
  };

  Some(out)
}

#[derive(Debug, Error)]
#[error("Can't convert url (\"{}\") to filename.", .url)]
pub struct UrlToFilenameConversionError {
  url: String,
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

struct LocalCacheSubPath {
  pub has_hash: bool,
  pub parts: Vec<String>,
}

impl LocalCacheSubPath {
  pub fn as_path_from_root(&self, root_path: &Path) -> PathBuf {
    let mut path = root_path.to_path_buf();
    for part in &self.parts {
      path.push(part);
    }
    path
  }
}

fn url_to_local_sub_path(
  url: &Url,
) -> Result<LocalCacheSubPath, UrlToFilenameConversionError> {
  // https://stackoverflow.com/a/31976060/188246
  static FORBIDDEN_CHARS: Lazy<HashSet<char>> = Lazy::new(|| {
    HashSet::from(['?', '<', '>', ':', '*', '|', '\\', ':', '"', '\'', '/'])
  });

  fn has_forbidden_chars(segment: &str) -> bool {
    segment.chars().any(|c| {
      let is_uppercase = c.is_ascii_alphabetic() && !c.is_ascii_lowercase();
      FORBIDDEN_CHARS.contains(&c)
        // do not allow uppercase letters in order to make this work
        // the same on case insensitive file systems
        || is_uppercase
    })
  }

  fn has_known_extension(path: &str) -> bool {
    let path = path.to_lowercase();
    path.ends_with(".js")
      || path.ends_with(".ts")
      || path.ends_with(".jsx")
      || path.ends_with(".tsx")
      || path.ends_with(".mts")
      || path.ends_with(".mjs")
      || path.ends_with(".json")
      || path.ends_with(".wasm")
  }

  fn short_hash(data: &str) -> String {
    format!("#{}", &util::checksum::gen(&[data.as_bytes()])[..10])
  }

  fn should_hash_part(part: &str, is_last: bool) -> bool {
    let hash_context_specific = if is_last {
      // if the last part does not have a known extension, hash it in order to
      // prevent collisions with a directory of the same name
      !has_known_extension(part)
    } else {
      // if any non-ending path part has a known extension, hash it in order to
      // prevent collisions where a filename has the same name as a directory name
      has_known_extension(part)
    };

    // the hash symbol at the start designates a hash for the url part
    hash_context_specific || part.starts_with('#') || has_forbidden_chars(part)
  }

  // get the base url
  let port_separator = "_"; // make this shorter with just an underscore
  let Some(base_parts) = base_url_to_filename_parts(url, port_separator) else {
    return Err(UrlToFilenameConversionError { url: url.to_string() });
  };

  // first, try to get the filename of the path
  let path_segments = url.path_segments().unwrap();
  let mut parts = base_parts
    .into_iter()
    .chain(path_segments.map(|s| s.to_string()))
    .collect::<Vec<_>>();

  // push the query parameter onto the last part
  if let Some(query) = url.query() {
    let last_part = parts.last_mut().unwrap();
    last_part.push_str("?");
    last_part.push_str(query);
  }

  let mut has_hash = false;
  let parts_len = parts.len();
  let parts = parts
    .into_iter()
    .enumerate()
    .map(|(i, part)| {
      let is_last = i == parts_len - 1;
      if should_hash_part(&part, is_last) {
        has_hash = true;
        short_hash(&part)
      } else {
        part
      }
    })
    .collect::<Vec<_>>();

  Ok(LocalCacheSubPath { has_hash, parts })
}

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
  cache: Arc<HttpCache>,
}

impl MaybeHttpCacheItem {
  #[cfg(test)]
  pub fn read_to_string(&self) -> Result<Option<String>, AnyError> {
    let Some(bytes) = self.read_to_bytes()? else {
      return Ok(None);
    };
    Ok(Some(String::from_utf8(bytes)?))
  }

  pub fn read_to_bytes(&self) -> Result<Option<Vec<u8>>, AnyError> {
    let cache_filepath = self.cache.get_active_cache_filepath(&self.url)?;
    match read_file_bytes(&cache_filepath)? {
      Some(bytes) => Ok(Some(bytes)),
      None => {
        if self.cache.check_copy_global_to_local(&self.url)? {
          // try again now that it's saved
          read_file_bytes(&cache_filepath)
        } else {
          Ok(None)
        }
      }
    }
  }

  pub fn read_metadata(&self) -> Result<Option<CachedUrlMetadata>, AnyError> {
    match &self.cache.local {
      Some(local) => {
        if let Some(metadata) = local.manifest.get_metadata(&self.url) {
          return Ok(Some(metadata));
        } else {
          if self.cache.check_copy_global_to_local(&self.url)? {
            // try again now that it's saved
            Ok(local.manifest.get_metadata(&self.url))
          } else {
            Ok(None)
          }
        }
      }
      None => {
        let cache_filepath = self.cache.get_active_cache_filepath(&self.url)?;
        match read_metadata(&cache_filepath)? {
          Some(metadata) => Ok(Some(metadata)),
          None => Ok(None),
        }
      }
    }
  }
}

#[derive(Debug, Clone)]
pub struct HttpCachePaths {
  pub global: PathBuf,
  pub local: Option<PathBuf>,
}

#[derive(Debug)]
struct HttpCacheLocal {
  path: PathBuf,
  manifest: LocalCacheManifest,
}

#[derive(Debug)]
pub struct HttpCache {
  global_path: PathBuf,
  local: Option<HttpCacheLocal>,
}

impl HttpCache {
  pub fn new(paths: HttpCachePaths) -> Self {
    assert!(paths.global.is_absolute());
    let local = if let Some(path) = paths.local {
      assert!(path.is_absolute());
      let manifest = LocalCacheManifest::new(path.join("manifest.json"));
      Some(HttpCacheLocal { path, manifest })
    } else {
      None
    };
    Self {
      global_path: paths.global,
      local,
    }
  }

  /// Returns a new instance.
  ///
  /// `global_cache_path` must be an absolute path.
  pub fn new_global(global_cache_path: PathBuf) -> Self {
    Self::new(HttpCachePaths {
      global: global_cache_path,
      local: None,
    })
  }

  /// Ensures the location of the cache.
  fn ensure_dir_exists(&self, path: &Path) -> io::Result<()> {
    if path.is_dir() {
      return Ok(());
    }
    fs::create_dir_all(path).map_err(|e| {
      io::Error::new(
        e.kind(),
        format!(
          "Could not create remote modules cache location: {path:?}\nCheck the permission of the directory."
        ),
      )
    })
  }

  pub fn get_modified_time(
    &self,
    url: &Url,
  ) -> Result<Option<SystemTime>, AnyError> {
    let filepath = self.get_active_cache_filepath(url)?;
    match fs::metadata(filepath) {
      Ok(metadata) => Ok(Some(metadata.modified()?)),
      Err(err) if err.kind() == io::ErrorKind::NotFound => {
        if self.check_copy_global_to_local(url)? {
          // try again now that it's saved
          return self.get_modified_time(url);
        }
        Ok(None)
      }
      Err(err) => Err(err.into()),
    }
  }

  // DEPRECATED: Where the file is stored and how it's stored should be an implementation
  // detail of the cache.
  #[deprecated(note = "Do not assume the cache will be stored at a file path.")]
  pub fn get_global_cache_location(&self) -> &PathBuf {
    &self.global_path
  }

  // DEPRECATED: Where the file is stored and how it's stored should be an implementation
  // detail of the cache.
  #[deprecated(note = "Do not assume the cache will be stored at a file path.")]
  pub fn get_global_cache_filepath(
    &self,
    url: &Url,
  ) -> Result<PathBuf, AnyError> {
    Ok(self.global_path.join(url_to_filename(url)?))
  }

  fn get_active_cache_filepath(&self, url: &Url) -> Result<PathBuf, AnyError> {
    Ok(match &self.local {
      Some(local) => url_to_local_sub_path(url)?.as_path_from_root(&local.path),
      None => self.global_path.join(url_to_filename(url)?),
    })
  }

  #[cfg(test)]
  pub fn write_metadata(
    &self,
    url: &Url,
    meta_data: &CachedUrlMetadata,
  ) -> Result<(), AnyError> {
    let cache_path = self.get_active_cache_filepath(url)?;
    write_metadata(&cache_path, meta_data)
  }

  // TODO(bartlomieju): this method should check headers file
  // and validate against ETAG/Last-modified-as headers.
  // ETAG check is currently done in `cli/file_fetcher.rs`.
  pub fn get(
    self: &Arc<Self>,
    url: &Url,
  ) -> Result<MaybeHttpCacheItem, AnyError> {
    Ok(MaybeHttpCacheItem {
      url: url.clone(),
      cache: self.clone(),
    })
  }

  pub fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> Result<(), AnyError> {
    let cache_filepath = self.get_active_cache_filepath(url)?;
    // Create parent directory
    let parent_filename = cache_filepath
      .parent()
      .expect("Cache filename should have a parent dir");
    self.ensure_dir_exists(parent_filename)?;
    // Cache content
    util::fs::atomic_write_file(&cache_filepath, content, CACHE_PERM)?;

    if let Some(local) = &self.local {
      let sub_path = url_to_local_sub_path(url)?;
      local.manifest.insert_data(sub_path, url.clone(), headers);
    } else {
      let metadata = CachedUrlMetadata {
        time: SystemTime::now(),
        url: url.to_string(),
        headers,
      };
      write_metadata(&cache_filepath, &metadata)?;
    }

    Ok(())
  }

  pub fn contains(&self, url: &Url) -> bool {
    let Ok(cache_filepath) = self.get_active_cache_filepath(url) else {
      return false
    };
    cache_filepath.is_file()
  }

  fn check_copy_global_to_local(&self, url: &Url) -> Result<bool, AnyError> {
    let Some(local_cache) = &self.local else {
      return Ok(false);
    };
    #[allow(deprecated)]
    let global_filepath = self.get_global_cache_filepath(url)?;
    let Some(cached_bytes) = read_file_bytes(&global_filepath)? else {
      return Ok(false);
    };

    let Some(metadata) = read_metadata(&global_filepath)? else {
      return Ok(false);
    };

    let local_cache_sub_path = url_to_local_sub_path(url)?;
    let local_file_path =
      local_cache_sub_path.as_path_from_root(&local_cache.path);
    self.ensure_dir_exists(local_file_path.parent().unwrap())?;
    atomic_write_file(&local_file_path, &cached_bytes, CACHE_PERM)?;
    local_cache.manifest.insert_data(
      local_cache_sub_path,
      url.clone(),
      metadata.headers,
    );

    Ok(true)
  }
}

fn read_file_bytes(path: &Path) -> Result<Option<Vec<u8>>, AnyError> {
  match std::fs::read(path) {
    Ok(s) => Ok(Some(s)),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(err) => Err(err.into()),
  }
}

fn read_metadata(path: &Path) -> Result<Option<CachedUrlMetadata>, AnyError> {
  let path = path.with_extension("metadata.json");
  match fs::read_to_string(path) {
    Ok(metadata) => Ok(Some(serde_json::from_str(&metadata)?)),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(err) => Err(err.into()),
  }
}

fn write_metadata(
  path: &Path,
  meta_data: &CachedUrlMetadata,
) -> Result<(), AnyError> {
  let path = path.with_extension("metadata.json");
  let json = serde_json::to_string_pretty(meta_data)?;
  util::fs::atomic_write_file(&path, json, CACHE_PERM)?;
  Ok(())
}

#[derive(Debug, Default, Clone)]
struct LocalCacheManifestData {
  serialized: SerializedLocalCacheManifestData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SerializedLocalCacheManifestDataModule {
  pub path: String,
  #[serde(
    default = "IndexMap::new",
    skip_serializing_if = "IndexMap::is_empty"
  )]
  pub headers: IndexMap<String, String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct SerializedLocalCacheManifestData {
  pub modules:
    IndexMap<ModuleSpecifier, SerializedLocalCacheManifestDataModule>,
}

#[derive(Debug)]
struct LocalCacheManifest {
  file_path: PathBuf,
  data: RwLock<LocalCacheManifestData>,
}

impl LocalCacheManifest {
  pub fn new(file_path: PathBuf) -> Self {
    // todo: debug log when deserialization fails
    let serialized: SerializedLocalCacheManifestData =
      std::fs::read(&file_path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default();
    Self {
      data: RwLock::new(LocalCacheManifestData { serialized }),
      file_path,
    }
  }

  pub fn insert_data(
    &self,
    sub_path: LocalCacheSubPath,
    specifier: ModuleSpecifier,
    original_headers: HashMap<String, String>,
  ) {
    let mut headers_subset = IndexMap::new();
    // todo: investigate other headers to keep
    // todo: extract to function
    if let Some(value) = original_headers.get("x-typescript-types") {
      headers_subset.insert("x-typescript-types".to_string(), value.clone());
    }
    if let Some(value) = original_headers.get("x-deno-warning") {
      headers_subset.insert("x-deno-warning".to_string(), value.clone());
    }
    let mut data = self.data.write();
    let is_empty = headers_subset.is_empty() && !sub_path.has_hash;
    let has_changed = if is_empty {
      data.serialized.modules.remove(&specifier).is_some()
    } else {
      let new_data = SerializedLocalCacheManifestDataModule {
        path: sub_path.parts.join("/"),
        headers: headers_subset,
      };
      if data.serialized.modules.get(&specifier) == Some(&new_data) {
        false
      } else {
        data.serialized.modules.insert(specifier.clone(), new_data);
        true
      }
    };

    if has_changed {
      let _ = atomic_write_file(
        &self.file_path,
        serde_json::to_string_pretty(&data.serialized).unwrap(),
        CACHE_PERM,
      );
    }
  }

  pub fn get_metadata(&self, specifier: &Url) -> Option<CachedUrlMetadata> {
    let data = self.data.read();
    let Some(module) = data.serialized.modules.get(specifier) else {
      let folder_path = self.file_path.parent().unwrap();
      let sub_path = url_to_local_sub_path(specifier).ok()?;
      if sub_path.has_hash {
        return None;
      }
      let file_path = sub_path.as_path_from_root(&folder_path);
      // todo(THIS PR): use fs trait
      return if file_path.exists() {
        Some(CachedUrlMetadata {
          headers: Default::default(),
          url: specifier.to_string(),
          time: SystemTime::now(),
        })
      } else {
        None
      };
    };
    let headers = module
      .headers
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect::<HashMap<_, _>>();
    Some(CachedUrlMetadata {
      headers,
      url: specifier.to_string(),
      time: SystemTime::now(),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;
  use test_util::TempDir;

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
    let cache = HttpCache::new_global(cache_path.to_path_buf());
    assert!(!cache.global_path.exists());
    cache
      .set(
        &Url::parse("http://example.com/foo/bar.js").unwrap(),
        HeadersMap::new(),
        b"hello world",
      )
      .expect("Failed to add to cache");
    assert!(cache.ensure_dir_exists(&cache.global_path).is_ok());
    assert!(cache_path.is_dir());
  }

  #[test]
  fn test_get_set() {
    let dir = TempDir::new();
    let cache = Arc::new(HttpCache::new_global(dir.path().to_path_buf()));
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
    let cache_item = cache.get(&url).unwrap();
    let content = cache_item.read_to_string().unwrap().unwrap();
    let headers = cache_item.read_metadata().unwrap().unwrap().headers;
    assert_eq!(content, "Hello world");
    assert_eq!(
      headers.get("content-type").unwrap(),
      "application/javascript"
    );
    assert_eq!(headers.get("etag").unwrap(), "as5625rqdsfb");
    assert_eq!(headers.get("foobar"), None);
  }

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
}
