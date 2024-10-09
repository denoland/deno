// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

//! Code for local node_modules resolution.

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crate::args::LifecycleScriptsConfig;
use crate::colors;
use async_trait::async_trait;
use deno_ast::ModuleSpecifier;
use deno_cache_dir::npm::mixed_case_package_name_decode;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_resolver::npm::normalize_pkg_name_for_node_modules_deno_folder;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::NodePermissions;
use deno_semver::package::PackageNv;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::ReferrerNotFoundError;
use serde::Deserialize;
use serde::Serialize;

use crate::args::NpmInstallDepsProvider;
use crate::cache::CACHE_PERM;
use crate::util::fs::atomic_write_file_with_retries;
use crate::util::fs::canonicalize_path_maybe_not_exists_with_fs;
use crate::util::fs::clone_dir_recursive;
use crate::util::fs::symlink_dir;
use crate::util::fs::LaxSingleProcessFsFlag;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressMessagePrompt;

use super::super::cache::NpmCache;
use super::super::cache::TarballCache;
use super::super::resolution::NpmResolution;
use super::common::NpmPackageFsResolver;
use super::common::RegistryReadPermissionChecker;

/// Resolver that creates a local node_modules directory
/// and resolves packages from it.
#[derive(Debug)]
pub struct LocalNpmPackageResolver {
  cache: Arc<NpmCache>,
  fs: Arc<dyn deno_fs::FileSystem>,
  npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
  progress_bar: ProgressBar,
  resolution: Arc<NpmResolution>,
  tarball_cache: Arc<TarballCache>,
  root_node_modules_path: PathBuf,
  root_node_modules_url: Url,
  system_info: NpmSystemInfo,
  registry_read_permission_checker: RegistryReadPermissionChecker,
  lifecycle_scripts: LifecycleScriptsConfig,
}

impl LocalNpmPackageResolver {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    cache: Arc<NpmCache>,
    fs: Arc<dyn deno_fs::FileSystem>,
    npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
    progress_bar: ProgressBar,
    resolution: Arc<NpmResolution>,
    tarball_cache: Arc<TarballCache>,
    node_modules_folder: PathBuf,
    system_info: NpmSystemInfo,
    lifecycle_scripts: LifecycleScriptsConfig,
  ) -> Self {
    Self {
      cache,
      fs: fs.clone(),
      npm_install_deps_provider,
      progress_bar,
      resolution,
      tarball_cache,
      registry_read_permission_checker: RegistryReadPermissionChecker::new(
        fs,
        node_modules_folder.clone(),
      ),
      root_node_modules_url: Url::from_directory_path(&node_modules_folder)
        .unwrap(),
      root_node_modules_path: node_modules_folder,
      system_info,
      lifecycle_scripts,
    }
  }

  fn resolve_package_root(&self, path: &Path) -> PathBuf {
    let mut last_found = path;
    loop {
      let parent = last_found.parent().unwrap();
      if parent.file_name().unwrap() == "node_modules" {
        return last_found.to_path_buf();
      } else {
        last_found = parent;
      }
    }
  }

  fn resolve_folder_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<PathBuf>, std::io::Error> {
    let Some(relative_url) =
      self.root_node_modules_url.make_relative(specifier)
    else {
      return Ok(None);
    };
    if relative_url.starts_with("../") {
      return Ok(None);
    }
    // it's within the directory, so use it
    let Some(path) = specifier.to_file_path().ok() else {
      return Ok(None);
    };
    // Canonicalize the path so it's not pointing to the symlinked directory
    // in `node_modules` directory of the referrer.
    canonicalize_path_maybe_not_exists_with_fs(&path, self.fs.as_ref())
      .map(Some)
  }

  fn resolve_package_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<PathBuf>, AnyError> {
    let Some(local_path) = self.resolve_folder_for_specifier(specifier)? else {
      return Ok(None);
    };
    let package_root_path = self.resolve_package_root(&local_path);
    Ok(Some(package_root_path))
  }
}

#[async_trait(?Send)]
impl NpmPackageFsResolver for LocalNpmPackageResolver {
  fn root_dir_url(&self) -> &Url {
    &self.root_node_modules_url
  }

  fn node_modules_path(&self) -> Option<&Path> {
    Some(self.root_node_modules_path.as_ref())
  }

  fn maybe_package_folder(&self, id: &NpmPackageId) -> Option<PathBuf> {
    let cache_folder_id = self
      .resolution
      .resolve_pkg_cache_folder_id_from_pkg_id(id)?;
    // package is stored at:
    // node_modules/.deno/<package_cache_folder_id_folder_name>/node_modules/<package_name>
    Some(
      self
        .root_node_modules_path
        .join(".deno")
        .join(get_package_folder_id_folder_name(&cache_folder_id))
        .join("node_modules")
        .join(&cache_folder_id.nv.name),
    )
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    let maybe_local_path = self
      .resolve_folder_for_specifier(referrer)
      .map_err(|err| PackageFolderResolveIoError {
        package_name: name.to_string(),
        referrer: referrer.clone(),
        source: err,
      })?;
    let Some(local_path) = maybe_local_path else {
      return Err(
        ReferrerNotFoundError {
          referrer: referrer.clone(),
          referrer_extra: None,
        }
        .into(),
      );
    };
    let package_root_path = self.resolve_package_root(&local_path);
    let mut current_folder = package_root_path.as_path();
    while let Some(parent_folder) = current_folder.parent() {
      current_folder = parent_folder;
      let node_modules_folder = if current_folder.ends_with("node_modules") {
        Cow::Borrowed(current_folder)
      } else {
        Cow::Owned(current_folder.join("node_modules"))
      };

      let sub_dir = join_package_name(&node_modules_folder, name);
      if self.fs.is_dir_sync(&sub_dir) {
        return Ok(sub_dir);
      }

      if current_folder == self.root_node_modules_path {
        break;
      }
    }

    Err(
      PackageNotFoundError {
        package_name: name.to_string(),
        referrer: referrer.clone(),
        referrer_extra: None,
      }
      .into(),
    )
  }

  fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<NpmPackageCacheFolderId>, AnyError> {
    let Some(folder_path) =
      self.resolve_package_folder_from_specifier(specifier)?
    else {
      return Ok(None);
    };
    let folder_name = folder_path.parent().unwrap().to_string_lossy();
    Ok(get_package_folder_id_from_folder_name(&folder_name))
  }

  async fn cache_packages(&self) -> Result<(), AnyError> {
    sync_resolution_with_fs(
      &self.resolution.snapshot(),
      &self.cache,
      &self.npm_install_deps_provider,
      &self.progress_bar,
      &self.tarball_cache,
      &self.root_node_modules_path,
      &self.system_info,
      &self.lifecycle_scripts,
    )
    .await
  }

  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &'a Path,
  ) -> Result<Cow<'a, Path>, AnyError> {
    self
      .registry_read_permission_checker
      .ensure_registry_read_permission(permissions, path)
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

/// Creates a pnpm style folder structure.
#[allow(clippy::too_many_arguments)]
async fn sync_resolution_with_fs(
  snapshot: &NpmResolutionSnapshot,
  cache: &Arc<NpmCache>,
  npm_install_deps_provider: &NpmInstallDepsProvider,
  progress_bar: &ProgressBar,
  tarball_cache: &Arc<TarballCache>,
  root_node_modules_dir_path: &Path,
  system_info: &NpmSystemInfo,
  lifecycle_scripts: &LifecycleScriptsConfig,
) -> Result<(), AnyError> {
  if snapshot.is_empty()
    && npm_install_deps_provider.workspace_pkgs().is_empty()
  {
    return Ok(()); // don't create the directory
  }

  let deno_local_registry_dir = root_node_modules_dir_path.join(".deno");
  let deno_node_modules_dir = deno_local_registry_dir.join("node_modules");
  fs::create_dir_all(&deno_node_modules_dir).with_context(|| {
    format!("Creating '{}'", deno_local_registry_dir.display())
  })?;
  let bin_node_modules_dir_path = root_node_modules_dir_path.join(".bin");
  fs::create_dir_all(&bin_node_modules_dir_path).with_context(|| {
    format!("Creating '{}'", bin_node_modules_dir_path.display())
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
  let mut newest_packages_by_name: HashMap<&String, &NpmResolutionPackage> =
    HashMap::with_capacity(package_partitions.packages.len());
  let bin_entries =
    Rc::new(RefCell::new(super::common::bin_entries::BinEntries::new()));
  let mut lifecycle_scripts =
    super::common::lifecycle_scripts::LifecycleScripts::new(
      lifecycle_scripts,
      LocalLifecycleScripts {
        deno_local_registry_dir: &deno_local_registry_dir,
      },
    );
  let packages_with_deprecation_warnings = Arc::new(Mutex::new(Vec::new()));

  let mut package_tags: HashMap<&PackageNv, Vec<&str>> = HashMap::new();
  for (package_req, package_nv) in snapshot.package_reqs() {
    if let Some(tag) = package_req.version_req.tag() {
      package_tags.entry(package_nv).or_default().push(tag);
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
      .map(|tags| tags.join(","))
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
          .await?;
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
          move || {
            clone_dir_recursive(&cache_folder, &package_path)?;
            // write out a file that indicates this folder has been initialized
            fs::write(initialized_file, tags)?;

            Ok::<_, AnyError>(())
          }
        })
        .await??;

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
        Ok::<_, AnyError>(())
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

      clone_dir_recursive(&source_path, &package_path)?;
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

  let mut found_names: HashMap<&String, &PackageNv> = HashMap::new();

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
          fs::create_dir_all(&dest_node_modules).with_context(|| {
            format!("Creating '{}'", dest_node_modules.display())
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
    bin_entries.finish(snapshot, &bin_node_modules_dir_path)?;
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
      Some(root_node_modules_dir_path),
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
  ) -> std::result::Result<(), deno_core::anyhow::Error> {
    std::fs::write(self.ran_scripts_file(package), "")?;
    Ok(())
  }

  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, std::path::PathBuf)],
  ) -> Result<(), AnyError> {
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
      atomic_write_file_with_retries(&self.file_path, data, CACHE_PERM).ok()
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

fn get_package_folder_id_folder_name(
  folder_id: &NpmPackageCacheFolderId,
) -> String {
  let copy_str = if folder_id.copy_index == 0 {
    Cow::Borrowed("")
  } else {
    Cow::Owned(format!("_{}", folder_id.copy_index))
  };
  let nv = &folder_id.nv;
  let name = normalize_pkg_name_for_node_modules_deno_folder(&nv.name);
  format!("{}@{}{}", name, nv.version, copy_str)
}

fn get_package_folder_id_from_folder_name(
  folder_name: &str,
) -> Option<NpmPackageCacheFolderId> {
  let folder_name = folder_name.replace('+', "/");
  let (name, ending) = folder_name.rsplit_once('@')?;
  let name = if let Some(encoded_name) = name.strip_prefix('_') {
    mixed_case_package_name_decode(encoded_name)?
  } else {
    name.to_string()
  };
  let (raw_version, copy_index) = match ending.split_once('_') {
    Some((raw_version, copy_index)) => {
      let copy_index = copy_index.parse::<u8>().ok()?;
      (raw_version, copy_index)
    }
    None => (ending, 0),
  };
  let version = deno_semver::Version::parse_from_npm(raw_version).ok()?;
  Some(NpmPackageCacheFolderId {
    nv: PackageNv { name, version },
    copy_index,
  })
}

fn symlink_package_dir(
  old_path: &Path,
  new_path: &Path,
) -> Result<(), AnyError> {
  let new_parent = new_path.parent().unwrap();
  if new_parent.file_name().unwrap() != "node_modules" {
    // create the parent folder that will contain the symlink
    fs::create_dir_all(new_parent)
      .with_context(|| format!("Creating '{}'", new_parent.display()))?;
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
    symlink_dir(&old_path_relative, new_path).map_err(Into::into)
  }
}

#[cfg(windows)]
fn junction_or_symlink_dir(
  old_path_relative: &Path,
  old_path: &Path,
  new_path: &Path,
) -> Result<(), AnyError> {
  static USE_JUNCTIONS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

  if USE_JUNCTIONS.load(std::sync::atomic::Ordering::Relaxed) {
    // Use junctions because they're supported on ntfs file systems without
    // needing to elevate privileges on Windows.
    // Note: junctions don't support relative paths, so we need to use the
    // absolute path here.
    return junction::create(old_path, new_path)
      .context("Failed creating junction in node_modules folder");
  }

  match symlink_dir(old_path_relative, new_path) {
    Ok(()) => Ok(()),
    Err(symlink_err)
      if symlink_err.kind() == std::io::ErrorKind::PermissionDenied =>
    {
      USE_JUNCTIONS.store(true, std::sync::atomic::Ordering::Relaxed);
      junction::create(old_path, new_path).map_err(Into::into)
    }
    Err(symlink_err) => Err(
      AnyError::from(symlink_err)
        .context("Failed creating symlink in node_modules folder"),
    ),
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
  use deno_npm::NpmPackageCacheFolderId;
  use deno_semver::package::PackageNv;
  use test_util::TempDir;

  use super::*;

  #[test]
  fn test_get_package_folder_id_folder_name() {
    let cases = vec![
      (
        NpmPackageCacheFolderId {
          nv: PackageNv::from_str("@types/foo@1.2.3").unwrap(),
          copy_index: 1,
        },
        "@types+foo@1.2.3_1".to_string(),
      ),
      (
        NpmPackageCacheFolderId {
          nv: PackageNv::from_str("JSON@3.2.1").unwrap(),
          copy_index: 0,
        },
        "_jjju6tq@3.2.1".to_string(),
      ),
    ];
    for (input, output) in cases {
      assert_eq!(get_package_folder_id_folder_name(&input), output);
      let folder_id = get_package_folder_id_from_folder_name(&output).unwrap();
      assert_eq!(folder_id, input);
    }
  }

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
