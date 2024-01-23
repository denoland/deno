// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::future::Future;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::futures::StreamExt;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_semver::package::PackageReq;

use crate::args::PackageJsonDepsProvider;
use crate::util::sync::AtomicFlag;

use super::CliNpmRegistryApi;
use super::NpmResolution;

#[derive(Debug)]
struct PackageJsonDepsInstallerInner {
  deps_provider: Arc<PackageJsonDepsProvider>,
  has_installed_flag: AtomicFlag,
  npm_registry_api: Arc<CliNpmRegistryApi>,
  npm_resolution: Arc<NpmResolution>,
}

impl PackageJsonDepsInstallerInner {
  pub fn reqs_with_info_futures<'a>(
    &self,
    reqs: &'a [&'a PackageReq],
  ) -> FuturesOrdered<
    impl Future<
      Output = Result<
        (&'a PackageReq, Arc<deno_npm::registry::NpmPackageInfo>),
        NpmRegistryPackageInfoLoadError,
      >,
    >,
  > {
    FuturesOrdered::from_iter(reqs.iter().map(|req| {
      let api = self.npm_registry_api.clone();
      async move {
        let info = api.package_info(&req.name).await?;
        Ok::<_, NpmRegistryPackageInfoLoadError>((*req, info))
      }
    }))
  }
}

/// Holds and controls installing dependencies from package.json.
#[derive(Debug, Default)]
pub struct PackageJsonDepsInstaller(Option<PackageJsonDepsInstallerInner>);

impl PackageJsonDepsInstaller {
  pub fn new(
    deps_provider: Arc<PackageJsonDepsProvider>,
    npm_registry_api: Arc<CliNpmRegistryApi>,
    npm_resolution: Arc<NpmResolution>,
  ) -> Self {
    Self(Some(PackageJsonDepsInstallerInner {
      deps_provider,
      has_installed_flag: Default::default(),
      npm_registry_api,
      npm_resolution,
    }))
  }

  /// Creates an installer that never installs local packages during
  /// resolution. A top level install will be a no-op.
  pub fn no_op() -> Self {
    Self(None)
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

    let package_reqs = inner.deps_provider.reqs().unwrap_or_default();

    // check if something needs resolving before bothering to load all
    // the package information (which is slow)
    if package_reqs.iter().all(|req| {
      inner
        .npm_resolution
        .resolve_pkg_id_from_pkg_req(req)
        .is_ok()
    }) {
      log::debug!(
        "All package.json deps resolvable. Skipping top level install."
      );
      return Ok(()); // everything is already resolvable
    }

    let mut reqs_with_info_futures =
      inner.reqs_with_info_futures(&package_reqs);

    while let Some(result) = reqs_with_info_futures.next().await {
      let (req, info) = result?;
      let result = inner
        .npm_resolution
        .resolve_pkg_req_as_pending_with_info(req, &info);
      if let Err(err) = result {
        if inner.npm_registry_api.mark_force_reload() {
          log::debug!("Failed to resolve package. Retrying. Error: {err:#}");
          // re-initialize
          reqs_with_info_futures = inner.reqs_with_info_futures(&package_reqs);
        } else {
          return Err(err.into());
        }
      }
    }

    Ok(())
  }
}
