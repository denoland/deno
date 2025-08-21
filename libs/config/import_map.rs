// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;

use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;

/// Attempts to resolve any `npm:` and `jsr:` dependencies
/// in the import map's imports and scopes.
pub fn import_map_deps(
  import_map: &serde_json::Value,
) -> HashSet<JsrDepPackageReq> {
  let values = imports_values(import_map.get("imports"))
    .into_iter()
    .chain(scope_values(import_map.get("scopes")));
  values_to_set(values)
}

pub(crate) fn imports_values(
  value: Option<&serde_json::Value>,
) -> Vec<&String> {
  let Some(obj) = value.and_then(|v| v.as_object()) else {
    return Vec::new();
  };
  let mut items = Vec::with_capacity(obj.len());
  for value in obj.values() {
    if let serde_json::Value::String(value) = value {
      items.push(value);
    }
  }
  items
}

pub(crate) fn scope_values(value: Option<&serde_json::Value>) -> Vec<&String> {
  let Some(obj) = value.and_then(|v| v.as_object()) else {
    return Vec::new();
  };
  obj.values().flat_map(|v| imports_values(Some(v))).collect()
}

pub(crate) fn values_to_set<'a>(
  values: impl Iterator<Item = &'a String>,
) -> HashSet<JsrDepPackageReq> {
  let mut entries = HashSet::new();
  for value in values {
    if let Some(dep_req) = value_to_dep_req(value) {
      entries.insert(dep_req);
    }
  }
  entries
}

pub(crate) fn value_to_dep_req(value: &str) -> Option<JsrDepPackageReq> {
  match JsrPackageReqReference::from_str(value) {
    Ok(req_ref) => Some(JsrDepPackageReq::jsr(req_ref.into_inner().req)),
    _ => match NpmPackageReqReference::from_str(value) {
      Ok(req_ref) => Some(JsrDepPackageReq::npm(req_ref.into_inner().req)),
      _ => None,
    },
  }
}
