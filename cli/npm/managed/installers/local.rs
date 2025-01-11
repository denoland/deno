// Copyright 2018-2025 the Deno authors. MIT license.

//! Code for local node_modules resolution.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::Mutex;
use deno_error::JsErrorBox;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_resolver::npm::get_package_folder_id_folder_name;
use deno_resolver::npm::managed::NpmResolution;
use deno_semver::package::PackageNv;
use deno_semver::StackString;
use serde::Deserialize;
use serde::Serialize;

use super::common::bin_entries;
use super::common::NpmPackageFsInstaller;
use crate::args::LifecycleScriptsConfig;
use crate::args::NpmInstallDepsProvider;
use crate::cache::CACHE_PERM;
use crate::colors;
use crate::npm::managed::PackageCaching;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmTarballCache;
use crate::sys::CliSys;
use crate::util::fs::clone_dir_recursive;
use crate::util::fs::symlink_dir;
use crate::util::fs::LaxSingleProcessFsFlag;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressMessagePrompt;

/// Resolver that creates a local node_modules directory
/// and resolves packages from it.
#[derive(Debug)]
pub struct LocalNpmPackageInstaller {
  cache: Arc<CliNpmCache>,
  npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
  progress_bar: ProgressBar,
  resolution: Arc<NpmResolution>,
  sys: CliSys,
  tarball_cache: Arc<CliNpmTarballCache>,
  root_node_modules_path: PathBuf,
  system_info: NpmSystemInfo,
  lifecycle_scripts: LifecycleScriptsConfig,
}

impl LocalNpmPackageInstaller {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    cache: Arc<CliNpmCache>,
    npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
    progress_bar: ProgressBar,
    resolution: Arc<NpmResolution>,
    sys: CliSys,
    tarball_cache: Arc<CliNpmTarballCache>,
    node_modules_folder: PathBuf,
    system_info: NpmSystemInfo,
    lifecycle_scripts: LifecycleScriptsConfig,
  ) -> Self {
    Self {
      cache,
      npm_install_deps_provider,
      progress_bar,
      resolution,
      tarball_cache,
      sys,
      root_node_modules_path: node_modules_folder,
      system_info,
      lifecycle_scripts,
    }
  }
}

#[async_trait(?Send)]
impl NpmPackageFsInstaller for LocalNpmPackageInstaller {
  async fn cache_packages<'a>(
    &self,
    caching: PackageCaching<'a>,
  ) -> Result<(), JsErrorBox> {
    let snapshot = match caching {
      PackageCaching::All => self.resolution.snapshot(),
      PackageCaching::Only(reqs) => self.resolution.subset(&reqs),
    };
    sync_resolution_with_fs(
      &snapshot,
      &self.cache,
      &self.npm_install_deps_provider,
      &self.progress_bar,
      &self.tarball_cache,
      &self.root_node_modules_path,
      &self.sys,
      &self.system_info,
      &self.lifecycle_scripts,
    )
    .await
    .map_err(JsErrorBox::from_err)
  }
}

/// `node_modules/.deno/<package>/node_modules/<package_name>`
///
/// Where the actual package is stored.
fn local_node_modules_package_contents_path(
  local_registry_dir: &Path,
  package: &NpmResolutionPackage,
) -> PathBuf {
  local_registry_dir
    .join(get_package_folder_id_folder_name(
      &package.get_package_cache_folder_id(),
    ))
    .join("node_modules")
    .join(&package.id.nv.name)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SyncResolutionWithFsError {
  #[class(inherit)]
  #[error("Creating '{path}'")]
  Creating {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  CopyDirRecursive(#[from] crate::util::fs::CopyDirRecursiveError),
  #[class(inherit)]
  #[error(transparent)]
  SymlinkPackageDir(#[from] SymlinkPackageDirError),
  #[class(inherit)]
  #[error(transparent)]
  BinEntries(#[from] bin_entries::BinEntriesError),
  #[class(inherit)]
  #[error(transparent)]
  LifecycleScripts(
    #[from] super::common::lifecycle_scripts::LifecycleScriptsError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

/// Creates a pnpm style folder structure.
#[allow(clippy::too_many_arguments)]
async fn sync_resolution_with_fs(
  snapshot: &NpmResolutionSnapshot,
  cache: &Arc<CliNpmCache>,
  npm_install_deps_provider: &NpmInstallDepsProvider,
  progress_bar: &ProgressBar,
  tarball_cache: &Arc<CliNpmTarballCache>,
  root_node_modules_dir_path: &Path,
  sys: &CliSys,
  system_info: &NpmSystemInfo,
  lifecycle_scripts: &LifecycleScriptsConfig,
) -> Result<(), SyncResolutionWithFsError> {
  if snapshot.is_empty()
    && npm_install_deps_provider.workspace_pkgs().is_empty()
  {
    return Ok(()); // don't create the directory
  }

  // don't set up node_modules (and more importantly try to acquire the file lock)
  // if we're running as part of a lifecycle script
  if super::common::lifecycle_scripts::is_running_lifecycle_script() {
    return Ok(());
  }

  let deno_local_registry_dir = root_node_modules_dir_path.join(".deno");
  let deno_node_modules_dir = deno_local_registry_dir.join("node_modules");
  fs::create_dir_all(&deno_node_modules_dir).map_err(|source| {
    SyncResolutionWithFsError::Creating {
      path: deno_local_registry_dir.to_path_buf(),
      source,
    }
  })?;
  let bin_node_modules_dir_path = root_node_modules_dir_path.join(".bin");
  fs::create_dir_all(&bin_node_modules_dir_path).map_err(|source| {
    SyncResolutionWithFsError::Creating {
      path: deno_local_registry_dir.to_path_buf(),
      source,
    }
  })?;

  let single_process_lock = LaxSingleProcessFsFlag::lock(
    deno_local_registry_dir.join(".deno.lock"),
    // similar message used by cargo build
    "waiting for file lock on node_modules directory",
  )
  .await;

  // load this after we get the directory lock
  let mut setup_cache =
    SetupCache::load(deno_local_registry_dir.join(".setup-cache.bin"));

  let pb_clear_guard = progress_bar.clear_guard(); // prevent flickering

  // 1. Write all the packages out the .deno directory.
  //
  // Copy (hardlink in future) <global_registry_cache>/<package_id>/ to
  // node_modules/.deno/<package_folder_id_folder_name>/node_modules/<package_name>
  let package_partitions =
    snapshot.all_system_packages_partitioned(system_info);
  let mut cache_futures = FuturesUnordered::new();
  let mut newest_packages_by_name: HashMap<
    &StackString,
    &NpmResolutionPackage,
  > = HashMap::with_capacity(package_partitions.packages.len());
  let bin_entries = Rc::new(RefCell::new(bin_entries::BinEntries::new()));
  let mut lifecycle_scripts =
    super::common::lifecycle_scripts::LifecycleScripts::new(
      lifecycle_scripts,
      LocalLifecycleScripts {
        deno_local_registry_dir: &deno_local_registry_dir,
      },
    );
  let packages_with_deprecation_warnings = Arc::new(Mutex::new(Vec::new()));

  let mut package_tags: HashMap<&PackageNv, BTreeSet<&str>> = HashMap::new();
  for (package_req, package_nv) in snapshot.package_reqs() {
    if let Some(tag) = package_req.version_req.tag() {
      package_tags.entry(package_nv).or_default().insert(tag);
    }
  }

  for package in &package_partitions.packages {
    if let Some(current_pkg) =
      newest_packages_by_name.get_mut(&package.id.nv.name)
    {
      if current_pkg.id.nv.cmp(&package.id.nv) == Ordering::Less {
        *current_pkg = package;
      }
    } else {
      newest_packages_by_name.insert(&package.id.nv.name, package);
    };

    let package_folder_name =
      get_package_folder_id_folder_name(&package.get_package_cache_folder_id());
    let folder_path = deno_local_registry_dir.join(&package_folder_name);
    let tags = package_tags
      .get(&package.id.nv)
      .map(|tags| {
        capacity_builder::StringBuilder::<String>::build(|builder| {
          for (i, tag) in tags.iter().enumerate() {
            if i > 0 {
              builder.append(',')
            }
            builder.append(*tag);
          }
        })
        .unwrap()
      })
      .unwrap_or_default();
    enum PackageFolderState {
      UpToDate,
      Uninitialized,
      TagsOutdated,
    }
    let initialized_file = folder_path.join(".initialized");
    let package_state = std::fs::read_to_string(&initialized_file)
      .map(|s| {
        if s != tags {
          PackageFolderState::TagsOutdated
        } else {
          PackageFolderState::UpToDate
        }
      })
      .unwrap_or(PackageFolderState::Uninitialized);
    if !cache
      .cache_setting()
      .should_use_for_npm_package(&package.id.nv.name)
      || matches!(package_state, PackageFolderState::Uninitialized)
    {
      // cache bust the dep from the dep setup cache so the symlinks
      // are forced to be recreated
      setup_cache.remove_dep(&package_folder_name);

      let folder_path = folder_path.clone();
      let bin_entries_to_setup = bin_entries.clone();
      let packages_with_deprecation_warnings =
        packages_with_deprecation_warnings.clone();

      cache_futures.push(async move {
        tarball_cache
          .ensure_package(&package.id.nv, &package.dist)
          .await
          .map_err(JsErrorBox::from_err)?;
        let pb_guard = progress_bar.update_with_prompt(
          ProgressMessagePrompt::Initialize,
          &package.id.nv.to_string(),
        );
        let sub_node_modules = folder_path.join("node_modules");
        let package_path =
          join_package_name(&sub_node_modules, &package.id.nv.name);
        let cache_folder = cache.package_folder_for_nv(&package.id.nv);

        deno_core::unsync::spawn_blocking({
          let package_path = package_path.clone();
          let sys = sys.clone();
          move || {
            clone_dir_recursive(&sys, &cache_folder, &package_path)?;
            // write out a file that indicates this folder has been initialized
            fs::write(initialized_file, tags)?;

            Ok::<_, SyncResolutionWithFsError>(())
          }
        })
        .await
        .map_err(JsErrorBox::from_err)?
        .map_err(JsErrorBox::from_err)?;

        if package.bin.is_some() {
          bin_entries_to_setup.borrow_mut().add(package, package_path);
        }

        if let Some(deprecated) = &package.deprecated {
          packages_with_deprecation_warnings
            .lock()
            .push((package.id.clone(), deprecated.clone()));
        }

        // finally stop showing the progress bar
        drop(pb_guard); // explicit for clarity
        Ok::<_, JsErrorBox>(())
      });
    } else if matches!(package_state, PackageFolderState::TagsOutdated) {
      fs::write(initialized_file, tags)?;
    }

    let sub_node_modules = folder_path.join("node_modules");
    let package_path =
      join_package_name(&sub_node_modules, &package.id.nv.name);
    lifecycle_scripts.add(package, package_path.into());
  }

  while let Some(result) = cache_futures.next().await {
    result?; // surface the first error
  }

  // 2. Create any "copy" packages, which are used for peer dependencies
  for package in &package_partitions.copy_packages {
    let package_cache_folder_id = package.get_package_cache_folder_id();
    let destination_path = deno_local_registry_dir
      .join(get_package_folder_id_folder_name(&package_cache_folder_id));
    let initialized_file = destination_path.join(".initialized");
    if !initialized_file.exists() {
      let sub_node_modules = destination_path.join("node_modules");
      let package_path =
        join_package_name(&sub_node_modules, &package.id.nv.name);

      let source_path = join_package_name(
        &deno_local_registry_dir
          .join(get_package_folder_id_folder_name(
            &package_cache_folder_id.with_no_count(),
          ))
          .join("node_modules"),
        &package.id.nv.name,
      );

      clone_dir_recursive(sys, &source_path, &package_path)?;
      // write out a file that indicates this folder has been initialized
      fs::write(initialized_file, "")?;
    }
  }

  // 3. Symlink all the dependencies into the .deno directory.
  //
  // Symlink node_modules/.deno/<package_id>/node_modules/<dep_name> to
  // node_modules/.deno/<dep_id>/node_modules/<dep_package_name>
  for package in package_partitions.iter_all() {
    let package_folder_name =
      get_package_folder_id_folder_name(&package.get_package_cache_folder_id());
    let sub_node_modules = deno_local_registry_dir
      .join(&package_folder_name)
      .join("node_modules");
    let mut dep_setup_cache = setup_cache.with_dep(&package_folder_name);
    for (name, dep_id) in &package.dependencies {
      let dep = snapshot.package_from_id(dep_id).unwrap();
      if package.optional_dependencies.contains(name)
        && !dep.system.matches_system(system_info)
      {
        continue; // this isn't a dependency for the current system
      }
      let dep_cache_folder_id = dep.get_package_cache_folder_id();
      let dep_folder_name =
        get_package_folder_id_folder_name(&dep_cache_folder_id);
      if dep_setup_cache.insert(name, &dep_folder_name) {
        let dep_folder_path = join_package_name(
          &deno_local_registry_dir
            .join(dep_folder_name)
            .join("node_modules"),
          &dep_id.nv.name,
        );
        symlink_package_dir(
          &dep_folder_path,
          &join_package_name(&sub_node_modules, name),
        )?;
      }
    }
  }

  let mut found_names: HashMap<&StackString, &PackageNv> = HashMap::new();

  // set of node_modules in workspace packages that we've already ensured exist
  let mut existing_child_node_modules_dirs: HashSet<PathBuf> = HashSet::new();

  // 4. Create symlinks for package json dependencies
  {
    for remote in npm_install_deps_provider.remote_pkgs() {
      let remote_pkg = if let Ok(remote_pkg) =
        snapshot.resolve_pkg_from_pkg_req(&remote.req)
      {
        remote_pkg
      } else if remote.req.version_req.tag().is_some() {
        // couldn't find a match, and `resolve_best_package_id`
        // panics if you give it a tag
        continue;
      } else if let Some(remote_id) = snapshot
        .resolve_best_package_id(&remote.req.name, &remote.req.version_req)
      {
        snapshot.package_from_id(&remote_id).unwrap()
      } else {
        continue; // skip, package not found
      };
      let Some(remote_alias) = &remote.alias else {
        continue;
      };
      let alias_clashes = remote.req.name != *remote_alias
        && newest_packages_by_name.contains_key(remote_alias);
      let install_in_child = {
        // we'll install in the child if the alias is taken by another package, or
        // if there's already a package with the same name but different version
        // linked into the root
        match found_names.entry(remote_alias) {
          Entry::Occupied(nv) => {
            // alias to a different package (in case of duplicate aliases)
            // or the version doesn't match the version in the root node_modules
            alias_clashes || &remote_pkg.id.nv != *nv.get()
          }
          Entry::Vacant(entry) => {
            entry.insert(&remote_pkg.id.nv);
            alias_clashes
          }
        }
      };
      let target_folder_name = get_package_folder_id_folder_name(
        &remote_pkg.get_package_cache_folder_id(),
      );
      let local_registry_package_path = join_package_name(
        &deno_local_registry_dir
          .join(&target_folder_name)
          .join("node_modules"),
        &remote_pkg.id.nv.name,
      );
      if install_in_child {
        // symlink the dep into the package's child node_modules folder
        let dest_node_modules = remote.base_dir.join("node_modules");
        if !existing_child_node_modules_dirs.contains(&dest_node_modules) {
          fs::create_dir_all(&dest_node_modules).map_err(|source| {
            SyncResolutionWithFsError::Creating {
              path: dest_node_modules.clone(),
              source,
            }
          })?;
          existing_child_node_modules_dirs.insert(dest_node_modules.clone());
        }
        let mut dest_path = dest_node_modules;
        dest_path.push(remote_alias);

        symlink_package_dir(&local_registry_package_path, &dest_path)?;
      } else {
        // symlink the package into `node_modules/<alias>`
        if setup_cache
          .insert_root_symlink(&remote_pkg.id.nv.name, &target_folder_name)
        {
          symlink_package_dir(
            &local_registry_package_path,
            &join_package_name(root_node_modules_dir_path, remote_alias),
          )?;
        }
      }
    }
  }

  // 5. Create symlinks for the remaining top level packages in the node_modules folder.
  // (These may be present if they are not in the package.json dependencies)
  // Symlink node_modules/.deno/<package_id>/node_modules/<package_name> to
  // node_modules/<package_name>
  let mut ids = snapshot
    .top_level_packages()
    .filter(|f| !found_names.contains_key(&f.nv.name))
    .collect::<Vec<_>>();
  ids.sort_by(|a, b| b.cmp(a)); // create determinism and only include the latest version
  for id in ids {
    match found_names.entry(&id.nv.name) {
      Entry::Occupied(_) => {
        continue; // skip, already handled
      }
      Entry::Vacant(entry) => {
        entry.insert(&id.nv);
      }
    }
    let package = snapshot.package_from_id(id).unwrap();
    let target_folder_name =
      get_package_folder_id_folder_name(&package.get_package_cache_folder_id());
    if setup_cache.insert_root_symlink(&id.nv.name, &target_folder_name) {
      let local_registry_package_path = join_package_name(
        &deno_local_registry_dir
          .join(target_folder_name)
          .join("node_modules"),
        &id.nv.name,
      );

      symlink_package_dir(
        &local_registry_package_path,
        &join_package_name(root_node_modules_dir_path, &id.nv.name),
      )?;
    }
  }

  // 6. Create a node_modules/.deno/node_modules/<package-name> directory with
  // the remaining packages
  for package in newest_packages_by_name.values() {
    match found_names.entry(&package.id.nv.name) {
      Entry::Occupied(_) => {
        continue; // skip, already handled
      }
      Entry::Vacant(entry) => {
        entry.insert(&package.id.nv);
      }
    }

    let target_folder_name =
      get_package_folder_id_folder_name(&package.get_package_cache_folder_id());
    if setup_cache.insert_deno_symlink(&package.id.nv.name, &target_folder_name)
    {
      let local_registry_package_path = join_package_name(
        &deno_local_registry_dir
          .join(target_folder_name)
          .join("node_modules"),
        &package.id.nv.name,
      );

      symlink_package_dir(
        &local_registry_package_path,
        &join_package_name(&deno_node_modules_dir, &package.id.nv.name),
      )?;
    }
  }

  // 7. Set up `node_modules/.bin` entries for packages that need it.
  {
    let bin_entries = std::mem::take(&mut *bin_entries.borrow_mut());
    bin_entries.finish(
      snapshot,
      &bin_node_modules_dir_path,
      |setup_outcome| {
        match setup_outcome {
          bin_entries::EntrySetupOutcome::MissingEntrypoint {
            package,
            package_path,
            ..
          } if super::common::lifecycle_scripts::has_lifecycle_scripts(
            package,
            package_path,
          ) && lifecycle_scripts.can_run_scripts(&package.id.nv)
            && !lifecycle_scripts.has_run_scripts(package) =>
          {
            // ignore, it might get fixed when the lifecycle scripts run.
            // if not, we'll warn then
          }
          outcome => outcome.warn_if_failed(),
        }
      },
    )?;
  }

  // 8. Create symlinks for the workspace packages
  {
    // todo(dsherret): this is not exactly correct because it should
    // install correctly for a workspace (potentially in sub directories),
    // but this is good enough for a first pass
    for workspace in npm_install_deps_provider.workspace_pkgs() {
      let Some(workspace_alias) = &workspace.alias else {
        continue;
      };
      symlink_package_dir(
        &workspace.target_dir,
        &root_node_modules_dir_path.join(workspace_alias),
      )?;
    }
  }

  {
    let packages_with_deprecation_warnings =
      packages_with_deprecation_warnings.lock();
    if !packages_with_deprecation_warnings.is_empty() {
      log::warn!(
        "{} The following packages are deprecated:",
        colors::yellow("Warning")
      );
      let len = packages_with_deprecation_warnings.len();
      for (idx, (package_id, msg)) in
        packages_with_deprecation_warnings.iter().enumerate()
      {
        if idx != len - 1 {
          log::warn!(
            "┠─ {}",
            colors::gray(format!("npm:{:?} ({})", package_id, msg))
          );
        } else {
          log::warn!(
            "┖─ {}",
            colors::gray(format!("npm:{:?} ({})", package_id, msg))
          );
        }
      }
    }
  }

  lifecycle_scripts
    .finish(
      snapshot,
      &package_partitions.packages,
      root_node_modules_dir_path,
      progress_bar,
    )
    .await?;

  setup_cache.save();
  drop(single_process_lock);
  drop(pb_clear_guard);

  Ok(())
}

/// `node_modules/.deno/<package>/`
fn local_node_modules_package_folder(
  local_registry_dir: &Path,
  package: &NpmResolutionPackage,
) -> PathBuf {
  local_registry_dir.join(get_package_folder_id_folder_name(
    &package.get_package_cache_folder_id(),
  ))
}

struct LocalLifecycleScripts<'a> {
  deno_local_registry_dir: &'a Path,
}

impl<'a> LocalLifecycleScripts<'a> {
  /// `node_modules/.deno/<package>/.scripts-run`
  fn ran_scripts_file(&self, package: &NpmResolutionPackage) -> PathBuf {
    local_node_modules_package_folder(self.deno_local_registry_dir, package)
      .join(".scripts-run")
  }

  /// `node_modules/.deno/<package>/.scripts-warned`
  fn warned_scripts_file(&self, package: &NpmResolutionPackage) -> PathBuf {
    local_node_modules_package_folder(self.deno_local_registry_dir, package)
      .join(".scripts-warned")
  }
}

impl<'a> super::common::lifecycle_scripts::LifecycleScriptsStrategy
  for LocalLifecycleScripts<'a>
{
  fn package_path(&self, package: &NpmResolutionPackage) -> PathBuf {
    local_node_modules_package_contents_path(
      self.deno_local_registry_dir,
      package,
    )
  }

  fn did_run_scripts(
    &self,
    package: &NpmResolutionPackage,
  ) -> std::result::Result<(), std::io::Error> {
    std::fs::write(self.ran_scripts_file(package), "")?;
    Ok(())
  }

  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, std::path::PathBuf)],
  ) -> Result<(), std::io::Error> {
    if !packages.is_empty() {
      log::warn!("{} The following packages contained npm lifecycle scripts ({}) that were not executed:", colors::yellow("Warning"), colors::gray("preinstall/install/postinstall"));

      for (package, _) in packages {
        log::warn!("┠─ {}", colors::gray(format!("npm:{}", package.id.nv)));
      }

      log::warn!("┃");
      log::warn!(
        "┠─ {}",
        colors::italic("This may cause the packages to not work correctly.")
      );
      log::warn!("┖─ {}", colors::italic("To run lifecycle scripts, use the `--allow-scripts` flag with `deno install`:"));
      let packages_comma_separated = packages
        .iter()
        .map(|(p, _)| format!("npm:{}", p.id.nv))
        .collect::<Vec<_>>()
        .join(",");
      log::warn!(
        "   {}",
        colors::bold(format!(
          "deno install --allow-scripts={}",
          packages_comma_separated
        ))
      );

      for (package, _) in packages {
        let _ignore_err = fs::write(self.warned_scripts_file(package), "");
      }
    }
    Ok(())
  }

  fn has_warned(&self, package: &NpmResolutionPackage) -> bool {
    self.warned_scripts_file(package).exists()
  }

  fn has_run(&self, package: &NpmResolutionPackage) -> bool {
    self.ran_scripts_file(package).exists()
  }
}

// Uses BTreeMap to preserve the ordering of the elements in memory, to ensure
// the file generated from this datastructure is deterministic.
// See: https://github.com/denoland/deno/issues/24479
/// Represents a dependency at `node_modules/.deno/<package_id>/`
struct SetupCacheDep<'a> {
  previous: Option<&'a BTreeMap<String, String>>,
  current: &'a mut BTreeMap<String, String>,
}

impl<'a> SetupCacheDep<'a> {
  pub fn insert(&mut self, name: &str, target_folder_name: &str) -> bool {
    self
      .current
      .insert(name.to_string(), target_folder_name.to_string());
    if let Some(previous_target) = self.previous.and_then(|p| p.get(name)) {
      previous_target != target_folder_name
    } else {
      true
    }
  }
}

// Uses BTreeMap to preserve the ordering of the elements in memory, to ensure
// the file generated from this datastructure is deterministic.
// See: https://github.com/denoland/deno/issues/24479
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct SetupCacheData {
  root_symlinks: BTreeMap<String, String>,
  deno_symlinks: BTreeMap<String, String>,
  dep_symlinks: BTreeMap<String, BTreeMap<String, String>>,
}

/// It is very slow to try to re-setup the symlinks each time, so this will
/// cache what we've setup on the last run and only update what is necessary.
/// Obviously this could lead to issues if the cache gets out of date with the
/// file system, such as if the user manually deletes a symlink.
struct SetupCache {
  file_path: PathBuf,
  previous: Option<SetupCacheData>,
  current: SetupCacheData,
}

impl SetupCache {
  pub fn load(file_path: PathBuf) -> Self {
    let previous = std::fs::read(&file_path)
      .ok()
      .and_then(|data| bincode::deserialize(&data).ok());
    Self {
      file_path,
      previous,
      current: Default::default(),
    }
  }

  pub fn save(&self) -> bool {
    if let Some(previous) = &self.previous {
      if previous == &self.current {
        return false; // nothing to save
      }
    }

    bincode::serialize(&self.current).ok().and_then(|data| {
      atomic_write_file_with_retries(
        &CliSys::default(),
        &self.file_path,
        &data,
        CACHE_PERM,
      )
      .ok()
    });
    true
  }

  /// Inserts and checks for the existence of a root symlink
  /// at `node_modules/<package_name>` pointing to
  /// `node_modules/.deno/<package_id>/`
  pub fn insert_root_symlink(
    &mut self,
    name: &str,
    target_folder_name: &str,
  ) -> bool {
    self
      .current
      .root_symlinks
      .insert(name.to_string(), target_folder_name.to_string());
    if let Some(previous_target) = self
      .previous
      .as_ref()
      .and_then(|p| p.root_symlinks.get(name))
    {
      previous_target != target_folder_name
    } else {
      true
    }
  }

  /// Inserts and checks for the existence of a symlink at
  /// `node_modules/.deno/node_modules/<package_name>` pointing to
  /// `node_modules/.deno/<package_id>/`
  pub fn insert_deno_symlink(
    &mut self,
    name: &str,
    target_folder_name: &str,
  ) -> bool {
    self
      .current
      .deno_symlinks
      .insert(name.to_string(), target_folder_name.to_string());
    if let Some(previous_target) = self
      .previous
      .as_ref()
      .and_then(|p| p.deno_symlinks.get(name))
    {
      previous_target != target_folder_name
    } else {
      true
    }
  }

  pub fn remove_dep(&mut self, parent_name: &str) {
    if let Some(previous) = &mut self.previous {
      previous.dep_symlinks.remove(parent_name);
    }
  }

  pub fn with_dep(&mut self, parent_name: &str) -> SetupCacheDep<'_> {
    SetupCacheDep {
      previous: self
        .previous
        .as_ref()
        .and_then(|p| p.dep_symlinks.get(parent_name)),
      current: self
        .current
        .dep_symlinks
        .entry(parent_name.to_string())
        .or_default(),
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SymlinkPackageDirError {
  #[class(inherit)]
  #[error("Creating '{parent}'")]
  Creating {
    parent: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] std::io::Error),
  #[cfg(windows)]
  #[class(inherit)]
  #[error("Creating junction in node_modules folder")]
  FailedCreatingJunction {
    #[source]
    #[inherit]
    source: std::io::Error,
  },
}

fn symlink_package_dir(
  old_path: &Path,
  new_path: &Path,
) -> Result<(), SymlinkPackageDirError> {
  let new_parent = new_path.parent().unwrap();
  if new_parent.file_name().unwrap() != "node_modules" {
    // create the parent folder that will contain the symlink
    fs::create_dir_all(new_parent).map_err(|source| {
      SymlinkPackageDirError::Creating {
        parent: new_parent.to_path_buf(),
        source,
      }
    })?;
  }

  // need to delete the previous symlink before creating a new one
  let _ignore = fs::remove_dir_all(new_path);

  let old_path_relative =
    crate::util::path::relative_path(new_parent, old_path)
      .unwrap_or_else(|| old_path.to_path_buf());

  #[cfg(windows)]
  {
    junction_or_symlink_dir(&old_path_relative, old_path, new_path)
  }
  #[cfg(not(windows))]
  {
    symlink_dir(&crate::sys::CliSys::default(), &old_path_relative, new_path)
      .map_err(Into::into)
  }
}

#[cfg(windows)]
fn junction_or_symlink_dir(
  old_path_relative: &Path,
  old_path: &Path,
  new_path: &Path,
) -> Result<(), SymlinkPackageDirError> {
  static USE_JUNCTIONS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

  if USE_JUNCTIONS.load(std::sync::atomic::Ordering::Relaxed) {
    // Use junctions because they're supported on ntfs file systems without
    // needing to elevate privileges on Windows.
    // Note: junctions don't support relative paths, so we need to use the
    // absolute path here.
    return junction::create(old_path, new_path).map_err(|source| {
      SymlinkPackageDirError::FailedCreatingJunction { source }
    });
  }

  match symlink_dir(&crate::sys::CliSys::default(), old_path_relative, new_path)
  {
    Ok(()) => Ok(()),
    Err(symlink_err)
      if symlink_err.kind() == std::io::ErrorKind::PermissionDenied =>
    {
      USE_JUNCTIONS.store(true, std::sync::atomic::Ordering::Relaxed);
      junction::create(old_path, new_path).map_err(|source| {
        SymlinkPackageDirError::FailedCreatingJunction { source }
      })
    }
    Err(symlink_err) => {
      log::warn!(
        "{} Unexpected error symlinking node_modules: {symlink_err}",
        colors::yellow("Warning")
      );
      USE_JUNCTIONS.store(true, std::sync::atomic::Ordering::Relaxed);
      junction::create(old_path, new_path).map_err(|source| {
        SymlinkPackageDirError::FailedCreatingJunction { source }
      })
    }
  }
}

fn join_package_name(path: &Path, package_name: &str) -> PathBuf {
  let mut path = path.to_path_buf();
  // ensure backslashes are used on windows
  for part in package_name.split('/') {
    path = path.join(part);
  }
  path
}

#[cfg(test)]
mod test {
  use test_util::TempDir;

  use super::*;

  #[test]
  fn test_setup_cache() {
    let temp_dir = TempDir::new();
    let cache_bin_path = temp_dir.path().join("cache.bin").to_path_buf();
    let mut cache = SetupCache::load(cache_bin_path.clone());
    assert!(cache.insert_deno_symlink("package-a", "package-a@1.0.0"));
    assert!(cache.insert_root_symlink("package-a", "package-a@1.0.0"));
    assert!(cache
      .with_dep("package-a")
      .insert("package-b", "package-b@1.0.0"));
    assert!(cache.save());

    let mut cache = SetupCache::load(cache_bin_path.clone());
    assert!(!cache.insert_deno_symlink("package-a", "package-a@1.0.0"));
    assert!(!cache.insert_root_symlink("package-a", "package-a@1.0.0"));
    assert!(!cache
      .with_dep("package-a")
      .insert("package-b", "package-b@1.0.0"));
    assert!(!cache.save());
    assert!(cache.insert_root_symlink("package-b", "package-b@0.2.0"));
    assert!(cache.save());

    let mut cache = SetupCache::load(cache_bin_path);
    cache.remove_dep("package-a");
    assert!(cache
      .with_dep("package-a")
      .insert("package-b", "package-b@1.0.0"));
  }
}
