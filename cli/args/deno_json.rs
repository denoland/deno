// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;

use deno_core::serde_json;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;

pub fn deno_json_deps(
  config: &deno_config::ConfigFile,
) -> HashSet<JsrDepPackageReq> {
  let values = imports_values(config.json.imports.as_ref())
    .into_iter()
    .chain(scope_values(config.json.scopes.as_ref()));
  values_to_set(values)
}

fn imports_values(value: Option<&serde_json::Value>) -> Vec<&String> {
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

fn scope_values(value: Option<&serde_json::Value>) -> Vec<&String> {
  let Some(obj) = value.and_then(|v| v.as_object()) else {
    return Vec::new();
  };
  obj.values().flat_map(|v| imports_values(Some(v))).collect()
}

fn values_to_set<'a>(
  values: impl Iterator<Item = &'a String>,
) -> HashSet<JsrDepPackageReq> {
  let mut entries = HashSet::new();
  for value in values {
    if let Ok(req_ref) = JsrPackageReqReference::from_str(value) {
      entries.insert(JsrDepPackageReq::jsr(req_ref.into_inner().req));
    } else if let Ok(req_ref) = NpmPackageReqReference::from_str(value) {
      entries.insert(JsrDepPackageReq::npm(req_ref.into_inner().req));
    }
  }
  entries
}
