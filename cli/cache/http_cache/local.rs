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
use deno_core::url::Url;
use indexmap::IndexMap;
use once_cell::sync::Lazy;

use crate::cache::CACHE_PERM;
use crate::http_util::HeadersMap;
use crate::util;
use crate::util::fs::atomic_write_file;

use super::common::base_url_to_filename_parts;
use super::common::read_file_bytes;
use super::global::GlobalHttpCache;
use super::global::UrlToFilenameConversionError;
use super::CachedUrlMetadata;
use super::HttpCache;
use super::HttpCacheItemKey;

/// A vendor/ folder http cache for the lsp that provides functionality
/// for doing a reverse mapping.
#[derive(Debug)]
pub struct LocalLspHttpCache {
  cache: LocalHttpCache,
}

impl LocalLspHttpCache {
  pub fn new(path: PathBuf, global_cache: Arc<GlobalHttpCache>) -> Self {
    assert!(path.is_absolute());
    let manifest = LocalCacheManifest::new_for_lsp(path.join("manifest.json"));
    Self {
      cache: LocalHttpCache {
        path,
        manifest,
        global_cache,
      },
    }
  }

  pub fn get_file_url(&self, url: &Url) -> Option<Url> {
    let sub_path = {
      let data = self.cache.manifest.data.read();
      let maybe_content_type =
        data.get(url).and_then(|d| d.content_type_header());
      url_to_local_sub_path(url, maybe_content_type).ok()?
    };
    let path = sub_path.as_path_from_root(&self.cache.path);
    if path.exists() {
      Url::from_file_path(path).ok()
    } else {
      None
    }
  }

  pub fn get_remote_url(&self, path: &Path) -> Option<Url> {
    let Ok(path) = path.strip_prefix(&self.cache.path) else {
      return None; // not in this directory
    };
    let components = path
      .components()
      .map(|c| c.as_os_str().to_string_lossy())
      .collect::<Vec<_>>();
    if components
      .last()
      .map(|c| c.starts_with('#'))
      .unwrap_or(false)
    {
      // the file itself will have an entry in the manifest
      let data = self.cache.manifest.data.read();
      data.get_reverse_mapping(path)
    } else if let Some(last_index) =
      components.iter().rposition(|c| c.starts_with('#'))
    {
      // get the mapping to the deepest hashed directory and
      // then add the remaining path components to the url
      let dir_path: PathBuf = components[..last_index + 1].iter().fold(
        PathBuf::new(),
        |mut path, c| {
          path.push(c.as_ref());
          path
        },
      );
      let dir_url = self
        .cache
        .manifest
        .data
        .read()
        .get_reverse_mapping(&dir_path)?;
      let file_url =
        dir_url.join(&components[last_index + 1..].join("/")).ok()?;
      Some(file_url)
    } else {
      // we can work backwards from the path to the url
      let mut parts = Vec::new();
      for (i, part) in path.components().enumerate() {
        let part = part.as_os_str().to_string_lossy();
        if i == 0 {
          let mut result = String::new();
          let part = if let Some(part) = part.strip_prefix("http_") {
            result.push_str("http://");
            part
          } else {
            result.push_str("https://");
            &part
          };
          if let Some((domain, port)) = part.rsplit_once('_') {
            result.push_str(&format!("{}:{}", domain, port));
          } else {
            result.push_str(part);
          }
          parts.push(result);
        } else {
          parts.push(part.to_string());
        }
      }
      Url::parse(&parts.join("/")).ok()
    }
  }
}

impl HttpCache for LocalLspHttpCache {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> Result<HttpCacheItemKey<'a>, AnyError> {
    self.cache.cache_item_key(url)
  }

  fn contains(&self, url: &Url) -> bool {
    self.cache.contains(url)
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> Result<(), AnyError> {
    self.cache.set(url, headers, content)
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<SystemTime>, AnyError> {
    self.cache.read_modified_time(key)
  }

  fn read_file_bytes(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<Vec<u8>>, AnyError> {
    self.cache.read_file_bytes(key)
  }

  fn read_metadata(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<CachedUrlMetadata>, AnyError> {
    self.cache.read_metadata(key)
  }
}

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

  /// Copies the file from the global cache to the local cache returning
  /// if the data was successfully copied to the local cache.
  fn check_copy_global_to_local(&self, url: &Url) -> Result<bool, AnyError> {
    let global_key = self.global_cache.cache_item_key(url)?;
    let Some(metadata) = self.global_cache.read_metadata(&global_key)? else {
      return Ok(false);
    };

    let local_path =
      url_to_local_sub_path(url, headers_content_type(&metadata.headers))?;

    if !metadata.is_redirect() {
      let Some(cached_bytes) = self.global_cache.read_file_bytes(&global_key)? else {
        return Ok(false);
      };

      let local_file_path = local_path.as_path_from_root(&self.path);
      // if we're here, then this will be set
      atomic_write_file(&local_file_path, cached_bytes, CACHE_PERM)?;
    }

    self
      .manifest
      .insert_data(local_path, url.clone(), metadata.headers);

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
    #[cfg(debug_assertions)]
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
    let sub_path = url_to_local_sub_path(url, headers_content_type(&headers))?;

    if !is_redirect {
      // Cache content
      atomic_write_file(
        &sub_path.as_path_from_root(&self.path),
        content,
        CACHE_PERM,
      )?;
    }

    self.manifest.insert_data(sub_path, url.clone(), headers);

    Ok(())
  }

  fn read_file_bytes(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<Vec<u8>>, AnyError> {
    #[cfg(debug_assertions)]
    debug_assert!(key.is_local_key);

    let metadata = self.get_url_metadata_checking_global_cache(key.url)?;
    match metadata {
      Some(data) => {
        if data.is_redirect() {
          // return back an empty file for redirect
          Ok(Some(Vec::new()))
        } else {
          // if it's not a redirect, then it should have a file path
          let cache_file_path = url_to_local_sub_path(
            key.url,
            headers_content_type(&data.headers),
          )?
          .as_path_from_root(&self.path);
          Ok(read_file_bytes(&cache_file_path)?)
        }
      }
      None => Ok(None),
    }
  }

  fn read_metadata(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<CachedUrlMetadata>, AnyError> {
    #[cfg(debug_assertions)]
    debug_assert!(key.is_local_key);

    self.get_url_metadata_checking_global_cache(key.url)
  }
}

pub(super) struct LocalCacheSubPath {
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

  pub fn as_relative_path(&self) -> PathBuf {
    let mut path = PathBuf::with_capacity(self.parts.len());
    for part in &self.parts {
      path.push(part);
    }
    path
  }
}

fn headers_content_type(headers: &HeadersMap) -> Option<&str> {
  headers.get("content-type").map(|s| s.as_str())
}

fn url_to_local_sub_path(
  url: &Url,
  content_type: Option<&str>,
) -> Result<LocalCacheSubPath, UrlToFilenameConversionError> {
  // https://stackoverflow.com/a/31976060/188246
  static FORBIDDEN_CHARS: Lazy<HashSet<char>> = Lazy::new(|| {
    HashSet::from(['?', '<', '>', ':', '*', '|', '\\', ':', '"', '\'', '/'])
  });
  // https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
  static FORBIDDEN_WINDOWS_NAMES: Lazy<HashSet<&'static str>> =
    Lazy::new(|| {
      let set = HashSet::from([
        "con", "prn", "aux", "nul", "com0", "com1", "com2", "com3", "com4",
        "com5", "com6", "com7", "com8", "com9", "lpt0", "lpt1", "lpt2", "lpt3",
        "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
      ]);
      // ensure everything is lowercase because we'll be comparing
      // lowercase filenames against this
      debug_assert!(set.iter().all(|s| s.to_lowercase() == *s));
      set
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

  fn get_extension(url: &Url, content_type: Option<&str>) -> &'static str {
    MediaType::from_specifier_and_content_type(url, content_type)
      .as_ts_extension()
  }

  fn short_hash(data: &str, last_ext: Option<&str>) -> String {
    // This function is a bit of a balancing act between readability
    // and avoiding collisions.
    let hash = util::checksum::gen(&[data.as_bytes()]);
    // keep the paths short because of windows path limit
    const MAX_LENGTH: usize = 20;
    let mut sub = String::with_capacity(MAX_LENGTH);
    for c in data.chars().take(MAX_LENGTH) {
      // don't include the query string (only use it in the hash)
      if c == '?' {
        break;
      }
      if FORBIDDEN_CHARS.contains(&c) {
        sub.push('_');
      } else {
        sub.extend(c.to_lowercase());
      }
    }
    let sub = match last_ext {
      Some(ext) => sub.strip_suffix(ext).unwrap_or(&sub),
      None => &sub,
    };
    let ext = last_ext.unwrap_or("");
    if sub.is_empty() {
      format!("#{}{}", &hash[..7], ext)
    } else {
      format!("#{}_{}{}", &sub, &hash[..5], ext)
    }
  }

  fn should_hash_part(part: &str, last_ext: Option<&str>) -> bool {
    if part.is_empty() || part.len() > 30 {
      // keep short due to windows path limit
      return true;
    }
    let hash_context_specific = if let Some(last_ext) = last_ext {
      // if the last part does not have a known extension, hash it in order to
      // prevent collisions with a directory of the same name
      !has_known_extension(part) || !part.ends_with(last_ext)
    } else {
      // if any non-ending path part has a known extension, hash it in order to
      // prevent collisions where a filename has the same name as a directory name
      has_known_extension(part)
    };

    // the hash symbol at the start designates a hash for the url part
    hash_context_specific
      || part.starts_with('#')
      || has_forbidden_chars(part)
      || last_ext.is_none() && FORBIDDEN_WINDOWS_NAMES.contains(part)
      || part.ends_with('.')
  }

  // get the base url
  let port_separator = "_"; // make this shorter with just an underscore
  let Some(mut base_parts) = base_url_to_filename_parts(url, port_separator) else {
    return Err(UrlToFilenameConversionError { url: url.to_string() });
  };

  if base_parts[0] == "https" {
    base_parts.remove(0);
  } else {
    let scheme = base_parts.remove(0);
    base_parts[0] = format!("{}_{}", scheme, base_parts[0]);
  }

  // first, try to get the filename of the path
  let path_segments = url_path_segments(url);
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
      let last_ext = if is_last {
        Some(get_extension(url, content_type))
      } else {
        None
      };
      if should_hash_part(&part, last_ext) {
        has_hash = true;
        short_hash(&part, last_ext)
      } else {
        part
      }
    })
    .collect::<Vec<_>>();

  Ok(LocalCacheSubPath { has_hash, parts })
}

#[derive(Debug)]
struct LocalCacheManifest {
  file_path: PathBuf,
  data: RwLock<manifest::LocalCacheManifestData>,
}

impl LocalCacheManifest {
  pub fn new(file_path: PathBuf) -> Self {
    Self::new_internal(file_path, false)
  }

  pub fn new_for_lsp(file_path: PathBuf) -> Self {
    Self::new_internal(file_path, true)
  }

  fn new_internal(file_path: PathBuf, use_reverse_mapping: bool) -> Self {
    let text = std::fs::read_to_string(&file_path).ok();
    Self {
      data: RwLock::new(manifest::LocalCacheManifestData::new(
        text.as_deref(),
        use_reverse_mapping,
      )),
      file_path,
    }
  }

  pub fn insert_data(
    &self,
    sub_path: LocalCacheSubPath,
    url: Url,
    mut original_headers: HashMap<String, String>,
  ) {
    fn should_keep_content_type_header(
      url: &Url,
      headers: &HashMap<String, String>,
    ) -> bool {
      // only keep the location header if it can't be derived from the url
      MediaType::from_specifier(url)
        != MediaType::from_specifier_and_headers(url, Some(headers))
    }

    let mut headers_subset = IndexMap::new();

    const HEADER_KEYS_TO_KEEP: [&str; 4] = [
      // keep alphabetical for cleanliness in the output
      "content-type",
      "location",
      "x-deno-warning",
      "x-typescript-types",
    ];
    for key in HEADER_KEYS_TO_KEEP {
      if key == "content-type"
        && !should_keep_content_type_header(&url, &original_headers)
      {
        continue;
      }
      if let Some((k, v)) = original_headers.remove_entry(key) {
        headers_subset.insert(k, v);
      }
    }

    let mut data = self.data.write();
    let add_module_entry = headers_subset.is_empty()
      && !sub_path
        .parts
        .last()
        .map(|s| s.starts_with('#'))
        .unwrap_or(false);
    let mut has_changed = if add_module_entry {
      data.remove(&url, &sub_path)
    } else {
      let new_data = manifest::SerializedLocalCacheManifestDataModule {
        headers: headers_subset,
      };
      if data.get(&url) == Some(&new_data) {
        false
      } else {
        data.insert(url.clone(), &sub_path, new_data);
        true
      }
    };

    if sub_path.has_hash {
      let url_path_parts = url_path_segments(&url).collect::<Vec<_>>();
      let base_url = {
        let mut url = url.clone();
        url.set_path("/");
        url.set_query(None);
        url.set_fragment(None);
        url
      };
      for (i, local_part) in sub_path.parts[1..sub_path.parts.len() - 1]
        .iter()
        .enumerate()
      {
        if local_part.starts_with('#') {
          let mut url = base_url.clone();
          url.set_path(&format!("{}/", url_path_parts[..i + 1].join("/")));
          if data.add_directory(url, sub_path.parts[..i + 2].join("/")) {
            has_changed = true;
          }
        }
      }
    }

    if has_changed {
      // don't bother ensuring the directory here because it will
      // eventually be created by files being added to the cache
      let result =
        atomic_write_file(&self.file_path, data.as_json(), CACHE_PERM);
      if let Err(err) = result {
        log::debug!("Failed saving local cache manifest: {:#}", err);
      }
    }
  }

  pub fn get_metadata(&self, url: &Url) -> Option<CachedUrlMetadata> {
    let data = self.data.read();
    match data.get(url) {
      Some(module) => {
        let headers = module
          .headers
          .iter()
          .map(|(k, v)| (k.to_string(), v.to_string()))
          .collect::<HashMap<_, _>>();
        let sub_path = if headers.contains_key("location") {
          Cow::Borrowed(&self.file_path)
        } else {
          let sub_path =
            url_to_local_sub_path(url, headers_content_type(&headers)).ok()?;
          let folder_path = self.file_path.parent().unwrap();
          Cow::Owned(sub_path.as_path_from_root(folder_path))
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
      None => {
        let folder_path = self.file_path.parent().unwrap();
        let sub_path = url_to_local_sub_path(url, None).ok()?;
        if sub_path
          .parts
          .last()
          .map(|s| s.starts_with('#'))
          .unwrap_or(false)
        {
          // only filenames without a hash are considered as in the cache
          // when they don't have a metadata entry
          return None;
        }
        let file_path = sub_path.as_path_from_root(folder_path);
        if let Ok(metadata) = file_path.metadata() {
          Some(CachedUrlMetadata {
            headers: Default::default(),
            url: url.to_string(),
            time: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
          })
        } else {
          None
        }
      }
    }
  }
}

// This is in a separate module in order to enforce keeping
// the internal implementation private.
mod manifest {
  use std::collections::HashMap;
  use std::path::Path;
  use std::path::PathBuf;

  use deno_core::serde_json;
  use deno_core::url::Url;
  use indexmap::IndexMap;
  use serde::Deserialize;
  use serde::Serialize;

  use super::url_to_local_sub_path;
  use super::LocalCacheSubPath;

  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  pub struct SerializedLocalCacheManifestDataModule {
    #[serde(
      default = "IndexMap::new",
      skip_serializing_if = "IndexMap::is_empty"
    )]
    pub headers: IndexMap<String, String>,
  }

  impl SerializedLocalCacheManifestDataModule {
    pub fn content_type_header(&self) -> Option<&str> {
      self.headers.get("content-type").map(|s| s.as_str())
    }
  }

  #[derive(Debug, Default, Clone, Serialize, Deserialize)]
  struct SerializedLocalCacheManifestData {
    #[serde(
      default = "IndexMap::new",
      skip_serializing_if = "IndexMap::is_empty"
    )]
    pub folders: IndexMap<Url, String>,
    #[serde(
      default = "IndexMap::new",
      skip_serializing_if = "IndexMap::is_empty"
    )]
    pub modules: IndexMap<Url, SerializedLocalCacheManifestDataModule>,
  }

  #[derive(Debug, Default, Clone)]
  pub(super) struct LocalCacheManifestData {
    serialized: SerializedLocalCacheManifestData,
    // reverse mapping used in the lsp
    reverse_mapping: Option<HashMap<PathBuf, Url>>,
  }

  impl LocalCacheManifestData {
    pub fn new(maybe_text: Option<&str>, use_reverse_mapping: bool) -> Self {
      let serialized: SerializedLocalCacheManifestData = maybe_text
        .and_then(|text| match serde_json::from_str(text) {
          Ok(data) => Some(data),
          Err(err) => {
            log::debug!("Failed deserializing local cache manifest: {:#}", err);
            None
          }
        })
        .unwrap_or_default();
      let reverse_mapping = if use_reverse_mapping {
        Some(
          serialized
            .modules
            .iter()
            .filter_map(|(url, module)| {
              if module.headers.contains_key("location") {
                return None;
              }
              url_to_local_sub_path(url, module.content_type_header())
                .ok()
                .map(|local_path| {
                  let path = if cfg!(windows) {
                    PathBuf::from(local_path.parts.join("\\"))
                  } else {
                    PathBuf::from(local_path.parts.join("/"))
                  };
                  (path, url.clone())
                })
            })
            .chain(serialized.folders.iter().map(|(url, local_path)| {
              let path = if cfg!(windows) {
                PathBuf::from(local_path.replace('/', "\\"))
              } else {
                PathBuf::from(local_path)
              };
              (path, url.clone())
            }))
            .collect::<HashMap<_, _>>(),
        )
      } else {
        None
      };
      Self {
        serialized,
        reverse_mapping,
      }
    }

    pub fn get(
      &self,
      url: &Url,
    ) -> Option<&SerializedLocalCacheManifestDataModule> {
      self.serialized.modules.get(url)
    }

    pub fn get_reverse_mapping(&self, path: &Path) -> Option<Url> {
      debug_assert!(self.reverse_mapping.is_some()); // only call this if you're in the lsp
      self
        .reverse_mapping
        .as_ref()
        .and_then(|mapping| mapping.get(path))
        .cloned()
    }

    pub fn add_directory(&mut self, url: Url, local_path: String) -> bool {
      if let Some(current) = self.serialized.folders.get(&url) {
        if *current == local_path {
          return false;
        }
      }

      if let Some(reverse_mapping) = &mut self.reverse_mapping {
        reverse_mapping.insert(
          if cfg!(windows) {
            PathBuf::from(local_path.replace('/', "\\"))
          } else {
            PathBuf::from(&local_path)
          },
          url.clone(),
        );
      }

      self.serialized.folders.insert(url, local_path);
      true
    }

    pub fn insert(
      &mut self,
      url: Url,
      sub_path: &LocalCacheSubPath,
      new_data: SerializedLocalCacheManifestDataModule,
    ) {
      if let Some(reverse_mapping) = &mut self.reverse_mapping {
        reverse_mapping.insert(sub_path.as_relative_path(), url.clone());
      }
      self.serialized.modules.insert(url, new_data);
    }

    pub fn remove(&mut self, url: &Url, sub_path: &LocalCacheSubPath) -> bool {
      if self.serialized.modules.remove(url).is_some() {
        if let Some(reverse_mapping) = &mut self.reverse_mapping {
          reverse_mapping.remove(&sub_path.as_relative_path());
        }
        true
      } else {
        false
      }
    }

    pub fn as_json(&self) -> String {
      serde_json::to_string_pretty(&self.serialized).unwrap()
    }
  }
}

fn url_path_segments(url: &Url) -> impl Iterator<Item = &str> {
  url
    .path()
    .strip_prefix('/')
    .unwrap_or(url.path())
    .split('/')
}

#[cfg(test)]
mod test {
  use super::*;

  use deno_core::serde_json::json;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  #[test]
  fn test_url_to_local_sub_path() {
    run_test("https://deno.land/x/mod.ts", &[], "deno.land/x/mod.ts");
    run_test(
      "http://deno.land/x/mod.ts",
      &[],
      // http gets added to the folder name, but not https
      "http_deno.land/x/mod.ts",
    );
    run_test(
      // capital letter in filename
      "https://deno.land/x/MOD.ts",
      &[],
      "deno.land/x/#mod_fa860.ts",
    );
    run_test(
      // query string
      "https://deno.land/x/mod.ts?testing=1",
      &[],
      "deno.land/x/#mod_2eb80.ts",
    );
    run_test(
      // capital letter in directory
      "https://deno.land/OTHER/mod.ts",
      &[],
      "deno.land/#other_1c55d/mod.ts",
    );
    run_test(
      // under max of 30 chars
      "https://deno.land/x/012345678901234567890123456.js",
      &[],
      "deno.land/x/012345678901234567890123456.js",
    );
    run_test(
      // max 30 chars
      "https://deno.land/x/0123456789012345678901234567.js",
      &[],
      "deno.land/x/#01234567890123456789_836de.js",
    );
    run_test(
      // forbidden char
      "https://deno.land/x/mod's.js",
      &[],
      "deno.land/x/#mod_s_44fc8.js",
    );
    run_test(
      // no extension
      "https://deno.land/x/mod",
      &[("content-type", "application/typescript")],
      "deno.land/x/#mod_e55cf.ts",
    );
    run_test(
      // known extension in directory is not allowed
      // because it could conflict with a file of the same name
      "https://deno.land/x/mod.js/mod.js",
      &[],
      "deno.land/x/#mod.js_59c58/mod.js",
    );
    run_test(
      // slash slash in path
      "http://localhost//mod.js",
      &[],
      "http_localhost/#e3b0c44/mod.js",
    );
    run_test(
      // headers same extension
      "https://deno.land/x/mod.ts",
      &[("content-type", "application/typescript")],
      "deno.land/x/mod.ts",
    );
    run_test(
      // headers different extension... We hash this because
      // if someone deletes the manifest file, then we don't want
      // https://deno.land/x/mod.ts to resolve as a typescript file
      "https://deno.land/x/mod.ts",
      &[("content-type", "application/javascript")],
      "deno.land/x/#mod.ts_e8c36.js",
    );
    run_test(
      // not allowed windows folder name
      "https://deno.land/x/con/con.ts",
      &[],
      "deno.land/x/#con_1143d/con.ts",
    );
    run_test(
      // disallow ending a directory with a period
      // https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
      "https://deno.land/x/test./main.ts",
      &[],
      "deno.land/x/#test._4ee3d/main.ts",
    );

    #[track_caller]
    fn run_test(url: &str, headers: &[(&str, &str)], expected: &str) {
      let url = Url::parse(url).unwrap();
      let headers = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
      let result =
        url_to_local_sub_path(&url, headers_content_type(&headers)).unwrap();
      let parts = result.parts.join("/");
      assert_eq!(parts, expected);
      assert_eq!(
        result.parts.iter().any(|p| p.starts_with('#')),
        result.has_hash
      )
    }
  }

  #[test]
  fn test_local_global_cache() {
    let temp_dir = TempDir::new();
    let global_cache_path = temp_dir.path().join("global");
    let local_cache_path = temp_dir.path().join("local");
    let global_cache =
      Arc::new(GlobalHttpCache::new(global_cache_path.to_path_buf()));
    let local_cache =
      LocalHttpCache::new(local_cache_path.to_path_buf(), global_cache.clone());

    let manifest_file = local_cache_path.join("manifest.json");
    // mapped url
    {
      let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(
          &url,
          HashMap::from([(
            "content-type".to_string(),
            "application/typescript".to_string(),
          )]),
          content.as_bytes(),
        )
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
          .unwrap(),
        content
      );
      let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
      // won't have any headers because the content-type is derivable from the url
      assert_eq!(metadata.headers, HashMap::new());
      assert_eq!(metadata.url, url.to_string());
      // no manifest file yet
      assert!(!manifest_file.exists());

      // now try deleting the global cache and we should still be able to load it
      global_cache_path.remove_dir_all();
      assert_eq!(
        String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
          .unwrap(),
        content
      );
    }

    // file that's directly mappable to a url
    {
      let content = "export const a = 1;";
      local_cache_path
        .join("deno.land")
        .join("main.js")
        .write(content);

      // now we should be able to read this file because it's directly mappable to a url
      let url = Url::parse("https://deno.land/main.js").unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
          .unwrap(),
        content
      );
      let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
      assert_eq!(metadata.headers, HashMap::new());
      assert_eq!(metadata.url, url.to_string());
    }

    // now try a file with a different content-type header
    {
      let url =
        Url::parse("https://deno.land/x/different_content_type.ts").unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(
          &url,
          HashMap::from([(
            "content-type".to_string(),
            "application/javascript".to_string(),
          )]),
          content.as_bytes(),
        )
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
          .unwrap(),
        content
      );
      let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
      assert_eq!(
        metadata.headers,
        HashMap::from([(
          "content-type".to_string(),
          "application/javascript".to_string(),
        )])
      );
      assert_eq!(metadata.url, url.to_string());
      assert_eq!(
        manifest_file.read_json_value(),
        json!({
          "modules": {
            "https://deno.land/x/different_content_type.ts": {
              "headers": {
                "content-type": "application/javascript"
              }
            }
          }
        })
      );
      // delete the manifest file
      manifest_file.remove_file();

      // Now try resolving the key again and the content type should still be application/javascript.
      // This is maintained because we hash the filename when the headers don't match the extension.
      let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
      assert_eq!(
        metadata.headers,
        HashMap::from([(
          "content-type".to_string(),
          "application/javascript".to_string(),
        )])
      );
    }

    // reset the local cache
    local_cache_path.remove_dir_all();
    let local_cache =
      LocalHttpCache::new(local_cache_path.to_path_buf(), global_cache.clone());

    // now try caching a file with many headers
    {
      let url = Url::parse("https://deno.land/x/my_file.ts").unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(
          &url,
          HashMap::from([
            (
              "content-type".to_string(),
              "application/typescript".to_string(),
            ),
            ("x-typescript-types".to_string(), "./types.d.ts".to_string()),
            ("x-deno-warning".to_string(), "Stop right now.".to_string()),
            (
              "x-other-header".to_string(),
              "Thank you very much.".to_string(),
            ),
          ]),
          content.as_bytes(),
        )
        .unwrap();
      let check_output = |local_cache: &LocalHttpCache| {
        let key = local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            local_cache.read_file_bytes(&key).unwrap().unwrap()
          )
          .unwrap(),
          content
        );
        let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
        assert_eq!(
          metadata.headers,
          HashMap::from([
            ("x-typescript-types".to_string(), "./types.d.ts".to_string(),),
            ("x-deno-warning".to_string(), "Stop right now.".to_string(),)
          ])
        );
        assert_eq!(metadata.url, url.to_string());
        assert_eq!(
          manifest_file.read_json_value(),
          json!({
            "modules": {
              "https://deno.land/x/my_file.ts": {
                "headers": {
                  "x-deno-warning": "Stop right now.",
                  "x-typescript-types": "./types.d.ts"
                }
              }
            }
          })
        );
      };
      check_output(&local_cache);
      // now ensure it's the same when re-creating the cache
      check_output(&LocalHttpCache::new(
        local_cache_path.to_path_buf(),
        global_cache.clone(),
      ));
    }

    // reset the local cache
    local_cache_path.remove_dir_all();
    let local_cache =
      LocalHttpCache::new(local_cache_path.to_path_buf(), global_cache.clone());

    // try a file that can't be mapped to the file system
    {
      {
        let url =
          Url::parse("https://deno.land/INVALID/Module.ts?dev").unwrap();
        let content = "export const test = 5;";
        global_cache
          .set(&url, HashMap::new(), content.as_bytes())
          .unwrap();
        let key = local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            local_cache.read_file_bytes(&key).unwrap().unwrap()
          )
          .unwrap(),
          content
        );
        let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
        // won't have any headers because the content-type is derivable from the url
        assert_eq!(metadata.headers, HashMap::new());
        assert_eq!(metadata.url, url.to_string());
      }

      // now try a file in the same directory, but that maps to the local filesystem
      {
        let url = Url::parse("https://deno.land/INVALID/module2.ts").unwrap();
        let content = "export const test = 4;";
        global_cache
          .set(&url, HashMap::new(), content.as_bytes())
          .unwrap();
        let key = local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            local_cache.read_file_bytes(&key).unwrap().unwrap()
          )
          .unwrap(),
          content
        );
        assert!(local_cache_path
          .join("deno.land/#invalid_1ee01/module2.ts")
          .exists());

        // ensure we can still read this file with a new local cache
        let local_cache = LocalHttpCache::new(
          local_cache_path.to_path_buf(),
          global_cache.clone(),
        );
        assert_eq!(
          String::from_utf8(
            local_cache.read_file_bytes(&key).unwrap().unwrap()
          )
          .unwrap(),
          content
        );
      }

      assert_eq!(
        manifest_file.read_json_value(),
        json!({
          "modules": {
            "https://deno.land/INVALID/Module.ts?dev": {
            }
          },
          "folders": {
            "https://deno.land/INVALID/": "deno.land/#invalid_1ee01",
          }
        })
      );
    }

    // reset the local cache
    local_cache_path.remove_dir_all();
    let local_cache =
      LocalHttpCache::new(local_cache_path.to_path_buf(), global_cache.clone());

    // now try a redirect
    {
      let url = Url::parse("https://deno.land/redirect.ts").unwrap();
      global_cache
        .set(
          &url,
          HashMap::from([("location".to_string(), "./x/mod.ts".to_string())]),
          "Redirecting to other url...".as_bytes(),
        )
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      let metadata = local_cache.read_metadata(&key).unwrap().unwrap();
      assert_eq!(
        metadata.headers,
        HashMap::from([("location".to_string(), "./x/mod.ts".to_string())])
      );
      assert_eq!(metadata.url, url.to_string());
      assert_eq!(
        manifest_file.read_json_value(),
        json!({
          "modules": {
            "https://deno.land/redirect.ts": {
              "headers": {
                "location": "./x/mod.ts"
              }
            }
          }
        })
      );
    }
  }

  #[test]
  fn test_lsp_local_cache() {
    let temp_dir = TempDir::new();
    let global_cache_path = temp_dir.path().join("global");
    let local_cache_path = temp_dir.path().join("local");
    let global_cache =
      Arc::new(GlobalHttpCache::new(global_cache_path.to_path_buf()));
    let local_cache = LocalLspHttpCache::new(
      local_cache_path.to_path_buf(),
      global_cache.clone(),
    );

    // mapped url
    {
      let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(
          &url,
          HashMap::from([(
            "content-type".to_string(),
            "application/typescript".to_string(),
          )]),
          content.as_bytes(),
        )
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
          .unwrap(),
        content
      );

      // check getting the file url works
      let file_url = local_cache.get_file_url(&url);
      let expected = local_cache_path
        .uri_dir()
        .join("deno.land/x/mod.ts")
        .unwrap();
      assert_eq!(file_url, Some(expected));

      // get the reverse mapping
      let mapping = local_cache.get_remote_url(
        local_cache_path
          .join("deno.land")
          .join("x")
          .join("mod.ts")
          .as_path(),
      );
      assert_eq!(mapping.as_ref(), Some(&url));
    }

    // now try a file with a different content-type header
    {
      let url =
        Url::parse("https://deno.land/x/different_content_type.ts").unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(
          &url,
          HashMap::from([(
            "content-type".to_string(),
            "application/javascript".to_string(),
          )]),
          content.as_bytes(),
        )
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(local_cache.read_file_bytes(&key).unwrap().unwrap())
          .unwrap(),
        content
      );

      let file_url = local_cache.get_file_url(&url).unwrap();
      let path = file_url.to_file_path().unwrap();
      assert!(path.exists());
      let mapping = local_cache.get_remote_url(&path);
      assert_eq!(mapping.as_ref(), Some(&url));
    }

    // try http specifiers that can't be mapped to the file system
    {
      let urls = [
        "http://deno.land/INVALID/Module.ts?dev",
        "http://deno.land/INVALID/SubDir/Module.ts?dev",
      ];
      for url in urls {
        let url = Url::parse(url).unwrap();
        let content = "export const test = 5;";
        global_cache
          .set(&url, HashMap::new(), content.as_bytes())
          .unwrap();
        let key = local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            local_cache.read_file_bytes(&key).unwrap().unwrap()
          )
          .unwrap(),
          content
        );

        let file_url = local_cache.get_file_url(&url).unwrap();
        let path = file_url.to_file_path().unwrap();
        assert!(path.exists());
        let mapping = local_cache.get_remote_url(&path);
        assert_eq!(mapping.as_ref(), Some(&url));
      }

      // now try a files in the same and sub directories, that maps to the local filesystem
      let urls = [
        "http://deno.land/INVALID/module2.ts",
        "http://deno.land/INVALID/SubDir/module3.ts",
        "http://deno.land/INVALID/SubDir/sub_dir/module4.ts",
      ];
      for url in urls {
        let url = Url::parse(url).unwrap();
        let content = "export const test = 4;";
        global_cache
          .set(&url, HashMap::new(), content.as_bytes())
          .unwrap();
        let key = local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            local_cache.read_file_bytes(&key).unwrap().unwrap()
          )
          .unwrap(),
          content
        );
        let file_url = local_cache.get_file_url(&url).unwrap();
        let path = file_url.to_file_path().unwrap();
        assert!(path.exists());
        let mapping = local_cache.get_remote_url(&path);
        assert_eq!(mapping.as_ref(), Some(&url));

        // ensure we can still get this file with a new local cache
        let local_cache = LocalLspHttpCache::new(
          local_cache_path.to_path_buf(),
          global_cache.clone(),
        );
        let file_url = local_cache.get_file_url(&url).unwrap();
        let path = file_url.to_file_path().unwrap();
        assert!(path.exists());
        let mapping = local_cache.get_remote_url(&path);
        assert_eq!(mapping.as_ref(), Some(&url));
      }
    }
  }
}
