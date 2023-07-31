// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use deno_ast::MediaType;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_core::serde_json;
use deno_core::url::Url;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;

use crate::cache::CACHE_PERM;
use crate::http_util::HeadersMap;
use crate::util;
use crate::util::fs::atomic_write_file;

use super::common::base_url_to_filename_parts;
use super::common::ensure_dir_exists;
use super::common::read_file_bytes;
use super::global::GlobalHttpCache;
use super::global::UrlToFilenameConversionError;
use super::CachedUrlMetadata;
use super::HttpCache;
use super::HttpCacheItemKey;

#[derive(Debug)]
pub struct LocalHttpCache {
  path: PathBuf,
  manifest: LocalCacheManifest,
  global_cache: Arc<GlobalHttpCache>,
}

impl LocalHttpCache {
  pub fn new(path: PathBuf, global_cache: Arc<GlobalHttpCache>) -> Self {
    assert!(path.is_absolute());
    let manifest = LocalCacheManifest::new(path.join("manifest.json"));
    Self {
      path,
      manifest,
      global_cache,
    }
  }

  fn get_cache_filepath(
    &self,
    url: &Url,
    headers: &HeadersMap,
  ) -> Result<PathBuf, AnyError> {
    Ok(url_to_local_sub_path(url, headers)?.as_path_from_root(&self.path))
  }

  /// Copies the file from the global cache to the local cache returning
  /// if the data was successfully copied to the local cache.
  fn check_copy_global_to_local(&self, url: &Url) -> Result<bool, AnyError> {
    let global_key = self.global_cache.cache_item_key(url)?;
    let Some(metadata) = self.global_cache.read_metadata(&global_key)? else {
      return Ok(false);
    };

    if !metadata.is_redirect() {
      let Some(cached_bytes) = self.global_cache.read_file_bytes(&global_key)? else {
        return Ok(false);
      };

      let local_file_path = self.get_cache_filepath(url, &metadata.headers)?;
      // if we're here, then this will be set
      ensure_dir_exists(local_file_path.parent().unwrap())?;
      atomic_write_file(&local_file_path, cached_bytes, CACHE_PERM)?;
    }
    self.manifest.insert_data(
      url_to_local_sub_path(url, &metadata.headers)?,
      url.clone(),
      metadata.headers,
    );

    Ok(true)
  }

  fn get_url_metadata_checking_global_cache(
    &self,
    url: &Url,
  ) -> Result<Option<CachedUrlMetadata>, AnyError> {
    if let Some(metadata) = self.manifest.get_metadata(url) {
      Ok(Some(metadata))
    } else if self.check_copy_global_to_local(url)? {
      // try again now that it's saved
      Ok(self.manifest.get_metadata(url))
    } else {
      Ok(None)
    }
  }
}

impl HttpCache for LocalHttpCache {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> Result<HttpCacheItemKey<'a>, AnyError> {
    Ok(HttpCacheItemKey {
      #[cfg(debug_assertions)]
      is_local_key: true,
      url,
      file_path: None, // need to compute this every time
    })
  }

  fn contains(&self, url: &Url) -> bool {
    self.manifest.get_metadata(url).is_some()
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<SystemTime>, AnyError> {
    debug_assert!(key.is_local_key);
    self
      .get_url_metadata_checking_global_cache(key.url)
      .map(|m| m.map(|m| m.time))
  }

  fn set(
    &self,
    url: &Url,
    headers: crate::http_util::HeadersMap,
    content: &[u8],
  ) -> Result<(), AnyError> {
    let is_redirect = headers.contains_key("location");
    if !is_redirect {
      let cache_filepath = self.get_cache_filepath(url, &headers)?;
      // Create parent directory
      let parent_filename = cache_filepath.parent().unwrap();
      ensure_dir_exists(parent_filename)?;
      // Cache content
      util::fs::atomic_write_file(&cache_filepath, content, CACHE_PERM)?;
    }

    let sub_path = url_to_local_sub_path(url, &headers)?;
    self.manifest.insert_data(sub_path, url.clone(), headers);

    Ok(())
  }

  fn read_file_bytes(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<Vec<u8>>, AnyError> {
    debug_assert!(key.is_local_key);

    let metadata = self.get_url_metadata_checking_global_cache(key.url)?;
    match metadata {
      Some(data) => {
        if data.is_redirect() {
          // return back an empty file for redirect
          Ok(Some(Vec::new()))
        } else {
          // if it's not a redirect, then it should have a file path
          let cache_filepath =
            self.get_cache_filepath(key.url, &data.headers)?;
          Ok(read_file_bytes(&cache_filepath)?)
        }
      }
      None => Ok(None),
    }
  }

  fn read_metadata(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<CachedUrlMetadata>, AnyError> {
    debug_assert!(key.is_local_key);

    self.get_url_metadata_checking_global_cache(key.url)
  }
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

// todo(THIS PR): unit tests
fn url_to_local_sub_path(
  url: &Url,
  headers: &HeadersMap,
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

  fn get_extension(url: &Url, headers: &HeadersMap) -> &'static str {
    MediaType::from_specifier_and_headers(url, Some(headers)).as_ts_extension()
  }

  fn short_hash(data: &str, ext: Option<&str>) -> String {
    // This function is a bit of a balancing act between readability
    // and avoiding collisions.
    let checksum = util::checksum::gen(&[data.as_bytes()]);
    let sub = data
      .to_lowercase()
      .chars()
      .map(|c| if FORBIDDEN_CHARS.contains(&c) { 'x' } else { c })
      .take(20) // keep the paths short because of windows path limit
      .collect::<String>();
    let sub = match ext {
      Some(ext) => sub.strip_suffix(ext).unwrap_or(&sub),
      None => &sub,
    };
    let ext = ext.unwrap_or("");
    if sub.is_empty() {
      format!("#{}{}", &checksum[..7], ext)
    } else {
      format!("#{}_{}{}", &sub, &checksum[..5], ext)
    }
  }

  fn should_hash_part(part: &str, is_last: bool) -> bool {
    if part.len() > 30 {
      // keep short due to windows path limit
      return true;
    }
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
    last_part.push('?');
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
        short_hash(
          &part,
          if is_last {
            Some(get_extension(url, headers))
          } else {
            None
          },
        )
      } else {
        part
      }
    })
    .collect::<Vec<_>>();

  Ok(LocalCacheSubPath { has_hash, parts })
}

#[derive(Debug, Default, Clone)]
struct LocalCacheManifestData {
  serialized: SerializedLocalCacheManifestData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SerializedLocalCacheManifestDataModule {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub path: Option<String>,
  #[serde(
    default = "IndexMap::new",
    skip_serializing_if = "IndexMap::is_empty"
  )]
  pub headers: IndexMap<String, String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct SerializedLocalCacheManifestData {
  pub modules: IndexMap<Url, SerializedLocalCacheManifestDataModule>,
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
    url: Url,
    mut original_headers: HashMap<String, String>,
  ) {
    let mut headers_subset = IndexMap::new();

    // todo: investigate other headers to keep (etag?)
    const HEADER_KEYS_TO_KEEP: [&str; 4] = [
      "content-type",
      "location",
      "x-typescript-types",
      "x-deno-warning",
    ];
    for key in HEADER_KEYS_TO_KEEP {
      if let Some((k, v)) = original_headers.remove_entry(key) {
        headers_subset.insert(k, v);
      }
    }

    let mut data = self.data.write();
    let is_empty = headers_subset.is_empty() && !sub_path.has_hash;
    let has_changed = if is_empty {
      data.serialized.modules.remove(&url).is_some()
    } else {
      let new_data = SerializedLocalCacheManifestDataModule {
        path: if headers_subset.contains_key("location") {
          None
        } else {
          Some(sub_path.parts.join("/"))
        },
        headers: headers_subset,
      };
      if data.serialized.modules.get(&url) == Some(&new_data) {
        false
      } else {
        data.serialized.modules.insert(url.clone(), new_data);
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

  pub fn get_metadata(&self, url: &Url) -> Option<CachedUrlMetadata> {
    let data = self.data.read();
    let Some(module) = data.serialized.modules.get(url) else {
      let folder_path = self.file_path.parent().unwrap();
      let sub_path = url_to_local_sub_path(url, &Default::default()).ok()?;
      if sub_path.has_hash {
        // only paths without a hash are considered as in the cache
        // when they don't have a metadata entry
        return None;
      }
      let file_path = sub_path.as_path_from_root(folder_path);
      // todo(THIS PR): use fs trait
      return if let Ok(metadata) = file_path.metadata() {
        Some(CachedUrlMetadata {
          headers: Default::default(),
          url: url.to_string(),
          time: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
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
    let sub_path = match &module.path {
      Some(sub_path) => {
        Cow::Owned(self.file_path.parent().unwrap().join(sub_path))
      }
      None => Cow::Borrowed(&self.file_path),
    };
    let Ok(metadata) = sub_path.metadata() else {
      return None;
    };

    Some(CachedUrlMetadata {
      headers,
      url: url.to_string(),
      time: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
    })
  }
}
