// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use serde::Deserialize;
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

use super::cache::HttpCache;
use super::cache::HttpCacheItemKey;
use crate::cache::CacheEntry;
use crate::cache::CacheReadFileError;
use crate::cache::Checksum;
use crate::cache::SerializedCachedUrlMetadata;
use crate::cache::url_to_filename;
use crate::common::HeadersMap;

mod cache_file;

#[sys_traits::auto_impl]
pub trait GlobalHttpCacheSys:
  FsCreateDirAll
  + FsMetadata
  + FsOpen
  + FsRead
  + FsRemoveFile
  + FsRename
  + ThreadSleep
  + SystemRandom
  + SystemTimeNow
  + std::fmt::Debug
  + MaybeSend
  + MaybeSync
  + Clone
{
}

#[allow(clippy::disallowed_types, reason = "arc wrapper type")]
pub type GlobalHttpCacheRc<TSys> =
  deno_maybe_sync::MaybeArc<GlobalHttpCache<TSys>>;

#[derive(Debug)]
pub struct GlobalHttpCache<Sys: GlobalHttpCacheSys> {
  path: PathBuf,
  pub(crate) sys: Sys,
}

impl<Sys: GlobalHttpCacheSys> GlobalHttpCache<Sys> {
  pub fn new(sys: Sys, path: PathBuf) -> Self {
    #[cfg(not(target_arch = "wasm32"))]
    assert!(path.is_absolute());
    Self { path, sys }
  }

  pub fn dir_path(&self) -> &PathBuf {
    &self.path
  }

  pub fn local_path_for_url(&self, url: &Url) -> std::io::Result<PathBuf> {
    Ok(self.path.join(url_to_filename(url)?))
  }

  #[inline]
  fn key_file_path<'a>(&self, key: &'a HttpCacheItemKey) -> &'a PathBuf {
    // The key file path is always set for the global cache because
    // the file will always exist, unlike the local cache, which won't
    // have this for redirects.
    key.file_path.as_ref().unwrap()
  }
}

impl<TSys: GlobalHttpCacheSys> HttpCache for GlobalHttpCache<TSys> {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> std::io::Result<HttpCacheItemKey<'a>> {
    Ok(HttpCacheItemKey {
      #[cfg(debug_assertions)]
      is_local_key: false,
      url,
      file_path: Some(self.local_path_for_url(url)?),
    })
  }

  fn contains(&self, url: &Url) -> bool {
    let Ok(cache_filepath) = self.local_path_for_url(url) else {
      return false;
    };
    self.sys.fs_is_file(&cache_filepath).unwrap_or(false)
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    match self.sys.fs_metadata(self.key_file_path(key)) {
      Ok(metadata) => match metadata.modified() {
        Ok(time) => Ok(Some(time)),
        Err(_) => Ok(Some(self.sys.sys_time_now())),
      },
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(err) => Err(err),
    }
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> std::io::Result<()> {
    let cache_filepath = self.local_path_for_url(url)?;
    cache_file::write(
      &self.sys,
      &cache_filepath,
      content,
      &SerializedCachedUrlMetadata {
        time: Some(
          self
            .sys
            .sys_time_now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        ),
        url: url.to_string(),
        headers,
      },
    )?;

    Ok(())
  }

  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<CacheEntry>, CacheReadFileError> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    let maybe_file = cache_file::read(&self.sys, self.key_file_path(key))?;

    if let Some(file) = &maybe_file
      && let Some(expected_checksum) = maybe_checksum
    {
      expected_checksum
        .check(key.url, &file.content)
        .map_err(CacheReadFileError::ChecksumIntegrity)?;
    }

    Ok(maybe_file)
  }

  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>> {
    // targeted deserialize
    #[derive(Deserialize)]
    struct SerializedHeaders {
      pub headers: HeadersMap,
    }

    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    let maybe_metadata = cache_file::read_metadata::<SerializedHeaders>(
      &self.sys,
      self.key_file_path(key),
    )?;
    Ok(maybe_metadata.map(|m| m.headers))
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    // targeted deserialize
    #[derive(Deserialize)]
    struct SerializedTime {
      pub time: Option<u64>,
    }

    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);
    let maybe_metadata = cache_file::read_metadata::<SerializedTime>(
      &self.sys,
      self.key_file_path(key),
    )?;
    Ok(maybe_metadata.and_then(|m| {
      Some(SystemTime::UNIX_EPOCH + Duration::from_secs(m.time?))
    }))
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_url_to_filename() {
    let test_cases = [
      (
        "https://deno.land/x/foo.ts",
        "https/deno.land/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8",
      ),
      (
        "https://deno.land:8080/x/foo.ts",
        "https/deno.land_PORT8080/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8",
      ),
      (
        "https://deno.land/",
        "https/deno.land/8a5edab282632443219e051e4ade2d1d5bbc671c781051bf1437897cbdfea0f1",
      ),
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
      ),
    ];

    for (url, expected) in test_cases.iter() {
      let u = Url::parse(url).unwrap();
      let p = url_to_filename(&u).unwrap();
      assert_eq!(p, PathBuf::from(expected));
    }
  }
}
