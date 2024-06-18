// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_npm::registry::parse_dep_entry_name_and_raw_version;
use deno_runtime::deno_node::PackageJson;
use deno_semver::npm::NpmVersionReqParseError;
use deno_semver::package::PackageReq;
use deno_semver::VersionReq;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum PackageJsonDepValueParseError {
  #[error(transparent)]
  VersionReq(#[from] NpmVersionReqParseError),
  #[error("Not implemented scheme '{scheme}'")]
  Unsupported { scheme: String },
}

pub type PackageJsonDeps =
  IndexMap<String, Result<PackageReq, PackageJsonDepValueParseError>>;

#[derive(Debug, Default)]
pub struct PackageJsonDepsProvider(Option<PackageJsonDeps>);

impl PackageJsonDepsProvider {
  pub fn new(deps: Option<PackageJsonDeps>) -> Self {
    Self(deps)
  }

  pub fn deps(&self) -> Option<&PackageJsonDeps> {
    self.0.as_ref()
  }

  pub fn reqs(&self) -> Option<Vec<&PackageReq>> {
    match &self.0 {
      Some(deps) => {
        let mut package_reqs = deps
          .values()
          .filter_map(|r| r.as_ref().ok())
          .collect::<Vec<_>>();
        package_reqs.sort(); // deterministic resolution
        Some(package_reqs)
      }
      None => None,
    }
  }
}

/// Attempts to discover the package.json file, maybe stopping when it
/// reaches the specified `maybe_stop_at` directory.
pub fn discover_from(
  start: &Path,
  maybe_stop_at: Option<PathBuf>,
) -> Result<Option<PackageJson>, AnyError> {
  const PACKAGE_JSON_NAME: &str = "package.json";

  // note: ancestors() includes the `start` path
  for ancestor in start.ancestors() {
    let path = ancestor.join(PACKAGE_JSON_NAME);

    let source = match std::fs::read_to_string(&path) {
      Ok(source) => source,
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        if let Some(stop_at) = maybe_stop_at.as_ref() {
          if ancestor == stop_at {
            break;
          }
        }
        continue;
      }
      Err(err) => bail!(
        "Error loading package.json at {}. {:#}",
        path.display(),
        err
      ),
    };

    let package_json = PackageJson::load_from_string(path.clone(), source)?;
    log::debug!("package.json file found at '{}'", path.display());
    return Ok(Some(package_json));
  }

  log::debug!("No package.json file found");
  Ok(None)
}
