// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use dashmap::DashMap;
use deno_core::serde_json;
use deno_graph::packages::JsrPackageInfo;
use deno_graph::packages::JsrPackageInfoVersion;
use deno_graph::packages::JsrPackageVersionInfo;
use deno_semver::Version;
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
}

fn select_version<'a, I>(versions: I, req: &PackageReq) -> Option<Version>
where
  I: IntoIterator<Item = (&'a Version, &'a JsrPackageInfoVersion)>,
{
  let mut versions = versions.into_iter().collect::<Vec<_>>();
  versions.sort_by_key(|(v, _)| *v);
  versions
    .into_iter()
    .rev()
    .find(|(v, i)| {
      !i.yanked && req.version_req.tag().is_none() && req.version_req.matches(v)
    })
    .map(|(v, _)| v.clone())
}

impl JsrFetchResolver {
  pub fn new(file_fetcher: Arc<CliFileFetcher>) -> Self {
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
      let package_info = self.package_info(&name).await;
      if package_info.is_none() {
        log::debug!("no package info found for jsr:{name}");
        return None;
      }
      let package_info = package_info?;
      // Find the first matching version of the package.
      let version = select_version(&package_info.versions, req);
      let version = if let Some(version) = version {
        version
      } else {
        let info = self.force_refresh_package_info(&name).await;
        let Some(info) = info else {
          log::debug!("no package info found for jsr:{name}");
          return None;
        };
        let version = select_version(&info.versions, req);
        let Some(version) = version else {
          log::debug!("no matching version found for jsr:{req}");
          return None;
        };
        version
      };
      Some(PackageNv { name, version })
    };
    let nv = maybe_get_nv().await;

    self.nv_by_req.insert(req.clone(), nv.clone());
    nv
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
