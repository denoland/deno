// Copyright 2018-2026 the Deno authors. MIT license.

use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use import_map::ImportMap;

pub fn import_map_deps(
  import_map: &ImportMap,
) -> impl Iterator<Item = JsrDepPackageReq> {
  fn from_map(
    specifier_map: &import_map::SpecifierMap,
  ) -> impl Iterator<Item = &str> {
    specifier_map.entries().flat_map(|e| e.raw_value)
  }

  from_map(import_map.imports())
    .chain(
      import_map
        .scopes()
        .flat_map(|scope| from_map(scope.imports)),
    )
    .filter_map(value_to_dep_req)
}

/// Attempts to resolve any `npm:` and `jsr:` dependencies
/// in the import map's imports and scopes.
pub fn import_map_deps_from_value(
  import_map: &serde_json::Value,
) -> impl Iterator<Item = JsrDepPackageReq> {
  imports_values(import_map.get("imports"))
    .chain(scope_values(import_map.get("scopes")))
    .filter_map(value_to_dep_req)
}

pub(crate) fn imports_values(
  value: Option<&serde_json::Value>,
) -> impl Iterator<Item = &str> {
  value
    .and_then(|v| v.as_object())
    .map(|obj| {
      obj.values().filter_map(|v| match v {
        serde_json::Value::String(value) => Some(value.as_ref()),
        _ => None,
      })
    })
    .into_iter()
    .flatten()
}

pub(crate) fn scope_values(
  value: Option<&serde_json::Value>,
) -> impl Iterator<Item = &str> {
  value
    .and_then(|v| v.as_object())
    .map(|obj| obj.values().flat_map(|v| imports_values(Some(v))))
    .into_iter()
    .flatten()
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
