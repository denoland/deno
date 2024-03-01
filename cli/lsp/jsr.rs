// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::jsr_api_url;
use crate::args::jsr_url;
use crate::file_fetcher::FileFetcher;
use dashmap::DashMap;
use deno_cache_dir::HttpCache;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use deno_graph::packages::JsrPackageInfo;
use deno_graph::packages::JsrPackageVersionInfo;
use deno_lockfile::Lockfile;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::Version;
use serde::Deserialize;
use std::borrow::Cow;
use std::sync::Arc;

use super::cache::LSP_DISALLOW_GLOBAL_TO_LOCAL_COPY;
use super::search::PackageSearchApi;

/// Keep in sync with `JsrFetchResolver`!
#[derive(Debug)]
pub struct JsrCacheResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  /// The `module_graph` field of the version infos should be forcibly absent.
  /// It can be large and we don't want to store it.
  info_by_nv: DashMap<PackageNv, Option<Arc<JsrPackageVersionInfo>>>,
  info_by_name: DashMap<String, Option<Arc<JsrPackageInfo>>>,
  cache: Arc<dyn HttpCache>,
}

impl JsrCacheResolver {
  pub fn new(
    cache: Arc<dyn HttpCache>,
    lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    let nv_by_req = DashMap::new();
    if let Some(lockfile) = lockfile {
      for (req_url, nv_url) in &lockfile.lock().content.packages.specifiers {
        let Some(req) = req_url.strip_prefix("jsr:") else {
          continue;
        };
        let Some(nv) = nv_url.strip_prefix("jsr:") else {
          continue;
        };
        let Ok(req) = PackageReq::from_str(req) else {
          continue;
        };
        let Ok(nv) = PackageNv::from_str(nv) else {
          continue;
        };
        nv_by_req.insert(req, Some(nv));
      }
    }
    Self {
      nv_by_req,
      info_by_nv: Default::default(),
      info_by_name: Default::default(),
      cache: cache.clone(),
    }
  }

  pub fn req_to_nv(&self, req: &PackageReq) -> Option<PackageNv> {
    if let Some(nv) = self.nv_by_req.get(req) {
      return nv.value().clone();
    }
    let maybe_get_nv = || {
      let name = req.name.clone();
      let package_info = self.package_info(&name)?;
      // Find the first matching version of the package which is cached.
      let mut versions = package_info.versions.keys().collect::<Vec<_>>();
      versions.sort();
      let version = versions
        .into_iter()
        .rev()
        .find(|v| {
          if req.version_req.tag().is_some() || !req.version_req.matches(v) {
            return false;
          }
          let nv = PackageNv {
            name: name.clone(),
            version: (*v).clone(),
          };
          self.package_version_info(&nv).is_some()
        })
        .cloned()?;
      Some(PackageNv { name, version })
    };
    let nv = maybe_get_nv();
    self.nv_by_req.insert(req.clone(), nv.clone());
    nv
  }

  pub fn jsr_to_registry_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let req_ref = JsrPackageReqReference::from_str(specifier.as_str()).ok()?;
    let req = req_ref.req().clone();
    let maybe_nv = self.req_to_nv(&req);
    let nv = maybe_nv.as_ref()?;
    let info = self.package_version_info(nv)?;
    let path = info.export(&normalize_export_name(req_ref.sub_path()))?;
    jsr_url()
      .join(&format!("{}/{}/{}", &nv.name, &nv.version, &path))
      .ok()
  }

  pub fn lookup_export_for_path(
    &self,
    nv: &PackageNv,
    path: &str,
  ) -> Option<String> {
    let info = self.package_version_info(nv)?;
    let path = path.strip_prefix("./").unwrap_or(path);
    for (export, path_) in info.exports() {
      if path_.strip_prefix("./").unwrap_or(path_) == path {
        return Some(export.strip_prefix("./").unwrap_or(export).to_string());
      }
    }
    None
  }

  pub fn lookup_req_for_nv(&self, nv: &PackageNv) -> Option<PackageReq> {
    for entry in self.nv_by_req.iter() {
      let Some(nv_) = entry.value() else {
        continue;
      };
      if nv_ == nv {
        return Some(entry.key().clone());
      }
    }
    None
  }

  pub fn package_info(&self, name: &str) -> Option<Arc<JsrPackageInfo>> {
    if let Some(info) = self.info_by_name.get(name) {
      return info.value().clone();
    }
    let read_cached_package_info = || {
      let meta_url = jsr_url().join(&format!("{}/meta.json", name)).ok()?;
      let meta_bytes = read_cached_url(&meta_url, &self.cache)?;
      serde_json::from_slice::<JsrPackageInfo>(&meta_bytes).ok()
    };
    let info = read_cached_package_info().map(Arc::new);
    self.info_by_name.insert(name.to_string(), info.clone());
    info
  }

  pub fn package_version_info(
    &self,
    nv: &PackageNv,
  ) -> Option<Arc<JsrPackageVersionInfo>> {
    if let Some(info) = self.info_by_nv.get(nv) {
      return info.value().clone();
    }
    let read_cached_package_version_info = || {
      let meta_url = jsr_url()
        .join(&format!("{}/{}_meta.json", &nv.name, &nv.version))
        .ok()?;
      let meta_bytes = read_cached_url(&meta_url, &self.cache)?;
      partial_jsr_package_version_info_from_slice(&meta_bytes).ok()
    };
    let info = read_cached_package_version_info().map(Arc::new);
    self.info_by_nv.insert(nv.clone(), info.clone());
    info
  }
}

fn read_cached_url(
  url: &ModuleSpecifier,
  cache: &Arc<dyn HttpCache>,
) -> Option<Vec<u8>> {
  cache
    .read_file_bytes(
      &cache.cache_item_key(url).ok()?,
      None,
      LSP_DISALLOW_GLOBAL_TO_LOCAL_COPY,
    )
    .ok()?
}

/// This is similar to a subset of `JsrCacheResolver` which fetches rather than
/// just reads the cache. Keep in sync!
#[derive(Debug)]
pub struct JsrFetchResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  /// The `module_graph` field of the version infos should be forcibly absent.
  /// It can be large and we don't want to store it.
  info_by_nv: DashMap<PackageNv, Option<Arc<JsrPackageVersionInfo>>>,
  info_by_name: DashMap<String, Option<Arc<JsrPackageInfo>>>,
  file_fetcher: FileFetcher,
}

impl JsrFetchResolver {
  pub fn new(file_fetcher: FileFetcher) -> Self {
    Self {
      nv_by_req: Default::default(),
      info_by_nv: Default::default(),
      info_by_name: Default::default(),
      file_fetcher,
    }
  }

  pub async fn req_to_nv(&self, req: &PackageReq) -> Option<PackageNv> {
    if let Some(nv) = self.nv_by_req.get(req) {
      return nv.value().clone();
    }
    let maybe_get_nv = || async {
      let name = req.name.clone();
      let package_info = self.package_info(&name).await?;
      // Find the first matching version of the package which is cached.
      let mut versions = package_info.versions.keys().collect::<Vec<_>>();
      versions.sort();
      let version = versions
        .into_iter()
        .rev()
        .find(|v| req.version_req.tag().is_none() && req.version_req.matches(v))
        .cloned()?;
      Some(PackageNv { name, version })
    };
    let nv = maybe_get_nv().await;
    self.nv_by_req.insert(req.clone(), nv.clone());
    nv
  }

  pub async fn package_info(&self, name: &str) -> Option<Arc<JsrPackageInfo>> {
    if let Some(info) = self.info_by_name.get(name) {
      return info.value().clone();
    }
    let read_cached_package_info = || async {
      let meta_url = jsr_url().join(&format!("{}/meta.json", name)).ok()?;
      let file = self
        .file_fetcher
        .fetch(&meta_url, PermissionsContainer::allow_all())
        .await
        .ok()?;
      serde_json::from_slice::<JsrPackageInfo>(&file.source).ok()
    };
    let info = read_cached_package_info().await.map(Arc::new);
    self.info_by_name.insert(name.to_string(), info.clone());
    info
  }

  pub async fn package_version_info(
    &self,
    nv: &PackageNv,
  ) -> Option<Arc<JsrPackageVersionInfo>> {
    if let Some(info) = self.info_by_nv.get(nv) {
      return info.value().clone();
    }
    let read_cached_package_version_info = || async {
      let meta_url = jsr_url()
        .join(&format!("{}/{}_meta.json", &nv.name, &nv.version))
        .ok()?;
      let file = self
        .file_fetcher
        .fetch(&meta_url, PermissionsContainer::allow_all())
        .await
        .ok()?;
      partial_jsr_package_version_info_from_slice(&file.source).ok()
    };
    let info = read_cached_package_version_info().await.map(Arc::new);
    self.info_by_nv.insert(nv.clone(), info.clone());
    info
  }
}

#[derive(Debug, Clone)]
pub struct CliJsrSearchApi {
  file_fetcher: FileFetcher,
  /// We only store this here so the completion system has access to a resolver
  /// that always uses the global cache.
  resolver: Arc<JsrFetchResolver>,
  search_cache: Arc<DashMap<String, Arc<Vec<String>>>>,
  versions_cache: Arc<DashMap<String, Arc<Vec<Version>>>>,
  exports_cache: Arc<DashMap<PackageNv, Arc<Vec<String>>>>,
}

impl CliJsrSearchApi {
  pub fn new(file_fetcher: FileFetcher) -> Self {
    let resolver = Arc::new(JsrFetchResolver::new(file_fetcher.clone()));
    Self {
      file_fetcher,
      resolver,
      search_cache: Default::default(),
      versions_cache: Default::default(),
      exports_cache: Default::default(),
    }
  }

  pub fn get_resolver(&self) -> &Arc<JsrFetchResolver> {
    &self.resolver
  }
}

#[async_trait::async_trait]
impl PackageSearchApi for CliJsrSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError> {
    if let Some(names) = self.search_cache.get(query) {
      return Ok(names.clone());
    }
    let mut search_url = jsr_api_url().clone();
    search_url
      .path_segments_mut()
      .map_err(|_| anyhow!("Custom jsr URL cannot be a base."))?
      .pop_if_empty()
      .push("packages");
    search_url.query_pairs_mut().append_pair("query", query);
    let file = self
      .file_fetcher
      .fetch(&search_url, PermissionsContainer::allow_all())
      .await?
      .into_text_decoded()?;
    let names = Arc::new(parse_jsr_search_response(&file.source)?);
    self.search_cache.insert(query.to_string(), names.clone());
    Ok(names)
  }

  async fn versions(&self, name: &str) -> Result<Arc<Vec<Version>>, AnyError> {
    if let Some(versions) = self.versions_cache.get(name) {
      return Ok(versions.clone());
    }
    let info = self
      .resolver
      .package_info(name)
      .await
      .ok_or_else(|| anyhow!("JSR package info not found: {}", name))?;
    let mut versions = info.versions.keys().cloned().collect::<Vec<_>>();
    versions.sort();
    versions.reverse();
    let versions = Arc::new(versions);
    self
      .versions_cache
      .insert(name.to_string(), versions.clone());
    Ok(versions)
  }

  async fn exports(
    &self,
    nv: &PackageNv,
  ) -> Result<Arc<Vec<String>>, AnyError> {
    if let Some(exports) = self.exports_cache.get(nv) {
      return Ok(exports.clone());
    }
    let info = self
      .resolver
      .package_version_info(nv)
      .await
      .ok_or_else(|| anyhow!("JSR package version info not found: {}", nv))?;
    let mut exports = info
      .exports()
      .map(|(n, _)| n.to_string())
      .collect::<Vec<_>>();
    exports.sort();
    let exports = Arc::new(exports);
    self.exports_cache.insert(nv.clone(), exports.clone());
    Ok(exports)
  }
}

// TODO(nayeemrmn): This is duplicated from a private function in deno_graph
// 0.65.1. Make it public or cleanup otherwise.
fn normalize_export_name(sub_path: Option<&str>) -> Cow<str> {
  let Some(sub_path) = sub_path else {
    return Cow::Borrowed(".");
  };
  if sub_path.is_empty() || matches!(sub_path, "/" | ".") {
    Cow::Borrowed(".")
  } else {
    let sub_path = if sub_path.starts_with('/') {
      Cow::Owned(format!(".{}", sub_path))
    } else if !sub_path.starts_with("./") {
      Cow::Owned(format!("./{}", sub_path))
    } else {
      Cow::Borrowed(sub_path)
    };
    if let Some(prefix) = sub_path.strip_suffix('/') {
      Cow::Owned(prefix.to_string())
    } else {
      sub_path
    }
  }
}

/// This is a roundabout way of deserializing `JsrPackageVersionInfo`,
/// because we only want the `exports` field and `module_graph` is large.
fn partial_jsr_package_version_info_from_slice(
  slice: &[u8],
) -> serde_json::Result<JsrPackageVersionInfo> {
  let mut info = serde_json::from_slice::<serde_json::Value>(slice)?;
  Ok(JsrPackageVersionInfo {
    manifest: Default::default(), // not used by the LSP (only caching checks this in deno_graph)
    exports: info
      .as_object_mut()
      .and_then(|o| o.remove("exports"))
      .unwrap_or_default(),
    module_graph: None,
  })
}

fn parse_jsr_search_response(source: &str) -> Result<Vec<String>, AnyError> {
  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Item {
    scope: String,
    name: String,
    version_count: usize,
  }
  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Response {
    items: Vec<Item>,
  }
  let items = serde_json::from_str::<Response>(source)?.items;
  Ok(
    items
      .into_iter()
      .filter(|i| i.version_count > 0)
      .map(|i| format!("@{}/{}", i.scope, i.name))
      .collect(),
  )
}
