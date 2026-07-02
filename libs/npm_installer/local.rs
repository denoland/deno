// Copyright 2018-2026 the Deno authors. MIT license.

//! Code for local node_modules resolution.

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Error as AnyError;
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
use deno_npm_cache::NpmCacheSys;
use deno_npm_cache::TarballCache;
use deno_npm_cache::hard_link_file;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_resolver::npm::get_package_folder_id_folder_name;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_semver::StackString;
use deno_semver::package::PackageNv;
use deno_terminal::colors;
use futures::FutureExt;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use parking_lot::Mutex;
use serde::Deserialize;
use serde::Serialize;
use sys_traits::FsDirEntry;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsWrite;
use sys_traits::PathsInErrorsExt;
use sys_traits::SysWithPathsInErrors;

use crate::BinEntries;
use crate::CachedNpmPackageExtraInfoProvider;
use crate::ExpectedExtraInfo;
use crate::LifecycleScriptsConfig;
use crate::NpmPackageExtraInfoProvider;
use crate::NpmPackageFsInstaller;
use crate::PackageCaching;
use crate::Reporter;
use crate::bin_entries::EntrySetupOutcome;
use crate::bin_entries::SetupBinEntrySys;
use crate::flag::LaxSingleProcessFsFlag;
use crate::flag::LaxSingleProcessFsFlagSys;
use crate::fs::CloneDirRecursiveSys;
use crate::fs::clone_dir_recursive;
use crate::fs::symlink_dir;
use crate::lifecycle_scripts::LifecycleScripts;
use crate::lifecycle_scripts::LifecycleScriptsExecutor;
use crate::lifecycle_scripts::LifecycleScriptsExecutorOptions;
use crate::lifecycle_scripts::LifecycleScriptsStrategy;
use crate::lifecycle_scripts::has_lifecycle_scripts;
use crate::lifecycle_scripts::is_running_lifecycle_script;
use crate::package_json::InstallWorkspacePkgDep;
use crate::package_json::NpmInstallDepsProvider;
use crate::process_state::NpmProcessState;

#[sys_traits::auto_impl]
pub trait LocalNpmInstallSys:
  NpmCacheSys
  + CloneDirRecursiveSys
  + SetupBinEntrySys
  + LaxSingleProcessFsFlagSys
  + sys_traits::EnvVar
  + sys_traits::FsSymlinkDir
  + sys_traits::FsCreateJunction
  + sys_traits::FsRemoveDir
{
}

#[derive(Debug)]
pub struct LocalNpmPackageInstallerOptions {
  pub clean_on_install: bool,
  pub lifecycle_scripts: Arc<LifecycleScriptsConfig>,
  pub node_modules_folder: PathBuf,
  pub reporter: Option<Arc<dyn crate::InstallReporter>>,
  pub system_info: NpmSystemInfo,
  /// Whether `jsr:` dependencies are being installed into `node_modules` via
  /// JSR's npm compatibility registry (the `jsrDepsInNodeModules` config
  /// option). Gates the `@scope/name` alias symlinks and the `.npmrc` write
  /// for `@jsr/*` packages.
  pub jsr_deps_in_node_modules: bool,
}

/// Resolver that creates a local node_modules directory
/// and resolves packages from it.
pub struct LocalNpmPackageInstaller<
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
  jsr_deps_in_node_modules: bool,
}

impl<
  THttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: LocalNpmInstallSys,
> std::fmt::Debug for LocalNpmPackageInstaller<THttpClient, TReporter, TSys>
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalNpmPackageInstaller")
      .field("npm_cache", &self.npm_cache)
      .field("npm_install_deps_provider", &self.npm_install_deps_provider)
      .field("reporter", &self.reporter)
      .field("resolution", &self.resolution)
      .field("sys", &self.sys)
      .field("tarball_cache", &self.tarball_cache)
      .field("clean_on_install", &self.clean_on_install)
      .field("lifecycle_scripts_config", &self.lifecycle_scripts_config)
      .field("root_node_modules_path", &self.root_node_modules_path)
      .field("system_info", &self.system_info)
      .finish()
  }
}

pub(crate) struct InitializingGuard {
  pub(crate) nv: PackageNv,
  pub(crate) install_reporter: Arc<dyn crate::InstallReporter>,
}

impl Drop for InitializingGuard {
  fn drop(&mut self) {
    self.install_reporter.initialized(&self.nv);
  }
}

impl<
  THttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: LocalNpmInstallSys,
> LocalNpmPackageInstaller<THttpClient, TReporter, TSys>
{
  #[allow(clippy::too_many_arguments, reason = "construction")]
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
      jsr_deps_in_node_modules: options.jsr_deps_in_node_modules,
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
    let deno_local_registry_dir = self.root_node_modules_path.join(".deno");
    if has_no_packages
      && (!self.clean_on_install
        || !self.sys.fs_exists_no_err(&deno_local_registry_dir))
    {
      return Ok(()); // don't create the directory
    }

    // don't set up node_modules (and more importantly try to acquire the file lock)
    // if we're running as part of a lifecycle script
    if is_running_lifecycle_script(&self.sys) {
      return Ok(());
    }

    let sys = self.sys.with_paths_in_errors();
    let deno_node_modules_dir = deno_local_registry_dir.join("node_modules");
    sys.fs_create_dir_all(&deno_node_modules_dir)?;
    let bin_node_modules_dir_path = self.root_node_modules_path.join(".bin");
    let single_process_lock = LaxSingleProcessFsFlag::lock(
      sys.as_ref(),
      deno_local_registry_dir.join(".deno.lock"),
      &self.reporter,
      // similar message used by cargo build
      "waiting for file lock on node_modules directory",
    )
    .await;

    let package_partitions =
      snapshot.all_system_packages_partitioned(&self.system_info);
    let pb_clear_guard = self.reporter.clear_guard(); // prevent flickering

    // load this after we get the directory lock
    let mut setup_cache = LocalSetupCache::load(
      sys.as_ref().clone(),
      deno_local_registry_dir.join(".setup-cache.bin"),
    );

    // 1. Check if packages changed and clean up if needed
    if self.clean_on_install {
      let root_folder_names =
        root_package_folder_names(snapshot, &self.npm_install_deps_provider);
      let packages_hash =
        calculate_packages_hash(&package_partitions, &root_folder_names);
      if setup_cache.packages_changed(packages_hash) {
        cleanup_unused_packages(
          sys.as_ref(),
          &self.root_node_modules_path,
          &deno_local_registry_dir,
          &package_partitions,
          &root_folder_names,
          &mut setup_cache,
        );
      }
      setup_cache.set_clean_packages_hash(packages_hash);
    }

    // 2. Write all the packages out the .deno directory.
    //
    // Copy (hardlink in future) <global_registry_cache>/<package_id>/ to
    // node_modules/.deno/<package_folder_id_folder_name>/node_modules/<package_name>
    let workspace_lifecycle_packages =
      self.resolve_workspace_lifecycle_packages(snapshot)?;
    let mut cache_futures = FuturesUnordered::new();
    let mut newest_packages_by_name: HashMap<
      &StackString,
      &NpmResolutionPackage,
    > = HashMap::with_capacity(package_partitions.packages.len());
    let bin_entries = Rc::new(RefCell::new(BinEntries::new(sys)));
    let lifecycle_scripts = Rc::new(RefCell::new(LifecycleScripts::new(
      sys.as_ref(),
      &self.lifecycle_scripts_config,
      LocalLifecycleScripts {
        sys: sys.as_ref(),
        deno_local_registry_dir: &deno_local_registry_dir,
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

      let package_folder_name = get_package_folder_id_folder_name(
        &package.get_package_cache_folder_id(),
      );
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
      let package_state = if tags.is_empty() {
        if sys.fs_exists_no_err(&initialized_file) {
          PackageFolderState::UpToDate
        } else {
          PackageFolderState::Uninitialized
        }
      } else {
        self
          .sys
          .fs_read_to_string(&initialized_file)
          .map(|s| {
            if s != tags {
              PackageFolderState::TagsOutdated
            } else {
              PackageFolderState::UpToDate
            }
          })
          .unwrap_or(PackageFolderState::Uninitialized)
      };
      if !self
        .npm_cache
        .cache_setting()
        .should_use_for_npm_package(&package.id.nv.name)
        || matches!(package_state, PackageFolderState::Uninitialized)
      {
        if let Some(dist) = &package.dist {
          // cache bust the dep from the dep setup cache so the symlinks
          // are forced to be recreated
          setup_cache.remove_dep(&package_folder_name);

          let folder_path = folder_path.clone();
          let packages_with_deprecation_warnings =
            packages_with_deprecation_warnings.clone();
          let extra_info_provider = extra_info_provider.clone();
          let lifecycle_scripts = lifecycle_scripts.clone();
          let bin_entries_to_setup = bin_entries.clone();
          let install_reporter = self.install_reporter.clone();
          let lifecycle_script_init_cwds = lifecycle_script_init_cwds.clone();

          cache_futures.push(
            async move {
              self
                .tarball_cache
                .ensure_package(&package.id.nv, dist)
                .await
                .map_err(JsErrorBox::from_err)?;
              let pb_guard =
                self.reporter.on_initializing(&package.id.nv.to_string());
              let _initialization_guard =
                install_reporter.as_ref().map(|install_reporter| {
                  install_reporter.initializing(&package.id.nv);
                  InitializingGuard {
                    nv: package.id.nv.clone(),
                    install_reporter: install_reporter.clone(),
                  }
                });
              let sub_node_modules = folder_path.join("node_modules");
              let package_path = join_package_name(
                Cow::Owned(sub_node_modules),
                &package.id.nv.name,
              );
              let cache_folder =
                self.npm_cache.package_folder_for_nv(&package.id.nv);

              let handle = crate::rt::spawn_blocking({
                let package_path = package_path.clone();
                let sys = self.sys.clone();
                move || {
                  clone_dir_recursive(&sys, &cache_folder, &package_path)?;
                  // write out a file that indicates this folder has been initialized
                  write_initialized_file(&sys, &initialized_file, &tags)?;

                  Ok::<_, SyncResolutionWithFsError>(())
                }
              });
              let needs_extra_from_disk = package.extra.is_none()
                // When using abbreviated packument format, has_scripts may
                // be true (from hasInstallScript) while extra.scripts is
                // empty. In that case, read from disk to get real scripts.
                || (package.has_scripts
                  && package
                    .extra
                    .as_ref()
                    .is_some_and(|e| e.scripts.is_empty()));
              let extra = if (package.has_bin
                || package.has_scripts
                || package.is_deprecated)
                && needs_extra_from_disk
              {
                // Wait for extraction to complete first, since
                // get_package_extra_info may read from the on-disk
                // package.json which doesn't exist until extraction finishes.
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

              // finally stop showing the progress bar
              drop(pb_guard); // explicit for clarity
              Ok::<_, JsErrorBox>(())
            }
            .boxed_local(),
          );
        }
      } else {
        if matches!(package_state, PackageFolderState::TagsOutdated) {
          write_initialized_file(sys.as_ref(), &initialized_file, &tags)?;
        }

        if package.has_bin || package.has_scripts {
          let bin_entries_to_setup = bin_entries.clone();
          let lifecycle_scripts = lifecycle_scripts.clone();
          let extra_info_provider = extra_info_provider.clone();
          let lifecycle_script_init_cwds = lifecycle_script_init_cwds.clone();
          let sub_node_modules = folder_path.join("node_modules");
          let package_path = join_package_name(
            Cow::Owned(sub_node_modules),
            &package.id.nv.name,
          );
          cache_futures.push(
            async move {
              let extra = extra_info_provider
                .get_package_extra_info(
                  &package.id.nv,
                  &package_path,
                  ExpectedExtraInfo::from_package(package),
                )
                .await
                .map_err(JsErrorBox::from_err)?;

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

              Ok(())
            }
            .boxed_local(),
          );
        }
      }
    }

    // Wait for all npm package installations to complete before applying patches
    // This prevents race conditions where npm packages could overwrite patch files
    while let Some(result) = cache_futures.next().await {
      result?; // surface the first error
    }

    // 3. Setup the patch packages
    for patch_pkg in self.npm_install_deps_provider.patch_pkgs() {
      // there might be multiple ids per package due to peer dep copy packages
      for id in snapshot.package_ids_for_nv(&patch_pkg.nv) {
        let package = snapshot.package_from_id(id).unwrap();
        let package_folder_name = get_package_folder_id_folder_name(
          &package.get_package_cache_folder_id(),
        );
        // node_modules/.deno/<package_folder_id_folder_name>/node_modules/<package_name> -> local package folder
        let target = join_package_name(
          Cow::Owned(
            deno_local_registry_dir
              .join(&package_folder_name)
              .join("node_modules"),
          ),
          &patch_pkg.nv.name,
        );

        cache_futures.push(
          async move {
            let from_path = patch_pkg.target_dir.clone();
            let sys = self.sys.clone();
            crate::rt::spawn_blocking({
              move || {
                clone_dir_recursive_except_node_modules_child(
                  &sys, &from_path, &target,
                )
              }
            })
            .await
            .map_err(JsErrorBox::from_err)?
            .map_err(JsErrorBox::from_err)?;
            Ok::<_, JsErrorBox>(())
          }
          .boxed_local(),
        );
      }
    }

    // copy packages copy from the main packages, so wait
    // until these are all done
    while let Some(result) = cache_futures.next().await {
      result?; // surface the first error
    }

    // 4. Create any "copy" packages, which are used for peer dependencies
    for package in &package_partitions.copy_packages {
      let package_cache_folder_id = package.get_package_cache_folder_id();
      let destination_path = deno_local_registry_dir
        .join(get_package_folder_id_folder_name(&package_cache_folder_id));
      let initialized_file = destination_path.join(".initialized");
      if !sys.fs_exists_no_err(&initialized_file) {
        let sub_node_modules = destination_path.join("node_modules");
        let package_path =
          join_package_name(Cow::Owned(sub_node_modules), &package.id.nv.name);
        let source_path = join_package_name(
          Cow::Owned(
            deno_local_registry_dir
              .join(get_package_folder_id_folder_name(
                &package_cache_folder_id.with_no_count(),
              ))
              .join("node_modules"),
          ),
          &package.id.nv.name,
        );

        cache_futures.push(
          async move {
            let sys = self.sys.clone();
            crate::rt::spawn_blocking(move || {
              clone_dir_recursive(&sys, &source_path, &package_path)
                .map_err(JsErrorBox::from_err)?;
              // write out a file that indicates this folder has been initialized
              create_initialized_file(&sys, &initialized_file)
                .map_err(JsErrorBox::from_err)?;
              Ok::<_, JsErrorBox>(())
            })
            .await
            .map_err(JsErrorBox::from_err)?
            .map_err(JsErrorBox::from_err)?;
            Ok::<_, JsErrorBox>(())
          }
          .boxed_local(),
        );
      }
    }

    while let Some(result) = cache_futures.next().await {
      result?; // surface the first error
    }
    drop(cache_futures);

    // 5. Symlink all the dependencies into the .deno directory.
    //
    // Symlink node_modules/.deno/<package_id>/node_modules/<dep_name> to
    // node_modules/.deno/<dep_id>/node_modules/<dep_package_name>
    for package in package_partitions.iter_all() {
      let package_folder_name = get_package_folder_id_folder_name(
        &package.get_package_cache_folder_id(),
      );
      let sub_node_modules = deno_local_registry_dir
        .join(&package_folder_name)
        .join("node_modules");
      let mut dep_setup_cache = setup_cache.with_dep(&package_folder_name);
      for (name, dep_id) in &package.dependencies {
        let dep = snapshot.package_from_id(dep_id).unwrap();
        if package.optional_dependencies.contains(name)
          && !dep.system.matches_system(&self.system_info)
        {
          continue; // this isn't a dependency for the current system
        }
        let dep_cache_folder_id = dep.get_package_cache_folder_id();
        let dep_folder_name =
          get_package_folder_id_folder_name(&dep_cache_folder_id);
        if package.dist.is_none()
          || dep_setup_cache.insert(name, &dep_folder_name)
        {
          let dep_folder_path = join_package_name(
            Cow::Owned(
              deno_local_registry_dir
                .join(dep_folder_name)
                .join("node_modules"),
            ),
            &dep_id.nv.name,
          );
          symlink_package_dir(
            sys.as_ref(),
            &dep_folder_path,
            &join_package_name(Cow::Borrowed(&sub_node_modules), name),
          )?;
        }
      }
    }

    let mut found_names: HashMap<&StackString, &PackageNv> = HashMap::new();

    // set of node_modules in workspace packages that we've already ensured exist
    let mut existing_child_node_modules_dirs: HashSet<PathBuf> = HashSet::new();

    // 6. Create symlinks for package json dependencies
    {
      for remote in self.npm_install_deps_provider.remote_pkgs() {
        let remote_pkg = match snapshot.resolve_pkg_from_pkg_req(&remote.req) {
          Ok(remote_pkg) => remote_pkg,
          _ => {
            if remote.req.version_req.tag().is_some() {
              // couldn't find a match, and `resolve_best_package_id`
              // panics if you give it a tag
              continue;
            } else {
              match snapshot.resolve_best_package_id(
                &remote.req.name,
                &remote.req.version_req,
              ) {
                Some(remote_id) => {
                  snapshot.package_from_id(&remote_id).unwrap()
                }
                _ => {
                  continue; // skip, package not found
                }
              }
            }
          }
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
          Cow::Owned(
            deno_local_registry_dir
              .join(&target_folder_name)
              .join("node_modules"),
          ),
          &remote_pkg.id.nv.name,
        );
        if install_in_child {
          // symlink the dep into the package's child node_modules folder
          let dest_node_modules = remote.base_dir.join("node_modules");
          if !existing_child_node_modules_dirs.contains(&dest_node_modules) {
            sys.fs_create_dir_all(&dest_node_modules)?;
            existing_child_node_modules_dirs.insert(dest_node_modules.clone());
          }
          let mut dest_path = dest_node_modules;
          dest_path.push(remote_alias);

          symlink_package_dir(
            sys.as_ref(),
            &local_registry_package_path,
            &dest_path,
          )?;
        } else {
          // symlink the package into `node_modules/<alias>`
          if setup_cache.insert_root_symlink(remote_alias, &target_folder_name)
          {
            symlink_package_dir(
              sys.as_ref(),
              &local_registry_package_path,
              &join_package_name(
                Cow::Borrowed(&self.root_node_modules_path),
                remote_alias,
              ),
            )?;
          }
        }
      }
    }

    // 7. Create symlinks for the remaining top level packages in the node_modules folder.
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
      let target_folder_name = get_package_folder_id_folder_name(
        &package.get_package_cache_folder_id(),
      );
      if setup_cache.insert_root_symlink(&id.nv.name, &target_folder_name) {
        let local_registry_package_path = join_package_name(
          Cow::Owned(
            deno_local_registry_dir
              .join(&target_folder_name)
              .join("node_modules"),
          ),
          &id.nv.name,
        );

        symlink_package_dir(
          sys.as_ref(),
          &local_registry_package_path,
          &join_package_name(
            Cow::Borrowed(&self.root_node_modules_path),
            &id.nv.name,
          ),
        )?;
      }

      // Additionally expose JSR npm-compat packages (`@jsr/scope__name`) under
      // their original `@scope/name` so external tooling resolves them like a
      // regular npm install (mirrors pnpm's `jsr:` alias symlink). This only
      // covers top-level packages; transitive `jsr:` deps resolve through their
      // `@jsr/...` name and don't need a root alias. The alias is skipped when a
      // real package already claims that root name, so we don't clobber it.
      // Only done when `jsrDepsInNodeModules` is enabled so projects that depend
      // on `npm:@jsr/...` packages directly are left untouched.
      if self.jsr_deps_in_node_modules
        && let Some(alias) = jsr_npm_name_to_original(&id.nv.name)
        && !newest_packages_by_name
          .contains_key(&StackString::from(alias.as_str()))
        && setup_cache.insert_root_symlink(&alias, &target_folder_name)
      {
        let local_registry_package_path = join_package_name(
          Cow::Owned(
            deno_local_registry_dir
              .join(&target_folder_name)
              .join("node_modules"),
          ),
          &id.nv.name,
        );
        symlink_package_dir(
          sys.as_ref(),
          &local_registry_package_path,
          &join_package_name(
            Cow::Borrowed(&self.root_node_modules_path),
            &alias,
          ),
        )?;
      }
    }

    // When `jsrDepsInNodeModules` is enabled and JSR packages were materialized
    // into `node_modules` (under their `@jsr/scope__name` npm-compat name), make
    // sure a `.npmrc` next to `node_modules` points the `@jsr` scope at JSR's
    // npm registry so external tooling (npm, pnpm, yarn) can resolve them too.
    // Gated on the config so we don't write into the project directory of users
    // that merely depend on `npm:@jsr/...` packages directly.
    if self.jsr_deps_in_node_modules
      && let Some(project_dir) = self.root_node_modules_path.parent()
      && package_partitions
        .packages
        .iter()
        .any(|package| package.id.nv.name.starts_with("@jsr/"))
    {
      ensure_jsr_npmrc(sys.as_ref(), project_dir)?;
    }

    // 8. Create a node_modules/.deno/node_modules/<package-name> directory with
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

      let target_folder_name = get_package_folder_id_folder_name(
        &package.get_package_cache_folder_id(),
      );
      if setup_cache
        .insert_deno_symlink(&package.id.nv.name, &target_folder_name)
      {
        let local_registry_package_path = join_package_name(
          Cow::Owned(
            deno_local_registry_dir
              .join(target_folder_name)
              .join("node_modules"),
          ),
          &package.id.nv.name,
        );

        symlink_package_dir(
          sys.as_ref(),
          &local_registry_package_path,
          &join_package_name(
            Cow::Borrowed(&deno_node_modules_dir),
            &package.id.nv.name,
          ),
        )?;
      }
    }

    // 9. Set up `node_modules/.bin` entries for packages that need it.
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
              // ignore, it might get fixed when the lifecycle scripts run.
              // if not, we'll warn then
            }
            outcome => outcome.warn_if_failed(),
          }
        },
      )?;
    }

    // 10. Create symlinks for the workspace packages
    {
      // todo(dsherret): this is not exactly correct because it should
      // install correctly for a workspace (potentially in sub directories),
      // but this is good enough for a first pass
      for pkg in self.npm_install_deps_provider.local_pkgs() {
        let Some(pkg_alias) = &pkg.alias else {
          continue;
        };
        symlink_package_dir(
          sys.as_ref(),
          &pkg.target_dir,
          &self.root_node_modules_path.join(pkg_alias),
        )?;
      }
    }

    // 11. Create a `node_modules` directory inside each workspace member and
    // symlink that member's direct dependencies into it. This mirrors how npm
    // and pnpm lay out workspaces so that native Node.js tooling (for example
    // `svelte-check`, `astro`, or `eslint` plugins) run from within a member
    // resolves the member's dependencies and sibling workspace members the same
    // way it would in a Node.js project. Without this, members share only the
    // workspace root's `node_modules`, which both hides missing dependencies
    // (shadow dependencies) and breaks tools that expect a local `node_modules`.
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
        if workspace_pkg.is_root {
          continue;
        }
        let member_node_modules = workspace_pkg.target_dir.join("node_modules");
        // Remove links to dependencies the member no longer declares (or to
        // sibling members that were removed) so they stop being resolvable,
        // mirroring how the root `node_modules` prunes stale links above. This
        // also covers a member that dropped all of its dependencies, which is
        // why it runs before the `deps.is_empty()` short-circuit.
        let keep_aliases: HashSet<&str> = workspace_pkg
          .deps
          .iter()
          .map(|dep| dep.alias().as_str())
          .collect();
        remove_stale_member_symlinks(
          sys.as_ref(),
          &member_node_modules,
          &keep_aliases,
        );
        // The member's direct npm dependencies that ship executables, gathered
        // while linking so their bins can be set up in the member's `.bin` once
        // every alias link exists.
        let mut bin_deps: Vec<MemberBinDep> = Vec::new();
        if !workspace_pkg.deps.is_empty() {
          let member_key = workspace_pkg.target_dir.to_string_lossy();
          let mut dep_cache = setup_cache.with_dep(&member_key);
          let mut created_dir = false;
          for dep in &workspace_pkg.deps {
            let (alias, target_path, cache_target) = match dep {
              InstallWorkspacePkgDep::Remote { alias, req } => {
                let Some(id) = resolve_remote_pkg_id(snapshot, req) else {
                  continue;
                };
                let package = snapshot.package_from_id(&id).unwrap();
                let target_folder_name = get_package_folder_id_folder_name(
                  &package.get_package_cache_folder_id(),
                );
                let local_registry_package_path = join_package_name(
                  Cow::Owned(
                    deno_local_registry_dir
                      .join(&target_folder_name)
                      .join("node_modules"),
                  ),
                  &package.id.nv.name,
                );
                if package.has_bin {
                  // Collected before the dep-cache short-circuit below so a
                  // dependency whose link is already current still contributes
                  // its bins (the alias link exists either way).
                  bin_deps.push(MemberBinDep {
                    package,
                    read_path: local_registry_package_path.clone(),
                    link_path: member_node_modules.join(alias.as_str()),
                  });
                }
                (alias, local_registry_package_path, target_folder_name)
              }
              InstallWorkspacePkgDep::Workspace { alias, nv } => {
                let Some(target_dir) = workspace_member_dirs.get(nv) else {
                  continue;
                };
                (alias, target_dir.to_path_buf(), nv.to_string())
              }
            };
            if !dep_cache.insert(alias, &cache_target) {
              continue;
            }
            if !created_dir {
              sys.fs_create_dir_all(&member_node_modules)?;
              created_dir = true;
            }
            symlink_package_dir(
              sys.as_ref(),
              &target_path,
              &member_node_modules.join(alias.as_str()),
            )?;
          }
        }
        // Populate (and prune) the member's `node_modules/.bin`. Runs even when
        // the member has no dependencies so a dropped last bin is cleaned up.
        setup_member_bin_entries(
          sys,
          snapshot,
          &extra_info_provider,
          &member_node_modules,
          &bin_deps,
        )
        .await?;
      }
    }

    for package in &workspace_lifecycle_packages {
      sys.fs_create_dir_all(local_node_modules_package_folder(
        &deno_local_registry_dir,
        &package.package,
      ))?;
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

    {
      let packages_with_deprecation_warnings =
        packages_with_deprecation_warnings.lock();
      if !packages_with_deprecation_warnings.is_empty() {
        use std::fmt::Write;
        let mut output = String::new();
        let _ = writeln!(
          &mut output,
          "{} {}",
          colors::yellow("╭"),
          colors::yellow_bold("Warning")
        );
        let _ = writeln!(&mut output, "{}", colors::yellow("│"));
        let _ = writeln!(
          &mut output,
          "{}  The following packages are deprecated:",
          colors::yellow("│"),
        );
        for (package_nv, msg) in packages_with_deprecation_warnings.iter() {
          let _ = writeln!(
            &mut output,
            "{}  {}",
            colors::yellow("│"),
            colors::gray(format!("npm:{:?} ({})", package_nv, msg))
          );
        }
        let _ = write!(&mut output, "{}", colors::yellow("╰─"));
        if let Some(install_reporter) = &self.install_reporter {
          install_reporter.deprecated_message(output);
        } else {
          log::warn!("{}", output);
        }
      }
    }

    let lifecycle_scripts_to_run = std::mem::replace(
      &mut *lifecycle_scripts.borrow_mut(),
      LifecycleScripts::new(
        sys.as_ref(),
        &self.lifecycle_scripts_config,
        LocalLifecycleScripts {
          sys: sys.as_ref(),
          deno_local_registry_dir: &deno_local_registry_dir,
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
        crate::process_state::NpmProcessStateLinkerMode::Isolated,
      )
      .as_serialized();

      self
        .lifecycle_scripts_executor
        .execute(LifecycleScriptsExecutorOptions {
          init_cwd: &self.lifecycle_scripts_config.initial_cwd,
          process_state: process_state.as_str(),
          root_node_modules_dir_path: &self.root_node_modules_path,
          on_ran_pkg_scripts: &|pkg| {
            create_initialized_file(
              sys.as_ref(),
              &ran_scripts_file(&deno_local_registry_dir, pkg),
            )
            .map_err(JsErrorBox::from_err)
          },
          snapshot,
          system_packages: &package_partitions.packages,
          additional_packages: &additional_packages,
          packages_with_scripts,
          extra_info_provider: &extra_info_provider,
        })
        .await
        .map_err(SyncResolutionWithFsError::LifecycleScripts)?
    }

    setup_cache.save();
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
}

struct WorkspaceLifecyclePackage {
  package: NpmResolutionPackage,
  package_path: PathBuf,
  scripts: HashMap<deno_semver::SmallStackString, String>,
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

#[async_trait(?Send)]
impl<
  THttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: LocalNpmInstallSys,
> NpmPackageFsInstaller
  for LocalNpmPackageInstaller<THttpClient, TReporter, TSys>
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

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SyncResolutionWithFsError {
  #[class(generic)]
  #[error(transparent)]
  LifecycleScripts(AnyError),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

pub(crate) fn clone_dir_recursive_except_node_modules_child(
  sys: &impl CloneDirRecursiveSys,
  from: &Path,
  to: &Path,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
  _ = sys.fs_remove_dir_all(to);
  sys.fs_create_dir_all(to)?;
  for entry in sys.fs_read_dir(from)? {
    let entry = entry?;
    if entry.file_name().to_str() == Some("node_modules") {
      continue; // ignore
    }
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      clone_dir_recursive_except_node_modules_child(
        sys.as_ref(),
        &new_from,
        &new_to,
      )?;
    } else if file_type.is_file() {
      hard_link_file(sys.as_ref(), &new_from, &new_to).or_else(|_| {
        // Remove any existing file first so the copy writes a new inode
        // rather than writing through a hardlinked destination, which
        // would corrupt the file at its other paths. Removing first also
        // breaks hardlinks to currently-executing binaries (ETXTBSY).
        let _ = sys.fs_remove_file(&new_to);
        sys.fs_copy(&new_from, &new_to).map(|_| ())
      })?;
    }
  }
  Ok(())
}

/// Reverse of JSR's npm-compatibility naming: `@jsr/scope__name` -> the
/// original JSR specifier name `@scope/name`. Returns `None` when `name` is not
/// a `@jsr/`-scoped package. Used to expose JSR packages installed via the npm
/// registry under their original names so external tooling (type checkers,
/// bundlers) can resolve them like pnpm/npm do.
fn jsr_npm_name_to_original(name: &str) -> Option<String> {
  let rest = name.strip_prefix("@jsr/")?;
  let (scope, pkg) = rest.split_once("__")?;
  Some(format!("@{scope}/{pkg}"))
}

/// Ensures the `.npmrc` next to the `node_modules` directory declares JSR's npm
/// compatibility registry (`@jsr:registry=https://npm.jsr.io`). This lets
/// external tooling (npm, pnpm, yarn) resolve the `@jsr/*` packages Deno
/// materialized into `node_modules`. A pre-existing `@jsr:registry` entry is
/// respected (left untouched); otherwise the line is appended to an existing
/// `.npmrc` or a new one is created.
fn ensure_jsr_npmrc<TSys: NpmCacheSys>(
  sys: &TSys,
  project_dir: &Path,
) -> Result<(), std::io::Error> {
  const JSR_REGISTRY_LINE: &str = "@jsr:registry=https://npm.jsr.io";
  const NPMRC_PERM: u32 = 0o644;

  let npmrc_path = project_dir.join(".npmrc");
  let existing = match sys.fs_read_to_string(&npmrc_path) {
    Ok(contents) => Some(contents.into_owned()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
    Err(err) => return Err(err),
  };
  if let Some(existing) = &existing
    && existing
      .lines()
      .any(|line| line.trim_start().starts_with("@jsr:registry"))
  {
    // Respect an existing `@jsr` registry configuration.
    return Ok(());
  }
  let mut contents = existing.unwrap_or_default();
  if !contents.is_empty() && !contents.ends_with('\n') {
    contents.push('\n');
  }
  contents.push_str(JSR_REGISTRY_LINE);
  contents.push('\n');
  atomic_write_file_with_retries(
    sys,
    &npmrc_path,
    contents.as_bytes(),
    NPMRC_PERM,
  )
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

/// `node_modules/.deno/<package>/.scripts-run`
fn ran_scripts_file(
  local_registry_dir: &Path,
  package: &NpmResolutionPackage,
) -> PathBuf {
  local_node_modules_package_folder(local_registry_dir, package)
    .join(".scripts-run")
}

struct LocalLifecycleScripts<'a, TSys: FsOpen + FsMetadata> {
  sys: &'a TSys,
  deno_local_registry_dir: &'a Path,
  install_reporter: Option<Arc<dyn crate::InstallReporter>>,
}

impl<TSys: FsOpen + FsMetadata> LocalLifecycleScripts<'_, TSys> {
  /// `node_modules/.deno/<package>/.scripts-warned`
  fn warned_scripts_file(&self, package: &NpmResolutionPackage) -> PathBuf {
    local_node_modules_package_folder(self.deno_local_registry_dir, package)
      .join(".scripts-warned")
  }
}

impl<TSys: FsOpen + FsMetadata> LifecycleScriptsStrategy
  for LocalLifecycleScripts<'_, TSys>
{
  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, std::path::PathBuf)],
  ) -> Result<(), std::io::Error> {
    use std::fmt::Write;
    let mut output = String::new();

    if !packages.is_empty() {
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
        let paths = packages
          .iter()
          .map(|(package, _)| self.warned_scripts_file(package))
          .collect::<Vec<_>>();
        install_reporter.scripts_not_run_warning(
          crate::lifecycle_scripts::LifecycleScriptsWarning::new(
            output,
            Box::new(move |sys| {
              for path in paths {
                let _ignore_err = create_initialized_file(sys, &path);
              }
            }),
          ),
        );
      } else {
        log::info!("{}", output);
        for (package, _) in packages {
          let _ignore_err = create_initialized_file(
            self.sys,
            &self.warned_scripts_file(package),
          );
        }
      }
    }
    Ok(())
  }

  fn has_warned(&self, package: &NpmResolutionPackage) -> bool {
    self.sys.fs_exists_no_err(self.warned_scripts_file(package))
  }

  fn has_run(&self, package: &NpmResolutionPackage) -> bool {
    self
      .sys
      .fs_exists_no_err(ran_scripts_file(self.deno_local_registry_dir, package))
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

impl SetupCacheDep<'_> {
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
  #[serde(default)]
  clean_packages_hash: u64,
}

/// It is very slow to try to re-setup the symlinks each time, so this will
/// cache what we've setup on the last run and only update what is necessary.
/// Obviously this could lead to issues if the cache gets out of date with the
/// file system, such as if the user manually deletes a symlink.
pub struct LocalSetupCache<TSys: NpmCacheSys> {
  file_path: PathBuf,
  previous: Option<SetupCacheData>,
  current: SetupCacheData,
  sys: TSys,
}

impl<TSys: NpmCacheSys> LocalSetupCache<TSys> {
  pub fn load(sys: TSys, file_path: PathBuf) -> Self {
    let previous = sys
      .fs_read(&file_path)
      .ok()
      .and_then(|data| bincode::deserialize(&data).ok());
    Self {
      file_path,
      previous,
      current: Default::default(),
      sys,
    }
  }

  pub fn save(&self) -> bool {
    if let Some(previous) = &self.previous
      && previous == &self.current
    {
      return false; // nothing to save
    }

    const CACHE_PERM: u32 = 0o644;
    bincode::serialize(&self.current).ok().and_then(|data| {
      atomic_write_file_with_retries(
        &self.sys,
        &self.file_path,
        &data,
        CACHE_PERM,
      )
      .ok()
    });
    true
  }

  pub fn remove_root_symlink(&mut self, name: &str) {
    self.current.root_symlinks.remove(name);
  }

  pub fn remove_deno_symlink(&mut self, name: &str) {
    self.current.deno_symlinks.remove(name);
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

  fn with_dep(&mut self, parent_name: &str) -> SetupCacheDep<'_> {
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

  /// Checks if the packages have changed since the last setup
  pub fn packages_changed(&self, current_hash: u64) -> bool {
    self
      .previous
      .as_ref()
      .map(|p| p.clean_packages_hash != current_hash)
      .unwrap_or(true) // If no previous cache, consider it changed
  }

  pub fn clear_previous(&mut self) {
    self.previous = None;
  }

  /// Updates the packages hash in the current cache
  pub fn set_clean_packages_hash(&mut self, hash: u64) {
    self.current.clean_packages_hash = hash;
  }
}

pub(crate) fn symlink_package_dir(
  sys: &(
     impl sys_traits::FsSymlinkDir
     + sys_traits::FsRemoveDir
     + sys_traits::FsRemoveFile
     + sys_traits::FsRemoveDirAll
     + sys_traits::FsCreateDirAll
     + sys_traits::FsCreateJunction
   ),
  old_path: &Path,
  new_path: &Path,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
  let new_parent = new_path.parent().unwrap();
  if new_parent.file_name().unwrap() != "node_modules" {
    // create the parent folder that will contain the symlink
    sys.fs_create_dir_all(new_parent)?
  }

  // need to delete the previous symlink before creating a new one
  let _ignore = remove_existing_entry(sys.as_ref(), new_path);

  let old_path_relative = relative_path(new_parent, old_path)
    .unwrap_or_else(|| old_path.to_path_buf());

  if sys_traits::impls::is_windows() {
    junction_or_symlink_dir(
      sys.as_ref(),
      &old_path_relative,
      old_path,
      new_path,
    )
  } else {
    symlink_dir(sys.as_ref(), &old_path_relative, new_path)
  }
}

fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
  pathdiff::diff_paths(to, from)
}

/// Removes whatever currently exists at `path` (file, directory, symlink, or
/// junction) so a fresh symlink/junction can be created in its place. A stale
/// entry can be left behind when a `node_modules` directory is reused across
/// runs, for example when it's restored from a CI cache.
fn remove_existing_entry(
  sys: &(
     impl sys_traits::FsRemoveDir
     + sys_traits::FsRemoveFile
     + sys_traits::FsRemoveDirAll
   ),
  path: &Path,
) -> Result<(), std::io::Error> {
  let is_not_found =
    |err: &std::io::Error| err.kind() == std::io::ErrorKind::NotFound;
  // First try the syscall appropriate for the entry's actual type. Its error
  // is discarded: if it fails we fall through to the recursive removal below
  // and surface that error instead, since it's the last and most complete
  // attempt.
  if sys_traits::impls::is_windows() {
    // On Windows a directory symlink or junction must be removed with
    // RemoveDirectory rather than DeleteFile, and `remove_dir_all` may fail on
    // a dangling directory symlink, so remove the link itself first.
    match sys.fs_remove_dir(path) {
      Ok(()) => return Ok(()),
      Err(err) if is_not_found(&err) => return Ok(()),
      Err(_) => {}
    }
    // It may instead be a file symlink, which needs DeleteFile.
    match sys.fs_remove_file(path) {
      Ok(()) => return Ok(()),
      Err(err) if is_not_found(&err) => return Ok(()),
      Err(_) => {}
    }
  } else {
    // On Unix unlinking a symlink does not follow it.
    match sys.fs_remove_file(path) {
      Ok(()) => return Ok(()),
      Err(err) if is_not_found(&err) => return Ok(()),
      Err(_) => {}
    }
  }
  // Fall back to removing a real (possibly non-empty) directory, surfacing
  // this error if even the recursive removal fails.
  match sys.fs_remove_dir_all(path) {
    Ok(()) => Ok(()),
    Err(err) if is_not_found(&err) => Ok(()),
    Err(err) => Err(err),
  }
}

/// Removes managed dependency links from a workspace member's `node_modules`
/// whose alias is no longer one of the member's current direct dependencies
/// (`keep_aliases`). Without this a dependency dropped from the member's
/// `package.json`, or a sibling workspace member that was removed, would leave a
/// stale link behind and stay resolvable from the member, recreating exactly the
/// shadow-dependency problem the per-member layout is meant to prevent. This is
/// the per-member counterpart to the root `node_modules` pruning done by
/// [`remove_unused_node_modules_symlinks`].
///
/// Only symlinks and junctions are touched; real files or directories a user
/// placed in the member's `node_modules` are left untouched.
pub(crate) fn remove_stale_member_symlinks<TSys: LocalNpmInstallSys>(
  sys: &TSys,
  member_node_modules: &Path,
  keep_aliases: &HashSet<&str>,
) {
  let paths_sys = sys.with_paths_in_errors();
  let entries = match paths_sys.fs_read_dir(member_node_modules) {
    Ok(entries) => entries,
    // The directory may not exist yet (first install) or be unreadable; either
    // way there is nothing to prune.
    Err(_) => return,
  };
  for entry in entries.flatten() {
    let file_name = entry.file_name();
    let Some(name) = file_name.to_str() else {
      continue;
    };
    // Leave `.bin`, `.deno`, and any other dot-entries alone: they are not the
    // per-dependency alias links this loop manages.
    if name.starts_with('.') {
      continue;
    }
    let entry_path = member_node_modules.join(&file_name);
    if name.starts_with('@') {
      // Scoped packages are linked one level deeper as `@scope/<pkg>`, so the
      // alias to match against is the full `@scope/<pkg>` path.
      if let Ok(children) = paths_sys.fs_read_dir(&entry_path) {
        for child in children.flatten() {
          let child_file_name = child.file_name();
          let Some(child_name) = child_file_name.to_str() else {
            continue;
          };
          let child_path = entry_path.join(&child_file_name);
          let alias = format!("{name}/{child_name}");
          if paths_sys.fs_read_link(&child_path).is_ok()
            && !keep_aliases.contains(alias.as_str())
          {
            let _ignore = remove_existing_entry(sys, &child_path);
          }
        }
      }
      // Drop the scope directory if pruning emptied it.
      if paths_sys
        .fs_read_dir(&entry_path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
      {
        let _ignore = paths_sys.fs_remove_dir(&entry_path);
      }
    } else if paths_sys.fs_read_link(&entry_path).is_ok()
      && !keep_aliases.contains(name)
    {
      let _ignore = remove_existing_entry(sys, &entry_path);
    }
  }
}

/// A workspace member's direct dependency that may contribute executables to
/// the member's `node_modules/.bin`.
pub(crate) struct MemberBinDep<'a> {
  /// The resolved npm package, used for its `bin` metadata.
  pub package: &'a NpmResolutionPackage,
  /// Where the package's `package.json` is read from: its real location in the
  /// layout (the `.deno` store path for the isolated linker, or the hoisted
  /// package directory for the hoisted linker).
  pub read_path: PathBuf,
  /// The member's own link to the package
  /// (`<member>/node_modules/<alias>`). The generated shim points here so a
  /// tool invoked as `node_modules/.bin/<tool>` from within the member resolves
  /// the member's copy.
  pub link_path: PathBuf,
}

/// Sets up (and prunes) a workspace member's `node_modules/.bin`, mirroring how
/// npm and pnpm populate each member's local `.bin` so tools invoked as
/// `node_modules/.bin/<tool>` from within the member (for example `eslint`,
/// `svelte-check`, or `astro`) resolve the member's own dependencies.
///
/// Any pre-existing entries are cleared first so an executable from a
/// dependency the member dropped stops resolving. This mirrors how the root
/// `.bin` is rebuilt, and it is the per-member counterpart that the
/// symlink-only [`remove_stale_member_symlinks`] cannot provide: on Windows the
/// shims are plain files (`<tool>`, `<tool>.cmd`, `<tool>.ps1`) rather than
/// symlinks.
///
/// Sibling workspace members are not npm packages and so are not included in
/// `bin_deps`; their executables are not linked into a member's `.bin` yet.
pub(crate) async fn setup_member_bin_entries<'a, TSys: LocalNpmInstallSys>(
  sys: SysWithPathsInErrors<'a, TSys>,
  snapshot: &'a NpmResolutionSnapshot,
  extra_info_provider: &CachedNpmPackageExtraInfoProvider,
  member_node_modules: &Path,
  bin_deps: &[MemberBinDep<'a>],
) -> Result<(), SyncResolutionWithFsError> {
  let member_bin_dir = member_node_modules.join(".bin");
  // Clear any existing `.bin` entries before re-creating them so executables
  // from dropped dependencies don't linger. Runs unconditionally (even with no
  // bins to add) so a member that dropped its last bin dependency is cleaned up
  // too. A member's `.bin` is small, so rebuilding it each install is cheap.
  if let Ok(entries) = sys.fs_read_dir(&member_bin_dir) {
    for entry in entries.flatten() {
      let Ok(file_type) = entry.file_type() else {
        continue;
      };
      if file_type.is_file() {
        let _ignore = sys.fs_remove_file(entry.path());
      } else {
        let _ignore = sys.fs_remove_dir_all(entry.path());
      }
    }
  }

  let mut bin_entries = BinEntries::new(sys);
  for dep in bin_deps {
    if !dep.package.has_bin {
      continue;
    }
    // Cached from the root setup that ran earlier, so this is a map lookup
    // rather than a disk read in the common case.
    let extra = extra_info_provider
      .get_package_extra_info(
        &dep.package.id.nv,
        &dep.read_path,
        ExpectedExtraInfo::from_package(dep.package),
      )
      .await
      .map_err(SyncResolutionWithFsError::Other)?;
    bin_entries.add(dep.package, &extra, dep.link_path.clone());
  }
  // Ignore setup failures here: every package linked into a member is also
  // linked at the root, whose `.bin` setup already reports a missing entrypoint
  // or a not-yet-run lifecycle script. Warning again per member would be noise.
  bin_entries.finish(snapshot, &member_bin_dir, |_outcome| {})?;
  Ok(())
}

/// Runs `create`, and if it fails because something already exists at `path`,
/// removes that entry and tries once more. This makes creating a symlink or
/// junction resilient to a stale entry left over from a previous run.
fn create_retry_if_exists(
  sys: &(
     impl sys_traits::FsRemoveDir
     + sys_traits::FsRemoveFile
     + sys_traits::FsRemoveDirAll
   ),
  path: &Path,
  mut create: impl FnMut() -> Result<(), std::io::Error>,
) -> Result<(), std::io::Error> {
  match create() {
    Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
      remove_existing_entry(sys, path)?;
      create()
    }
    result => result,
  }
}

fn junction_or_symlink_dir(
  sys: &(
     impl sys_traits::FsSymlinkDir
     + sys_traits::FsCreateJunction
     + sys_traits::FsRemoveDir
     + sys_traits::FsRemoveFile
     + sys_traits::FsRemoveDirAll
   ),
  old_path_relative: &Path,
  old_path: &Path,
  new_path: &Path,
) -> Result<(), std::io::Error> {
  static USE_JUNCTIONS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

  let sys = sys.with_paths_in_errors();

  // Use junctions because they're supported on ntfs file systems without
  // needing to elevate privileges on Windows.
  // Note: junctions don't support relative paths, so we need to use the
  // absolute path here.
  let create_junction = || {
    create_retry_if_exists(sys.as_ref(), new_path, || {
      sys.fs_create_junction(old_path, new_path)
    })
  };

  if USE_JUNCTIONS.load(std::sync::atomic::Ordering::Relaxed) {
    return create_junction();
  }

  match create_retry_if_exists(sys.as_ref(), new_path, || {
    symlink_dir(sys.as_ref(), old_path_relative, new_path)
  }) {
    Ok(()) => Ok(()),
    Err(symlink_err)
      if symlink_err.kind() == std::io::ErrorKind::PermissionDenied =>
    {
      USE_JUNCTIONS.store(true, std::sync::atomic::Ordering::Relaxed);
      create_junction()
    }
    Err(symlink_err) => {
      log::warn!(
        "{} Unexpected error symlinking node_modules: {symlink_err}",
        colors::yellow("Warning")
      );
      USE_JUNCTIONS.store(true, std::sync::atomic::Ordering::Relaxed);
      create_junction()
    }
  }
}

fn write_initialized_file(
  sys: &(impl FsWrite + FsOpen),
  path: &Path,
  text: &str,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
  if text.is_empty() {
    // one less syscall
    create_initialized_file(sys.as_ref(), path)
  } else {
    sys.fs_write(path, text)
  }
}

fn create_initialized_file<F: sys_traits::boxed::FsOpenBoxed + ?Sized>(
  sys: &F,
  path: &Path,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
  sys
    .fs_open_boxed(path, &sys_traits::OpenOptions::new_write())
    .map(|_| ())
}

pub(crate) fn join_package_name(
  mut path: Cow<'_, Path>,
  package_name: &str,
) -> PathBuf {
  // ensure backslashes are used on windows
  for part in package_name.split('/') {
    match path {
      Cow::Borrowed(inner) => path = Cow::Owned(inner.join(part)),
      Cow::Owned(ref mut path) => {
        path.push(part);
      }
    }
  }
  path.into_owned()
}

/// Calculates a hash of the current package set for change detection.
/// This allows us to detect when npm packages have been added, removed, or changed.
fn calculate_packages_hash(
  package_partitions: &deno_npm::resolution::NpmPackagesPartitioned,
  root_folder_names: &BTreeSet<String>,
) -> u64 {
  use std::hash::Hash;
  use std::hash::Hasher;

  let mut hasher = twox_hash::XxHash64::default();

  // Hash all package IDs (iter_all is deterministic)
  for package in package_partitions.iter_all() {
    package.id.hash(&mut hasher);
  }

  // also hash the packages expected at the root of node_modules so that
  // cleanup runs when a direct dependency is removed but stays in the
  // resolution as a transitive dependency
  for folder_name in root_folder_names {
    folder_name.hash(&mut hasher);
  }

  hasher.finish()
}

/// Calculates the set of package folder names that are expected to have a
/// symlink at the root of the node_modules directory (resolved package.json
/// and import map dependencies plus the snapshot's top level packages).
fn root_package_folder_names(
  snapshot: &NpmResolutionSnapshot,
  npm_install_deps_provider: &NpmInstallDepsProvider,
) -> BTreeSet<String> {
  let mut folder_names = BTreeSet::new();
  for id in snapshot.top_level_packages() {
    if let Some(package) = snapshot.package_from_id(id) {
      folder_names.insert(get_package_folder_id_folder_name(
        &package.get_package_cache_folder_id(),
      ));
    }
  }
  // resolve the same way as when creating the root symlinks for
  // package.json dependencies
  for remote in npm_install_deps_provider.remote_pkgs() {
    let package = match snapshot.resolve_pkg_from_pkg_req(&remote.req) {
      Ok(package) => package,
      _ => {
        if remote.req.version_req.tag().is_some() {
          continue;
        }
        match snapshot
          .resolve_best_package_id(&remote.req.name, &remote.req.version_req)
          .and_then(|id| snapshot.package_from_id(&id))
        {
          Some(package) => package,
          None => continue,
        }
      }
    };
    folder_names.insert(get_package_folder_id_folder_name(
      &package.get_package_cache_folder_id(),
    ));
  }
  folder_names
}

/// Cleans up unused packages from the node_modules/.deno directory.
/// This removes any package folders that are not part of the current resolution.
fn cleanup_unused_packages<TSys: LocalNpmInstallSys>(
  sys: &TSys,
  root_node_modules_dir: &Path,
  deno_local_registry_dir: &Path,
  package_partitions: &deno_npm::resolution::NpmPackagesPartitioned,
  root_folder_names: &BTreeSet<String>,
  setup_cache: &mut LocalSetupCache<TSys>,
) {
  // Collect all package folder names that should exist in .deno/
  let expected_folders: HashSet<_> = package_partitions
    .iter_all()
    .map(|package| {
      get_package_folder_id_folder_name(&package.get_package_cache_folder_id())
    })
    .collect();

  // Clean up package folders in .deno/ that are no longer needed
  if let Ok(entries) = sys.fs_read_dir(deno_local_registry_dir) {
    for entry in entries.flatten() {
      let file_name = entry.file_name();
      if let Some(name_str) = file_name.to_str() {
        if name_str == "node_modules" || name_str.starts_with(".deno.lock") {
          continue;
        }

        // If this folder is not expected, remove it
        if !expected_folders.contains(name_str) {
          let path = deno_local_registry_dir.join(file_name);
          let _ignore = sys.fs_remove_dir_all(&path);
        }
      }
    }
  }

  // Build set of package folder names that should exist
  let keep_names = package_partitions
    .iter_all()
    .map(|package| {
      get_package_folder_id_folder_name(&package.get_package_cache_folder_id())
    })
    .collect::<HashSet<_>>();

  // Clean up .deno/node_modules/* symlinks for packages no longer needed
  let deno_node_modules_dir = deno_local_registry_dir.join("node_modules");
  let _ignore = remove_unused_node_modules_symlinks(
    sys,
    &deno_node_modules_dir,
    &keep_names,
    &mut |name, path| {
      setup_cache.remove_deno_symlink(name);
      remove_existing_entry(sys, path)
    },
  );

  // Clean up root node_modules/* symlinks for packages that should no longer
  // be linked at the root. Only direct dependencies and top level packages
  // get a root symlink, so a package that remains in the resolution solely as
  // a transitive dependency must not stay linked at the root (#35083).
  let root_keep_names =
    root_folder_names.iter().cloned().collect::<HashSet<_>>();
  let _ignore = remove_unused_node_modules_symlinks(
    sys,
    root_node_modules_dir,
    &root_keep_names,
    &mut |name, path| {
      setup_cache.remove_root_symlink(name);
      remove_existing_entry(sys, path)
    },
  );

  // remove the .bin directory entries
  let bin_dir = root_node_modules_dir.join(".bin");
  if let Ok(entries) = sys.fs_read_dir(&bin_dir) {
    for entry in entries.flatten() {
      let Ok(file_type) = entry.file_type() else {
        continue;
      };
      if file_type.is_file() {
        let _ignore = sys.fs_remove_file(entry.path());
      } else {
        let _ignore = sys.fs_remove_dir_all(entry.path());
      }
    }
  }
}

/// Extracts the package folder name from a node_modules symlink target path.
/// e.g. node_modules/.deno/chalk@5.0.1/node_modules/chalk -> chalk@5.0.1
pub fn node_modules_package_actual_dir_to_name(
  path: &Path,
) -> Option<Cow<'_, str>> {
  let package_folder = path.parent()?.parent()?;
  if package_folder.file_name()? == std::ffi::OsStr::new("node_modules") {
    package_folder
      .parent()?
      .file_name()
      .map(|name| name.to_string_lossy())
  } else {
    package_folder
      .file_name()
      .map(|name| name.to_string_lossy())
  }
}

/// Remove symlinks from a node_modules directory where the target package
/// is not in the keep_names set. The on_remove callback is responsible for
/// the actual removal and receives the package name and path.
pub fn remove_unused_node_modules_symlinks<TSys: LocalNpmInstallSys>(
  sys: &TSys,
  dir: &Path,
  keep_names: &HashSet<String>,
  on_remove: &mut dyn FnMut(&str, &Path) -> std::io::Result<()>,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
  let entries = match sys.fs_read_dir(dir) {
    Ok(entries) => entries,
    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()), // Directory doesn't exist, nothing to clean
    Err(e) => return Err(e),
  };

  for entry in entries.flatten() {
    let entry_path = dir.join(entry.file_name());
    // use fs_read_link to detect both symlinks and junctions on Windows
    // (is_symlink() returns false for junctions)
    if let Ok(target) = sys.fs_read_link(&entry_path) {
      let name = node_modules_package_actual_dir_to_name(&target);
      if let Some(name) = name
        && !keep_names.contains(&*name)
      {
        on_remove(&name, &entry_path)?;
      }
    } else if entry.file_name().to_string_lossy().starts_with('@')
      && entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
    {
      remove_unused_node_modules_symlinks(
        sys.as_ref(),
        &entry_path,
        keep_names,
        on_remove,
      )?;
      if sys
        .fs_read_dir(&entry_path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
      {
        let _ignore = sys.fs_remove_dir(&entry_path);
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsMetadata;
  use sys_traits::FsRead;
  use sys_traits::FsWrite;
  use test_util::TempDir;

  use super::*;

  #[test]
  fn test_setup_cache() {
    let temp_dir = TempDir::new();
    let cache_bin_path = temp_dir.path().join("cache.bin").to_path_buf();
    let sys = sys_traits::impls::RealSys;
    let mut cache = LocalSetupCache::load(sys.clone(), cache_bin_path.clone());
    assert!(cache.insert_deno_symlink("package-a", "package-a@1.0.0"));
    assert!(cache.insert_root_symlink("package-a", "package-a@1.0.0"));
    assert!(
      cache
        .with_dep("package-a")
        .insert("package-b", "package-b@1.0.0")
    );
    assert!(cache.save());

    let mut cache = LocalSetupCache::load(sys.clone(), cache_bin_path.clone());
    assert!(!cache.insert_deno_symlink("package-a", "package-a@1.0.0"));
    assert!(!cache.insert_root_symlink("package-a", "package-a@1.0.0"));
    assert!(
      !cache
        .with_dep("package-a")
        .insert("package-b", "package-b@1.0.0")
    );
    assert!(!cache.save());
    assert!(cache.insert_root_symlink("package-b", "package-b@0.2.0"));
    assert!(cache.save());

    let mut cache = LocalSetupCache::load(sys, cache_bin_path);
    cache.remove_dep("package-a");
    assert!(
      cache
        .with_dep("package-a")
        .insert("package-b", "package-b@1.0.0")
    );
  }

  #[test]
  fn test_symlink_package_dir_replaces_existing_link() {
    let temp_dir = TempDir::new();
    let sys = sys_traits::impls::RealSys;
    let root = temp_dir.path().to_path_buf();

    let target_a = root.join("target_a");
    let target_b = root.join("target_b");
    sys.fs_create_dir_all(&target_a).unwrap();
    sys.fs_create_dir_all(&target_b).unwrap();
    sys.fs_write(target_a.join("marker.txt"), "a").unwrap();
    sys.fs_write(target_b.join("marker.txt"), "b").unwrap();

    let node_modules = root.join("node_modules");
    sys.fs_create_dir_all(&node_modules).unwrap();
    let link = node_modules.join("pkg");

    // First the link points at target_a.
    symlink_package_dir(&sys, &target_a, &link).unwrap();
    assert_eq!(sys.fs_read_to_string(link.join("marker.txt")).unwrap(), "a");

    // Re-creating over the pre-existing link must succeed (this is what
    // regressed on Windows: a stale directory symlink/junction has to be
    // removed before the new one can be created) and now resolve to target_b.
    symlink_package_dir(&sys, &target_b, &link).unwrap();
    assert_eq!(sys.fs_read_to_string(link.join("marker.txt")).unwrap(), "b");
  }

  #[test]
  fn test_create_retry_if_exists_clears_stale_entry() {
    let temp_dir = TempDir::new();
    let sys = sys_traits::impls::RealSys;
    let path = temp_dir.path().join("entry").to_path_buf();

    // A stale entry is sitting where we want to create something new.
    sys.fs_create_dir_all(&path).unwrap();

    let mut attempts = 0;
    let result = create_retry_if_exists(&sys, &path, || {
      attempts += 1;
      if attempts == 1 {
        // Simulate creation failing because the stale entry is in the way.
        Err(std::io::Error::new(
          std::io::ErrorKind::AlreadyExists,
          "already exists",
        ))
      } else {
        Ok(())
      }
    });

    assert!(result.is_ok());
    assert_eq!(attempts, 2);
    // The stale entry must have been removed before the retry.
    assert!(!sys.fs_exists(&path).unwrap());
  }

  #[test]
  fn test_create_retry_if_exists_passes_through_success() {
    let temp_dir = TempDir::new();
    let sys = sys_traits::impls::RealSys;
    let path = temp_dir.path().join("entry").to_path_buf();

    let mut attempts = 0;
    let result = create_retry_if_exists(&sys, &path, || {
      attempts += 1;
      Ok(())
    });

    assert!(result.is_ok());
    assert_eq!(attempts, 1);
  }

  #[test]
  fn test_node_modules_package_actual_dir_to_name() {
    assert_eq!(
      node_modules_package_actual_dir_to_name(Path::new(
        ".deno/chalk@5.0.1/node_modules/chalk"
      ))
      .as_deref(),
      Some("chalk@5.0.1")
    );
    assert_eq!(
      node_modules_package_actual_dir_to_name(Path::new(
        ".deno/@denotest+add@1.0.0/node_modules/@denotest/add"
      ))
      .as_deref(),
      Some("@denotest+add@1.0.0")
    );
  }

  #[test]
  fn test_jsr_npm_name_to_original() {
    assert_eq!(
      jsr_npm_name_to_original("@jsr/std__bytes").as_deref(),
      Some("@std/bytes")
    );
    assert_eq!(
      jsr_npm_name_to_original("@jsr/david__dax").as_deref(),
      Some("@david/dax")
    );
    // not a `@jsr/` package
    assert_eq!(jsr_npm_name_to_original("chalk"), None);
    assert_eq!(jsr_npm_name_to_original("@std/bytes"), None);
    // missing the `__` separator
    assert_eq!(jsr_npm_name_to_original("@jsr/std"), None);
  }

  #[test]
  fn test_ensure_jsr_npmrc() {
    let temp_dir = TempDir::new();
    let sys = sys_traits::impls::RealSys;
    let dir = temp_dir.path().to_path_buf();
    let npmrc_path = dir.join(".npmrc");

    // 1. No existing `.npmrc`: it gets created with the JSR registry line.
    ensure_jsr_npmrc(&sys, &dir).unwrap();
    assert_eq!(
      sys.fs_read_to_string(&npmrc_path).unwrap(),
      "@jsr:registry=https://npm.jsr.io\n"
    );

    // 2. Running again is idempotent (the entry already exists).
    ensure_jsr_npmrc(&sys, &dir).unwrap();
    assert_eq!(
      sys.fs_read_to_string(&npmrc_path).unwrap(),
      "@jsr:registry=https://npm.jsr.io\n"
    );

    // 3. An existing `.npmrc` without the entry gets it appended (preserving the
    // existing contents and adding a separating newline).
    sys
      .fs_write(&npmrc_path, "registry=https://example.com")
      .unwrap();
    ensure_jsr_npmrc(&sys, &dir).unwrap();
    assert_eq!(
      sys.fs_read_to_string(&npmrc_path).unwrap(),
      "registry=https://example.com\n@jsr:registry=https://npm.jsr.io\n"
    );

    // 4. A pre-existing `@jsr` registry configuration is left untouched.
    sys
      .fs_write(&npmrc_path, "@jsr:registry=https://example.com/jsr\n")
      .unwrap();
    ensure_jsr_npmrc(&sys, &dir).unwrap();
    assert_eq!(
      sys.fs_read_to_string(&npmrc_path).unwrap(),
      "@jsr:registry=https://example.com/jsr\n"
    );
  }
}
