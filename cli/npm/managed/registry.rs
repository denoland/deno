// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::future::Shared;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;

use crate::args::CacheSetting;
use crate::cache::CACHE_PERM;
use crate::http_util::HttpClient;
use crate::util::fs::atomic_write_file;
use crate::util::progress_bar::ProgressBar;
use crate::util::sync::AtomicFlag;

use super::cache::NpmCache;

#[derive(Debug)]
pub struct CliNpmRegistryApi(Option<Arc<CliNpmRegistryApiInner>>);

impl CliNpmRegistryApi {
  pub fn new(
    base_url: Url,
    cache: Arc<NpmCache>,
    http_client: Arc<HttpClient>,
    progress_bar: ProgressBar,
  ) -> Self {
    Self(Some(Arc::new(CliNpmRegistryApiInner {
      base_url,
      cache,
      force_reload_flag: Default::default(),
      mem_cache: Default::default(),
      previously_reloaded_packages: Default::default(),
      http_client,
      progress_bar,
    })))
  }

  /// Clears the internal memory cache.
  pub fn clear_memory_cache(&self) {
    self.inner().clear_memory_cache();
  }

  pub fn get_cached_package_info(
    &self,
    name: &str,
  ) -> Option<Arc<NpmPackageInfo>> {
    self.inner().get_cached_package_info(name)
  }

  pub fn base_url(&self) -> &Url {
    &self.inner().base_url
  }

  fn inner(&self) -> &Arc<CliNpmRegistryApiInner> {
    // this panicking indicates a bug in the code where this
    // wasn't initialized
    self.0.as_ref().unwrap()
  }
}

#[async_trait]
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
    // never force reload the registry information if reloading
    // is disabled or if we're already reloading
    if matches!(
      self.inner().cache.cache_setting(),
      CacheSetting::Only | CacheSetting::ReloadAll
    ) {
      return false;
    }
    if self.inner().force_reload_flag.raise() {
      self.clear_memory_cache(); // clear the cache to force reloading
      true
    } else {
      false
    }
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
  base_url: Url,
  cache: Arc<NpmCache>,
  force_reload_flag: AtomicFlag,
  mem_cache: Mutex<HashMap<String, CacheItem>>,
  previously_reloaded_packages: Mutex<HashSet<String>>,
  http_client: Arc<HttpClient>,
  progress_bar: ProgressBar,
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
          if (self.cache.cache_setting().should_use_for_npm_package(name) && !self.force_reload())
            // if this has been previously reloaded, then try loading from the
            // file system cache
            || !self.previously_reloaded_packages.lock().insert(name.to_string())
          {
            // attempt to load from the file cache
            if let Some(info) = self.load_file_cached_package_info(name) {
              let result = Some(Arc::new(info));
              mem_cache
                .insert(name.to_string(), CacheItem::Resolved(result.clone()));
              return Ok(result);
            }
          }

          let future = {
            let api = self.clone();
            let name = name.to_string();
            async move {
              api
                .load_package_info_from_registry(&name)
                .await
                .map(|info| info.map(Arc::new))
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

  fn force_reload(&self) -> bool {
    self.force_reload_flag.is_raised()
  }

  fn load_file_cached_package_info(
    &self,
    name: &str,
  ) -> Option<NpmPackageInfo> {
    match self.load_file_cached_package_info_result(name) {
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

  fn load_file_cached_package_info_result(
    &self,
    name: &str,
  ) -> Result<Option<NpmPackageInfo>, AnyError> {
    let file_cache_path = self.get_package_file_cache_path(name);
    let file_text = match fs::read_to_string(file_cache_path) {
      Ok(file_text) => file_text,
      Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
      Err(err) => return Err(err.into()),
    };
    match serde_json::from_str(&file_text) {
      Ok(package_info) => Ok(Some(package_info)),
      Err(err) => {
        // This scenario might mean we need to load more data from the
        // npm registry than before. So, just debug log while in debug
        // rather than panic.
        log::debug!(
          "error deserializing registry.json for '{}'. Reloading. {:?}",
          name,
          err
        );
        Ok(None)
      }
    }
  }

  fn save_package_info_to_file_cache(
    &self,
    name: &str,
    package_info: &NpmPackageInfo,
  ) {
    if let Err(err) =
      self.save_package_info_to_file_cache_result(name, package_info)
    {
      if cfg!(debug_assertions) {
        panic!("error saving cached npm package info for {name}: {err:#}");
      }
    }
  }

  fn save_package_info_to_file_cache_result(
    &self,
    name: &str,
    package_info: &NpmPackageInfo,
  ) -> Result<(), AnyError> {
    let file_cache_path = self.get_package_file_cache_path(name);
    let file_text = serde_json::to_string(&package_info)?;
    atomic_write_file(&file_cache_path, file_text, CACHE_PERM)?;
    Ok(())
  }

  async fn load_package_info_from_registry(
    &self,
    name: &str,
  ) -> Result<Option<NpmPackageInfo>, AnyError> {
    self
      .load_package_info_from_registry_inner(name)
      .await
      .with_context(|| {
        format!(
          "Error getting response at {} for package \"{}\"",
          self.get_package_url(name),
          name
        )
      })
  }

  async fn load_package_info_from_registry_inner(
    &self,
    name: &str,
  ) -> Result<Option<NpmPackageInfo>, AnyError> {
    if *self.cache.cache_setting() == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "An npm specifier not found in cache: \"{name}\", --cached-only is specified."
        )
      ));
    }

    let package_url = self.get_package_url(name);
    let guard = self.progress_bar.update(package_url.as_str());

    let maybe_bytes = self
      .http_client
      .download_with_progress(package_url, &guard)
      .await?;
    match maybe_bytes {
      Some(bytes) => {
        let package_info = serde_json::from_slice(&bytes)?;
        self.save_package_info_to_file_cache(name, &package_info);
        Ok(Some(package_info))
      }
      None => Ok(None),
    }
  }

  fn get_package_url(&self, name: &str) -> Url {
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
        .remove(b'/')
        .remove(b'@')
        .remove(b'_')
        .remove(b'~');
    let name = percent_encoding::utf8_percent_encode(name, &ASCII_SET);
    self.base_url.join(&name.to_string()).unwrap()
  }

  fn get_package_file_cache_path(&self, name: &str) -> PathBuf {
    let name_folder_path = self.cache.package_name_folder(name, &self.base_url);
    name_folder_path.join("registry.json")
  }

  fn clear_memory_cache(&self) {
    self.mem_cache.lock().clear();
  }

  pub fn get_cached_package_info(
    &self,
    name: &str,
  ) -> Option<Arc<NpmPackageInfo>> {
    let mem_cache = self.mem_cache.lock();
    if let Some(CacheItem::Resolved(maybe_info)) = mem_cache.get(name) {
      maybe_info.clone()
    } else {
      None
    }
  }
}
