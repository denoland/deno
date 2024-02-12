// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::deno_registry_url;
use deno_cache_dir::HttpCache;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_graph::packages::JsrPackageVersionInfo;
use deno_lockfile::Lockfile;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct JsrResolver {
  nv_by_req: HashMap<PackageReq, PackageNv>,
  /// The `module_graph` field of the version infos should be forcibly absent.
  /// It can be large and we don't want to store it.
  info_by_nv: HashMap<PackageNv, JsrPackageVersionInfo>,
}

impl JsrResolver {
  pub fn from_cache_and_lockfile(
    cache: Arc<dyn HttpCache>,
    lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    let mut nv_by_req = HashMap::new();
    let mut info_by_nv = HashMap::new();
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
        nv_by_req.insert(req, nv);
      }
    }
    for nv in nv_by_req.values() {
      if info_by_nv.contains_key(nv) {
        continue;
      }
      let Ok(meta_url) = deno_registry_url()
        .join(&format!("{}/{}_meta.json", &nv.name, &nv.version))
      else {
        continue;
      };
      let Ok(meta_cache_item_key) = cache.cache_item_key(&meta_url) else {
        continue;
      };
      let Ok(Some(meta_bytes)) = cache.read_file_bytes(&meta_cache_item_key)
      else {
        continue;
      };
      // This is a roundabout way of deserializing `JsrPackageVersionInfo`,
      // because we only want the `exports` field and `module_graph` is large.
      let Ok(info) = serde_json::from_slice::<serde_json::Value>(&meta_bytes)
      else {
        continue;
      };
      let info = JsrPackageVersionInfo {
        exports: json!(info.as_object().and_then(|o| o.get("exports"))),
        module_graph: None,
      };
      info_by_nv.insert(nv.clone(), info);
    }
    Self {
      nv_by_req,
      info_by_nv,
    }
  }

  pub fn jsr_to_registry_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let req_ref = JsrPackageReqReference::from_str(specifier.as_str()).ok()?;
    let nv = self.nv_by_req.get(req_ref.req())?;
    let info = self.info_by_nv.get(nv)?;
    let path = info.export(&normalize_export_name(req_ref.sub_path()))?;
    deno_registry_url()
      .join(&format!("{}/{}/{}", &nv.name, &nv.version, &path))
      .ok()
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
