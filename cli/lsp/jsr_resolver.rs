// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::jsr_url;
use dashmap::DashMap;
use deno_cache_dir::HttpCache;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use deno_graph::packages::JsrPackageInfo;
use deno_graph::packages::JsrPackageVersionInfo;
use deno_lockfile::Lockfile;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use std::borrow::Cow;
use std::sync::Arc;

#[derive(Debug)]
pub struct JsrResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  /// The `module_graph` field of the version infos should be forcibly absent.
  /// It can be large and we don't want to store it.
  info_by_nv: DashMap<PackageNv, Option<JsrPackageVersionInfo>>,
  info_by_name: DashMap<String, Option<JsrPackageInfo>>,
  cache: Arc<dyn HttpCache>,
}

impl JsrResolver {
  pub fn from_cache_and_lockfile(
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

  pub fn jsr_to_registry_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let req_ref = JsrPackageReqReference::from_str(specifier.as_str()).ok()?;
    let req = req_ref.req().clone();
    let maybe_nv = self.nv_by_req.entry(req.clone()).or_insert_with(|| {
      let name = req.name.clone();
      let maybe_package_info = self
        .info_by_name
        .entry(name.clone())
        .or_insert_with(|| read_cached_package_info(&name, &self.cache));
      let package_info = maybe_package_info.as_ref()?;
      // Find the first matching version of the package which is cached.
      let version = package_info
        .versions
        .keys()
        .find(|v| {
          if req.version_req.tag().is_some() || !req.version_req.matches(v) {
            return false;
          }
          let nv = PackageNv {
            name: name.clone(),
            version: (*v).clone(),
          };
          self
            .info_by_nv
            .entry(nv.clone())
            .or_insert_with(|| {
              read_cached_package_version_info(&nv, &self.cache)
            })
            .is_some()
        })
        .cloned()?;
      Some(PackageNv { name, version })
    });
    let nv = maybe_nv.as_ref()?;
    let maybe_info = self
      .info_by_nv
      .entry(nv.clone())
      .or_insert_with(|| read_cached_package_version_info(nv, &self.cache));
    let info = maybe_info.as_ref()?;
    let path = info.export(&normalize_export_name(req_ref.sub_path()))?;
    jsr_url()
      .join(&format!("{}/{}/{}", &nv.name, &nv.version, &path))
      .ok()
  }
}

fn read_cached_package_info(
  name: &str,
  cache: &Arc<dyn HttpCache>,
) -> Option<JsrPackageInfo> {
  let meta_url = jsr_url().join(&format!("{}/meta.json", name)).ok()?;
  let meta_cache_item_key = cache.cache_item_key(&meta_url).ok()?;
  let meta_bytes = cache.read_file_bytes(&meta_cache_item_key).ok()??;
  serde_json::from_slice::<JsrPackageInfo>(&meta_bytes).ok()
}

fn read_cached_package_version_info(
  nv: &PackageNv,
  cache: &Arc<dyn HttpCache>,
) -> Option<JsrPackageVersionInfo> {
  let meta_url = jsr_url()
    .join(&format!("{}/{}_meta.json", &nv.name, &nv.version))
    .ok()?;
  let meta_cache_item_key = cache.cache_item_key(&meta_url).ok()?;
  let meta_bytes = cache.read_file_bytes(&meta_cache_item_key).ok()??;
  // This is a roundabout way of deserializing `JsrPackageVersionInfo`,
  // because we only want the `exports` field and `module_graph` is large.
  let mut info =
    serde_json::from_slice::<serde_json::Value>(&meta_bytes).ok()?;
  Some(JsrPackageVersionInfo {
    exports: info.as_object_mut()?.remove("exports")?,
    module_graph: None,
  })
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
