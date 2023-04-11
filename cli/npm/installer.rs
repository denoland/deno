// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::futures::StreamExt;
use deno_npm::registry::NpmRegistryApi;

use crate::args::package_json::PackageJsonDeps;

use super::NpmRegistry;
use super::NpmResolution;

#[derive(Debug)]
struct PackageJsonDepsInstallerInner {
  has_installed: AtomicBool,
  npm_registry_api: NpmRegistry,
  npm_resolution: NpmResolution,
  package_deps: PackageJsonDeps,
}

/// Holds and controls installing dependencies from package.json.
#[derive(Debug, Clone, Default)]
pub struct PackageJsonDepsInstaller(Option<Arc<PackageJsonDepsInstallerInner>>);

impl PackageJsonDepsInstaller {
  pub fn new(
    npm_registry_api: NpmRegistry,
    npm_resolution: NpmResolution,
    deps: Option<PackageJsonDeps>,
  ) -> Self {
    Self(deps.map(|package_deps| {
      Arc::new(PackageJsonDepsInstallerInner {
        has_installed: AtomicBool::new(false),
        npm_registry_api,
        npm_resolution,
        package_deps,
      })
    }))
  }

  pub fn package_deps(&self) -> Option<&PackageJsonDeps> {
    self.0.as_ref().map(|inner| &inner.package_deps)
  }

  /// Gets if the package.json has the specified package name.
  pub fn has_package_name(&self, name: &str) -> bool {
    if let Some(package_deps) = self.package_deps() {
      // ensure this looks at the package name and not the
      // bare specifiers (do not look at the keys!)
      package_deps
        .values()
        .filter_map(|r| r.as_ref().ok())
        .any(|v| v.name == name)
    } else {
      false
    }
  }

  /// Installs the top level dependencies in the package.json file
  /// without going through and resolving the descendant dependencies yet.
  pub async fn ensure_top_level_install(&self) -> Result<(), AnyError> {
    use std::sync::atomic::Ordering;
    let inner = match &self.0 {
      Some(inner) => inner,
      None => return Ok(()),
    };

    if inner.has_installed.swap(true, Ordering::SeqCst) {
      return Ok(()); // already installed by something else
    }

    let mut package_reqs = inner
      .package_deps
      .values()
      .filter_map(|r| r.as_ref().ok())
      .collect::<Vec<_>>();
    package_reqs.sort(); // deterministic resolution

    let mut req_with_infos =
      FuturesOrdered::from_iter(package_reqs.into_iter().map(|req| {
        let api = inner.npm_registry_api.clone();
        async move {
          let info = api.package_info(&req.name).await?;
          Ok::<_, AnyError>((req, info))
        }
      }));

    while let Some(result) = req_with_infos.next().await {
      let (req, info) = result?;
      inner
        .npm_resolution
        .resolve_package_req_as_pending_with_info(req, &info)?;
    }

    Ok(())
  }
}
