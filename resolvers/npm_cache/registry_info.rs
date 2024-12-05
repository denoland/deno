// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Error as AnyError;
use async_trait::async_trait;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_unsync::sync::AtomicFlag;
use deno_unsync::sync::MultiRuntimeAsyncValueCreator;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use parking_lot::Mutex;
use url::Url;

use crate::remote::maybe_auth_header_for_npm_registry;
use crate::NpmCache;
use crate::NpmCacheEnv;
use crate::NpmCacheSetting;

type LoadResult = Result<FutureResult, Arc<AnyError>>;
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
  FsCached,
  /// An item is memory cached when it fails saving to the file system cache
  /// or the package does not exist.
  MemoryCached(Result<Option<Arc<NpmPackageInfo>>, Arc<AnyError>>),
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

// todo(#27198): refactor to store this only in the http cache

/// Downloads packuments from the npm registry.
///
/// This is shared amongst all the workers.
#[derive(Debug)]
pub struct RegistryInfoProvider<TEnv: NpmCacheEnv> {
  // todo(#27198): remove this
  cache: Arc<NpmCache<TEnv>>,
  env: Arc<TEnv>,
  npmrc: Arc<ResolvedNpmRc>,
  force_reload_flag: AtomicFlag,
  memory_cache: Mutex<MemoryCache>,
  previously_loaded_packages: Mutex<HashSet<String>>,
}

impl<TEnv: NpmCacheEnv> RegistryInfoProvider<TEnv> {
  pub fn new(
    cache: Arc<NpmCache<TEnv>>,
    env: Arc<TEnv>,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self {
      cache,
      env,
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
      self.clear_memory_cache();
      true
    } else {
      false
    }
  }

  pub fn as_npm_registry_api(self: &Arc<Self>) -> NpmRegistryApiAdapter<TEnv> {
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
      Err(err) => {
        Err(NpmRegistryPackageInfoLoadError::LoadError(Arc::new(err)))
      }
    }
  }

  pub async fn maybe_package_info(
    self: &Arc<Self>,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    self.load_package_info_inner(name).await.with_context(|| {
      format!(
        "Failed loading {} for package \"{}\"",
        get_package_url(&self.npmrc, name),
        name
      )
    })
  }

  async fn load_package_info_inner(
    self: &Arc<Self>,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
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
      MemoryCacheItem::FsCached => {
        // this struct previously loaded from the registry, so we can load it from the file system cache
        self
          .load_file_cached_package_info(name)
          .await
          .map(|info| Some(Arc::new(info)))
      }
      MemoryCacheItem::MemoryCached(maybe_info) => {
        maybe_info.clone().map_err(|e| anyhow!("{}", e))
      }
      MemoryCacheItem::Pending(value_creator) => {
        match value_creator.get().await {
          Ok(FutureResult::SavedFsCache(info)) => {
            // return back the future and mark this package as having
            // been saved in the cache for next time it's requested
            self.memory_cache.lock().try_insert(
              clear_id,
              name,
              MemoryCacheItem::FsCached,
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
            let return_err = anyhow!("{:#}", err);
            self.memory_cache.lock().try_insert(
              clear_id,
              name,
              MemoryCacheItem::MemoryCached(Err(err)),
            );
            Err(return_err)
          }
        }
      }
    }
  }

  async fn load_file_cached_package_info(
    &self,
    name: &str,
  ) -> Result<NpmPackageInfo, AnyError> {
    // this scenario failing should be exceptionally rare so let's
    // deal with improving it only when anyone runs into an issue
    let maybe_package_info = deno_unsync::spawn_blocking({
      let cache = self.cache.clone();
      let name = name.to_string();
      move || cache.load_package_info(&name)
    })
    .await
    .unwrap()
    .with_context(|| {
      format!(
        "Previously saved '{}' from the npm cache, but now it fails to load.",
        name
      )
    })?;
    match maybe_package_info {
      Some(package_info) => Ok(package_info),
      None => {
        bail!("The package '{}' previously saved its registry information to the file system cache, but that file no longer exists.", name)
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
          return std::future::ready(Err(Arc::new(err))).boxed_local()
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
        if let Some(info) = downloader.cache.load_package_info(&name)? {
          let result = Arc::new(info);
          return Ok(FutureResult::SavedFsCache(result));
        }
      }

      if *downloader.cache.cache_setting() == NpmCacheSetting::Only {
        return Err(deno_core::error::custom_error(
          "NotCached",
          format!(
            "npm package not found in cache: \"{name}\", --cached-only is specified."
          )
        ));
      }

      downloader.previously_loaded_packages.lock().insert(name.to_string());

      let maybe_bytes = downloader
        .env
        .download_with_retries_on_any_tokio_runtime(
          package_url,
          maybe_auth_header,
        )
        .await?;
      match maybe_bytes {
        Some(bytes) => {
          let future_result = deno_unsync::spawn_blocking(
            move || -> Result<FutureResult, AnyError> {
              let package_info = serde_json::from_slice(&bytes)?;
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
          .await??;
          Ok(future_result)
        }
        None => Ok(FutureResult::PackageNotExists),
      }
    }
    .map(|r| r.map_err(Arc::new))
    .boxed_local()
  }
}

pub struct NpmRegistryApiAdapter<TEnv: NpmCacheEnv>(
  Arc<RegistryInfoProvider<TEnv>>,
);

#[async_trait(?Send)]
impl<TEnv: NpmCacheEnv> NpmRegistryApi for NpmRegistryApiAdapter<TEnv> {
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
