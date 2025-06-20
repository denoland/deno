// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use capacity_builder::StringBuilder;
use deno_error::JsErrorBox;
use deno_lockfile::NpmPackageDependencyLockfileInfo;
use deno_lockfile::NpmPackageLockfileInfo;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_npm::resolution::AddPkgReqsOptions;
use deno_npm::resolution::DefaultTarballUrlProvider;
use deno_npm::resolution::NpmResolutionError;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmResolutionPackage;
use deno_npm_cache::NpmCacheHttpClient;
use deno_npm_cache::NpmCacheSys;
use deno_npm_cache::RegistryInfoProvider;
use deno_resolver::display::DisplayTreeNode;
use deno_resolver::lockfile::LockfileLock;
use deno_resolver::lockfile::LockfileSys;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::workspace::WorkspaceNpmLinkPackages;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::VersionReq;
use deno_terminal::colors;
use deno_unsync::sync::TaskQueue;

pub struct AddPkgReqsResult {
  /// Results from adding the individual packages.
  ///
  /// The indexes of the results correspond to the indexes of the provided
  /// package requirements.
  pub results: Vec<Result<PackageNv, NpmResolutionError>>,
  /// The final result of resolving and caching all the package requirements.
  pub dependencies_result: Result<(), JsErrorBox>,
}

#[sys_traits::auto_impl]
pub trait NpmResolutionInstallerSys: LockfileSys + NpmCacheSys {}

/// Updates the npm resolution with the provided package requirements.
#[derive(Debug)]
pub struct NpmResolutionInstaller<
  TNpmCacheHttpClient: NpmCacheHttpClient,
  TSys: NpmResolutionInstallerSys,
> {
  registry_info_provider: Arc<RegistryInfoProvider<TNpmCacheHttpClient, TSys>>,
  resolution: Arc<NpmResolutionCell>,
  maybe_lockfile: Option<Arc<LockfileLock<TSys>>>,
  link_packages: Arc<WorkspaceNpmLinkPackages>,
  update_queue: TaskQueue,
}

impl<
    TNpmCacheHttpClient: NpmCacheHttpClient,
    TSys: NpmResolutionInstallerSys,
  > NpmResolutionInstaller<TNpmCacheHttpClient, TSys>
{
  pub fn new(
    registry_info_provider: Arc<
      RegistryInfoProvider<TNpmCacheHttpClient, TSys>,
    >,
    resolution: Arc<NpmResolutionCell>,
    maybe_lockfile: Option<Arc<LockfileLock<TSys>>>,
    link_packages: Arc<WorkspaceNpmLinkPackages>,
  ) -> Self {
    Self {
      registry_info_provider,
      resolution,
      maybe_lockfile,
      link_packages,
      update_queue: Default::default(),
    }
  }

  pub async fn cache_package_info(
    &self,
    package_name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    // this will internally cache the package information
    self.registry_info_provider.package_info(package_name).await
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> AddPkgReqsResult {
    // only allow one thread in here at a time
    let _snapshot_lock = self.update_queue.acquire().await;
    let result = self.add_package_reqs_to_snapshot(package_reqs).await;

    AddPkgReqsResult {
      results: result.results,
      dependencies_result: match result.dep_graph_result {
        Ok(snapshot) => {
          self.resolution.set_snapshot(snapshot);
          Ok(())
        }
        Err(err) => Err(JsErrorBox::from_err(err)),
      },
    }
  }

  async fn add_package_reqs_to_snapshot(
    &self,
    package_reqs: &[PackageReq],
  ) -> deno_npm::resolution::AddPkgReqsResult {
    fn get_types_node_version() -> VersionReq {
      // WARNING: When bumping this version, check if anything needs to be
      // updated in the `setNodeOnlyGlobalNames` call in 99_main_compiler.js
      VersionReq::parse_from_npm("22.9.0 - 22.15.15").unwrap()
    }

    let snapshot = self.resolution.snapshot();
    if package_reqs
      .iter()
      .all(|req| snapshot.package_reqs().contains_key(req))
    {
      log::debug!("Snapshot already up to date. Skipping npm resolution.");
      return deno_npm::resolution::AddPkgReqsResult {
        results: package_reqs
          .iter()
          .map(|req| Ok(snapshot.package_reqs().get(req).unwrap().clone()))
          .collect(),
        dep_graph_result: Ok(snapshot),
        unmet_peer_diagnostics: Default::default(),
      };
    }
    log::debug!(
      /* this string is used in tests */
      "Running npm resolution."
    );
    let npm_registry_api = self.registry_info_provider.as_npm_registry_api();
    let result = snapshot
      .add_pkg_reqs(
        &npm_registry_api,
        AddPkgReqsOptions {
          package_reqs,
          types_node_version_req: Some(get_types_node_version()),
          link_packages: &self.link_packages.0,
        },
      )
      .await;
    let result = match &result.dep_graph_result {
      Err(NpmResolutionError::Resolution(err))
        if npm_registry_api.mark_force_reload() =>
      {
        log::debug!("{err:#}");
        log::debug!("npm resolution failed. Trying again...");

        // try again with forced reloading
        let snapshot = self.resolution.snapshot();
        snapshot
          .add_pkg_reqs(
            &npm_registry_api,
            AddPkgReqsOptions {
              package_reqs,
              types_node_version_req: Some(get_types_node_version()),
              link_packages: &self.link_packages.0,
            },
          )
          .await
      }
      _ => result,
    };

    self.registry_info_provider.clear_memory_cache();

    if !result.unmet_peer_diagnostics.is_empty()
      && log::log_enabled!(log::Level::Warn)
    {
      let root_node = DisplayTreeNode {
        text: format!(
          "{} The following peer dependency issues were found:",
          colors::yellow("Warning")
        ),
        children: result
          .unmet_peer_diagnostics
          .iter()
          .map(|diagnostic| {
            let mut node = DisplayTreeNode {
              text: format!(
                "peer {}: resolved to {}",
                diagnostic.dependency, diagnostic.resolved
              ),
              children: Vec::new(),
            };
            for ancestor in &diagnostic.ancestors {
              node = DisplayTreeNode {
                text: ancestor.to_string(),
                children: vec![node],
              };
            }
            node
          })
          .collect(),
      };
      let mut text = String::new();
      _ = root_node.print(&mut text);
      log::warn!("{}", text);
    }

    if let Ok(snapshot) = &result.dep_graph_result {
      self.populate_lockfile_from_snapshot(snapshot);
    }

    result
  }

  fn populate_lockfile_from_snapshot(&self, snapshot: &NpmResolutionSnapshot) {
    fn npm_package_to_lockfile_info(
      pkg: &NpmResolutionPackage,
    ) -> NpmPackageLockfileInfo {
      let dependencies = pkg
        .dependencies
        .iter()
        .filter_map(|(name, id)| {
          if pkg.optional_dependencies.contains(name) {
            None
          } else {
            Some(NpmPackageDependencyLockfileInfo {
              name: name.clone(),
              id: id.as_serialized(),
            })
          }
        })
        .collect();

      let optional_dependencies = pkg
        .optional_dependencies
        .iter()
        .filter_map(|name| {
          let id = pkg.dependencies.get(name)?;
          Some(NpmPackageDependencyLockfileInfo {
            name: name.clone(),
            id: id.as_serialized(),
          })
        })
        .collect();

      let optional_peers = pkg
        .optional_peer_dependencies
        .iter()
        .filter_map(|name| {
          let id = pkg.dependencies.get(name)?;
          Some(NpmPackageDependencyLockfileInfo {
            name: name.clone(),
            id: id.as_serialized(),
          })
        })
        .collect();
      NpmPackageLockfileInfo {
        serialized_id: pkg.id.as_serialized(),
        integrity: pkg.dist.as_ref().and_then(|dist| {
          dist.integrity().for_lockfile().map(|s| s.into_owned())
        }),
        dependencies,
        optional_dependencies,
        os: pkg.system.os.clone(),
        cpu: pkg.system.cpu.clone(),
        tarball: pkg.dist.as_ref().and_then(|dist| {
          // Omit the tarball URL if it's the standard NPM registry URL
          let tarbal_url_provider =
            deno_npm::resolution::NpmRegistryDefaultTarballUrlProvider;
          if dist.tarball == tarbal_url_provider.default_tarball_url(&pkg.id.nv)
          {
            None
          } else {
            Some(StackString::from_str(&dist.tarball))
          }
        }),
        deprecated: pkg.is_deprecated,
        bin: pkg.has_bin,
        scripts: pkg.has_scripts,
        optional_peers,
      }
    }

    let Some(lockfile) = &self.maybe_lockfile else {
      return;
    };

    let mut lockfile = lockfile.lock();
    for (package_req, nv) in snapshot.package_reqs() {
      let id = &snapshot.resolve_package_from_deno_module(nv).unwrap().id;
      lockfile.insert_package_specifier(
        JsrDepPackageReq::npm(package_req.clone()),
        {
          StringBuilder::<SmallStackString>::build(|builder| {
            builder.append(&id.nv.version);
            builder.append(&id.peer_dependencies);
          })
          .unwrap()
        },
      );
    }
    for package in snapshot.all_packages_for_every_system() {
      lockfile.insert_npm_package(npm_package_to_lockfile_info(package));
    }
  }
}
