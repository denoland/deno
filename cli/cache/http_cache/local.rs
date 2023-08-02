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
    if !is_redirect {
      let cache_filepath = self.get_cache_filepath(url, &headers)?;
      // Cache content
      atomic_write_file(&cache_filepath, content, CACHE_PERM)?;
    }

    let sub_path = url_to_local_sub_path(url, &headers)?;
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
    #[cfg(debug_assertions)]
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
    hash_context_specific || part.starts_with('#') || has_forbidden_chars(part)
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
  let path_segments = url
    .path()
    .strip_prefix('/')
    .unwrap_or(url.path())
    .split('/');
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
        Some(get_extension(url, headers))
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
    let serialized: SerializedLocalCacheManifestData =
      std::fs::read(&file_path)
        .ok()
        .and_then(|data| match serde_json::from_slice(&data) {
          Ok(data) => Some(data),
          Err(err) => {
            log::debug!("Failed deserializing local cache manifest: {:#}", err);
            None
          }
        })
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
      // don't bother ensuring the directory here because it will
      // eventually be created by files being added to the cache
      let result = atomic_write_file(
        &self.file_path,
        serde_json::to_string_pretty(&data.serialized).unwrap(),
        CACHE_PERM,
      );
      if let Err(err) = result {
        log::debug!("Failed saving local cache manifest: {:#}", err);
      }
    }
  }

  pub fn get_metadata(&self, url: &Url) -> Option<CachedUrlMetadata> {
    let data = self.data.read();
    match data.serialized.modules.get(url) {
      Some(module) => {
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
      None => {
        let folder_path = self.file_path.parent().unwrap();
        let sub_path = url_to_local_sub_path(url, &Default::default()).ok()?;
        if sub_path.has_hash {
          // only paths without a hash are considered as in the cache
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

    #[track_caller]
    fn run_test(url: &str, headers: &[(&str, &str)], expected: &str) {
      let url = Url::parse(url).unwrap();
      let headers = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
      let result = url_to_local_sub_path(&url, &headers).unwrap();
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
              "path": "deno.land/x/#different_content_ty_f15dc.js",
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
                "path": "deno.land/x/my_file.ts",
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
      let url = Url::parse("https://deno.land/INVALID/Module.ts?dev").unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
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

      assert_eq!(
        manifest_file.read_json_value(),
        json!({
          "modules": {
            "https://deno.land/INVALID/Module.ts?dev": {
              "path": "deno.land/#invalid_1ee01/#module_b8d2b.ts"
            }
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
}
