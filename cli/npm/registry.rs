// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::npm::NpmPackageNv;
use deno_graph::semver::VersionReq;
use once_cell::sync::Lazy;
use serde::Serialize;

use crate::args::package_json::parse_dep_entry_name_and_raw_version;
use crate::args::CacheSetting;
use crate::cache::CACHE_PERM;
use crate::http_util::HttpClient;
use crate::util::fs::atomic_write_file;
use crate::util::progress_bar::ProgressBar;

use super::cache::should_sync_download;
use super::cache::NpmCache;

// npm registry docs: https://github.com/npm/registry/blob/master/docs/REGISTRY-API.md

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct NpmPackageInfo {
  pub name: String,
  pub versions: HashMap<String, NpmPackageVersionInfo>,
  #[serde(rename = "dist-tags")]
  pub dist_tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NpmDependencyEntry {
  pub kind: NpmDependencyEntryKind,
  pub bare_specifier: String,
  pub name: String,
  pub version_req: VersionReq,
  /// When the dependency is also marked as a peer dependency,
  /// use this entry to resolve the dependency when it can't
  /// be resolved as a peer dependency.
  pub peer_dep_version_req: Option<VersionReq>,
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
        .cmp(self.version_req.version_text()),
      ordering => ordering,
    }
  }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct NpmPeerDependencyMeta {
  #[serde(default)]
  optional: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum NpmPackageVersionBinEntry {
  String(String),
  Map(HashMap<String, String>),
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NpmPackageVersionInfo {
  pub version: String,
  pub dist: NpmPackageVersionDistInfo,
  pub bin: Option<NpmPackageVersionBinEntry>,
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
      (key, value): (&String, &String),
      kind: NpmDependencyEntryKind,
    ) -> Result<NpmDependencyEntry, AnyError> {
      let (name, version_req) =
        parse_dep_entry_name_and_raw_version(key, value)?;
      let version_req =
        VersionReq::parse_from_npm(version_req).with_context(|| {
          format!("error parsing version requirement for dependency: {key}@{version_req}")
        })?;
      Ok(NpmDependencyEntry {
        kind,
        bare_specifier: key.to_string(),
        name: name.to_string(),
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
  pub fn integrity(&self) -> Cow<String> {
    self
      .integrity
      .as_ref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| Cow::Owned(format!("sha1-{}", self.shasum)))
  }
}

static NPM_REGISTRY_DEFAULT_URL: Lazy<Url> = Lazy::new(|| {
  let env_var_name = "NPM_CONFIG_REGISTRY";
  if let Ok(registry_url) = std::env::var(env_var_name) {
    // ensure there is a trailing slash for the directory
    let registry_url = format!("{}/", registry_url.trim_end_matches('/'));
    match Url::parse(&registry_url) {
      Ok(url) => {
        return url;
      }
      Err(err) => {
        log::debug!("Invalid {} environment variable: {:#}", env_var_name, err,);
      }
    }
  }

  Url::parse("https://registry.npmjs.org").unwrap()
});

#[derive(Clone, Debug)]
pub struct NpmRegistryApi(Arc<dyn NpmRegistryApiInner>);

impl NpmRegistryApi {
  pub fn default_url() -> &'static Url {
    &NPM_REGISTRY_DEFAULT_URL
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

  /// Creates an npm registry API that will be uninitialized
  /// and error for every request. This is useful for tests
  /// or for initializing the LSP.
  pub fn new_uninitialized() -> Self {
    Self(Arc::new(NullNpmRegistryApiInner))
  }

  #[cfg(test)]
  pub fn new_for_test(api: TestNpmRegistryApiInner) -> NpmRegistryApi {
    Self(Arc::new(api))
  }

  pub async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, AnyError> {
    let maybe_package_info = self.0.maybe_package_info(name).await?;
    match maybe_package_info {
      Some(package_info) => Ok(package_info),
      None => bail!("npm package '{}' does not exist", name),
    }
  }

  pub async fn package_version_info(
    &self,
    nv: &NpmPackageNv,
  ) -> Result<Option<NpmPackageVersionInfo>, AnyError> {
    let package_info = self.package_info(&nv.name).await?;
    Ok(package_info.versions.get(&nv.version.to_string()).cloned())
  }

  /// Caches all the package information in memory in parallel.
  pub async fn cache_in_parallel(
    &self,
    package_names: Vec<String>,
  ) -> Result<(), AnyError> {
    let mut unresolved_tasks = Vec::with_capacity(package_names.len());

    // cache the package info up front in parallel
    if should_sync_download() {
      // for deterministic test output
      let mut ordered_names = package_names;
      ordered_names.sort();
      for name in ordered_names {
        self.package_info(&name).await?;
      }
    } else {
      for name in package_names {
        let api = self.clone();
        unresolved_tasks.push(tokio::task::spawn(async move {
          // This is ok to call because api will internally cache
          // the package information in memory.
          api.package_info(&name).await
        }));
      }
    };

    for result in futures::future::join_all(unresolved_tasks).await {
      result??; // surface the first error
    }

    Ok(())
  }

  /// Clears the internal memory cache.
  pub fn clear_memory_cache(&self) {
    self.0.clear_memory_cache();
  }

  pub fn get_cached_package_info(
    &self,
    name: &str,
  ) -> Option<Arc<NpmPackageInfo>> {
    self.0.get_cached_package_info(name)
  }

  pub fn base_url(&self) -> &Url {
    self.0.base_url()
  }
}

#[async_trait]
trait NpmRegistryApiInner: std::fmt::Debug + Sync + Send + 'static {
  async fn maybe_package_info(
    &self,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError>;

  fn clear_memory_cache(&self);

  fn get_cached_package_info(&self, name: &str) -> Option<Arc<NpmPackageInfo>>;

  fn base_url(&self) -> &Url;
}

#[async_trait]
impl NpmRegistryApiInner for RealNpmRegistryApiInner {
  fn base_url(&self) -> &Url {
    &self.base_url
  }

  async fn maybe_package_info(
    &self,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    self.maybe_package_info(name).await
  }

  fn clear_memory_cache(&self) {
    self.mem_cache.lock().clear();
  }

  fn get_cached_package_info(&self, name: &str) -> Option<Arc<NpmPackageInfo>> {
    self.mem_cache.lock().get(name).cloned().flatten()
  }
}

#[derive(Debug)]
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
          format!(
            "Error getting response at {} for package \"{}\"",
            self.get_package_url(name),
            name
          )
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
    self.base_url.join(name).unwrap()
  }

  fn get_package_file_cache_path(&self, name: &str) -> PathBuf {
    let name_folder_path = self.cache.package_name_folder(name, &self.base_url);
    name_folder_path.join("registry.json")
  }
}

#[derive(Debug)]
struct NullNpmRegistryApiInner;

#[async_trait]
impl NpmRegistryApiInner for NullNpmRegistryApiInner {
  async fn maybe_package_info(
    &self,
    _name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    Err(deno_core::anyhow::anyhow!(
      "Deno bug. Please report. Registry API was not initialized."
    ))
  }

  fn clear_memory_cache(&self) {}

  fn get_cached_package_info(
    &self,
    _name: &str,
  ) -> Option<Arc<NpmPackageInfo>> {
    None
  }

  fn base_url(&self) -> &Url {
    NpmRegistryApi::default_url()
  }
}

/// Note: This test struct is not thread safe for setup
/// purposes. Construct everything on the same thread.
#[cfg(test)]
#[derive(Clone, Default, Debug)]
pub struct TestNpmRegistryApiInner {
  package_infos: Arc<Mutex<HashMap<String, NpmPackageInfo>>>,
}

#[cfg(test)]
impl TestNpmRegistryApiInner {
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
#[async_trait]
impl NpmRegistryApiInner for TestNpmRegistryApiInner {
  async fn maybe_package_info(
    &self,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    let result = self.package_infos.lock().get(name).cloned();
    Ok(result.map(Arc::new))
  }

  fn clear_memory_cache(&self) {
    // do nothing for the test api
  }

  fn get_cached_package_info(
    &self,
    _name: &str,
  ) -> Option<Arc<NpmPackageInfo>> {
    None
  }

  fn base_url(&self) -> &Url {
    NpmRegistryApi::default_url()
  }
}

#[cfg(test)]
mod test {
  use std::collections::HashMap;

  use deno_core::serde_json;

  use crate::npm::registry::NpmPackageVersionBinEntry;
  use crate::npm::NpmPackageVersionDistInfo;

  use super::NpmPackageVersionInfo;

  #[test]
  fn deserializes_minimal_pkg_info() {
    let text = r#"{ "version": "1.0.0", "dist": { "tarball": "value", "shasum": "test" } }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info,
      NpmPackageVersionInfo {
        version: "1.0.0".to_string(),
        dist: NpmPackageVersionDistInfo {
          tarball: "value".to_string(),
          shasum: "test".to_string(),
          integrity: None,
        },
        bin: None,
        dependencies: Default::default(),
        peer_dependencies: Default::default(),
        peer_dependencies_meta: Default::default()
      }
    );
  }

  #[test]
  fn deserializes_bin_entry() {
    // string
    let text = r#"{ "version": "1.0.0", "bin": "bin-value", "dist": { "tarball": "value", "shasum": "test" } }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.bin,
      Some(NpmPackageVersionBinEntry::String("bin-value".to_string()))
    );

    // map
    let text = r#"{ "version": "1.0.0", "bin": { "a": "a-value", "b": "b-value" }, "dist": { "tarball": "value", "shasum": "test" } }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.bin,
      Some(NpmPackageVersionBinEntry::Map(HashMap::from([
        ("a".to_string(), "a-value".to_string()),
        ("b".to_string(), "b-value".to_string()),
      ])))
    );
  }
}
