// Copyright 2018-2026 the Deno authors. MIT license.

//! Code for hoisted node_modules resolution.
//!
//! In hoisted mode, all packages are flattened into the top-level
//! `node_modules/` directory (like npm/yarn classic). When version
//! conflicts exist, the most commonly depended-upon version is hoisted
//! to the top level, and conflicting versions are nested under their
//! dependent's `node_modules/`.

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_error::JsErrorBox;
use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmPackageId;
use deno_npm::NpmPackageIdPeerDependencies;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmResolutionPackageSystemInfo;
use deno_npm::NpmSystemInfo;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm_cache::NpmCache;
use deno_npm_cache::NpmCacheHttpClient;
use deno_npm_cache::TarballCache;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_semver::StackString;
use deno_semver::package::PackageNv;
use deno_terminal::colors;
use futures::FutureExt;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use parking_lot::Mutex;
use sys_traits::FsDirEntry;
use sys_traits::FsMetadata;
use sys_traits::PathsInErrorsExt;

use crate::BinEntries;
use crate::CachedNpmPackageExtraInfoProvider;
use crate::ExpectedExtraInfo;
use crate::LifecycleScriptsConfig;
use crate::NpmPackageExtraInfoProvider;
use crate::NpmPackageFsInstaller;
use crate::PackageCaching;
use crate::Reporter;
use crate::bin_entries::EntrySetupOutcome;
use crate::flag::LaxSingleProcessFsFlag;
use crate::fs::clone_dir_recursive;
use crate::lifecycle_scripts::LifecycleScripts;
use crate::lifecycle_scripts::LifecycleScriptsExecutor;
use crate::lifecycle_scripts::LifecycleScriptsExecutorOptions;
use crate::lifecycle_scripts::LifecycleScriptsStrategy;
use crate::lifecycle_scripts::has_lifecycle_scripts;
use crate::lifecycle_scripts::is_running_lifecycle_script;
use crate::local::InitializingGuard;
use crate::local::LocalNpmInstallSys;
use crate::local::LocalNpmPackageInstallerOptions;
use crate::local::SyncResolutionWithFsError;
use crate::local::join_package_name;
use crate::package_json::InstallWorkspacePkgDep;
use crate::package_json::NpmInstallDepsProvider;
use crate::process_state::NpmProcessState;

/// Describes where each package should be placed in a hoisted layout.
struct HoistedLayout<'a> {
  /// Packages that go to `node_modules/<name>/`.
  top_level: HashMap<&'a StackString, &'a NpmResolutionPackage>,
  /// Packages that must be nested: `<parent_path>/node_modules/<dep>/`,
  /// where `parent_path` is relative to the root `node_modules/` dir and
  /// may itself include `.../node_modules/<name>` segments for parents
  /// that are themselves nested.
  nested: Vec<NestedPackage<'a>>,
}

struct NestedPackage<'a> {
  /// Path of the parent relative to the root `node_modules/` directory.
  /// For a top-level parent this is just `<name>`; for a nested parent
  /// it is `<ancestor>/node_modules/.../node_modules/<name>`.
  parent_path: PathBuf,
  #[allow(dead_code, reason = "used for future nested package resolution")]
  dep_name: &'a StackString,
  dep: &'a NpmResolutionPackage,
}

/// Compute the hoisted layout for all packages.
///
/// For each package name, the version with the most dependents is hoisted
/// to the top level. Other versions are nested under their parent packages.
fn compute_hoisted_layout<'a>(
  snapshot: &'a NpmResolutionSnapshot,
  packages: &'a [NpmResolutionPackage],
  system_info: &NpmSystemInfo,
) -> HoistedLayout<'a> {
  // Versions explicitly required by the root/workspace package.json(s).
  // These take priority when picking what to hoist, matching npm: a
  // version listed in package.json wins over a (possibly higher) version
  // pulled in only transitively.
  let mut root_version_for_name: HashMap<&StackString, &PackageNv> =
    HashMap::new();
  for root_nv in snapshot.package_reqs().values() {
    root_version_for_name
      .entry(&root_nv.name)
      .and_modify(|existing| {
        // Multiple workspace members directly require the same package
        // at different versions. Tie-break by higher version so the
        // result is deterministic; the loser gets nested.
        if root_nv.cmp(existing) == Ordering::Greater {
          *existing = root_nv;
        }
      })
      .or_insert(root_nv);
  }

  // Count how many packages depend on each (name, version) pair
  let mut version_dependents: HashMap<&PackageNv, usize> = HashMap::new();

  for package in packages {
    for dep_id in package.dependencies.values() {
      let dep = snapshot.package_from_id(dep_id).unwrap();
      *version_dependents.entry(&dep.id.nv).or_insert(0) += 1;
    }
  }

  // For each package name, pick the version to hoist. A version
  // required directly by the root/workspace always wins; otherwise we
  // fall back to "most depended on, then highest version".
  let mut best_version_for_name: HashMap<&StackString, &NpmResolutionPackage> =
    HashMap::new();

  for package in packages {
    let name = &package.id.nv.name;
    if let Some(root_nv) = root_version_for_name.get(name) {
      if package.id.nv == **root_nv {
        best_version_for_name.insert(name, package);
      }
      // Any other version of a root-required name will be nested.
      continue;
    }
    match best_version_for_name.get(name) {
      Some(current_best) => {
        let current_count = version_dependents
          .get(&current_best.id.nv)
          .copied()
          .unwrap_or(0);
        let new_count =
          version_dependents.get(&package.id.nv).copied().unwrap_or(0);
        if new_count > current_count
          || (new_count == current_count
            && package.id.nv.cmp(&current_best.id.nv) == Ordering::Greater)
        {
          best_version_for_name.insert(name, package);
        }
      }
      None => {
        best_version_for_name.insert(name, package);
      }
    }
  }

  // Walk the dependency tree breadth-first starting from the top-level
  // (hoisted) packages so that when a transitive package is itself
  // nested, its own conflicting deps get placed under the actual
  // nested location rather than under the top-level package that
  // happens to share the same name.
  //
  // For example with root deps `schema-utils@4` + `terser-webpack-plugin`:
  //   * `schema-utils@4` and `ajv@8` hoist to the top level.
  //   * `terser-webpack-plugin` transitively pulls `schema-utils@3`,
  //     which nests at `terser-webpack-plugin/node_modules/schema-utils`.
  //   * `schema-utils@3` needs `ajv@6`. The previous flat algorithm
  //     placed it at `node_modules/schema-utils/node_modules/ajv`,
  //     shadowing the top-level `ajv@8` for the hoisted `schema-utils@4`.
  //     With the BFS below it lands at
  //     `node_modules/terser-webpack-plugin/node_modules/schema-utils/node_modules/ajv`.
  let mut nested = Vec::new();
  // Each queue entry carries the parent's path (relative to the root
  // `node_modules/`), the parent package, and the chain of ancestor
  // packages whose `node_modules/<name>` segments lie on `parent_path`
  // — these are the candidates Node's resolver walks up to.
  let mut queue: VecDeque<(
    PathBuf,
    &NpmResolutionPackage,
    Vec<&NpmResolutionPackage>,
  )> = VecDeque::new();
  // `(parent_path, parent_nv)` we've already expanded — avoids
  // re-emitting the same edges when multiple paths reach the same
  // (path, package) pair.
  let mut visited: HashSet<(PathBuf, &PackageNv)> = HashSet::new();

  for (name, package) in &best_version_for_name {
    queue.push_back((PathBuf::from(name.as_str()), package, Vec::new()));
  }

  while let Some((parent_path, parent, parent_ancestors)) = queue.pop_front() {
    if !visited.insert((parent_path.clone(), &parent.id.nv)) {
      continue;
    }
    for (dep_name, dep_id) in &parent.dependencies {
      let dep = snapshot.package_from_id(dep_id).unwrap();

      if parent.optional_dependencies.contains(dep_name)
        && !dep.system.matches_system(system_info)
      {
        continue;
      }

      // Self-loop: parent depends on something at its own nv. Nesting
      // at `parent_path/node_modules/<name>` would just be a copy of
      // parent under itself.
      if parent.id.nv == dep.id.nv {
        continue;
      }

      // Simulate Node's directory walk-up from `parent`'s dir: it
      // binds the nearest `<ancestor>/node_modules/<dep.name>`, which
      // exists iff that ancestor has a *non-hoisted* dep with that
      // name (hoisted versions live at the root, not in the
      // ancestor's own node_modules). Match by name, not by nv — two
      // different versions of the same name on one path each shadow
      // further-up copies.
      //
      // If walk-up already finds `dep.id.nv`, no nesting is needed.
      // This subsumes the simple "hoisted at root" case, breaks
      // dependency cycles, and avoids redundant deeper nestings.
      let hoisted_for_name =
        best_version_for_name.get(&dep.id.nv.name).map(|p| &p.id.nv);
      let walkup_target = parent_ancestors
        .iter()
        .rev()
        .find_map(|ancestor| {
          ancestor.dependencies.values().find_map(|aid| {
            let apkg = snapshot.package_from_id(aid).unwrap();
            if apkg.id.nv.name == dep.id.nv.name
              && hoisted_for_name != Some(&apkg.id.nv)
            {
              Some(&apkg.id.nv)
            } else {
              None
            }
          })
        })
        .or(hoisted_for_name);

      if walkup_target == Some(&dep.id.nv) {
        continue;
      }

      let dep_path = parent_path
        .join("node_modules")
        .join(dep.id.nv.name.as_str());
      nested.push(NestedPackage {
        parent_path: parent_path.clone(),
        dep_name,
        dep,
      });
      let mut dep_ancestors = parent_ancestors.clone();
      dep_ancestors.push(parent);
      queue.push_back((dep_path, dep, dep_ancestors));
    }
  }

  HoistedLayout {
    top_level: best_version_for_name,
    nested,
  }
}

/// Installer that creates a hoisted (flat) node_modules directory.
pub struct HoistedNpmPackageInstaller<
  THttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: LocalNpmInstallSys,
> {
  lifecycle_scripts_executor: Arc<dyn LifecycleScriptsExecutor>,
  npm_cache: Arc<NpmCache<TSys>>,
  npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
  npm_package_extra_info_provider: Arc<NpmPackageExtraInfoProvider>,
  reporter: TReporter,
  resolution: Arc<NpmResolutionCell>,
  sys: TSys,
  tarball_cache: Arc<TarballCache<THttpClient, TSys>>,
  clean_on_install: bool,
  lifecycle_scripts_config: Arc<LifecycleScriptsConfig>,
  root_node_modules_path: PathBuf,
  system_info: NpmSystemInfo,
  install_reporter: Option<Arc<dyn crate::InstallReporter>>,
}

impl<
  THttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: LocalNpmInstallSys,
> std::fmt::Debug for HoistedNpmPackageInstaller<THttpClient, TReporter, TSys>
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("HoistedNpmPackageInstaller")
      .field("root_node_modules_path", &self.root_node_modules_path)
      .finish()
  }
}

impl<
  THttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: LocalNpmInstallSys,
> HoistedNpmPackageInstaller<THttpClient, TReporter, TSys>
{
  #[allow(
    clippy::too_many_arguments,
    reason = "many dependencies needed for installer construction"
  )]
  pub fn new(
    lifecycle_scripts_executor: Arc<dyn LifecycleScriptsExecutor>,
    npm_cache: Arc<NpmCache<TSys>>,
    npm_package_extra_info_provider: Arc<NpmPackageExtraInfoProvider>,
    npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
    reporter: TReporter,
    resolution: Arc<NpmResolutionCell>,
    sys: TSys,
    tarball_cache: Arc<TarballCache<THttpClient, TSys>>,
    options: LocalNpmPackageInstallerOptions,
  ) -> Self {
    Self {
      lifecycle_scripts_executor,
      npm_cache,
      npm_install_deps_provider,
      npm_package_extra_info_provider,
      reporter,
      resolution,
      tarball_cache,
      sys,
      clean_on_install: options.clean_on_install,
      lifecycle_scripts_config: options.lifecycle_scripts,
      root_node_modules_path: options.node_modules_folder,
      install_reporter: options.reporter,
      system_info: options.system_info,
    }
  }

  async fn sync_resolution_with_fs(
    &self,
    snapshot: &NpmResolutionSnapshot,
  ) -> Result<(), SyncResolutionWithFsError> {
    let has_no_packages = snapshot.is_empty()
      && self.npm_install_deps_provider.local_pkgs().is_empty()
      && !self
        .npm_install_deps_provider
        .workspace_pkgs()
        .iter()
        .any(|pkg| !pkg.scripts.is_empty());
    let deno_marker_dir = self.root_node_modules_path.join(".deno");
    if has_no_packages
      && (!self.clean_on_install
        || !self.sys.fs_exists_no_err(&deno_marker_dir))
    {
      return Ok(());
    }

    if is_running_lifecycle_script(&self.sys) {
      return Ok(());
    }

    let sys = self.sys.with_paths_in_errors();

    // Create the node_modules directory
    sys.fs_create_dir_all(&self.root_node_modules_path)?;

    // Use a marker directory for the lock file (reuse .deno for compatibility)
    sys.fs_create_dir_all(&deno_marker_dir)?;

    let bin_node_modules_dir_path = self.root_node_modules_path.join(".bin");
    let single_process_lock = LaxSingleProcessFsFlag::lock(
      sys.as_ref(),
      deno_marker_dir.join(".deno.lock"),
      &self.reporter,
      "waiting for file lock on node_modules directory",
    )
    .await;

    let package_partitions =
      snapshot.all_system_packages_partitioned(&self.system_info);
    let pb_clear_guard = self.reporter.clear_guard();

    // Compute the hoisted layout
    let layout = compute_hoisted_layout(
      snapshot,
      &package_partitions.packages,
      &self.system_info,
    );

    // 1. If clean install, remove stale packages
    if self.clean_on_install {
      cleanup_hoisted_packages(
        sys.as_ref(),
        &self.root_node_modules_path,
        &layout,
      );
    }

    // 2. Clone all packages from cache into their hoisted positions
    let workspace_lifecycle_packages =
      self.resolve_workspace_lifecycle_packages(snapshot)?;
    let mut cache_futures = FuturesUnordered::new();
    let bin_entries = Rc::new(RefCell::new(BinEntries::new(sys)));
    let lifecycle_scripts = Rc::new(RefCell::new(LifecycleScripts::new(
      sys.as_ref(),
      &self.lifecycle_scripts_config,
      HoistedLifecycleScripts {
        install_reporter: self.install_reporter.clone(),
      },
    )));
    let packages_with_deprecation_warnings = Arc::new(Mutex::new(Vec::new()));

    let mut package_tags: HashMap<&PackageNv, BTreeSet<&str>> = HashMap::new();
    for (package_req, package_nv) in snapshot.package_reqs() {
      if let Some(tag) = package_req.version_req.tag() {
        package_tags.entry(package_nv).or_default().insert(tag);
      }
    }

    let extra_info_provider = Arc::new(CachedNpmPackageExtraInfoProvider::new(
      self.npm_package_extra_info_provider.clone(),
    ));

    // Map a package to the workspace member directory that declares it as a
    // direct dependency, so its lifecycle scripts run with `INIT_CWD` pointing
    // at that member rather than the workspace root.
    let lifecycle_script_init_cwds: Rc<HashMap<NpmPackageId, Vec<PathBuf>>> =
      Rc::new(crate::lifecycle_scripts::member_dep_init_cwds(
        &self.npm_install_deps_provider,
        snapshot,
        self.root_node_modules_path.parent(),
      ));

    // Clone top-level (hoisted) packages
    for package in layout.top_level.values() {
      let package_path = join_package_name(
        Cow::Borrowed(&self.root_node_modules_path),
        &package.id.nv.name,
      );

      let packages_with_deprecation_warnings =
        packages_with_deprecation_warnings.clone();
      let extra_info_provider = extra_info_provider.clone();
      let lifecycle_scripts = lifecycle_scripts.clone();
      let lifecycle_script_init_cwds = lifecycle_script_init_cwds.clone();
      let bin_entries_to_setup = bin_entries.clone();
      let install_reporter = self.install_reporter.clone();

      if let Some(dist) = &package.dist {
        cache_futures.push(
          self
            .clone_package(
              package,
              dist,
              package_path,
              install_reporter,
              extra_info_provider,
              bin_entries_to_setup,
              lifecycle_scripts,
              lifecycle_script_init_cwds,
              packages_with_deprecation_warnings,
            )
            .boxed_local(),
        );
      }
    }

    // Wait for top-level clones
    while let Some(result) = cache_futures.next().await {
      result?;
    }

    // 3. Clone nested packages (version conflicts)
    for nested in &layout.nested {
      let parent_path = self.root_node_modules_path.join(&nested.parent_path);
      let nested_node_modules = parent_path.join("node_modules");
      sys.fs_create_dir_all(&nested_node_modules)?;
      let package_path = join_package_name(
        Cow::Owned(nested_node_modules),
        &nested.dep.id.nv.name,
      );

      let packages_with_deprecation_warnings =
        packages_with_deprecation_warnings.clone();
      let extra_info_provider = extra_info_provider.clone();
      let lifecycle_scripts = lifecycle_scripts.clone();
      let lifecycle_script_init_cwds = lifecycle_script_init_cwds.clone();
      let bin_entries_to_setup = bin_entries.clone();
      let install_reporter = self.install_reporter.clone();

      if let Some(dist) = &nested.dep.dist {
        cache_futures.push(
          self
            .clone_package(
              nested.dep,
              dist,
              package_path,
              install_reporter,
              extra_info_provider,
              bin_entries_to_setup,
              lifecycle_scripts,
              lifecycle_script_init_cwds,
              packages_with_deprecation_warnings,
            )
            .boxed_local(),
        );
      }
    }

    while let Some(result) = cache_futures.next().await {
      result?;
    }

    // 4. Setup patch packages
    for patch_pkg in self.npm_install_deps_provider.patch_pkgs() {
      let target = join_package_name(
        Cow::Borrowed(&self.root_node_modules_path),
        &patch_pkg.nv.name,
      );

      cache_futures.push(
        async move {
          let from_path = patch_pkg.target_dir.clone();
          let sys = self.sys.clone();
          crate::rt::spawn_blocking(move || {
            crate::local::clone_dir_recursive_except_node_modules_child(
              &sys, &from_path, &target,
            )
          })
          .await
          .map_err(JsErrorBox::from_err)?
          .map_err(JsErrorBox::from_err)?;
          Ok::<_, JsErrorBox>(())
        }
        .boxed_local(),
      );
    }

    while let Some(result) = cache_futures.next().await {
      result?;
    }
    drop(cache_futures);

    // 5. Set up bin entries
    {
      let bin_entries = match Rc::try_unwrap(bin_entries) {
        Ok(bin_entries) => bin_entries.into_inner(),
        Err(_) => panic!("Should have sole ref to rc."),
      };
      bin_entries.finish(
        snapshot,
        &bin_node_modules_dir_path,
        |setup_outcome| {
          let lifecycle_scripts = lifecycle_scripts.borrow();
          match setup_outcome {
            EntrySetupOutcome::MissingEntrypoint {
              package,
              package_path,
              extra,
              ..
            } if has_lifecycle_scripts(sys.as_ref(), extra, package_path)
              && lifecycle_scripts.can_run_scripts(&package.id.nv)
              && !lifecycle_scripts.has_run_scripts(package) =>
            {
              // ignore, it might get fixed when the lifecycle scripts run
            }
            outcome => outcome.warn_if_failed(),
          }
        },
      )?;
    }

    // 6. Create symlinks for workspace packages
    {
      for pkg in self.npm_install_deps_provider.local_pkgs() {
        let Some(pkg_alias) = &pkg.alias else {
          continue;
        };
        crate::local::symlink_package_dir(
          sys.as_ref(),
          &pkg.target_dir,
          &self.root_node_modules_path.join(pkg_alias),
        )?;
      }
    }

    // 7. Create a `node_modules` directory inside each workspace member and
    // symlink that member's direct dependencies into it. This mirrors how npm
    // and pnpm lay out workspaces so that native Node.js tooling run from
    // within a member resolves the member's dependencies and sibling workspace
    // members. Each dependency is linked to its actual location in the hoisted
    // layout (a top-level package, or a nested one in case of conflicts).
    {
      let workspace_member_dirs: HashMap<&PackageNv, &Path> = self
        .npm_install_deps_provider
        .workspace_pkgs()
        .iter()
        .map(|pkg| (&pkg.nv, pkg.target_dir.as_path()))
        .collect();
      for workspace_pkg in self.npm_install_deps_provider.workspace_pkgs() {
        // The workspace root's `node_modules` is already fully set up above.
        // (Comparing paths here is unreliable: `root_node_modules_path` is
        // canonicalized while `target_dir` is not, so on Windows they can
        // differ by 8.3 short names or casing for the same directory.)
        if workspace_pkg.is_root || workspace_pkg.deps.is_empty() {
          continue;
        }
        let member_node_modules = workspace_pkg.target_dir.join("node_modules");
        let mut created_dir = false;
        for dep in &workspace_pkg.deps {
          let (alias, target_path) = match dep {
            InstallWorkspacePkgDep::Remote { alias, req } => {
              let Some(id) = resolve_remote_pkg_id(snapshot, req) else {
                continue;
              };
              let Some(target_path) = hoisted_package_path(
                &layout,
                &self.root_node_modules_path,
                &id.nv,
              ) else {
                continue;
              };
              (alias, target_path)
            }
            InstallWorkspacePkgDep::Workspace { alias, nv } => {
              let Some(target_dir) = workspace_member_dirs.get(nv) else {
                continue;
              };
              (alias, target_dir.to_path_buf())
            }
          };
          if !created_dir {
            sys.fs_create_dir_all(&member_node_modules)?;
            created_dir = true;
          }
          crate::local::symlink_package_dir(
            sys.as_ref(),
            &target_path,
            &member_node_modules.join(alias.as_str()),
          )?;
        }
      }
    }

    for package in &workspace_lifecycle_packages {
      lifecycle_scripts.borrow_mut().add(
        &package.package,
        &NpmPackageExtraInfo {
          scripts: package.scripts.clone(),
          ..Default::default()
        },
        Cow::Borrowed(&package.package_path),
        Vec::new(),
      );
    }

    // Deprecation warnings
    {
      let packages_with_deprecation_warnings =
        packages_with_deprecation_warnings.lock();
      if !packages_with_deprecation_warnings.is_empty() {
        use std::fmt::Write;
        let mut output = String::new();
        let _ = writeln!(
          &mut output,
          "{} The following packages are deprecated:",
          colors::yellow("Warning")
        );
        let len = packages_with_deprecation_warnings.len();
        for (idx, (package_nv, msg)) in
          packages_with_deprecation_warnings.iter().enumerate()
        {
          if idx != len - 1 {
            let _ = writeln!(
              &mut output,
              "┠─ {}",
              colors::gray(format!("npm:{:?} ({})", package_nv, msg))
            );
          } else {
            let _ = write!(
              &mut output,
              "┖─ {}",
              colors::gray(format!("npm:{:?} ({})", package_nv, msg))
            );
          }
        }
        if let Some(install_reporter) = &self.install_reporter {
          install_reporter.deprecated_message(output);
        } else {
          log::warn!("{}", output);
        }
      }
    }

    // Lifecycle scripts
    let lifecycle_scripts_to_run = std::mem::replace(
      &mut *lifecycle_scripts.borrow_mut(),
      LifecycleScripts::new(
        sys.as_ref(),
        &self.lifecycle_scripts_config,
        HoistedLifecycleScripts {
          install_reporter: self.install_reporter.clone(),
        },
      ),
    );
    drop(lifecycle_scripts);
    lifecycle_scripts_to_run.warn_not_run_scripts()?;

    let packages_with_scripts =
      lifecycle_scripts_to_run.packages_with_scripts();
    if !packages_with_scripts.is_empty() {
      let additional_packages = workspace_lifecycle_packages
        .iter()
        .map(|package| &package.package)
        .collect::<Vec<_>>();
      let process_state = NpmProcessState::new_local(
        snapshot.as_valid_serialized(),
        &self.root_node_modules_path,
        crate::process_state::NpmProcessStateLinkerMode::Hoisted,
      )
      .as_serialized();

      self
        .lifecycle_scripts_executor
        .execute(LifecycleScriptsExecutorOptions {
          init_cwd: &self.lifecycle_scripts_config.initial_cwd,
          process_state: process_state.as_str(),
          root_node_modules_dir_path: &self.root_node_modules_path,
          on_ran_pkg_scripts: &|_pkg| Ok(()),
          snapshot,
          system_packages: &package_partitions.packages,
          additional_packages: &additional_packages,
          packages_with_scripts,
          extra_info_provider: &extra_info_provider,
        })
        .await
        .map_err(SyncResolutionWithFsError::LifecycleScripts)?
    }

    drop(single_process_lock);
    drop(pb_clear_guard);

    Ok(())
  }

  fn resolve_workspace_lifecycle_packages(
    &self,
    snapshot: &NpmResolutionSnapshot,
  ) -> Result<Vec<WorkspaceLifecyclePackage>, JsErrorBox> {
    let workspace_pkgs = self.npm_install_deps_provider.workspace_pkgs();
    let workspace_pkg_ids = workspace_pkgs
      .iter()
      .map(|pkg| {
        (
          pkg.nv.clone(),
          NpmPackageId {
            nv: pkg.nv.clone(),
            peer_dependencies: NpmPackageIdPeerDependencies::from([]),
          },
        )
      })
      .collect::<HashMap<_, _>>();
    let mut packages = Vec::with_capacity(workspace_pkgs.len());
    for workspace_pkg in workspace_pkgs {
      let mut dependencies = HashMap::with_capacity(workspace_pkg.deps.len());
      for dep in &workspace_pkg.deps {
        match dep {
          InstallWorkspacePkgDep::Remote { alias, req } => {
            if let Some(id) = resolve_remote_pkg_id(snapshot, req) {
              dependencies.insert(alias.clone(), id);
            }
          }
          InstallWorkspacePkgDep::Workspace { alias, nv } => {
            if let Some(id) = workspace_pkg_ids.get(nv) {
              dependencies.insert(alias.clone(), id.clone());
            }
          }
        }
      }
      let id = workspace_pkg_ids
        .get(&workspace_pkg.nv)
        .expect("workspace package id should exist")
        .clone();
      packages.push(WorkspaceLifecyclePackage {
        package: NpmResolutionPackage {
          id,
          copy_index: 0,
          system: NpmResolutionPackageSystemInfo::default(),
          dist: None,
          dependencies,
          optional_dependencies: Default::default(),
          optional_peer_dependencies: Default::default(),
          extra: Some(NpmPackageExtraInfo {
            scripts: workspace_pkg.scripts.clone(),
            ..Default::default()
          }),
          is_deprecated: false,
          has_bin: false,
          has_scripts: !workspace_pkg.scripts.is_empty(),
        },
        package_path: workspace_pkg.target_dir.clone(),
        scripts: workspace_pkg.scripts.clone(),
      });
    }
    Ok(packages)
  }

  #[allow(
    clippy::too_many_arguments,
    reason = "many parameters needed for package cloning"
  )]
  async fn clone_package<'a>(
    &'a self,
    package: &'a NpmResolutionPackage,
    dist: &'a deno_npm::registry::NpmPackageVersionDistInfo,
    package_path: PathBuf,
    install_reporter: Option<Arc<dyn crate::InstallReporter>>,
    extra_info_provider: Arc<CachedNpmPackageExtraInfoProvider>,
    bin_entries_to_setup: Rc<RefCell<BinEntries<'a, impl LocalNpmInstallSys>>>,
    lifecycle_scripts: Rc<RefCell<LifecycleScripts<'a, impl FsMetadata>>>,
    lifecycle_script_init_cwds: Rc<HashMap<NpmPackageId, Vec<PathBuf>>>,
    packages_with_deprecation_warnings: Arc<Mutex<Vec<(PackageNv, String)>>>,
  ) -> Result<(), JsErrorBox> {
    self
      .tarball_cache
      .ensure_package(&package.id.nv, dist)
      .await
      .map_err(JsErrorBox::from_err)?;
    let pb_guard = self.reporter.on_initializing(&package.id.nv.to_string());
    let _initialization_guard =
      install_reporter.as_ref().map(|install_reporter| {
        install_reporter.initializing(&package.id.nv);
        InitializingGuard {
          nv: package.id.nv.clone(),
          install_reporter: install_reporter.clone(),
        }
      });

    let cache_folder = self.npm_cache.package_folder_for_nv(&package.id.nv);

    let handle = crate::rt::spawn_blocking({
      let package_path = package_path.clone();
      let sys = self.sys.clone();
      move || {
        clone_dir_recursive(&sys, &cache_folder, &package_path)?;
        Ok::<_, SyncResolutionWithFsError>(())
      }
    });

    let needs_extra_from_disk = package.extra.is_none()
      || (package.has_scripts
        && package.extra.as_ref().is_some_and(|e| e.scripts.is_empty()));
    let extra =
      if (package.has_bin || package.has_scripts || package.is_deprecated)
        && needs_extra_from_disk
      {
        handle
          .await
          .map_err(JsErrorBox::from_err)?
          .map_err(JsErrorBox::from_err)?;
        extra_info_provider
          .get_package_extra_info(
            &package.id.nv,
            &package_path,
            ExpectedExtraInfo::from_package(package),
          )
          .await
          .map_err(JsErrorBox::from_err)?
      } else {
        let (result, extra) = futures::future::join(
          handle,
          std::future::ready(Ok::<_, JsErrorBox>(
            package.extra.clone().unwrap_or_default(),
          )),
        )
        .await;
        result
          .map_err(JsErrorBox::from_err)?
          .map_err(JsErrorBox::from_err)?;
        extra?
      };

    if package.has_bin {
      bin_entries_to_setup.borrow_mut().add(
        package,
        &extra,
        package_path.to_path_buf(),
      );
    }

    if package.has_scripts {
      let init_cwds = lifecycle_script_init_cwds
        .get(&package.id)
        .cloned()
        .unwrap_or_default();
      lifecycle_scripts.borrow_mut().add(
        package,
        &extra,
        package_path.into(),
        init_cwds,
      );
    }

    if package.is_deprecated
      && let Some(deprecated) = &extra.deprecated
    {
      packages_with_deprecation_warnings
        .lock()
        .push((package.id.nv.clone(), deprecated.clone()));
    }

    drop(pb_guard);
    Ok(())
  }
}

struct WorkspaceLifecyclePackage {
  package: NpmResolutionPackage,
  package_path: PathBuf,
  scripts: HashMap<deno_semver::SmallStackString, String>,
}

/// Resolves the on-disk location of a package version in the hoisted layout,
/// either as a top-level package or a nested one. Returns `None` if the version
/// isn't placed anywhere (for example a conflicting version that no package
/// depends on).
fn hoisted_package_path(
  layout: &HoistedLayout,
  root_node_modules_path: &Path,
  nv: &PackageNv,
) -> Option<PathBuf> {
  if let Some(top) = layout.top_level.get(&nv.name)
    && top.id.nv == *nv
  {
    return Some(join_package_name(
      Cow::Borrowed(root_node_modules_path),
      &nv.name,
    ));
  }
  for nested in &layout.nested {
    if nested.dep.id.nv == *nv {
      return Some(join_package_name(
        Cow::Owned(
          root_node_modules_path
            .join(&nested.parent_path)
            .join("node_modules"),
        ),
        &nv.name,
      ));
    }
  }
  None
}

fn resolve_remote_pkg_id(
  snapshot: &NpmResolutionSnapshot,
  req: &deno_semver::package::PackageReq,
) -> Option<NpmPackageId> {
  match snapshot.resolve_pkg_from_pkg_req(req) {
    Ok(pkg) => Some(pkg.id.clone()),
    Err(_) if req.version_req.tag().is_some() => None,
    Err(_) => snapshot.resolve_best_package_id(&req.name, &req.version_req),
  }
}

fn cleanup_hoisted_packages(
  sys: &impl LocalNpmInstallSys,
  root_node_modules_path: &Path,
  layout: &HoistedLayout,
) {
  let expected_names: HashSet<&str> =
    layout.top_level.keys().map(|k| k.as_str()).collect();

  fn remove_unexpected_package(
    sys: &impl LocalNpmInstallSys,
    path: &Path,
    name: &str,
    expected_names: &HashSet<&str>,
  ) {
    if expected_names.contains(name) {
      return;
    }
    let _ = sys.fs_remove_dir_all(path);
  }

  if let Ok(entries) = sys.fs_read_dir(root_node_modules_path) {
    for entry in entries.flatten() {
      let name = entry.file_name();
      let name_str = name.to_string_lossy();
      // Skip hidden dirs (.deno, .bin) and expected packages
      if name_str.starts_with('.') || expected_names.contains(name_str.as_ref())
      {
        continue;
      }
      if name_str.starts_with('@') {
        let scope_path = root_node_modules_path.join(&*name_str);
        let Ok(scope_entries) = sys.fs_read_dir(&scope_path) else {
          continue;
        };
        for scope_entry in scope_entries.flatten() {
          let package_name = scope_entry.file_name();
          let package_name = package_name.to_string_lossy();
          let full_name = format!("{name_str}/{package_name}");
          remove_unexpected_package(
            sys,
            &scope_entry.path(),
            &full_name,
            &expected_names,
          );
        }
        if sys
          .fs_read_dir(&scope_path)
          .map(|mut entries| entries.next().is_none())
          .unwrap_or(false)
        {
          let _ = sys.fs_remove_dir(&scope_path);
        }
        continue;
      }
      let path = root_node_modules_path.join(&*name_str);
      remove_unexpected_package(sys, &path, &name_str, &expected_names);
    }
  }
}

#[async_trait(?Send)]
impl<
  THttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: LocalNpmInstallSys,
> NpmPackageFsInstaller
  for HoistedNpmPackageInstaller<THttpClient, TReporter, TSys>
{
  async fn cache_packages<'a>(
    &self,
    caching: PackageCaching<'a>,
  ) -> Result<(), JsErrorBox> {
    let snapshot = match caching {
      PackageCaching::All => self.resolution.snapshot(),
      PackageCaching::Only(reqs) => self.resolution.subset(&reqs),
    };
    self
      .sync_resolution_with_fs(&snapshot)
      .await
      .map_err(JsErrorBox::from_err)
  }
}

struct HoistedLifecycleScripts {
  install_reporter: Option<Arc<dyn crate::InstallReporter>>,
}

impl LifecycleScriptsStrategy for HoistedLifecycleScripts {
  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, PathBuf)],
  ) -> Result<(), std::io::Error> {
    if !packages.is_empty() {
      use std::fmt::Write;
      let mut output = String::new();
      _ = writeln!(
        &mut output,
        "{} {}",
        colors::yellow("╭"),
        colors::yellow_bold("Warning")
      );
      _ = writeln!(&mut output, "{}", colors::yellow("│"));
      _ = writeln!(
        &mut output,
        "{}  Ignored build scripts for packages:",
        colors::yellow("│"),
      );
      for (package, _) in packages {
        _ = writeln!(
          &mut output,
          "{}  {}",
          colors::yellow("│"),
          colors::italic(format!("npm:{}", package.id.nv))
        );
      }
      _ = writeln!(&mut output, "{}", colors::yellow("│"));
      _ = writeln!(
        &mut output,
        "{}  Run \"{}\" to run build scripts.",
        colors::yellow("│"),
        colors::bold("deno approve-scripts")
      );
      _ = write!(&mut output, "{}", colors::yellow("╰─"));

      if let Some(install_reporter) = &self.install_reporter {
        install_reporter.scripts_not_run_warning(
          crate::lifecycle_scripts::LifecycleScriptsWarning::new(
            output,
            Box::new(|_sys| {}),
          ),
        );
      } else {
        log::info!("{}", output);
      }
    }
    Ok(())
  }

  fn has_warned(&self, _package: &NpmResolutionPackage) -> bool {
    // For hoisted mode, we don't persistently track this yet
    false
  }

  fn has_run(&self, _package: &NpmResolutionPackage) -> bool {
    // For hoisted mode, we don't persistently track this yet
    false
  }
}
