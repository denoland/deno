// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_runtime::colors;
use serde::Serialize;

use crate::args::CacheSetting;
use crate::cache::CACHE_PERM;
use crate::http_util::HttpClient;
use crate::util::fs::atomic_write_file;
use crate::util::progress_bar::ProgressBar;

use super::cache::NpmCache;
use super::resolution::NpmVersionMatcher;
use super::semver::NpmVersion;
use super::semver::NpmVersionReq;

// npm registry docs: https://github.com/npm/registry/blob/master/docs/REGISTRY-API.md

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct NpmPackageInfo {
  pub name: String,
  pub versions: HashMap<String, NpmPackageVersionInfo>,
  #[serde(rename = "dist-tags")]
  pub dist_tags: HashMap<String, String>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum NpmDependencyEntryKind {
  Dep,
  Peer,
  OptionalPeer,
}

impl NpmDependencyEntryKind {
  pub fn is_optional(&self) -> bool {
    matches!(self, NpmDependencyEntryKind::OptionalPeer)
  }
}

#[derive(Debug, Eq, PartialEq)]
pub struct NpmDependencyEntry {
  pub kind: NpmDependencyEntryKind,
  pub bare_specifier: String,
  pub name: String,
  pub version_req: NpmVersionReq,
  /// When the dependency is also marked as a peer dependency,
  /// use this entry to resolve the dependency when it can't
  /// be resolved as a peer dependency.
  pub peer_dep_version_req: Option<NpmVersionReq>,
}

impl PartialOrd for NpmDependencyEntry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for NpmDependencyEntry {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    // sort the dependencies alphabetically by name then by version descending
    match self.name.cmp(&other.name) {
      // sort by newest to oldest
      Ordering::Equal => other
        .version_req
        .version_text()
        .cmp(&self.version_req.version_text()),
      ordering => ordering,
    }
  }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct NpmPeerDependencyMeta {
  #[serde(default)]
  optional: bool,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NpmPackageVersionInfo {
  pub version: String,
  pub dist: NpmPackageVersionDistInfo,
  // Bare specifier to version (ex. `"typescript": "^3.0.1") or possibly
  // package and version (ex. `"typescript-3.0.1": "npm:typescript@3.0.1"`).
  #[serde(default)]
  pub dependencies: HashMap<String, String>,
  #[serde(default)]
  pub peer_dependencies: HashMap<String, String>,
  #[serde(default)]
  pub peer_dependencies_meta: HashMap<String, NpmPeerDependencyMeta>,
}

impl NpmPackageVersionInfo {
  pub fn dependencies_as_entries(
    &self,
  ) -> Result<Vec<NpmDependencyEntry>, AnyError> {
    fn parse_dep_entry(
      entry: (&String, &String),
      kind: NpmDependencyEntryKind,
    ) -> Result<NpmDependencyEntry, AnyError> {
      let bare_specifier = entry.0.clone();
      let (name, version_req) =
        if let Some(package_and_version) = entry.1.strip_prefix("npm:") {
          if let Some((name, version)) = package_and_version.rsplit_once('@') {
            (name.to_string(), version.to_string())
          } else {
            bail!("could not find @ symbol in npm url '{}'", entry.1);
          }
        } else {
          (entry.0.clone(), entry.1.clone())
        };
      let version_req =
        NpmVersionReq::parse(&version_req).with_context(|| {
          format!(
            "error parsing version requirement for dependency: {}@{}",
            bare_specifier, version_req
          )
        })?;
      Ok(NpmDependencyEntry {
        kind,
        bare_specifier,
        name,
        version_req,
        peer_dep_version_req: None,
      })
    }

    let mut result = HashMap::with_capacity(
      self.dependencies.len() + self.peer_dependencies.len(),
    );
    for entry in &self.peer_dependencies {
      let is_optional = self
        .peer_dependencies_meta
        .get(entry.0)
        .map(|d| d.optional)
        .unwrap_or(false);
      let kind = match is_optional {
        true => NpmDependencyEntryKind::OptionalPeer,
        false => NpmDependencyEntryKind::Peer,
      };
      let entry = parse_dep_entry(entry, kind)?;
      result.insert(entry.bare_specifier.clone(), entry);
    }
    for entry in &self.dependencies {
      let entry = parse_dep_entry(entry, NpmDependencyEntryKind::Dep)?;
      // people may define a dependency as a peer dependency as well,
      // so in those cases, attempt to resolve as a peer dependency,
      // but then use this dependency version requirement otherwise
      if let Some(peer_dep_entry) = result.get_mut(&entry.bare_specifier) {
        peer_dep_entry.peer_dep_version_req = Some(entry.version_req);
      } else {
        result.insert(entry.bare_specifier.clone(), entry);
      }
    }
    Ok(result.into_values().collect())
  }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmPackageVersionDistInfo {
  /// URL to the tarball.
  pub tarball: String,
  shasum: String,
  integrity: Option<String>,
}

impl NpmPackageVersionDistInfo {
  #[cfg(test)]
  pub fn new(
    tarball: String,
    shasum: String,
    integrity: Option<String>,
  ) -> Self {
    Self {
      tarball,
      shasum,
      integrity,
    }
  }

  pub fn integrity(&self) -> Cow<String> {
    self
      .integrity
      .as_ref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| Cow::Owned(format!("sha1-{}", self.shasum)))
  }
}

pub trait NpmRegistryApi: Clone + Sync + Send + 'static {
  fn maybe_package_info(
    &self,
    name: &str,
  ) -> BoxFuture<'static, Result<Option<Arc<NpmPackageInfo>>, AnyError>>;

  fn package_info(
    &self,
    name: &str,
  ) -> BoxFuture<'static, Result<Arc<NpmPackageInfo>, AnyError>> {
    let api = self.clone();
    let name = name.to_string();
    async move {
      let maybe_package_info = api.maybe_package_info(&name).await?;
      match maybe_package_info {
        Some(package_info) => Ok(package_info),
        None => bail!("npm package '{}' does not exist", name),
      }
    }
    .boxed()
  }

  fn package_version_info(
    &self,
    name: &str,
    version: &NpmVersion,
  ) -> BoxFuture<'static, Result<Option<NpmPackageVersionInfo>, AnyError>> {
    let api = self.clone();
    let name = name.to_string();
    let version = version.to_string();
    async move {
      let package_info = api.package_info(&name).await?;
      Ok(package_info.versions.get(&version).cloned())
    }
    .boxed()
  }

  /// Clears the internal memory cache.
  fn clear_memory_cache(&self);
}

#[derive(Clone)]
pub struct RealNpmRegistryApi(Arc<RealNpmRegistryApiInner>);

impl RealNpmRegistryApi {
  pub fn default_url() -> Url {
    // todo(dsherret): remove DENO_NPM_REGISTRY in the future (maybe May 2023)
    let env_var_names = ["NPM_CONFIG_REGISTRY", "DENO_NPM_REGISTRY"];
    for env_var_name in env_var_names {
      if let Ok(registry_url) = std::env::var(env_var_name) {
        // ensure there is a trailing slash for the directory
        let registry_url = format!("{}/", registry_url.trim_end_matches('/'));
        match Url::parse(&registry_url) {
          Ok(url) => {
            if env_var_name == "DENO_NPM_REGISTRY" {
              log::warn!(
                "{}",
                colors::yellow(concat!(
                  "DENO_NPM_REGISTRY was intended for internal testing purposes only. ",
                  "Please update to NPM_CONFIG_REGISTRY instead.",
                )),
              );
            }
            return url;
          }
          Err(err) => {
            log::debug!(
              "Invalid {} environment variable: {:#}",
              env_var_name,
              err,
            );
          }
        }
      }
    }

    Url::parse("https://registry.npmjs.org").unwrap()
  }

  pub fn new(
    base_url: Url,
    cache: NpmCache,
    http_client: HttpClient,
    progress_bar: ProgressBar,
  ) -> Self {
    Self(Arc::new(RealNpmRegistryApiInner {
      base_url,
      cache,
      mem_cache: Default::default(),
      previously_reloaded_packages: Default::default(),
      http_client,
      progress_bar,
    }))
  }

  pub fn base_url(&self) -> &Url {
    &self.0.base_url
  }
}

impl NpmRegistryApi for RealNpmRegistryApi {
  fn maybe_package_info(
    &self,
    name: &str,
  ) -> BoxFuture<'static, Result<Option<Arc<NpmPackageInfo>>, AnyError>> {
    let api = self.clone();
    let name = name.to_string();
    async move { api.0.maybe_package_info(&name).await }.boxed()
  }

  fn clear_memory_cache(&self) {
    self.0.mem_cache.lock().clear();
  }
}

struct RealNpmRegistryApiInner {
  base_url: Url,
  cache: NpmCache,
  mem_cache: Mutex<HashMap<String, Option<Arc<NpmPackageInfo>>>>,
  previously_reloaded_packages: Mutex<HashSet<String>>,
  http_client: HttpClient,
  progress_bar: ProgressBar,
}

impl RealNpmRegistryApiInner {
  pub async fn maybe_package_info(
    &self,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    let maybe_maybe_info = self.mem_cache.lock().get(name).cloned();
    if let Some(maybe_info) = maybe_maybe_info {
      Ok(maybe_info)
    } else {
      let mut maybe_package_info = None;
      if self.cache.cache_setting().should_use_for_npm_package(name)
        // if this has been previously reloaded, then try loading from the
        // file system cache
        || !self.previously_reloaded_packages.lock().insert(name.to_string())
      {
        // attempt to load from the file cache
        maybe_package_info = self.load_file_cached_package_info(name);
      }

      if maybe_package_info.is_none() {
        maybe_package_info = self
          .load_package_info_from_registry(name)
          .await
          .with_context(|| {
          format!("Error getting response at {}", self.get_package_url(name))
        })?;
      }
      let maybe_package_info = maybe_package_info.map(Arc::new);

      // Not worth the complexity to ensure multiple in-flight requests
      // for the same package only request once because with how this is
      // used that should never happen.
      let mut mem_cache = self.mem_cache.lock();
      Ok(match mem_cache.get(name) {
        // another thread raced here, so use its result instead
        Some(info) => info.clone(),
        None => {
          mem_cache.insert(name.to_string(), maybe_package_info.clone());
          maybe_package_info
        }
      })
    }
  }

  fn load_file_cached_package_info(
    &self,
    name: &str,
  ) -> Option<NpmPackageInfo> {
    match self.load_file_cached_package_info_result(name) {
      Ok(value) => value,
      Err(err) => {
        if cfg!(debug_assertions) {
          panic!(
            "error loading cached npm package info for {}: {:#}",
            name, err
          );
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
        panic!(
          "error saving cached npm package info for {}: {:#}",
          name, err
        );
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
    std::fs::create_dir_all(file_cache_path.parent().unwrap())?;
    atomic_write_file(&file_cache_path, file_text, CACHE_PERM)?;
    Ok(())
  }

  async fn load_package_info_from_registry(
    &self,
    name: &str,
  ) -> Result<Option<NpmPackageInfo>, AnyError> {
    if *self.cache.cache_setting() == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "An npm specifier not found in cache: \"{}\", --cached-only is specified.",
          name
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
    self.base_url.join(name).unwrap()
  }

  fn get_package_file_cache_path(&self, name: &str) -> PathBuf {
    let name_folder_path = self.cache.package_name_folder(name, &self.base_url);
    name_folder_path.join("registry.json")
  }
}

/// Note: This test struct is not thread safe for setup
/// purposes. Construct everything on the same thread.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct TestNpmRegistryApi {
  package_infos: Arc<Mutex<HashMap<String, NpmPackageInfo>>>,
}

#[cfg(test)]
impl TestNpmRegistryApi {
  pub fn add_package_info(&self, name: &str, info: NpmPackageInfo) {
    let previous = self.package_infos.lock().insert(name.to_string(), info);
    assert!(previous.is_none());
  }

  pub fn ensure_package(&self, name: &str) {
    if !self.package_infos.lock().contains_key(name) {
      self.add_package_info(
        name,
        NpmPackageInfo {
          name: name.to_string(),
          ..Default::default()
        },
      );
    }
  }

  pub fn ensure_package_version(&self, name: &str, version: &str) {
    self.ensure_package(name);
    let mut infos = self.package_infos.lock();
    let info = infos.get_mut(name).unwrap();
    if !info.versions.contains_key(version) {
      info.versions.insert(
        version.to_string(),
        NpmPackageVersionInfo {
          version: version.to_string(),
          ..Default::default()
        },
      );
    }
  }

  pub fn add_dependency(
    &self,
    package_from: (&str, &str),
    package_to: (&str, &str),
  ) {
    let mut infos = self.package_infos.lock();
    let info = infos.get_mut(package_from.0).unwrap();
    let version = info.versions.get_mut(package_from.1).unwrap();
    version
      .dependencies
      .insert(package_to.0.to_string(), package_to.1.to_string());
  }

  pub fn add_dist_tag(&self, package_name: &str, tag: &str, version: &str) {
    let mut infos = self.package_infos.lock();
    let info = infos.get_mut(package_name).unwrap();
    info.dist_tags.insert(tag.to_string(), version.to_string());
  }

  pub fn add_peer_dependency(
    &self,
    package_from: (&str, &str),
    package_to: (&str, &str),
  ) {
    let mut infos = self.package_infos.lock();
    let info = infos.get_mut(package_from.0).unwrap();
    let version = info.versions.get_mut(package_from.1).unwrap();
    version
      .peer_dependencies
      .insert(package_to.0.to_string(), package_to.1.to_string());
  }

  pub fn add_optional_peer_dependency(
    &self,
    package_from: (&str, &str),
    package_to: (&str, &str),
  ) {
    let mut infos = self.package_infos.lock();
    let info = infos.get_mut(package_from.0).unwrap();
    let version = info.versions.get_mut(package_from.1).unwrap();
    version
      .peer_dependencies
      .insert(package_to.0.to_string(), package_to.1.to_string());
    version.peer_dependencies_meta.insert(
      package_to.0.to_string(),
      NpmPeerDependencyMeta { optional: true },
    );
  }
}

#[cfg(test)]
impl NpmRegistryApi for TestNpmRegistryApi {
  fn maybe_package_info(
    &self,
    name: &str,
  ) -> BoxFuture<'static, Result<Option<Arc<NpmPackageInfo>>, AnyError>> {
    let result = self.package_infos.lock().get(name).cloned();
    Box::pin(deno_core::futures::future::ready(Ok(result.map(Arc::new))))
  }

  fn clear_memory_cache(&self) {
    // do nothing for the test api
  }
}
