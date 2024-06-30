// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_config::package_json::PackageJsonDeps;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_node::load_pkg_json;
use deno_runtime::deno_node::PackageJson;
use deno_semver::package::PackageReq;

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
) -> Result<Option<Arc<PackageJson>>, AnyError> {
  const PACKAGE_JSON_NAME: &str = "package.json";

  // note: ancestors() includes the `start` path
  for ancestor in start.ancestors() {
    let path = ancestor.join(PACKAGE_JSON_NAME);

    let package_json = match load_pkg_json(&RealFs, &path) {
      Ok(Some(package_json)) => package_json,
      Ok(None) => {
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

    log::debug!("package.json file found at '{}'", path.display());
    return Ok(Some(package_json));
  }

  log::debug!("No package.json file found");
  Ok(None)
}
