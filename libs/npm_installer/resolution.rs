// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use capacity_builder::StringBuilder;
use deno_error::JsErrorBox;
use deno_lockfile::NpmPackageDependencyLockfileInfo;
use deno_lockfile::NpmPackageLockfileInfo;
use deno_npm::NpmResolutionPackage;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_npm::resolution::AddPkgReqsOptions;
use deno_npm::resolution::DefaultTarballUrlProvider;
use deno_npm::resolution::NpmResolutionError;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::UnmetPeerDepDiagnostic;
use deno_npm_cache::NpmCacheHttpClient;
use deno_npm_cache::NpmCacheSys;
use deno_npm_cache::RegistryInfoProvider;
use deno_resolver::display::DisplayTreeNode;
use deno_resolver::factory::NpmVersionResolverRc;
use deno_resolver::lockfile::LockfileLock;
use deno_resolver::lockfile::LockfileSys;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageKind;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_terminal::colors;
use deno_unsync::sync::AtomicFlag;
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

pub type HasJsExecutionStartedFlagRc = Arc<HasJsExecutionStartedFlag>;

/// A flag that indicates if JS execution has started, which
/// will tell the npm resolution to not do a deduplication pass
/// and instead npm resolution should only be additive.
#[derive(Debug, Default)]
pub struct HasJsExecutionStartedFlag(AtomicFlag);

impl HasJsExecutionStartedFlag {
  #[inline(always)]
  pub fn raise(&self) -> bool {
    self.0.raise()
  }

  #[inline(always)]
  pub fn is_raised(&self) -> bool {
    self.0.is_raised()
  }
}

#[sys_traits::auto_impl]
pub trait NpmResolutionInstallerSys: LockfileSys + NpmCacheSys {}

/// Updates the npm resolution with the provided package requirements.
#[derive(Debug)]
pub struct NpmResolutionInstaller<
  TNpmCacheHttpClient: NpmCacheHttpClient,
  TSys: NpmResolutionInstallerSys,
> {
  has_js_execution_started_flag: HasJsExecutionStartedFlagRc,
  npm_version_resolver: NpmVersionResolverRc,
  registry_info_provider: Arc<RegistryInfoProvider<TNpmCacheHttpClient, TSys>>,
  reporter: Option<Arc<dyn deno_npm::resolution::Reporter>>,
  resolution: Arc<NpmResolutionCell>,
  maybe_lockfile: Option<Arc<LockfileLock<TSys>>>,
  update_queue: TaskQueue,
}

impl<TNpmCacheHttpClient: NpmCacheHttpClient, TSys: NpmResolutionInstallerSys>
  NpmResolutionInstaller<TNpmCacheHttpClient, TSys>
{
  pub fn new(
    has_js_execution_started_flag: HasJsExecutionStartedFlagRc,
    npm_version_resolver: NpmVersionResolverRc,
    registry_info_provider: Arc<
      RegistryInfoProvider<TNpmCacheHttpClient, TSys>,
    >,
    reporter: Option<Arc<dyn deno_npm::resolution::Reporter>>,
    resolution: Arc<NpmResolutionCell>,
    maybe_lockfile: Option<Arc<LockfileLock<TSys>>>,
  ) -> Self {
    Self {
      has_js_execution_started_flag,
      npm_version_resolver,
      registry_info_provider,
      reporter,
      resolution,
      maybe_lockfile,
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

  /// Run a resolution install if the npm snapshot is in a pending state
  /// due to a config file change.
  pub async fn install_if_pending(&self) -> Result<(), NpmResolutionError> {
    self.add_package_reqs_inner(&[]).await.1
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> AddPkgReqsResult {
    let (results, dependencies_result) =
      self.add_package_reqs_inner(package_reqs).await;
    AddPkgReqsResult {
      results,
      dependencies_result: dependencies_result.map_err(JsErrorBox::from_err),
    }
  }

  async fn add_package_reqs_inner(
    &self,
    package_reqs: &[PackageReq],
  ) -> (
    Vec<Result<PackageNv, NpmResolutionError>>,
    Result<(), NpmResolutionError>,
  ) {
    // only allow one thread in here at a time
    let _snapshot_lock = self.update_queue.acquire().await;
    let result = self.add_package_reqs_to_snapshot(package_reqs).await;

    (
      result.results,
      result.dep_graph_result.map(|snapshot| {
        self.resolution.mark_not_pending();
        self.resolution.set_snapshot(snapshot);
      }),
    )
  }

  async fn add_package_reqs_to_snapshot(
    &self,
    package_reqs: &[PackageReq],
  ) -> deno_npm::resolution::AddPkgReqsResult {
    let snapshot = self.resolution.snapshot();
    if !self.resolution.is_pending()
      && package_reqs
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
    let should_dedup = !self.has_js_execution_started_flag.is_raised();
    let result = snapshot
      .add_pkg_reqs(
        self.registry_info_provider.as_ref(),
        AddPkgReqsOptions {
          package_reqs,
          should_dedup,
          version_resolver: &self.npm_version_resolver,
        },
        self.reporter.as_deref(),
      )
      .await;
    let result = match &result.dep_graph_result {
      Err(NpmResolutionError::Resolution(err))
        if self.registry_info_provider.mark_force_reload() =>
      {
        log::debug!("{err:#}");
        log::debug!("npm resolution failed. Trying again...");

        // try again with forced reloading
        let snapshot = self.resolution.snapshot();
        snapshot
          .add_pkg_reqs(
            self.registry_info_provider.as_ref(),
            AddPkgReqsOptions {
              package_reqs,
              should_dedup,
              version_resolver: &self.npm_version_resolver,
            },
            self.reporter.as_deref(),
          )
          .await
      }
      _ => result,
    };

    self.registry_info_provider.clear_memory_cache();

    if !result.unmet_peer_diagnostics.is_empty()
      && log::log_enabled!(log::Level::Warn)
    {
      let root_node =
        peer_dep_diagnostics_to_display_tree(&result.unmet_peer_diagnostics);
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
    lockfile.content.packages.npm.clear();
    lockfile
      .content
      .packages
      .specifiers
      .retain(|req, _| match req.kind {
        PackageKind::Npm => false,
        PackageKind::Jsr => true,
      });
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

fn peer_dep_diagnostics_to_display_tree(
  diagnostics: &[UnmetPeerDepDiagnostic],
) -> DisplayTreeNode {
  struct MergedNode {
    text: Rc<String>,
    children: RefCell<Vec<Rc<MergedNode>>>,
  }

  // combine the nodes into a unified tree
  let mut nodes: BTreeMap<Rc<String>, Rc<MergedNode>> = BTreeMap::new();
  let mut top_level_nodes = Vec::new();

  for diagnostic in diagnostics {
    let text = Rc::new(format!(
      "peer {}: resolved to {}",
      diagnostic.dependency, diagnostic.resolved
    ));
    let mut node = Rc::new(MergedNode {
      text: text.clone(),
      children: Default::default(),
    });
    let mut found_ancestor = false;
    for ancestor in &diagnostic.ancestors {
      let nv_string = Rc::new(ancestor.to_string());
      if let Some(current_node) = nodes.get(&nv_string) {
        {
          let mut children = current_node.children.borrow_mut();
          if let Err(insert_index) =
            children.binary_search_by(|n| n.text.cmp(&node.text))
          {
            children.insert(insert_index, node);
          }
        }
        node = current_node.clone();
        found_ancestor = true;
        break;
      } else {
        let current_node = Rc::new(MergedNode {
          text: nv_string.clone(),
          children: RefCell::new(vec![node]),
        });
        nodes.insert(nv_string.clone(), current_node.clone());
        node = current_node;
      }
    }
    if !found_ancestor {
      top_level_nodes.push(node);
    }
  }

  // now output it
  let mut root_node = DisplayTreeNode {
    text: format!(
      "{} The following peer dependency issues were found:",
      colors::yellow("Warning")
    ),
    children: Vec::new(),
  };

  fn convert_node(node: &Rc<MergedNode>) -> DisplayTreeNode {
    DisplayTreeNode {
      text: node.text.to_string(),
      children: node.children.borrow().iter().map(convert_node).collect(),
    }
  }

  for top_level_node in top_level_nodes {
    root_node.children.push(convert_node(&top_level_node));
  }

  root_node
}

#[cfg(test)]
mod test {
  use deno_semver::Version;
  use deno_semver::package::PackageNv;

  use super::*;

  #[test]
  fn same_ancestor_peer_dep_message() {
    let peer_deps = Vec::from([
      UnmetPeerDepDiagnostic {
        ancestors: vec![PackageNv::from_str("a@1.0.0").unwrap()],
        dependency: PackageReq::from_str("b@*").unwrap(),
        resolved: Version::parse_standard("1.0.0").unwrap(),
      },
      UnmetPeerDepDiagnostic {
        // same ancestor as above
        ancestors: vec![PackageNv::from_str("a@1.0.0").unwrap()],
        dependency: PackageReq::from_str("c@*").unwrap(),
        resolved: Version::parse_standard("1.0.0").unwrap(),
      },
    ]);
    let display_tree = peer_dep_diagnostics_to_display_tree(&peer_deps);
    assert_eq!(display_tree.children.len(), 1);
    assert_eq!(display_tree.children[0].children.len(), 2);
  }
}
