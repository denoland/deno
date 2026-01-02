// Copyright 2018-2025 the Deno authors. MIT license.

//! Code for local node_modules resolution.

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Error as AnyError;
use async_trait::async_trait;
use deno_error::JsErrorBox;
use deno_npm::NpmResolutionPackage;
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
{
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
  lifecycle_scripts_config: Arc<LifecycleScriptsConfig>,
  root_node_modules_path: PathBuf,
  system_info: NpmSystemInfo,
  install_reporter: Option<Arc<dyn crate::InstallReporter>>,
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
      .field("lifecycle_scripts_config", &self.lifecycle_scripts_config)
      .field("root_node_modules_path", &self.root_node_modules_path)
      .field("system_info", &self.system_info)
      .finish()
  }
}

struct InitializingGuard {
  nv: PackageNv,
  install_reporter: Arc<dyn crate::InstallReporter>,
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
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    lifecycle_scripts_executor: Arc<dyn LifecycleScriptsExecutor>,
    npm_cache: Arc<NpmCache<TSys>>,
    npm_package_extra_info_provider: Arc<NpmPackageExtraInfoProvider>,
    npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
    reporter: TReporter,
    resolution: Arc<NpmResolutionCell>,
    sys: TSys,
    tarball_cache: Arc<TarballCache<THttpClient, TSys>>,
    node_modules_folder: PathBuf,
    lifecycle_scripts: Arc<LifecycleScriptsConfig>,
    system_info: NpmSystemInfo,
    install_reporter: Option<Arc<dyn crate::InstallReporter>>,
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
      lifecycle_scripts_config: lifecycle_scripts,
      root_node_modules_path: node_modules_folder,
      system_info,
      install_reporter,
    }
  }

  async fn sync_resolution_with_fs(
    &self,
    snapshot: &NpmResolutionSnapshot,
  ) -> Result<(), SyncResolutionWithFsError> {
    if snapshot.is_empty()
      && self.npm_install_deps_provider.local_pkgs().is_empty()
    {
      return Ok(()); // don't create the directory
    }

    // don't set up node_modules (and more importantly try to acquire the file lock)
    // if we're running as part of a lifecycle script
    if is_running_lifecycle_script(&self.sys) {
      return Ok(());
    }

    let deno_local_registry_dir = self.root_node_modules_path.join(".deno");
    let deno_node_modules_dir = deno_local_registry_dir.join("node_modules");
    self
      .sys
      .fs_create_dir_all(&deno_node_modules_dir)
      .map_err(|source| SyncResolutionWithFsError::Creating {
        path: deno_node_modules_dir.to_path_buf(),
        source,
      })?;
    let bin_node_modules_dir_path = self.root_node_modules_path.join(".bin");
    self
      .sys
      .fs_create_dir_all(&bin_node_modules_dir_path)
      .map_err(|source| SyncResolutionWithFsError::Creating {
        path: bin_node_modules_dir_path.to_path_buf(),
        source,
      })?;

    let single_process_lock = LaxSingleProcessFsFlag::lock(
      &self.sys,
      deno_local_registry_dir.join(".deno.lock"),
      &self.reporter,
      // similar message used by cargo build
      "waiting for file lock on node_modules directory",
    )
    .await;

    // load this after we get the directory lock
    let mut setup_cache = LocalSetupCache::load(
      self.sys.clone(),
      deno_local_registry_dir.join(".setup-cache.bin"),
    );

    let pb_clear_guard = self.reporter.clear_guard(); // prevent flickering

    // 1. Write all the packages out the .deno directory.
    //
    // Copy (hardlink in future) <global_registry_cache>/<package_id>/ to
    // node_modules/.deno/<package_folder_id_folder_name>/node_modules/<package_name>
    let package_partitions =
      snapshot.all_system_packages_partitioned(&self.system_info);
    let mut cache_futures = FuturesUnordered::new();
    let mut newest_packages_by_name: HashMap<
      &StackString,
      &NpmResolutionPackage,
    > = HashMap::with_capacity(package_partitions.packages.len());
    let bin_entries = Rc::new(RefCell::new(BinEntries::new(&self.sys)));
    let lifecycle_scripts = Rc::new(RefCell::new(LifecycleScripts::new(
      &self.sys,
      &self.lifecycle_scripts_config,
      LocalLifecycleScripts {
        sys: &self.sys,
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
        if self.sys.fs_exists_no_err(&initialized_file) {
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
              let extra_fut = if (package.has_bin
                || package.has_scripts
                || package.is_deprecated)
                && package.extra.is_none()
              {
                extra_info_provider
                  .get_package_extra_info(
                    &package.id.nv,
                    &package_path,
                    ExpectedExtraInfo::from_package(package),
                  )
                  .boxed_local()
              } else {
                std::future::ready(Ok(
                  package.extra.clone().unwrap_or_default(),
                ))
                .boxed_local()
              };

              let (result, extra) =
                futures::future::join(handle, extra_fut).await;
              result
                .map_err(JsErrorBox::from_err)?
                .map_err(JsErrorBox::from_err)?;
              let extra = extra.map_err(JsErrorBox::from_err)?;

              if package.has_bin {
                bin_entries_to_setup.borrow_mut().add(
                  package,
                  &extra,
                  package_path.to_path_buf(),
                );
              }

              if package.has_scripts {
                lifecycle_scripts.borrow_mut().add(
                  package,
                  &extra,
                  package_path.into(),
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
          write_initialized_file(&self.sys, &initialized_file, &tags)?;
        }

        if package.has_bin || package.has_scripts {
          let bin_entries_to_setup = bin_entries.clone();
          let lifecycle_scripts = lifecycle_scripts.clone();
          let extra_info_provider = extra_info_provider.clone();
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
                lifecycle_scripts.borrow_mut().add(
                  package,
                  &extra,
                  package_path.into(),
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

    // 2. Setup the patch packages
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

    // 3. Create any "copy" packages, which are used for peer dependencies
    for package in &package_partitions.copy_packages {
      let package_cache_folder_id = package.get_package_cache_folder_id();
      let destination_path = deno_local_registry_dir
        .join(get_package_folder_id_folder_name(&package_cache_folder_id));
      let initialized_file = destination_path.join(".initialized");
      if !self.sys.fs_exists_no_err(&initialized_file) {
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
              create_initialized_file(&sys, &initialized_file)?;
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

    // 4. Symlink all the dependencies into the .deno directory.
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
            &self.sys,
            &dep_folder_path,
            &join_package_name(Cow::Borrowed(&sub_node_modules), name),
          )?;
        }
      }
    }

    let mut found_names: HashMap<&StackString, &PackageNv> = HashMap::new();

    // set of node_modules in workspace packages that we've already ensured exist
    let mut existing_child_node_modules_dirs: HashSet<PathBuf> = HashSet::new();

    // 5. Create symlinks for package json dependencies
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
            self.sys.fs_create_dir_all(&dest_node_modules).map_err(
              |source| SyncResolutionWithFsError::Creating {
                path: dest_node_modules.clone(),
                source,
              },
            )?;
            existing_child_node_modules_dirs.insert(dest_node_modules.clone());
          }
          let mut dest_path = dest_node_modules;
          dest_path.push(remote_alias);

          symlink_package_dir(
            &self.sys,
            &local_registry_package_path,
            &dest_path,
          )?;
        } else {
          // symlink the package into `node_modules/<alias>`
          if setup_cache
            .insert_root_symlink(&remote_pkg.id.nv.name, &target_folder_name)
          {
            symlink_package_dir(
              &self.sys,
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

    // 6. Create symlinks for the remaining top level packages in the node_modules folder.
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
              .join(target_folder_name)
              .join("node_modules"),
          ),
          &id.nv.name,
        );

        symlink_package_dir(
          &self.sys,
          &local_registry_package_path,
          &join_package_name(
            Cow::Borrowed(&self.root_node_modules_path),
            &id.nv.name,
          ),
        )?;
      }
    }

    // 7. Create a node_modules/.deno/node_modules/<package-name> directory with
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
          &self.sys,
          &local_registry_package_path,
          &join_package_name(
            Cow::Borrowed(&deno_node_modules_dir),
            &package.id.nv.name,
          ),
        )?;
      }
    }

    // 8. Set up `node_modules/.bin` entries for packages that need it.
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
            } if has_lifecycle_scripts(&self.sys, extra, package_path)
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

    // 9. Create symlinks for the workspace packages
    {
      // todo(dsherret): this is not exactly correct because it should
      // install correctly for a workspace (potentially in sub directories),
      // but this is good enough for a first pass
      for pkg in self.npm_install_deps_provider.local_pkgs() {
        let Some(pkg_alias) = &pkg.alias else {
          continue;
        };
        symlink_package_dir(
          &self.sys,
          &pkg.target_dir,
          &self.root_node_modules_path.join(pkg_alias),
        )?;
      }
    }

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

    let lifecycle_scripts = std::mem::replace(
      &mut *lifecycle_scripts.borrow_mut(),
      LifecycleScripts::new(
        &self.sys,
        &self.lifecycle_scripts_config,
        LocalLifecycleScripts {
          sys: &self.sys,
          deno_local_registry_dir: &deno_local_registry_dir,
          install_reporter: self.install_reporter.clone(),
        },
      ),
    );
    lifecycle_scripts.warn_not_run_scripts()?;

    let packages_with_scripts = lifecycle_scripts.packages_with_scripts();
    if !packages_with_scripts.is_empty() {
      let process_state = NpmProcessState::new_local(
        snapshot.as_valid_serialized(),
        &self.root_node_modules_path,
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
              &self.sys,
              &ran_scripts_file(&deno_local_registry_dir, pkg),
            )
          },
          snapshot,
          system_packages: &package_partitions.packages,
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
  #[class(inherit)]
  #[error("Creating '{path}'")]
  Creating {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Copying '{from}' to '{to}'")]
  Copying {
    from: PathBuf,
    to: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  CopyDirRecursive(#[from] crate::fs::CopyDirRecursiveError),
  #[class(inherit)]
  #[error(transparent)]
  SymlinkPackageDir(#[from] SymlinkPackageDirError),
  #[class(inherit)]
  #[error(transparent)]
  BinEntries(#[from] crate::bin_entries::BinEntriesError),
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

fn clone_dir_recursive_except_node_modules_child(
  sys: &impl CloneDirRecursiveSys,
  from: &Path,
  to: &Path,
) -> Result<(), SyncResolutionWithFsError> {
  _ = sys.fs_remove_dir_all(to);
  sys.fs_create_dir_all(to).map_err(|source| {
    SyncResolutionWithFsError::Creating {
      path: to.to_path_buf(),
      source,
    }
  })?;
  for entry in sys.fs_read_dir(from)? {
    let entry = entry?;
    if entry.file_name().to_str() == Some("node_modules") {
      continue; // ignore
    }
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      clone_dir_recursive_except_node_modules_child(sys, &new_from, &new_to)?;
    } else if file_type.is_file() {
      hard_link_file(sys, &new_from, &new_to).or_else(|_| {
        sys
          .fs_copy(&new_from, &new_to)
          .map_err(|source| SyncResolutionWithFsError::Copying {
            from: new_from.clone(),
            to: new_to.clone(),
            source,
          })
          .map(|_| ())
      })?;
    }
  }
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
  #[class(inherit)]
  #[error("Creating junction in node_modules folder")]
  FailedCreatingJunction {
    #[source]
    #[inherit]
    source: std::io::Error,
  },
}

fn symlink_package_dir(
  sys: &(
     impl sys_traits::FsSymlinkDir
     + sys_traits::FsRemoveDirAll
     + sys_traits::FsCreateDirAll
     + sys_traits::FsCreateJunction
   ),
  old_path: &Path,
  new_path: &Path,
) -> Result<(), SymlinkPackageDirError> {
  let new_parent = new_path.parent().unwrap();
  if new_parent.file_name().unwrap() != "node_modules" {
    // create the parent folder that will contain the symlink
    sys.fs_create_dir_all(new_parent).map_err(|source| {
      SymlinkPackageDirError::Creating {
        parent: new_parent.to_path_buf(),
        source,
      }
    })?;
  }

  // need to delete the previous symlink before creating a new one
  let _ignore = sys.fs_remove_dir_all(new_path);

  let old_path_relative = relative_path(new_parent, old_path)
    .unwrap_or_else(|| old_path.to_path_buf());

  if sys_traits::impls::is_windows() {
    junction_or_symlink_dir(sys, &old_path_relative, old_path, new_path)
  } else {
    symlink_dir(sys, &old_path_relative, new_path).map_err(Into::into)
  }
}

fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
  pathdiff::diff_paths(to, from)
}

fn junction_or_symlink_dir(
  sys: &(impl sys_traits::FsSymlinkDir + sys_traits::FsCreateJunction),
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
    return sys
      .fs_create_junction(old_path, new_path)
      .map_err(|source| SymlinkPackageDirError::FailedCreatingJunction {
        source,
      });
  }

  match symlink_dir(sys, old_path_relative, new_path) {
    Ok(()) => Ok(()),
    Err(symlink_err)
      if symlink_err.kind() == std::io::ErrorKind::PermissionDenied =>
    {
      USE_JUNCTIONS.store(true, std::sync::atomic::Ordering::Relaxed);
      sys
        .fs_create_junction(old_path, new_path)
        .map_err(|source| SymlinkPackageDirError::FailedCreatingJunction {
          source,
        })
    }
    Err(symlink_err) => {
      log::warn!(
        "{} Unexpected error symlinking node_modules: {symlink_err}",
        colors::yellow("Warning")
      );
      USE_JUNCTIONS.store(true, std::sync::atomic::Ordering::Relaxed);
      sys
        .fs_create_junction(old_path, new_path)
        .map_err(|source| SymlinkPackageDirError::FailedCreatingJunction {
          source,
        })
    }
  }
}

fn write_initialized_file(
  sys: &(impl FsWrite + FsOpen),
  path: &Path,
  text: &str,
) -> Result<(), JsErrorBox> {
  if text.is_empty() {
    // one less syscall
    create_initialized_file(sys, path)
  } else {
    sys.fs_write(path, text).map_err(|err| {
      JsErrorBox::generic(format!(
        "Failed writing '{}': {}",
        path.display(),
        err
      ))
    })
  }
}

fn create_initialized_file<F: sys_traits::boxed::FsOpenBoxed + ?Sized>(
  sys: &F,
  path: &Path,
) -> Result<(), JsErrorBox> {
  sys
    .fs_open_boxed(path, &sys_traits::OpenOptions::new_write())
    .map(|_| ())
    .map_err(|err| {
      JsErrorBox::generic(format!(
        "Failed to create '{}': {}",
        path.display(),
        err
      ))
    })
}

fn join_package_name(mut path: Cow<'_, Path>, package_name: &str) -> PathBuf {
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

#[cfg(test)]
mod test {
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
}
