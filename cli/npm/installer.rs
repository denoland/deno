// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::future::Future;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::futures::StreamExt;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_semver::npm::NpmPackageReq;

use crate::args::package_json::PackageJsonDeps;
use crate::util::sync::AtomicFlag;

use super::CliNpmRegistryApi;
use super::NpmResolution;

#[derive(Debug)]
struct PackageJsonDepsInstallerInner {
  has_installed_flag: AtomicFlag,
  npm_registry_api: CliNpmRegistryApi,
  npm_resolution: NpmResolution,
  package_deps: PackageJsonDeps,
}

impl PackageJsonDepsInstallerInner {
  pub fn reqs(&self) -> Vec<&NpmPackageReq> {
    let mut package_reqs = self
      .package_deps
      .values()
      .filter_map(|r| r.as_ref().ok())
      .collect::<Vec<_>>();
    package_reqs.sort(); // deterministic resolution
    package_reqs
  }

  pub fn reqs_with_info_futures(
    &self,
  ) -> FuturesOrdered<
    impl Future<
      Output = Result<
        (&NpmPackageReq, Arc<deno_npm::registry::NpmPackageInfo>),
        NpmRegistryPackageInfoLoadError,
      >,
    >,
  > {
    let package_reqs = self.reqs();

    FuturesOrdered::from_iter(package_reqs.into_iter().map(|req| {
      let api = self.npm_registry_api.clone();
      async move {
        let info = api.package_info(&req.name).await?;
        Ok::<_, NpmRegistryPackageInfoLoadError>((req, info))
      }
    }))
  }
}

/// Holds and controls installing dependencies from package.json.
#[derive(Debug, Clone, Default)]
pub struct PackageJsonDepsInstaller(Option<Arc<PackageJsonDepsInstallerInner>>);

impl PackageJsonDepsInstaller {
  pub fn new(
    npm_registry_api: CliNpmRegistryApi,
    npm_resolution: NpmResolution,
    deps: Option<PackageJsonDeps>,
  ) -> Self {
    Self(deps.map(|package_deps| {
      Arc::new(PackageJsonDepsInstallerInner {
        has_installed_flag: Default::default(),
        npm_registry_api,
        npm_resolution,
        package_deps,
      })
    }))
  }

  pub fn package_deps(&self) -> Option<&PackageJsonDeps> {
    self.0.as_ref().map(|inner| &inner.package_deps)
  }

  /// Installs the top level dependencies in the package.json file
  /// without going through and resolving the descendant dependencies yet.
  pub async fn ensure_top_level_install(&self) -> Result<(), AnyError> {
    let inner = match &self.0 {
      Some(inner) => inner,
      None => return Ok(()),
    };

    if !inner.has_installed_flag.raise() {
      return Ok(()); // already installed by something else
    }

    let mut reqs_with_info_futures = inner.reqs_with_info_futures();

    while let Some(result) = reqs_with_info_futures.next().await {
      let (req, info) = result?;
      let result = inner
        .npm_resolution
        .resolve_package_req_as_pending_with_info(req, &info);
      if let Err(err) = result {
        if inner.npm_registry_api.mark_force_reload() {
          log::debug!("Failed to resolve package. Retrying. Error: {err:#}");
          // re-initialize
          reqs_with_info_futures = inner.reqs_with_info_futures();
        } else {
          return Err(err.into());
        }
      }
    }

    Ok(())
  }
}
