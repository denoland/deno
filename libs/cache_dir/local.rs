// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use deno_media_type::MediaType;
use deno_path_util::fs::atomic_write_file_with_retries;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use sys_traits::FsCreateDirAll;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsOpen;
use sys_traits::FsRead;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::SystemTimeNow;
use sys_traits::ThreadSleep;
use url::Url;

use super::Checksum;
use super::HttpCache;
use super::HttpCacheItemKey;
use super::common::HeadersMap;
use super::common::base_url_to_filename_parts;
use super::common::checksum;
use crate::CACHE_PERM;
use crate::SerializedCachedUrlMetadata;
use crate::cache::CacheEntry;
use crate::cache::CacheReadFileError;
use crate::cache::GlobalToLocalCopy;
use crate::global::GlobalHttpCacheRc;

#[sys_traits::auto_impl]
pub trait LocalHttpCacheSys:
  FsCreateDirAll
  + FsMetadata
  + FsOpen
  + FsRead
  + FsRemoveFile
  + FsRename
  + ThreadSleep
  + SystemRandom
  + SystemTimeNow
  + MaybeSend
  + MaybeSync
  + std::fmt::Debug
  + Clone
{
}

/// A vendor/ folder http cache for the lsp that provides functionality
/// for doing a reverse mapping.
#[derive(Debug)]
pub struct LocalLspHttpCache<TSys: LocalHttpCacheSys> {
  cache: LocalHttpCache<TSys>,
}

impl<TSys: LocalHttpCacheSys> LocalLspHttpCache<TSys> {
  pub fn new(path: PathBuf, global_cache: GlobalHttpCacheRc<TSys>) -> Self {
    #[cfg(not(target_arch = "wasm32"))]
    assert!(path.is_absolute());
    let manifest = LocalCacheManifest::new_for_lsp(
      path.join("manifest.json"),
      global_cache.sys.clone(),
    );
    Self {
      cache: LocalHttpCache {
        path,
        manifest,
        global_cache,
        // In the LSP, we disallow the cache from automatically copying from
        // the global cache to the local cache for technical reasons.
        //
        // 1. We need to verify the checksums from the lockfile are correct when
        //    moving from the global to the local cache.
        // 2. We need to verify the checksums for JSR https specifiers match what
        //    is found in the package's manifest.
        allow_global_to_local: GlobalToLocalCopy::Disallow,
        jsr_registry_url: None, // only used when GlobalToLocalCopy::Allow
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
    if self.cache.env().fs_is_file_no_err(&path) {
      deno_path_util::url_from_file_path(&path).ok()
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

impl<TSys: LocalHttpCacheSys> HttpCache for LocalLspHttpCache<TSys> {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> std::io::Result<HttpCacheItemKey<'a>> {
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
  ) -> std::io::Result<()> {
    self.cache.set(url, headers, content)
  }

  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<crate::cache::CacheEntry>, CacheReadFileError> {
    self.cache.get(key, maybe_checksum)
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    self.cache.read_modified_time(key)
  }

  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>> {
    self.cache.read_headers(key)
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    self.cache.read_modified_time(key)
  }
}

#[allow(clippy::disallowed_types, reason = "arc wrapper type")]
pub type LocalHttpCacheRc<TSys> =
  deno_maybe_sync::MaybeArc<LocalHttpCache<TSys>>;

#[derive(Debug)]
pub struct LocalHttpCache<TSys: LocalHttpCacheSys> {
  path: PathBuf,
  manifest: LocalCacheManifest<TSys>,
  global_cache: GlobalHttpCacheRc<TSys>,
  allow_global_to_local: GlobalToLocalCopy,
  jsr_registry_url: Option<Url>,
}

impl<TSys: LocalHttpCacheSys> LocalHttpCache<TSys> {
  pub fn new(
    path: PathBuf,
    global_cache: GlobalHttpCacheRc<TSys>,
    allow_global_to_local: GlobalToLocalCopy,
    jsr_registry_url: Url,
  ) -> Self {
    #[cfg(not(target_arch = "wasm32"))]
    assert!(path.is_absolute());
    let manifest = LocalCacheManifest::new(
      path.join("manifest.json"),
      global_cache.sys.clone(),
    );
    Self {
      path,
      manifest,
      global_cache,
      allow_global_to_local,
      jsr_registry_url: Some(jsr_registry_url),
    }
  }

  #[inline]
  fn env(&self) -> &TSys {
    &self.global_cache.sys
  }

  fn get_url_headers(&self, url: &Url) -> std::io::Result<Option<HeadersMap>> {
    if let Some(metadata) = self.manifest.get_stored_headers(url) {
      return Ok(Some(metadata));
    }

    // if the local path exists, don't copy the headers from the global cache
    // to the local
    let local_path = url_to_local_sub_path(url, None)?;
    if self
      .env()
      .fs_is_file_no_err(local_path.as_path_from_root(&self.path))
    {
      return Ok(Some(Default::default()));
    }

    if !self.allow_global_to_local.is_true() {
      return Ok(None);
    }

    // not found locally, so try to copy from the global manifest
    let global_key = self.global_cache.cache_item_key(url)?;
    let Some(headers) = self.global_cache.read_headers(&global_key)? else {
      return Ok(None);
    };

    let local_path =
      url_to_local_sub_path(url, headers_content_type(&headers))?;
    self.manifest.insert_data(local_path, url.clone(), headers);

    Ok(Some(self.manifest.get_stored_headers(url).unwrap_or_else(
      || {
        // if it's not in the stored headers at this point then that means
        // the file has no headers that need to be stored for the local cache
        Default::default()
      },
    )))
  }

  pub fn local_path_for_url(
    &self,
    url: &Url,
  ) -> std::io::Result<Option<PathBuf>> {
    if let Some(headers) = self.get_url_headers(url)? {
      let is_redirect = headers.contains_key("location");
      if is_redirect {
        return Ok(None);
      }

      let local_path =
        url_to_local_sub_path(url, headers_content_type(&headers))?;
      Ok(Some(local_path.as_path_from_root(&self.path)))
    } else {
      Ok(None)
    }
  }

  fn transform_content_on_copy_to_local<'a>(
    &self,
    url: &Url,
    content: Cow<'a, [u8]>,
  ) -> Cow<'a, [u8]> {
    let Some(jsr_url) = &self.jsr_registry_url else {
      return content;
    };
    if is_jsr_version_metadata_url(url, jsr_url)
      && let Some(data) = transform_jsr_version_metadata(&content)
    {
      return Cow::Owned(data);
    }
    content
  }
}

fn is_jsr_version_metadata_url(url: &Url, jsr_url: &Url) -> bool {
  // example url: https://jsr.io/@david/dax/0.43.0_meta.json
  let Some(suffix) = url.as_str().strip_prefix(jsr_url.as_str()) else {
    return false;
  };
  let Some(suffix) = suffix.strip_prefix('@') else {
    return false;
  };
  let Some(prefix) = suffix.strip_suffix("_meta.json") else {
    return false;
  };
  prefix.chars().filter(|c| *c == '/').count() == 2
}

fn transform_jsr_version_metadata(content: &[u8]) -> Option<Vec<u8>> {
  let checksum = checksum(content);
  let mut json_data =
    serde_json::from_slice::<serde_json::Value>(content).ok()?;
  let obj = json_data.as_object_mut()?;
  let keys_to_remove = obj
    .keys()
    .filter(|k| k.starts_with("moduleGraph"))
    .cloned()
    .collect::<Vec<_>>();
  for key in keys_to_remove {
    obj.remove(&key);
  }
  obj.insert("lockfileChecksum".into(), checksum.into());
  serde_json::to_vec(&json_data).ok()
}

impl<TSys: LocalHttpCacheSys> HttpCache for LocalHttpCache<TSys> {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> std::io::Result<HttpCacheItemKey<'a>> {
    Ok(HttpCacheItemKey {
      #[cfg(debug_assertions)]
      is_local_key: true,
      url,
      file_path: None, // need to compute this every time
    })
  }

  fn contains(&self, url: &Url) -> bool {
    self
      .get_url_headers(url)
      .ok()
      .map(|d| d.is_some())
      .unwrap_or(false)
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    #[cfg(debug_assertions)]
    debug_assert!(key.is_local_key);

    if let Some(headers) = self.get_url_headers(key.url)? {
      let local_path =
        url_to_local_sub_path(key.url, headers_content_type(&headers))?;
      if let Ok(metadata) = self
        .env()
        .fs_metadata(local_path.as_path_from_root(&self.path))
        && let Ok(modified_time) = metadata.modified()
      {
        return Ok(Some(modified_time));
      }
    }

    // fallback to the global cache
    let global_key = self.global_cache.cache_item_key(key.url)?;
    self.global_cache.read_modified_time(&global_key)
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> std::io::Result<()> {
    let is_redirect = headers.contains_key("location");
    let sub_path = url_to_local_sub_path(url, headers_content_type(&headers))?;

    if !is_redirect {
      let content =
        self.transform_content_on_copy_to_local(url, Cow::Borrowed(content));
      // Cache content
      atomic_write_file_with_retries(
        self.env(),
        &sub_path.as_path_from_root(&self.path),
        &content,
        CACHE_PERM,
      )?;
    }

    self.manifest.insert_data(sub_path, url.clone(), headers);

    Ok(())
  }

  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<CacheEntry>, CacheReadFileError> {
    #[cfg(debug_assertions)]
    debug_assert!(key.is_local_key);

    let maybe_headers = self.get_url_headers(key.url)?;
    match maybe_headers {
      Some(headers) => {
        let is_redirect = headers.contains_key("location");
        let bytes: Cow<'static, [u8]> = if is_redirect {
          // return back an empty file for redirect
          Cow::Borrowed(&[])
        } else {
          // if it's not a redirect, then it should have a file path
          let local_file_path =
            url_to_local_sub_path(key.url, headers_content_type(&headers))?
              .as_path_from_root(&self.path);
          let file_bytes_result = self.env().fs_read(&local_file_path);
          match file_bytes_result {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
              if self.allow_global_to_local.is_true() {
                // only check the checksum when copying from the global to the local cache
                let global_key = self.global_cache.cache_item_key(key.url)?;
                let maybe_global_cache_file =
                  self.global_cache.get(&global_key, maybe_checksum)?;
                if let Some(file) = maybe_global_cache_file {
                  let content = self
                    .transform_content_on_copy_to_local(key.url, file.content);
                  atomic_write_file_with_retries(
                    self.env(),
                    &local_file_path,
                    &content,
                    CACHE_PERM,
                  )?;
                  content
                } else {
                  return Ok(None);
                }
              } else {
                return Ok(None);
              }
            }
            Err(err) => return Err(CacheReadFileError::Io(err)),
          }
        };
        Ok(Some(CacheEntry {
          metadata: SerializedCachedUrlMetadata {
            headers,
            url: key.url.to_string(),
            // not used for the local cache
            time: None,
          },
          content: bytes,
        }))
      }
      None => Ok(None),
    }
  }

  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>> {
    #[cfg(debug_assertions)]
    debug_assert!(key.is_local_key);

    self.get_url_headers(key.url)
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    // This will never be called for the local cache in practice
    // because only the LSP ever reads this time for telling if
    // a file should be re-downloaded when respecting cache headers
    // and it only does this using a global cache
    self.read_modified_time(key)
  }
}

pub(super) struct LocalCacheSubPath<'a> {
  pub has_hash: bool,
  pub parts: Vec<Cow<'a, str>>,
}

impl LocalCacheSubPath<'_> {
  pub fn as_path_from_root(&self, root_path: &Path) -> PathBuf {
    let mut path = root_path.to_path_buf();
    for part in &self.parts {
      path.push(part.as_ref());
    }
    path
  }

  pub fn as_relative_path(&self) -> PathBuf {
    let mut path =
      PathBuf::with_capacity(self.parts.iter().map(|p| p.len() + 1).sum());
    for part in &self.parts {
      path.push(part.as_ref());
    }
    path
  }
}

fn headers_content_type(headers: &HeadersMap) -> Option<&str> {
  headers.get("content-type").map(|s| s.as_str())
}

fn url_to_local_sub_path<'a>(
  url: &'a Url,
  content_type: Option<&str>,
) -> std::io::Result<LocalCacheSubPath<'a>> {
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
    let media_type =
      MediaType::from_specifier_and_content_type(url, content_type);
    match media_type {
      MediaType::JavaScript => ".js",
      MediaType::Jsx => ".jsx",
      MediaType::Mjs => ".mjs",
      MediaType::Cjs => ".cjs",
      MediaType::TypeScript => ".ts",
      MediaType::Mts => ".mts",
      MediaType::Cts => ".cts",
      MediaType::Dts => ".d.ts",
      MediaType::Dmts => ".d.mts",
      MediaType::Dcts => ".d.cts",
      MediaType::Tsx => ".tsx",
      MediaType::Css => ".css",
      MediaType::Json => ".json",
      MediaType::Jsonc => ".jsonc",
      MediaType::Json5 => ".json5",
      MediaType::Markdown => ".md",
      MediaType::Wasm => ".wasm",
      MediaType::SourceMap => ".js",
      MediaType::Html => ".html",
      MediaType::Sql => ".sql",
      MediaType::Unknown => ".js",
    }
  }

  fn short_hash(data: &str, last_ext: Option<&str>) -> String {
    // This function is a bit of a balancing act between readability
    // and avoiding collisions.
    let hash = checksum(data.as_bytes());
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
  let Some(mut base_parts) = base_url_to_filename_parts(url, port_separator)
  else {
    return Err(std::io::Error::new(
      ErrorKind::InvalidInput,
      format!("Can't convert url (\"{}\") to filename.", url),
    ));
  };

  if base_parts[0] == "https" {
    base_parts.remove(0);
  } else {
    let scheme = base_parts.remove(0);
    base_parts[0] = Cow::Owned(format!("{}_{}", scheme, base_parts[0]));
  }

  // first, try to get the filename of the path
  let path_segments = url_path_segments(url);
  let mut parts = base_parts
    .into_iter()
    .chain(path_segments.map(Cow::Borrowed))
    .collect::<Vec<_>>();

  // push the query parameter onto the last part
  if let Some(query) = url.query() {
    let last_part = parts.last_mut().unwrap();
    let last_part = match last_part {
      Cow::Borrowed(_) => {
        *last_part = Cow::Owned(last_part.to_string());
        match last_part {
          Cow::Borrowed(_) => unreachable!(),
          Cow::Owned(s) => s,
        }
      }
      Cow::Owned(last_part) => last_part,
    };
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
        Cow::Owned(short_hash(&part, last_ext))
      } else {
        part
      }
    })
    .collect::<Vec<_>>();

  Ok(LocalCacheSubPath { has_hash, parts })
}

#[derive(Debug)]
struct LocalCacheManifest<
  Sys: FsCreateDirAll
    + FsMetadata
    + FsOpen
    + FsRead
    + FsRemoveFile
    + FsRename
    + ThreadSleep
    + SystemRandom
    + MaybeSend
    + MaybeSync
    + std::fmt::Debug,
> {
  sys: Sys,
  file_path: PathBuf,
  data: RwLock<manifest::LocalCacheManifestData>,
}

impl<
  Sys: FsCreateDirAll
    + FsMetadata
    + FsOpen
    + FsRead
    + FsRemoveFile
    + FsRename
    + ThreadSleep
    + SystemRandom
    + MaybeSend
    + MaybeSync
    + std::fmt::Debug
    + Clone,
> LocalCacheManifest<Sys>
{
  pub fn new(file_path: PathBuf, sys: Sys) -> Self {
    Self::new_internal(file_path, false, sys)
  }

  pub fn new_for_lsp(file_path: PathBuf, sys: Sys) -> Self {
    Self::new_internal(file_path, true, sys)
  }

  fn new_internal(
    file_path: PathBuf,
    use_reverse_mapping: bool,
    sys: Sys,
  ) -> Self {
    let text = sys
      .fs_read(&file_path)
      .ok()
      .and_then(|bytes| String::from_utf8(bytes.into_owned()).ok());
    Self {
      sys,
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

    let mut headers_subset = BTreeMap::new();

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
      let result = atomic_write_file_with_retries(
        &self.sys,
        &self.file_path,
        data.as_json().as_bytes(),
        CACHE_PERM,
      );
      if let Err(err) = result {
        log::debug!("Failed saving local cache manifest: {:#}", err);
      }
    }
  }

  pub fn get_stored_headers(&self, url: &Url) -> Option<HeadersMap> {
    let data = self.data.read();
    data.get(url).map(|module| {
      module
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<HashMap<_, _>>()
    })
  }
}

// This is in a separate module in order to enforce keeping
// the internal implementation private.
mod manifest {
  use std::collections::BTreeMap;
  use std::path::Path;
  use std::path::PathBuf;

  use serde::Deserialize;
  use serde::Serialize;
  use url::Url;

  use super::LocalCacheSubPath;
  use super::url_to_local_sub_path;

  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  pub struct SerializedLocalCacheManifestDataModule {
    #[serde(
      default = "BTreeMap::new",
      skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub headers: BTreeMap<String, String>,
  }

  impl SerializedLocalCacheManifestDataModule {
    pub fn content_type_header(&self) -> Option<&str> {
      self.headers.get("content-type").map(|s| s.as_str())
    }
  }

  // Using BTreeMap to make sure that the data is always sorted
  #[derive(Debug, Default, Clone, Serialize, Deserialize)]
  struct SerializedLocalCacheManifestData {
    #[serde(
      default = "BTreeMap::new",
      skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub folders: BTreeMap<Url, String>,
    #[serde(
      default = "BTreeMap::new",
      skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub modules: BTreeMap<Url, SerializedLocalCacheManifestDataModule>,
  }

  #[derive(Debug, Default, Clone)]
  pub(super) struct LocalCacheManifestData {
    serialized: SerializedLocalCacheManifestData,
    // reverse mapping used in the lsp
    reverse_mapping: Option<BTreeMap<PathBuf, Url>>,
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
            .collect::<BTreeMap<_, _>>(),
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
      if let Some(current) = self.serialized.folders.get(&url)
        && *current == local_path
      {
        return false;
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
  use deno_maybe_sync::new_rc;
  use pretty_assertions::assert_eq;
  use sys_traits::impls::RealSys;
  use tempfile::TempDir;
  use tempfile::tempdir;

  use super::*;
  use crate::GlobalHttpCache;

  struct TestCaches {
    global_cache: GlobalHttpCacheRc<RealSys>,
    local_cache: LocalHttpCacheRc<RealSys>,
    local_temp: PathBuf,
    _temp: TempDir,
  }

  impl TestCaches {
    fn new() -> Self {
      let temp = tempdir().unwrap();
      let global_temp = temp.path().join("global");
      let local_temp = temp.path().join("local");

      let global_cache = GlobalHttpCache::new(RealSys, global_temp);
      let global_cache = new_rc(global_cache);
      let local_cache = new_rc(LocalHttpCache::new(
        local_temp.clone(),
        global_cache.clone(),
        GlobalToLocalCopy::Allow,
        Url::parse("https://jsr.io/").unwrap(),
      ));
      Self {
        global_cache,
        local_cache,
        local_temp,
        _temp: temp,
      }
    }
  }

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
    run_test("https://deno.land/x/mod.wasm", &[], "deno.land/x/mod.wasm");

    #[track_caller]
    fn run_test(url: &str, headers: &[(&str, &str)], expected: &str) {
      let test_caches = TestCaches::new();

      let url = Url::parse(url).unwrap();
      let headers: HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
      test_caches
        .global_cache
        .set(&url, headers.clone(), b"console.log('hello');")
        .unwrap();
      let path = test_caches
        .local_cache
        .local_path_for_url(&url)
        .unwrap()
        .unwrap();
      let result =
        url_to_local_sub_path(&url, headers_content_type(&headers)).unwrap();
      let parts = result.parts.join("/");
      assert_eq!(path, test_caches.local_temp.join(&parts));
      assert_eq!(parts, expected);
      assert_eq!(
        result.parts.iter().any(|p| p.starts_with('#')),
        result.has_hash
      )
    }
  }

  #[test]
  fn local_path_no_headers() {
    let test_caches = TestCaches::new();
    let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
    assert!(matches!(
      test_caches.local_cache.local_path_for_url(&url),
      Ok(None)
    ));
  }

  #[test]
  fn local_path_redirect() {
    let test_caches = TestCaches::new();
    let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
    test_caches
      .global_cache
      .set(
        &url,
        HashMap::from([(
          "location".to_string(),
          "https://deno.land/x/mod.ts".to_string(),
        )]),
        b"",
      )
      .unwrap();
    assert!(matches!(
      test_caches.local_cache.local_path_for_url(&url),
      Ok(None)
    ));
  }

  #[test]
  fn test_copy_version_metadata_file() {
    let test_caches = TestCaches::new();
    let data =
      r#"{ "moduleGraph2": "testing", "checksums": { "test": "test" } }"#;
    // has the moduleGraph2 property stripped
    let expected_data = r#"{"checksums":{"test":"test"},"lockfileChecksum":"dc108ae9ffb13086cb1551692960c21893991a7f0f9dc770814ff21522fd1d48"}"#;
    {
      let metadata_url =
        Url::parse("https://jsr.io/@david/dax/1.2.3_meta.json").unwrap();
      test_caches
        .global_cache
        .set(&metadata_url, Default::default(), data.as_bytes())
        .unwrap();
      let key = test_caches
        .local_cache
        .cache_item_key(&metadata_url)
        .unwrap();
      let final_data =
        test_caches.local_cache.get(&key, None).unwrap().unwrap();
      assert_eq!(
        String::from_utf8(final_data.content.to_vec()).unwrap(),
        expected_data,
      );
    }
    {
      // now try just setting directly in the local cache
      let metadata_url =
        Url::parse("https://jsr.io/@david/dax/1.2.2_meta.json").unwrap();
      test_caches
        .local_cache
        .set(&metadata_url, Default::default(), data.as_bytes())
        .unwrap();
      let key = test_caches
        .local_cache
        .cache_item_key(&metadata_url)
        .unwrap();
      let final_data =
        test_caches.local_cache.get(&key, None).unwrap().unwrap();
      assert_eq!(
        String::from_utf8(final_data.content.to_vec()).unwrap(),
        expected_data,
      );
    }
  }

  #[test]
  fn test_is_jsr_version_metadata_url() {
    let cases = [
      ("https://jsr.io/@test/test/1.2.3_meta.json", true),
      ("https://jsr.io/@test/test/test/1.2.3_meta.json", false),
      ("https://jsr.io/@test/test/meta.json", false),
      ("https://jsr.io/test/test/1.2.3_meta.json", false),
      ("https://jsr.com/@test/test/1.2.3_meta.json", false),
    ];
    let jsr_url = Url::parse("https://jsr.io/").unwrap();
    for (url, expected) in cases {
      let value =
        is_jsr_version_metadata_url(&Url::parse(url).unwrap(), &jsr_url);
      assert_eq!(value, expected);
    }
  }

  #[test]
  fn test_transform_jsr_version_metadata() {
    let cases = [
      (
        r#"{ "moduleGraph1": "data", "moduleGraph2": "data", "moduleGraph3": "data", "other": "data" }"#,
        Some(
          r#"{"other":"data","lockfileChecksum":"1438025e1aa277249e97fa99a2283542ab1156b0967c3e0997f78bee22d121ad"}"#,
        ),
      ),
      (
        r#"{ "other": "data" }"#,
        Some(
          r#"{"other":"data","lockfileChecksum":"62db4d2ded7cb6348ccb1648b2a27ed96dbe0fadc42c8359024c0213bab2f0e5"}"#,
        ),
      ),
    ];

    for (input, expected) in cases {
      let output = transform_jsr_version_metadata(input.as_bytes());
      assert_eq!(
        output.map(|o| String::from_utf8(o).unwrap()).as_deref(),
        expected
      )
    }
  }
}
