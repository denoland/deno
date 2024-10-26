// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::future::Shared;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;

use crate::args::CacheSetting;
use crate::util::sync::AtomicFlag;

use super::cache::NpmCache;
use super::cache::RegistryInfoDownloader;

#[derive(Debug)]
pub struct CliNpmRegistryApi(Option<Arc<CliNpmRegistryApiInner>>);

impl CliNpmRegistryApi {
  pub fn new(
    cache: Arc<NpmCache>,
    registry_info_downloader: Arc<RegistryInfoDownloader>,
  ) -> Self {
    Self(Some(Arc::new(CliNpmRegistryApiInner {
      cache,
      force_reload_flag: Default::default(),
      mem_cache: Default::default(),
      previously_reloaded_packages: Default::default(),
      registry_info_downloader,
    })))
  }

  /// Clears the internal memory cache.
  pub fn clear_memory_cache(&self) {
    self.inner().clear_memory_cache();
  }

  fn inner(&self) -> &Arc<CliNpmRegistryApiInner> {
    // this panicking indicates a bug in the code where this
    // wasn't initialized
    self.0.as_ref().unwrap()
  }
}

#[async_trait(?Send)]
impl NpmRegistryApi for CliNpmRegistryApi {
  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    match self.inner().maybe_package_info(name).await {
      Ok(Some(info)) => Ok(info),
      Ok(None) => Err(NpmRegistryPackageInfoLoadError::PackageNotExists {
        package_name: name.to_string(),
      }),
      Err(err) => {
        Err(NpmRegistryPackageInfoLoadError::LoadError(Arc::new(err)))
      }
    }
  }

  fn mark_force_reload(&self) -> bool {
    self.inner().mark_force_reload()
  }
}

type CacheItemPendingResult =
  Result<Option<Arc<NpmPackageInfo>>, Arc<AnyError>>;

#[derive(Debug)]
enum CacheItem {
  Pending(Shared<BoxFuture<'static, CacheItemPendingResult>>),
  Resolved(Option<Arc<NpmPackageInfo>>),
}

#[derive(Debug)]
struct CliNpmRegistryApiInner {
  cache: Arc<NpmCache>,
  force_reload_flag: AtomicFlag,
  mem_cache: Mutex<HashMap<String, CacheItem>>,
  previously_reloaded_packages: Mutex<HashSet<String>>,
  registry_info_downloader: Arc<RegistryInfoDownloader>,
}

impl CliNpmRegistryApiInner {
  pub async fn maybe_package_info(
    self: &Arc<Self>,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    let (created, future) = {
      let mut mem_cache = self.mem_cache.lock();
      match mem_cache.get(name) {
        Some(CacheItem::Resolved(maybe_info)) => {
          return Ok(maybe_info.clone());
        }
        Some(CacheItem::Pending(future)) => (false, future.clone()),
        None => {
          let future = {
            let api = self.clone();
            let name = name.to_string();
            async move {
              if (api.cache.cache_setting().should_use_for_npm_package(&name) && !api.force_reload_flag.is_raised())
                // if this has been previously reloaded, then try loading from the
                // file system cache
                || !api.previously_reloaded_packages.lock().insert(name.to_string())
              {
                // attempt to load from the file cache
                if let Some(info) = api.load_file_cached_package_info(&name).await {
                  let result = Some(Arc::new(info));
                  return Ok(result);
                }
              }
              api.registry_info_downloader
                .load_package_info(&name)
                .await
                .map_err(Arc::new)
            }
            .boxed()
            .shared()
          };
          mem_cache
            .insert(name.to_string(), CacheItem::Pending(future.clone()));
          (true, future)
        }
      }
    };

    if created {
      match future.await {
        Ok(maybe_info) => {
          // replace the cache item to say it's resolved now
          self
            .mem_cache
            .lock()
            .insert(name.to_string(), CacheItem::Resolved(maybe_info.clone()));
          Ok(maybe_info)
        }
        Err(err) => {
          // purge the item from the cache so it loads next time
          self.mem_cache.lock().remove(name);
          Err(anyhow!("{:#}", err))
        }
      }
    } else {
      Ok(future.await.map_err(|err| anyhow!("{:#}", err))?)
    }
  }

  fn mark_force_reload(&self) -> bool {
    // never force reload the registry information if reloading
    // is disabled or if we're already reloading
    if matches!(
      self.cache.cache_setting(),
      CacheSetting::Only | CacheSetting::ReloadAll
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

  async fn load_file_cached_package_info(
    &self,
    name: &str,
  ) -> Option<NpmPackageInfo> {
    let result = deno_core::unsync::spawn_blocking({
      let cache = self.cache.clone();
      let name = name.to_string();
      move || cache.load_package_info(&name)
    })
    .await
    .unwrap();
    match result {
      Ok(value) => value,
      Err(err) => {
        if cfg!(debug_assertions) {
          panic!("error loading cached npm package info for {name}: {err:#}");
        } else {
          None
        }
      }
    }
  }

  fn clear_memory_cache(&self) {
    self.mem_cache.lock().clear();
  }
}
