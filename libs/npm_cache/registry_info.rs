// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use deno_error::JsErrorBox;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_unsync::sync::AtomicFlag;
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use parking_lot::Mutex;
use serde::Deserialize;
use serde::Serialize;
use url::Url;

use crate::NpmCache;
use crate::NpmCacheHttpClient;
use crate::NpmCacheHttpClientResponse;
use crate::NpmCacheSetting;
use crate::NpmCacheSys;
use crate::remote::maybe_auth_header_value_for_npm_registry;
use crate::rt::MultiRuntimeAsyncValueCreator;
use crate::rt::spawn;
use crate::rt::spawn_blocking;

type LoadResult = Result<FutureResult, Arc<JsErrorBox>>;
type LoadFuture = LocalBoxFuture<'static, LoadResult>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerializedCachedPackageInfo {
  #[serde(flatten)]
  pub info: NpmPackageInfo,
  /// Custom property that includes the etag.
  #[serde(
    default,
    skip_serializing_if = "Option::is_none",
    rename = "_deno.etag"
  )]
  pub etag: Option<String>,
}

#[derive(Debug, Clone)]
enum FutureResult {
  PackageNotExists,
  SavedFsCache(Arc<NpmPackageInfo>),
  ErroredFsCache(Arc<NpmPackageInfo>),
}

#[derive(Debug, Clone)]
enum MemoryCacheItem {
  /// The cache item hasn't loaded yet.
  Pending(Arc<MultiRuntimeAsyncValueCreator<LoadResult>>),
  /// The item has loaded in the past and was stored in the file system cache.
  /// There is no reason to request this package from the npm registry again
  /// for the duration of execution.
  FsCached(Arc<NpmPackageInfo>),
  /// An item is memory cached when it fails saving to the file system cache
  /// or the package does not exist.
  MemoryCached(Result<Option<Arc<NpmPackageInfo>>, Arc<JsErrorBox>>),
}

#[derive(Debug, Default)]
struct MemoryCache {
  clear_id: usize,
  items: HashMap<String, MemoryCacheItem>,
}

impl MemoryCache {
  #[inline(always)]
  pub fn clear(&mut self) {
    self.clear_id += 1;

    // if the item couldn't be saved to the fs cache, then we want to continue to hold it in memory
    // to avoid re-downloading it from the registry
    self
      .items
      .retain(|_, item| matches!(item, MemoryCacheItem::MemoryCached(Ok(_))));
  }

  #[inline(always)]
  pub fn clear_all(&mut self) {
    self.clear_id += 1;
    self.items.clear();
  }

  #[inline(always)]
  pub fn get(&self, key: &str) -> Option<&MemoryCacheItem> {
    self.items.get(key)
  }

  #[inline(always)]
  pub fn insert(&mut self, key: String, value: MemoryCacheItem) {
    self.items.insert(key, value);
  }

  #[inline(always)]
  pub fn try_insert(
    &mut self,
    clear_id: usize,
    key: &str,
    value: MemoryCacheItem,
  ) -> bool {
    if clear_id != self.clear_id {
      return false;
    }
    // if the clear_id is the same then the item should exist
    debug_assert!(self.items.contains_key(key));
    if let Some(item) = self.items.get_mut(key) {
      *item = value;
    }
    true
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("Failed loading {url} for package \"{name}\"")]
pub struct LoadPackageInfoError {
  url: Url,
  name: String,
  #[inherit]
  #[source]
  inner: LoadPackageInfoInnerError,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("{0}")]
pub struct LoadPackageInfoInnerError(pub Arc<JsErrorBox>);

#[derive(Debug)]
pub struct RegistryInfoStats {
  /// Number of package_info() calls that resolved from the memory cache
  /// without waiting for any I/O.
  pub cache_hits: AtomicUsize,
  /// Number of package_info() calls that had to wait for a pending
  /// load (either started by prefetch or by this call itself).
  pub pending_awaits: AtomicUsize,
  /// Number of load_package_info calls that actually started a new
  /// network download (as opposed to deduplicating onto an existing one).
  pub network_fetches: AtomicUsize,
  /// Current number of in-flight network requests.
  pub in_flight: AtomicUsize,
  /// Peak number of concurrent in-flight network requests.
  pub peak_in_flight: AtomicUsize,
  /// Number of prefetch_package_info() calls.
  pub prefetch_calls: AtomicUsize,
  /// Number of prefetch calls that were already cached (no-op).
  pub prefetch_already_cached: AtomicUsize,
  /// Number of prefetch calls skipped because the concurrency limit was hit.
  pub prefetch_skipped_at_capacity: AtomicUsize,
}

impl Default for RegistryInfoStats {
  fn default() -> Self {
    Self {
      cache_hits: AtomicUsize::new(0),
      pending_awaits: AtomicUsize::new(0),
      network_fetches: AtomicUsize::new(0),
      in_flight: AtomicUsize::new(0),
      peak_in_flight: AtomicUsize::new(0),
      prefetch_calls: AtomicUsize::new(0),
      prefetch_already_cached: AtomicUsize::new(0),
      prefetch_skipped_at_capacity: AtomicUsize::new(0),
    }
  }
}

impl std::fmt::Display for RegistryInfoStats {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "cache_hits={}, pending_awaits={}, network_fetches={}, peak_in_flight={}, \
       prefetch_calls={}, prefetch_already_cached={}, prefetch_skipped_at_capacity={}",
      self.cache_hits.load(Ordering::Relaxed),
      self.pending_awaits.load(Ordering::Relaxed),
      self.network_fetches.load(Ordering::Relaxed),
      self.peak_in_flight.load(Ordering::Relaxed),
      self.prefetch_calls.load(Ordering::Relaxed),
      self.prefetch_already_cached.load(Ordering::Relaxed),
      self.prefetch_skipped_at_capacity.load(Ordering::Relaxed),
    )
  }
}

/// Maximum number of concurrent speculative prefetch tasks.
/// This limits background prefetches to avoid overwhelming the registry,
/// while leaving the critical path (direct package_info() calls from
/// FuturesOrdered) unlimited so resolution is never blocked by prefetches.
///
/// Note: Because prefetch tasks race ahead of the critical path and create
/// Pending entries that the critical path deduplicates onto, this effectively
/// caps total concurrent downloads for prefetched packages. The critical path
/// only starts its own downloads for packages not yet prefetched. Setting this
/// too low starves the critical path of pre-warmed cache entries.
const MAX_CONCURRENT_PREFETCH_TASKS: usize = 50;

#[derive(Debug)]
struct RegistryInfoProviderInner<
  THttpClient: NpmCacheHttpClient,
  TSys: NpmCacheSys,
> {
  cache: Arc<NpmCache<TSys>>,
  http_client: Arc<THttpClient>,
  npmrc: Arc<ResolvedNpmRc>,
  force_reload_flag: AtomicFlag,
  memory_cache: Mutex<MemoryCache>,
  previously_loaded_packages: Mutex<HashSet<String>>,
  /// Tracks the number of in-flight prefetch tasks to limit concurrency.
  prefetch_in_flight: AtomicUsize,
  stats: RegistryInfoStats,
}

impl<THttpClient: NpmCacheHttpClient, TSys: NpmCacheSys>
  RegistryInfoProviderInner<THttpClient, TSys>
{
  async fn maybe_package_info(
    self: &Arc<Self>,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, LoadPackageInfoError> {
    self
      .load_package_info(name)
      .await
      .map_err(|err| LoadPackageInfoError {
        url: get_package_url(&self.npmrc, name),
        name: name.to_string(),
        inner: err,
      })
  }

  async fn load_package_info(
    self: &Arc<Self>,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, LoadPackageInfoInnerError> {
    let (value_creator, clear_id) = {
      let mut mem_cache = self.memory_cache.lock();
      let cache_item = if let Some(cache_item) = mem_cache.get(name) {
        cache_item.clone()
      } else {
        let value_creator = MultiRuntimeAsyncValueCreator::new({
          let downloader = self.clone();
          let name = name.to_string();
          Box::new(move || downloader.create_load_future(&name))
        });
        let cache_item = MemoryCacheItem::Pending(Arc::new(value_creator));
        mem_cache.insert(name.to_string(), cache_item.clone());
        cache_item
      };
      match cache_item {
        MemoryCacheItem::FsCached(info) => {
          self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
          return Ok(Some(info));
        }
        MemoryCacheItem::MemoryCached(maybe_info) => {
          self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
          return maybe_info.map_err(LoadPackageInfoInnerError);
        }
        MemoryCacheItem::Pending(value_creator) => {
          self.stats.pending_awaits.fetch_add(1, Ordering::Relaxed);
          (value_creator, mem_cache.clear_id)
        }
      }
    };

    match value_creator.get().await {
      Ok(FutureResult::SavedFsCache(info)) => {
        // return back the future and mark this package as having
        // been saved in the cache for next time it's requested
        self.memory_cache.lock().try_insert(
          clear_id,
          name,
          MemoryCacheItem::FsCached(info.clone()),
        );
        Ok(Some(info))
      }
      Ok(FutureResult::ErroredFsCache(info)) => {
        // since saving to the fs cache failed, keep the package information in memory
        self.memory_cache.lock().try_insert(
          clear_id,
          name,
          MemoryCacheItem::MemoryCached(Ok(Some(info.clone()))),
        );
        Ok(Some(info))
      }
      Ok(FutureResult::PackageNotExists) => {
        self.memory_cache.lock().try_insert(
          clear_id,
          name,
          MemoryCacheItem::MemoryCached(Ok(None)),
        );
        Ok(None)
      }
      Err(err) => {
        let return_err = err.clone();
        self.memory_cache.lock().try_insert(
          clear_id,
          name,
          MemoryCacheItem::MemoryCached(Err(err)),
        );
        Err(LoadPackageInfoInnerError(return_err))
      }
    }
  }

  fn create_load_future(self: &Arc<Self>, name: &str) -> LoadFuture {
    let downloader = self.clone();
    let package_url = get_package_url(&self.npmrc, name);
    let registry_config = self.npmrc.get_registry_config(name);
    let maybe_auth_header_value =
      match maybe_auth_header_value_for_npm_registry(registry_config) {
        Ok(maybe_auth_header_value) => maybe_auth_header_value,
        Err(err) => {
          return std::future::ready(Err(Arc::new(JsErrorBox::from_err(err))))
            .boxed_local();
        }
      };
    let name = name.to_string();
    async move {
      let maybe_file_cached = if (downloader.cache.cache_setting().should_use_for_npm_package(&name) && !downloader.force_reload_flag.is_raised())
        // if this has been previously reloaded, then try loading from the file system cache
        || downloader.previously_loaded_packages.lock().contains(&name)
      {
        // attempt to load from the file cache
        match downloader.cache.load_package_info(&name).await.map_err(JsErrorBox::from_err)? { Some(cached_info) => {
          return Ok(FutureResult::SavedFsCache(Arc::new(cached_info.info)));
        } _ => {
          None
        }}
      } else {
        downloader.cache.load_package_info(&name).await.ok().flatten()
      };

      if *downloader.cache.cache_setting() == NpmCacheSetting::Only {
        return Err(JsErrorBox::new(
          "NotCached",
          format!(
            "npm package not found in cache: \"{name}\", --cached-only is specified."
          )
        ));
      }

      downloader.previously_loaded_packages.lock().insert(name.to_string());

      let (maybe_etag, maybe_cached_info) = match maybe_file_cached {
        Some(cached_info) => (cached_info.etag, Some(cached_info.info)),
        None => (None, None)
      };

      downloader.stats.network_fetches.fetch_add(1, Ordering::Relaxed);
      let current = downloader.stats.in_flight.fetch_add(1, Ordering::Relaxed) + 1;
      downloader.stats.peak_in_flight.fetch_max(current, Ordering::Relaxed);
      let response = downloader
        .http_client
        .download_with_retries_on_any_tokio_runtime(
          package_url,
          maybe_auth_header_value,
          maybe_etag,
        )
        .await;
      downloader.stats.in_flight.fetch_sub(1, Ordering::Relaxed);
      let response = response.map_err(JsErrorBox::from_err)?;
      match response {
        NpmCacheHttpClientResponse::NotModified => {
          log::debug!("Respected etag for packument '{0}'", name); // used in the tests
          Ok(FutureResult::SavedFsCache(Arc::new(maybe_cached_info.unwrap())))
        },
        NpmCacheHttpClientResponse::NotFound => Ok(FutureResult::PackageNotExists),
        NpmCacheHttpClientResponse::Bytes(response) => {
          let future_result = spawn_blocking(
            move || -> Result<FutureResult, JsErrorBox> {
              let mut package_info: SerializedCachedPackageInfo = serde_json::from_slice(&response.bytes).map_err(JsErrorBox::from_err)?;
              package_info.etag = response.etag;
              match downloader.cache.save_package_info(&name, &package_info) {
                Ok(()) => {
                  Ok(FutureResult::SavedFsCache(Arc::new(package_info.info)))
                }
                Err(err) => {
                  log::debug!(
                    "Error saving package {} to cache: {:#}",
                    name,
                    err
                  );
                  Ok(FutureResult::ErroredFsCache(Arc::new(package_info.info)))
                }
              }
            },
          )
          .await
          .map_err(JsErrorBox::from_err)??;
          Ok(future_result)
        },
      }
    }
    .map(|r| r.map_err(Arc::new))
    .boxed_local()
  }

  fn mark_force_reload(&self) -> bool {
    // never force reload the registry information if reloading
    // is disabled or if we're already reloading
    if matches!(
      self.cache.cache_setting(),
      NpmCacheSetting::Only | NpmCacheSetting::ReloadAll
    ) {
      return false;
    }
    if self.force_reload_flag.raise() {
      self.memory_cache.lock().clear_all();
      true
    } else {
      false
    }
  }
}

/// Downloads packuments from the npm registry.
///
/// This is shared amongst all the workers.
#[derive(Debug)]
pub struct RegistryInfoProvider<
  THttpClient: NpmCacheHttpClient,
  TSys: NpmCacheSys,
>(Arc<RegistryInfoProviderInner<THttpClient, TSys>>);

impl<THttpClient: NpmCacheHttpClient, TSys: NpmCacheSys>
  RegistryInfoProvider<THttpClient, TSys>
{
  pub fn new(
    cache: Arc<NpmCache<TSys>>,
    http_client: Arc<THttpClient>,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self(Arc::new(RegistryInfoProviderInner {
      cache,
      http_client,
      npmrc,
      force_reload_flag: AtomicFlag::lowered(),
      memory_cache: Default::default(),
      previously_loaded_packages: Default::default(),
      prefetch_in_flight: AtomicUsize::new(0),
      stats: Default::default(),
    }))
  }

  /// Clears the internal memory cache.
  pub fn clear_memory_cache(&self) {
    self.0.memory_cache.lock().clear();
  }

  pub fn stats(&self) -> &RegistryInfoStats {
    &self.0.stats
  }

  pub async fn maybe_package_info(
    &self,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, LoadPackageInfoError> {
    self.0.maybe_package_info(name).await
  }
}

#[async_trait(?Send)]
impl<THttpClient: NpmCacheHttpClient, TSys: NpmCacheSys> NpmRegistryApi
  for RegistryInfoProvider<THttpClient, TSys>
{
  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    match self.maybe_package_info(name).await {
      Ok(Some(info)) => Ok(info),
      Ok(None) => Err(NpmRegistryPackageInfoLoadError::PackageNotExists {
        package_name: name.to_string(),
      }),
      Err(err) => Err(NpmRegistryPackageInfoLoadError::LoadError(Arc::new(
        JsErrorBox::from_err(err),
      ))),
    }
  }

  fn prefetch_package_info(&self, name: &str) {
    self.0.stats.prefetch_calls.fetch_add(1, Ordering::Relaxed);

    // Atomically check the cache AND insert a Pending entry if absent,
    // all under a single lock acquisition. This prevents the TOCTOU race
    // where multiple concurrent prefetch calls for the same package name
    // all pass the cache check (seeing no entry) before any of them
    // creates a Pending entry, each wasting a concurrency slot.
    {
      let mut mem_cache = self.0.memory_cache.lock();
      if mem_cache.get(name).is_some() {
        self.0.stats.prefetch_already_cached.fetch_add(1, Ordering::Relaxed);
        return;
      }

      // Limit concurrent prefetch tasks to avoid overwhelming the registry.
      // Check under the same lock to avoid races between check and insert.
      let current = self.0.prefetch_in_flight.load(Ordering::Relaxed);
      if current >= MAX_CONCURRENT_PREFETCH_TASKS {
        self.0.stats.prefetch_skipped_at_capacity.fetch_add(1, Ordering::Relaxed);
        return;
      }
      self.0.prefetch_in_flight.fetch_add(1, Ordering::Relaxed);

      // Insert the Pending entry now, while we hold the lock.
      // This ensures any subsequent prefetch_package_info or load_package_info
      // call for the same name sees this entry and deduplicates onto it.
      let value_creator = MultiRuntimeAsyncValueCreator::new({
        let downloader = self.0.clone();
        let name = name.to_string();
        Box::new(move || downloader.create_load_future(&name))
      });
      mem_cache.insert(
        name.to_string(),
        MemoryCacheItem::Pending(Arc::new(value_creator)),
      );
    }

    let inner = self.0.clone();
    let name = name.to_string();
    spawn(async move {
      let _ = inner.load_package_info(&name).await;
      inner.prefetch_in_flight.fetch_sub(1, Ordering::Relaxed);
    });
  }

  fn mark_force_reload(&self) -> bool {
    self.0.mark_force_reload()
  }
}

// todo(#27198): make this private and only use RegistryInfoProvider in the rest of
// the code
pub fn get_package_url(npmrc: &ResolvedNpmRc, name: &str) -> Url {
  let registry_url = npmrc.get_registry_url(name);
  // The '/' character in scoped package names "@scope/name" must be
  // encoded for older third party registries. Newer registries and
  // npm itself support both ways
  //   - encoded: https://registry.npmjs.org/@rollup%2fplugin-json
  //   - non-ecoded: https://registry.npmjs.org/@rollup/plugin-json
  // To support as many third party registries as possible we'll
  // always encode the '/' character.

  // list of all characters used in npm packages:
  //  !, ', (, ), *, -, ., /, [0-9], @, [A-Za-z], _, ~
  const ASCII_SET: percent_encoding::AsciiSet =
    percent_encoding::NON_ALPHANUMERIC
      .remove(b'!')
      .remove(b'\'')
      .remove(b'(')
      .remove(b')')
      .remove(b'*')
      .remove(b'-')
      .remove(b'.')
      .remove(b'@')
      .remove(b'_')
      .remove(b'~');
  let name = percent_encoding::utf8_percent_encode(name, &ASCII_SET);
  registry_url
    // Ensure that scoped package name percent encoding is lower cased
    // to match npm.
    .join(&name.to_string().replace("%2F", "%2f"))
    .unwrap()
}
