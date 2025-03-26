// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use deno_error::JsErrorBox;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_unsync::sync::AtomicFlag;
use deno_unsync::sync::MultiRuntimeAsyncValueCreator;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use parking_lot::Mutex;
use sys_traits::FsCreateDirAll;
use sys_traits::FsHardLink;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;
use url::Url;

use crate::remote::maybe_auth_header_for_npm_registry;
use crate::NpmCache;
use crate::NpmCacheHttpClient;
use crate::NpmCacheSetting;

type LoadResult = Result<FutureResult, Arc<JsErrorBox>>;
type LoadFuture = LocalBoxFuture<'static, LoadResult>;

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
// todo(#27198): refactor to store this only in the http cache

/// Downloads packuments from the npm registry.
///
/// This is shared amongst all the workers.
#[derive(Debug)]
pub struct RegistryInfoProvider<
  THttpClient: NpmCacheHttpClient,
  TSys: FsCreateDirAll
    + FsHardLink
    + FsMetadata
    + FsOpen
    + FsReadDir
    + FsRemoveFile
    + FsRename
    + ThreadSleep
    + SystemRandom
    + Send
    + Sync
    + 'static,
> {
  // todo(#27198): remove this
  cache: Arc<NpmCache<TSys>>,
  http_client: Arc<THttpClient>,
  npmrc: Arc<ResolvedNpmRc>,
  force_reload_flag: AtomicFlag,
  memory_cache: Mutex<MemoryCache>,
  previously_loaded_packages: Mutex<HashSet<String>>,
}

impl<
    THttpClient: NpmCacheHttpClient,
    TSys: FsCreateDirAll
      + FsHardLink
      + FsMetadata
      + FsOpen
      + FsReadDir
      + FsRemoveFile
      + FsRename
      + ThreadSleep
      + SystemRandom
      + Send
      + Sync
      + 'static,
  > RegistryInfoProvider<THttpClient, TSys>
{
  pub fn new(
    cache: Arc<NpmCache<TSys>>,
    http_client: Arc<THttpClient>,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self {
      cache,
      http_client,
      npmrc,
      force_reload_flag: AtomicFlag::lowered(),
      memory_cache: Default::default(),
      previously_loaded_packages: Default::default(),
    }
  }

  /// Clears the internal memory cache.
  pub fn clear_memory_cache(&self) {
    self.memory_cache.lock().clear();
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

  pub fn as_npm_registry_api(
    self: &Arc<Self>,
  ) -> NpmRegistryApiAdapter<THttpClient, TSys> {
    NpmRegistryApiAdapter(self.clone())
  }

  pub async fn package_info(
    self: &Arc<Self>,
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

  pub async fn maybe_package_info(
    self: &Arc<Self>,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, LoadPackageInfoError> {
    self.load_package_info_inner(name).await.map_err(|err| {
      LoadPackageInfoError {
        url: get_package_url(&self.npmrc, name),
        name: name.to_string(),
        inner: err,
      }
    })
  }

  async fn load_package_info_inner(
    self: &Arc<Self>,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, LoadPackageInfoInnerError> {
    let (cache_item, clear_id) = {
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
      (cache_item, mem_cache.clear_id)
    };

    match cache_item {
      MemoryCacheItem::FsCached(info) => Ok(Some(info)),
      MemoryCacheItem::MemoryCached(maybe_info) => {
        maybe_info.clone().map_err(LoadPackageInfoInnerError)
      }
      MemoryCacheItem::Pending(value_creator) => {
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
    }
  }

  fn create_load_future(self: &Arc<Self>, name: &str) -> LoadFuture {
    let downloader = self.clone();
    let package_url = get_package_url(&self.npmrc, name);
    let registry_config = self.npmrc.get_registry_config(name);
    let maybe_auth_header =
      match maybe_auth_header_for_npm_registry(registry_config) {
        Ok(maybe_auth_header) => maybe_auth_header,
        Err(err) => {
          return std::future::ready(Err(Arc::new(JsErrorBox::from_err(err))))
            .boxed_local()
        }
      };
    let name = name.to_string();
    async move {
      if (downloader.cache.cache_setting().should_use_for_npm_package(&name) && !downloader.force_reload_flag.is_raised())
        // if this has been previously reloaded, then try loading from the
        // file system cache
        || downloader.previously_loaded_packages.lock().contains(&name)
      {
        // attempt to load from the file cache
        if let Some(info) = downloader.cache.load_package_info(&name).map_err(JsErrorBox::from_err)? {
          let result = Arc::new(info);
          return Ok(FutureResult::SavedFsCache(result));
        }
      }

      if *downloader.cache.cache_setting() == NpmCacheSetting::Only {
        return Err(JsErrorBox::new(
          "NotCached",
          format!(
            "npm package not found in cache: \"{name}\", --cached-only is specified."
          )
        ));
      }

      downloader.previously_loaded_packages.lock().insert(name.to_string());

      let maybe_bytes = downloader
        .http_client
        .download_with_retries_on_any_tokio_runtime(
          package_url,
          maybe_auth_header,
        )
        .await.map_err(JsErrorBox::from_err)?;
      match maybe_bytes {
        Some(bytes) => {
          let future_result = deno_unsync::spawn_blocking(
            move || -> Result<FutureResult, JsErrorBox> {
              let package_info = serde_json::from_slice(&bytes).map_err(JsErrorBox::from_err)?;
              match downloader.cache.save_package_info(&name, &package_info) {
                Ok(()) => {
                  Ok(FutureResult::SavedFsCache(Arc::new(package_info)))
                }
                Err(err) => {
                  log::debug!(
                    "Error saving package {} to cache: {:#}",
                    name,
                    err
                  );
                  Ok(FutureResult::ErroredFsCache(Arc::new(package_info)))
                }
              }
            },
          )
          .await
          .map_err(JsErrorBox::from_err)??;
          Ok(future_result)
        }
        None => Ok(FutureResult::PackageNotExists),
      }
    }
    .map(|r| r.map_err(Arc::new))
    .boxed_local()
  }
}

pub struct NpmRegistryApiAdapter<
  THttpClient: NpmCacheHttpClient,
  TSys: FsCreateDirAll
    + FsHardLink
    + FsMetadata
    + FsOpen
    + FsReadDir
    + FsRemoveFile
    + FsRename
    + ThreadSleep
    + SystemRandom
    + Send
    + Sync
    + 'static,
>(Arc<RegistryInfoProvider<THttpClient, TSys>>);

#[async_trait(?Send)]
impl<
    THttpClient: NpmCacheHttpClient,
    TSys: FsCreateDirAll
      + FsHardLink
      + FsMetadata
      + FsOpen
      + FsReadDir
      + FsRemoveFile
      + FsRename
      + ThreadSleep
      + SystemRandom
      + Send
      + Sync
      + 'static,
  > NpmRegistryApi for NpmRegistryApiAdapter<THttpClient, TSys>
{
  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    self.0.package_info(name).await
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
