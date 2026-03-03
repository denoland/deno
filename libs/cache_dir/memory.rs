// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::time::UNIX_EPOCH;

use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use parking_lot::Mutex;
use sys_traits::SystemTimeNow;
use url::Url;

use crate::CacheEntry;
use crate::CacheReadFileError;
use crate::Checksum;
use crate::HeadersMap;
use crate::HttpCache;
use crate::HttpCacheItemKey;
use crate::SerializedCachedUrlMetadata;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct MemoryHttpCacheSystemTimeClock;

#[cfg(not(target_arch = "wasm32"))]
impl sys_traits::SystemTimeNow for MemoryHttpCacheSystemTimeClock {
  fn sys_time_now(&self) -> std::time::SystemTime {
    #[allow(clippy::disallowed_methods)]
    std::time::SystemTime::now()
  }
}

/// A simple in-memory cache mostly useful for testing.
#[derive(Debug)]
pub struct MemoryHttpCache<TSys: SystemTimeNow + MaybeSend + MaybeSync> {
  cache: Mutex<HashMap<Url, CacheEntry>>,
  clock: TSys,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for MemoryHttpCache<MemoryHttpCacheSystemTimeClock> {
  fn default() -> Self {
    Self::new(MemoryHttpCacheSystemTimeClock)
  }
}

impl<TSys: SystemTimeNow + std::fmt::Debug + MaybeSend + MaybeSync>
  MemoryHttpCache<TSys>
{
  pub fn new(clock: TSys) -> Self {
    Self {
      cache: Mutex::new(HashMap::new()),
      clock,
    }
  }
}

impl<TSys: SystemTimeNow + std::fmt::Debug + MaybeSend + MaybeSync> HttpCache
  for MemoryHttpCache<TSys>
{
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> std::io::Result<HttpCacheItemKey<'a>> {
    Ok(HttpCacheItemKey {
      #[cfg(debug_assertions)]
      is_local_key: false,
      url,
      file_path: None,
    })
  }

  fn contains(&self, url: &Url) -> bool {
    self.cache.lock().contains_key(url)
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> std::io::Result<()> {
    self.cache.lock().insert(
      url.clone(),
      CacheEntry {
        metadata: SerializedCachedUrlMetadata {
          headers,
          url: url.to_string(),
          time: Some(
            self
              .clock
              .sys_time_now()
              .duration_since(UNIX_EPOCH)
              .unwrap()
              .as_secs(),
          ),
        },
        content: Cow::Owned(content.to_vec()),
      },
    );
    Ok(())
  }

  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<CacheEntry>, CacheReadFileError> {
    self
      .cache
      .lock()
      .get(key.url)
      .cloned()
      .map(|entry| {
        if let Some(checksum) = maybe_checksum {
          checksum
            .check(key.url, &entry.content)
            .map_err(CacheReadFileError::ChecksumIntegrity)?;
        }
        Ok(entry)
      })
      .transpose()
  }

  fn read_modified_time(
    &self,
    _key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<std::time::SystemTime>> {
    Ok(None) // for now
  }

  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>> {
    Ok(
      self
        .cache
        .lock()
        .get(key.url)
        .map(|entry| entry.metadata.headers.clone()),
    )
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<std::time::SystemTime>> {
    Ok(self.cache.lock().get(key.url).and_then(|entry| {
      entry
        .metadata
        .time
        .map(|time| UNIX_EPOCH + std::time::Duration::from_secs(time))
    }))
  }
}
