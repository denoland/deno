// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;

use deno_config::deno_json::TsConfigForEmit;
use deno_core::serde_json;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;

#[cfg(test)] // happens to only be used by the tests at the moment
pub struct DenoConfigFsAdapter<'a>(
  pub &'a dyn deno_runtime::deno_fs::FileSystem,
);

#[cfg(test)]
impl<'a> deno_config::fs::DenoConfigFs for DenoConfigFsAdapter<'a> {
  fn read_to_string_lossy(
    &self,
    path: &std::path::Path,
  ) -> Result<String, std::io::Error> {
    self
      .0
      .read_text_file_lossy_sync(path, None)
      .map_err(|err| err.into_io_error())
  }

  fn stat_sync(
    &self,
    path: &std::path::Path,
  ) -> Result<deno_config::fs::FsMetadata, std::io::Error> {
    self
      .0
      .stat_sync(path)
      .map(|stat| deno_config::fs::FsMetadata {
        is_file: stat.is_file,
        is_directory: stat.is_directory,
        is_symlink: stat.is_symlink,
      })
      .map_err(|err| err.into_io_error())
  }

  fn read_dir(
    &self,
    path: &std::path::Path,
  ) -> Result<Vec<deno_config::fs::FsDirEntry>, std::io::Error> {
    self
      .0
      .read_dir_sync(path)
      .map_err(|err| err.into_io_error())
      .map(|entries| {
        entries
          .into_iter()
          .map(|e| deno_config::fs::FsDirEntry {
            path: path.join(e.name),
            metadata: deno_config::fs::FsMetadata {
              is_file: e.is_file,
              is_directory: e.is_directory,
              is_symlink: e.is_symlink,
            },
          })
          .collect()
      })
  }
}

pub fn deno_json_deps(
  config: &deno_config::deno_json::ConfigFile,
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

pub fn check_warn_tsconfig(ts_config: &TsConfigForEmit) {
  if let Some(ignored_options) = &ts_config.maybe_ignored_options {
    log::warn!("{}", ignored_options);
  }
  let serde_json::Value::Object(obj) = &ts_config.ts_config.0 else {
    return;
  };
  if obj.get("experimentalDecorators") == Some(&serde_json::Value::Bool(true)) {
    log::warn!(
      "{} experimentalDecorators compiler option is deprecated and may be removed at any time",
      deno_runtime::colors::yellow("Warning"),
    );
  }
}
