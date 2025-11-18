// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use dashmap::DashMap;
use deno_core::serde_json;
use deno_graph::JsrPackageReqNotFoundError;
use deno_graph::packages::JsrPackageInfo;
use deno_graph::packages::JsrPackageVersionInfo;
use deno_graph::packages::JsrPackageVersionResolver;
use deno_graph::packages::JsrVersionResolver;
use deno_semver::package::PackageName;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;

use crate::args::jsr_url;
use crate::file_fetcher::CliFileFetcher;

/// This is similar to a subset of `JsrCacheResolver` which fetches rather than
/// just reads the cache. Keep in sync!
#[derive(Debug)]
pub struct JsrFetchResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  /// The `module_graph` field of the version infos should be forcibly absent.
  /// It can be large and we don't want to store it.
  info_by_nv: DashMap<PackageNv, Option<Arc<JsrPackageVersionInfo>>>,
  info_by_name: DashMap<String, Option<Arc<JsrPackageInfo>>>,
  file_fetcher: Arc<CliFileFetcher>,
  jsr_version_resolver: Arc<JsrVersionResolver>,
}

impl JsrFetchResolver {
  pub fn new(
    file_fetcher: Arc<CliFileFetcher>,
    jsr_version_resolver: Arc<JsrVersionResolver>,
  ) -> Self {
    Self {
      nv_by_req: Default::default(),
      info_by_nv: Default::default(),
      info_by_name: Default::default(),
      file_fetcher,
      jsr_version_resolver,
    }
  }

  pub fn version_resolver_for_package<'a>(
    &'a self,
    name: &PackageName,
    info: &'a JsrPackageInfo,
  ) -> JsrPackageVersionResolver<'a> {
    self.jsr_version_resolver.get_for_package(name, info)
  }

  pub async fn req_to_nv(
    &self,
    req: &PackageReq,
  ) -> Result<Option<PackageNv>, JsrPackageReqNotFoundError> {
    if let Some(nv) = self.nv_by_req.get(req) {
      return Ok(nv.value().clone());
    }
    let maybe_get_nv = || async {
      let name = req.name.clone();
      let package_info = self.package_info(&name).await;
      let Some(package_info) = package_info else {
        log::debug!("no package info found for jsr:{name}");
        return Ok(None);
      };
      // Find the first matching version of the package.
      let version_resolver = self
        .jsr_version_resolver
        .get_for_package(&req.name, &package_info);
      let version =
        version_resolver.resolve_version(req, Vec::new().into_iter());
      let version = if let Ok(version) = version {
        version.version.clone()
      } else {
        let package_info = self.force_refresh_package_info(&name).await;
        let Some(package_info) = package_info else {
          log::debug!("no package info found for jsr:{name}");
          return Ok(None);
        };
        let version_resolver = self
          .jsr_version_resolver
          .get_for_package(&req.name, &package_info);
        version_resolver
          .resolve_version(req, Vec::new().into_iter())?
          .version
          .clone()
      };
      Ok(Some(PackageNv { name, version }))
    };
    let nv = maybe_get_nv().await?;

    self.nv_by_req.insert(req.clone(), nv.clone());
    Ok(nv)
  }

  pub async fn force_refresh_package_info(
    &self,
    name: &str,
  ) -> Option<Arc<JsrPackageInfo>> {
    let meta_url = self.meta_url(name)?;
    let file_fetcher = self.file_fetcher.clone();
    let file = file_fetcher
      .fetch_with_options(
        &meta_url,
        deno_resolver::file_fetcher::FetchPermissionsOptionRef::AllowAll,
        deno_resolver::file_fetcher::FetchOptions {
          maybe_cache_setting: Some(
            &deno_cache_dir::file_fetcher::CacheSetting::ReloadAll,
          ),
          ..Default::default()
        },
      )
      .await
      .ok()?;
    let info = serde_json::from_slice::<JsrPackageInfo>(&file.source).ok()?;
    let info = Arc::new(info);
    self
      .info_by_name
      .insert(name.to_string(), Some(info.clone()));
    Some(info)
  }

  fn meta_url(&self, name: &str) -> Option<deno_core::url::Url> {
    jsr_url().join(&format!("{}/meta.json", name)).ok()
  }

  // todo(dsherret): this should return error messages and only `None` when the package
  // doesn't exist
  pub async fn package_info(&self, name: &str) -> Option<Arc<JsrPackageInfo>> {
    if let Some(info) = self.info_by_name.get(name) {
      return info.value().clone();
    }
    let fetch_package_info = || async {
      let meta_url = self.meta_url(name)?;
      let file = self
        .file_fetcher
        .fetch_bypass_permissions(&meta_url)
        .await
        .ok()?;
      serde_json::from_slice::<JsrPackageInfo>(&file.source).ok()
    };
    let info = fetch_package_info().await.map(Arc::new);
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
    let fetch_package_version_info = || async {
      let meta_url = jsr_url()
        .join(&format!("{}/{}_meta.json", &nv.name, &nv.version))
        .ok()?;
      let file_fetcher = self.file_fetcher.clone();
      let file = file_fetcher
        .fetch_bypass_permissions(&meta_url)
        .await
        .ok()?;
      partial_jsr_package_version_info_from_slice(&file.source).ok()
    };
    let info = fetch_package_version_info().await.map(Arc::new);
    self.info_by_nv.insert(nv.clone(), info.clone());
    info
  }
}

/// This is a roundabout way of deserializing `JsrPackageVersionInfo`,
/// because we only want the `exports` field and `module_graph` is large.
pub fn partial_jsr_package_version_info_from_slice(
  slice: &[u8],
) -> serde_json::Result<JsrPackageVersionInfo> {
  let mut info = serde_json::from_slice::<serde_json::Value>(slice)?;
  Ok(JsrPackageVersionInfo {
    manifest: Default::default(), // not used by the LSP (only caching checks this in deno_graph)
    exports: info
      .as_object_mut()
      .and_then(|o| o.remove("exports"))
      .unwrap_or_default(),
    module_graph_1: None,
    module_graph_2: None,
    lockfile_checksum: None,
  })
}
