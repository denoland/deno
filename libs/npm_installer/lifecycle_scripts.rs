// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Error as AnyError;
use deno_error::JsErrorBox;
use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_semver::SmallStackString;
use deno_semver::Version;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use sys_traits::FsMetadata;

use crate::CachedNpmPackageExtraInfoProvider;
use crate::LifecycleScriptsConfig;
use crate::PackagesAllowedScripts;

#[derive(Debug)]
pub struct PackageWithScript<'a> {
  pub package: &'a NpmResolutionPackage,
  pub scripts: HashMap<SmallStackString, String>,
  pub package_folder: PathBuf,
}

pub struct LifecycleScriptsExecutorOptions<'a> {
  pub init_cwd: &'a Path,
  pub process_state: &'a str,
  pub root_node_modules_dir_path: &'a Path,
  pub on_ran_pkg_scripts:
    &'a dyn Fn(&NpmResolutionPackage) -> Result<(), JsErrorBox>,
  pub snapshot: &'a NpmResolutionSnapshot,
  pub system_packages: &'a [NpmResolutionPackage],
  pub packages_with_scripts: &'a [PackageWithScript<'a>],
  pub extra_info_provider: &'a CachedNpmPackageExtraInfoProvider,
}

pub struct LifecycleScriptsWarning {
  message: String,

  did_warn_fn: DidWarnFn,
}

impl std::fmt::Debug for LifecycleScriptsWarning {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LifecycleScriptsWarning")
      .field("message", &self.message)
      .finish()
  }
}

type DidWarnFn =
  Box<dyn FnOnce(&dyn sys_traits::boxed::FsOpenBoxed) + Send + Sync>;

impl LifecycleScriptsWarning {
  pub(crate) fn new(message: String, did_warn_fn: DidWarnFn) -> Self {
    Self {
      message,
      did_warn_fn,
    }
  }

  pub fn into_message(
    self,
    sys: &dyn sys_traits::boxed::FsOpenBoxed,
  ) -> String {
    (self.did_warn_fn)(sys);
    self.message
  }
}

#[derive(Debug)]
pub struct NullLifecycleScriptsExecutor;

#[async_trait::async_trait(?Send)]
impl LifecycleScriptsExecutor for NullLifecycleScriptsExecutor {
  async fn execute(
    &self,
    _options: LifecycleScriptsExecutorOptions<'_>,
  ) -> Result<(), AnyError> {
    Ok(())
  }
}

#[async_trait::async_trait(?Send)]
pub trait LifecycleScriptsExecutor: Sync + Send {
  async fn execute(
    &self,
    options: LifecycleScriptsExecutorOptions<'_>,
  ) -> Result<(), AnyError>;
}

pub trait LifecycleScriptsStrategy {
  fn can_run_scripts(&self) -> bool {
    true
  }

  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, PathBuf)],
  ) -> Result<(), std::io::Error>;

  fn has_warned(&self, package: &NpmResolutionPackage) -> bool;

  fn has_run(&self, package: &NpmResolutionPackage) -> bool;
}

pub fn has_lifecycle_scripts(
  sys: &impl FsMetadata,
  extra: &NpmPackageExtraInfo,
  package_path: &Path,
) -> bool {
  if let Some(install) = extra.scripts.get("install") {
    {
      // default script
      if !is_broken_default_install_script(sys, install, package_path) {
        return true;
      }
    }
  }
  extra.scripts.contains_key("preinstall")
    || extra.scripts.contains_key("postinstall")
}

// npm defaults to running `node-gyp rebuild` if there is a `binding.gyp` file
// but it always fails if the package excludes the `binding.gyp` file when they publish.
// (for example, `fsevents` hits this)
pub fn is_broken_default_install_script(
  sys: &impl FsMetadata,
  script: &str,
  package_path: &Path,
) -> bool {
  script == "node-gyp rebuild"
    && !sys.fs_exists_no_err(package_path.join("binding.gyp"))
}

pub struct LifecycleScripts<'a, TSys: FsMetadata> {
  sys: &'a TSys,
  packages_with_scripts: Vec<PackageWithScript<'a>>,
  packages_with_scripts_not_run: Vec<(&'a NpmResolutionPackage, PathBuf)>,

  config: &'a LifecycleScriptsConfig,
  strategy: Box<dyn LifecycleScriptsStrategy + 'a>,
}

impl<'a, TSys: FsMetadata> LifecycleScripts<'a, TSys> {
  pub fn new<TLifecycleScriptsStrategy: LifecycleScriptsStrategy + 'a>(
    sys: &'a TSys,
    config: &'a LifecycleScriptsConfig,
    strategy: TLifecycleScriptsStrategy,
  ) -> Self {
    Self {
      sys,
      config,
      packages_with_scripts: Vec::new(),
      packages_with_scripts_not_run: Vec::new(),
      strategy: Box::new(strategy),
    }
  }

  pub fn can_run_scripts(&self, package_nv: &PackageNv) -> bool {
    fn matches_nv(req: &PackageReq, package_nv: &PackageNv) -> bool {
      // we shouldn't support this being a tag because it's too complicated
      debug_assert!(req.version_req.tag().is_none());
      package_nv.name == req.name
        && req.version_req.matches(&package_nv.version)
    }

    if !self.strategy.can_run_scripts() {
      return false;
    }
    let matches_allowed = match &self.config.allowed {
      PackagesAllowedScripts::All => true,
      PackagesAllowedScripts::Some(allow_list) => {
        allow_list.iter().any(|req| matches_nv(req, package_nv))
      }
      PackagesAllowedScripts::None => false,
    };
    matches_allowed
      && !self
        .config
        .denied
        .iter()
        .any(|req| matches_nv(req, package_nv))
  }

  pub fn has_run_scripts(&self, package: &NpmResolutionPackage) -> bool {
    self.strategy.has_run(package)
  }

  /// Register a package for running lifecycle scripts, if applicable.
  ///
  /// `package_path` is the path containing the package's code (its root dir).
  /// `package_meta_path` is the path to serve as the base directory for lifecycle
  /// script-related metadata (e.g. to store whether the scripts have been run already)
  pub fn add(
    &mut self,
    package: &'a NpmResolutionPackage,
    extra: &NpmPackageExtraInfo,
    package_path: Cow<'_, Path>,
  ) {
    if has_lifecycle_scripts(self.sys, extra, &package_path) {
      if self.can_run_scripts(&package.id.nv) {
        if !self.has_run_scripts(package) {
          self.packages_with_scripts.push(PackageWithScript {
            package,
            scripts: extra.scripts.clone(),
            package_folder: package_path.into_owned(),
          });
        }
      } else if !self.has_run_scripts(package)
        && (self.config.explicit_install || !self.strategy.has_warned(package))
        && !(self.config.denied.iter().any(|d| {
          package.id.nv.name == d.name
            && d.version_req.matches(&package.id.nv.version)
        }))
      {
        // Skip adding `esbuild` as it is known that it can work properly without lifecycle script
        // being run, and it's also very popular - any project using Vite would raise warnings.
        {
          let nv = &package.id.nv;
          if nv.name == "esbuild"
            && nv.version >= Version::parse_standard("0.18.0").unwrap()
          {
            return;
          }
        }

        self
          .packages_with_scripts_not_run
          .push((package, package_path.into_owned()));
      }
    }
  }

  pub fn warn_not_run_scripts(&self) -> Result<(), std::io::Error> {
    if !self.packages_with_scripts_not_run.is_empty() {
      self
        .strategy
        .warn_on_scripts_not_run(&self.packages_with_scripts_not_run)?;
    }
    Ok(())
  }

  pub fn packages_with_scripts(&self) -> &[PackageWithScript<'a>] {
    &self.packages_with_scripts
  }
}

pub static LIFECYCLE_SCRIPTS_RUNNING_ENV_VAR: &str =
  "DENO_INTERNAL_IS_LIFECYCLE_SCRIPT";

pub fn is_running_lifecycle_script(sys: &impl sys_traits::EnvVar) -> bool {
  sys.env_var(LIFECYCLE_SCRIPTS_RUNNING_ENV_VAR).is_ok()
}

/// Groups packages with lifecycle scripts into topological layers using
/// Kahn's algorithm. Packages in the same layer have no inter-dependencies
/// (considering only packages that have lifecycle scripts), so they can
/// run in parallel. Later layers depend on earlier ones.
///
/// This considers transitive dependencies through the full snapshot, not
/// just direct dependencies. For example, if A depends on B (no scripts)
/// which depends on C (has scripts), A will be placed in a later layer
/// than C.
pub fn compute_lifecycle_script_layers<'a>(
  packages: &'a [PackageWithScript<'a>],
  snapshot: &NpmResolutionSnapshot,
) -> Vec<Vec<&'a PackageWithScript<'a>>> {
  if packages.len() <= 1 {
    return vec![packages.iter().collect()];
  }

  let start = std::time::Instant::now();
  let script_pkg_ids: HashSet<&NpmPackageId> =
    packages.iter().map(|p| &p.package.id).collect();
  let pkg_by_id: HashMap<&NpmPackageId, &PackageWithScript> =
    packages.iter().map(|p| (&p.package.id, p)).collect();

  // for each package, find transitive deps that have lifecycle scripts
  // (walking through intermediate packages that don't have scripts)
  let mut in_degree: HashMap<&NpmPackageId, usize> = HashMap::new();
  let mut dependents: HashMap<&NpmPackageId, Vec<&NpmPackageId>> =
    HashMap::new();
  for pkg in packages {
    let transitive_script_deps =
      find_transitive_script_deps(pkg.package, &script_pkg_ids, snapshot);
    in_degree.insert(&pkg.package.id, transitive_script_deps.len());
    for dep_id in transitive_script_deps {
      dependents.entry(dep_id).or_default().push(&pkg.package.id);
    }
  }

  // if no package has any script deps, everything is one layer
  if in_degree.values().all(|&deg| deg == 0) {
    return vec![packages.iter().collect()];
  }

  // peel off layers using Kahn's algorithm
  let mut layers = Vec::new();
  let mut queue: VecDeque<&NpmPackageId> = in_degree
    .iter()
    .filter(|(_, deg)| **deg == 0)
    .map(|(&id, _)| id)
    .collect();

  while !queue.is_empty() {
    let layer: Vec<&PackageWithScript> =
      queue.iter().map(|id| pkg_by_id[id]).collect();
    let mut next_queue = VecDeque::new();
    for id in queue.drain(..) {
      if let Some(deps) = dependents.get(id) {
        for &dep_id in deps {
          let deg = in_degree.get_mut(dep_id).unwrap();
          *deg -= 1;
          if *deg == 0 {
            next_queue.push_back(dep_id);
          }
        }
      }
    }
    layers.push(layer);
    queue = next_queue;
  }

  log::debug!(
    "Computed lifecycle script layers in {}ms.",
    start.elapsed().as_millis()
  );

  layers
}

/// Finds all transitive dependency package IDs that have lifecycle scripts,
/// walking through intermediate packages that may not have scripts themselves.
fn find_transitive_script_deps<'a>(
  package: &'a NpmResolutionPackage,
  script_pkg_ids: &HashSet<&'a NpmPackageId>,
  snapshot: &'a NpmResolutionSnapshot,
) -> HashSet<&'a NpmPackageId> {
  let mut result = HashSet::new();
  let mut visited = HashSet::new();
  let mut stack: Vec<&NpmPackageId> = package.dependencies.values().collect();

  while let Some(dep_id) = stack.pop() {
    if !visited.insert(dep_id) {
      continue;
    }
    if script_pkg_ids.contains(dep_id) {
      result.insert(dep_id);
      // don't walk further — this script package forms a layer boundary
      continue;
    }
    // walk through non-script packages to find transitive script deps
    if let Some(dep_pkg) = snapshot.package_from_id(dep_id) {
      stack.extend(dep_pkg.dependencies.values());
    }
  }

  result
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use std::path::PathBuf;

  use deno_npm::NpmPackageId;
  use deno_npm::resolution::NpmResolutionSnapshot;
  use deno_npm::resolution::SerializedNpmResolutionSnapshot;
  use deno_npm::resolution::SerializedNpmResolutionSnapshotPackage;
  use deno_semver::StackString;
  use deno_semver::package::PackageReq;

  use super::PackageWithScript;
  use super::compute_lifecycle_script_layers;

  fn pkg_id(s: &str) -> NpmPackageId {
    NpmPackageId::from_serialized(s).unwrap()
  }

  fn deps(pairs: &[(&str, &str)]) -> HashMap<StackString, NpmPackageId> {
    pairs
      .iter()
      .map(|(k, v)| (StackString::from(*k), pkg_id(v)))
      .collect()
  }

  fn pkg(
    id: &str,
    dependencies: &[(&str, &str)],
  ) -> SerializedNpmResolutionSnapshotPackage {
    SerializedNpmResolutionSnapshotPackage {
      id: pkg_id(id),
      system: Default::default(),
      dist: None,
      dependencies: deps(dependencies),
      optional_dependencies: Default::default(),
      optional_peer_dependencies: Default::default(),
      extra: None,
      is_deprecated: false,
      has_bin: false,
      has_scripts: false,
    }
  }

  fn make_snapshot(
    root: &[(&str, &str)],
    packages: Vec<SerializedNpmResolutionSnapshotPackage>,
  ) -> NpmResolutionSnapshot {
    let serialized = SerializedNpmResolutionSnapshot {
      root_packages: root
        .iter()
        .map(|(k, v)| (PackageReq::from_str(k).unwrap(), pkg_id(v)))
        .collect(),
      packages,
    };
    NpmResolutionSnapshot::new(serialized.into_valid().unwrap())
  }

  fn make_pkg_with_script<'a>(
    id: &str,
    snapshot: &'a NpmResolutionSnapshot,
  ) -> PackageWithScript<'a> {
    PackageWithScript {
      package: snapshot.package_from_id(&pkg_id(id)).unwrap(),
      scripts: HashMap::from([("install".into(), "node-gyp rebuild".into())]),
      package_folder: PathBuf::from(format!("/tmp/{id}")),
    }
  }

  /// extracts sorted package names from each layer for easy assertion
  fn layer_names(layers: &[Vec<&PackageWithScript>]) -> Vec<Vec<String>> {
    layers
      .iter()
      .map(|layer| {
        let mut names: Vec<String> = layer
          .iter()
          .map(|p| p.package.id.nv.name.to_string())
          .collect();
        names.sort();
        names
      })
      .collect()
  }

  #[test]
  fn single_package() {
    let snapshot =
      make_snapshot(&[("a@1", "a@1.0.0")], vec![pkg("a@1.0.0", &[])]);
    let pkgs = vec![make_pkg_with_script("a@1.0.0", &snapshot)];
    let layers = compute_lifecycle_script_layers(&pkgs, &snapshot);
    assert_eq!(layers.len(), 1);
    assert_eq!(layer_names(&layers), vec![vec!["a"]]);
  }

  #[test]
  fn no_interdependencies() {
    // a and b have scripts but don't depend on each other
    let snapshot = make_snapshot(
      &[("a@1", "a@1.0.0"), ("b@1", "b@1.0.0")],
      vec![pkg("a@1.0.0", &[]), pkg("b@1.0.0", &[])],
    );
    let pkgs = vec![
      make_pkg_with_script("a@1.0.0", &snapshot),
      make_pkg_with_script("b@1.0.0", &snapshot),
    ];
    let layers = compute_lifecycle_script_layers(&pkgs, &snapshot);
    assert_eq!(layers.len(), 1);
    assert_eq!(layer_names(&layers), vec![vec!["a", "b"]]);
  }

  #[test]
  fn direct_dependency_chain() {
    // a depends on b, both have scripts => two layers
    let snapshot = make_snapshot(
      &[("a@1", "a@1.0.0")],
      vec![pkg("a@1.0.0", &[("b", "b@1.0.0")]), pkg("b@1.0.0", &[])],
    );
    let pkgs = vec![
      make_pkg_with_script("a@1.0.0", &snapshot),
      make_pkg_with_script("b@1.0.0", &snapshot),
    ];
    let layers = compute_lifecycle_script_layers(&pkgs, &snapshot);
    assert_eq!(layer_names(&layers), vec![vec!["b"], vec!["a"]]);
  }

  #[test]
  fn transitive_through_non_script_package() {
    // a -> b (no scripts) -> c (has scripts)
    // a and c should be in different layers
    let snapshot = make_snapshot(
      &[("a@1", "a@1.0.0")],
      vec![
        pkg("a@1.0.0", &[("b", "b@1.0.0")]),
        pkg("b@1.0.0", &[("c", "c@1.0.0")]),
        pkg("c@1.0.0", &[]),
      ],
    );
    // only a and c have scripts, b does not
    let pkgs = vec![
      make_pkg_with_script("a@1.0.0", &snapshot),
      make_pkg_with_script("c@1.0.0", &snapshot),
    ];
    let layers = compute_lifecycle_script_layers(&pkgs, &snapshot);
    assert_eq!(layer_names(&layers), vec![vec!["c"], vec!["a"]]);
  }

  #[test]
  fn diamond_dependency() {
    // a -> b, a -> c, b -> d, c -> d
    // all have scripts
    // layer 0: d, layer 1: b + c, layer 2: a
    let snapshot = make_snapshot(
      &[("a@1", "a@1.0.0")],
      vec![
        pkg("a@1.0.0", &[("b", "b@1.0.0"), ("c", "c@1.0.0")]),
        pkg("b@1.0.0", &[("d", "d@1.0.0")]),
        pkg("c@1.0.0", &[("d", "d@1.0.0")]),
        pkg("d@1.0.0", &[]),
      ],
    );
    let pkgs = vec![
      make_pkg_with_script("a@1.0.0", &snapshot),
      make_pkg_with_script("b@1.0.0", &snapshot),
      make_pkg_with_script("c@1.0.0", &snapshot),
      make_pkg_with_script("d@1.0.0", &snapshot),
    ];
    let layers = compute_lifecycle_script_layers(&pkgs, &snapshot);
    assert_eq!(
      layer_names(&layers),
      vec![vec!["d"], vec!["b", "c"], vec!["a"]]
    );
  }

  #[test]
  fn empty_packages() {
    let snapshot = make_snapshot(&[], vec![]);
    let pkgs: Vec<PackageWithScript> = vec![];
    let layers = compute_lifecycle_script_layers(&pkgs, &snapshot);
    assert_eq!(layers.len(), 1);
    assert!(layers[0].is_empty());
  }
}
