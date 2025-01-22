// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_cache_dir::npm::NpmCacheDir;
use deno_cache_dir::DenoDirResolutionError;
use deno_cache_dir::GlobalHttpCacheRc;
use deno_cache_dir::HttpCacheRc;
use deno_cache_dir::LocalHttpCache;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::workspace::FolderConfigs;
use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::VendorEnablement;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceDirectoryEmptyOptions;
use deno_config::workspace::WorkspaceDiscoverError;
use deno_config::workspace::WorkspaceDiscoverOptions;
use deno_config::workspace::WorkspaceDiscoverStart;
use deno_npm::NpmSystemInfo;
use deno_path_util::fs::canonicalize_path_maybe_not_exists;
use deno_path_util::normalize_path;
use node_resolver::ConditionsFromResolutionMode;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolver;
use node_resolver::NodeResolverRc;
use node_resolver::PackageJsonResolver;
use node_resolver::PackageJsonResolverRc;
use sys_traits::EnvCacheDir;
use sys_traits::EnvCurrentDir;
use sys_traits::EnvHomeDir;
use sys_traits::EnvVar;
use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsRead;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::SystemTimeNow;
use sys_traits::ThreadSleep;
use thiserror::Error;

use crate::npm::managed::ManagedInNpmPkgCheckerCreateOptions;
use crate::npm::managed::ManagedNpmResolverCreateOptions;
use crate::npm::managed::NpmResolutionCellRc;
use crate::npm::ByonmNpmResolverCreateOptions;
use crate::npm::CreateInNpmPkgCheckerOptions;
use crate::npm::DenoInNpmPackageChecker;
use crate::npm::NpmReqResolver;
use crate::npm::NpmReqResolverOptions;
use crate::npm::NpmReqResolverRc;
use crate::npm::NpmResolver;
use crate::npm::NpmResolverCreateOptions;
use crate::npmrc::discover_npmrc_from_workspace;
use crate::npmrc::NpmRcDiscoverError;
use crate::npmrc::ResolvedNpmRcRc;
use crate::sloppy_imports::SloppyImportsCachedFs;
use crate::sloppy_imports::SloppyImportsResolver;
use crate::sloppy_imports::SloppyImportsResolverRc;
use crate::sync::new_rc;
use crate::sync::MaybeSend;
use crate::sync::MaybeSync;
use crate::DefaultDenoResolverRc;
use crate::DenoResolver;
use crate::DenoResolverOptions;
use crate::NodeAndNpmReqResolver;
use crate::NpmCacheDirRc;
use crate::WorkspaceResolverRc;

// todo(https://github.com/rust-lang/rust/issues/109737): remove once_cell after get_or_try_init is stabilized
#[cfg(feature = "sync")]
type MaybeOnceLock<T> = once_cell::sync::OnceCell<T>;
#[cfg(not(feature = "sync"))]
type MaybeOnceLock<T> = once_cell::unsync::OnceCell<T>;

#[derive(Debug)]
struct Deferred<T>(MaybeOnceLock<T>);

impl<T> Default for Deferred<T> {
  fn default() -> Self {
    Self(Default::default())
  }
}

impl<T> Deferred<T> {
  pub fn from_value(value: T) -> Self {
    Self(MaybeOnceLock::from(value))
  }

  #[inline(always)]
  pub fn get_or_try_init<TError>(
    &self,
    create: impl FnOnce() -> Result<T, TError>,
  ) -> Result<&T, TError> {
    self.0.get_or_try_init(create)
  }

  #[inline(always)]
  pub fn get_or_init(&self, create: impl FnOnce() -> T) -> &T {
    self.0.get_or_init(create)
  }

  // todo(THIS PR): make an async ok DeferredAsync
  pub async fn get_or_try_init_async(
    &self,
    // some futures passed here are boxed because it was discovered
    // that they were called a lot, causing other futures to get
    // really big causing stack overflows on Windows
    create: impl Future<Output = Result<T, anyhow::Error>>,
  ) -> Result<&T, anyhow::Error> {
    if self.0.get().is_none() {
      // todo(dsherret): it would be more ideal if this enforced a
      // single executor and then we could make some initialization
      // concurrent
      let val = create.await?;
      _ = self.0.set(val);
    }
    Ok(self.0.get().unwrap())
  }
}

#[allow(clippy::disallowed_types)]
type WorkspaceDirectoryRc = crate::sync::MaybeArc<WorkspaceDirectory>;

#[derive(Debug, Boxed)]
pub struct HttpCacheCreateError(pub Box<HttpCacheCreateErrorKind>);

#[derive(Debug, Error)]
pub enum HttpCacheCreateErrorKind {
  #[error(transparent)]
  DenoDirResolution(#[from] DenoDirResolutionError),
  #[error(transparent)]
  WorkspaceDiscover(#[from] WorkspaceDiscoverError),
}

#[derive(Debug, Boxed)]
pub struct NpmCacheDirCreateError(pub Box<NpmCacheDirCreateErrorKind>);

#[derive(Debug, Error)]
pub enum NpmCacheDirCreateErrorKind {
  #[error(transparent)]
  DenoDirResolution(#[from] DenoDirResolutionError),
  #[error(transparent)]
  NpmRcCreate(#[from] NpmRcCreateError),
}

#[derive(Debug, Boxed)]
pub struct NpmRcCreateError(pub Box<NpmRcCreateErrorKind>);

#[derive(Debug, Error)]
pub enum NpmRcCreateErrorKind {
  #[error(transparent)]
  WorkspaceDiscover(#[from] WorkspaceDiscoverError),
  #[error(transparent)]
  NpmRcDiscover(#[from] NpmRcDiscoverError),
}

#[derive(Debug, Boxed)]
pub struct NodeModulesModeResolveError(
  pub Box<NodeModulesModeResolveErrorKind>,
);

#[derive(Debug, Error)]
pub enum NodeModulesModeResolveErrorKind {
  #[error(transparent)]
  NodeModulesDirParse(#[from] deno_config::deno_json::NodeModulesDirParseError),
  #[error(transparent)]
  WorkspaceDiscover(#[from] WorkspaceDiscoverError),
}

#[derive(Debug, Boxed)]
pub struct NodeModulesFolderResolveError(
  pub Box<NodeModulesFolderResolveErrorKind>,
);

#[derive(Debug, Error)]
pub enum NodeModulesFolderResolveErrorKind {
  #[error(transparent)]
  NodeModulesDirParse(#[from] deno_config::deno_json::NodeModulesDirParseError),
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  NodeModulesModeResolve(#[from] NodeModulesModeResolveError),
  #[error(transparent)]
  Workspace(#[from] WorkspaceDiscoverError),
}

#[derive(Debug, Default)]
pub enum ConfigDiscoveryOption {
  #[default]
  DiscoverCwd,
  Discover {
    start_paths: Vec<PathBuf>,
  },
  Path(PathBuf),
  Disabled,
}

#[async_trait::async_trait(?Send)]
pub trait SpecifiedImportMapProvider:
  std::fmt::Debug + MaybeSend + MaybeSync
{
  async fn get(
    &self,
  ) -> Result<Option<deno_config::workspace::SpecifiedImportMap>, anyhow::Error>;
}

#[derive(Debug, Clone)]
pub struct DenoDirPathProviderOptions {
  pub maybe_custom_root: Option<PathBuf>,
}

#[allow(clippy::disallowed_types)]
pub type DenoDirPathProviderRc<TSys> =
  crate::sync::MaybeArc<DenoDirPathProvider<TSys>>;

/// Lazily creates the deno dir which might be useful in scenarios
/// where functionality wants to continue if the DENO_DIR can't be created.
#[derive(Debug)]
pub struct DenoDirPathProvider<
  TSys: EnvCacheDir + EnvHomeDir + EnvVar + EnvCurrentDir,
> {
  sys: TSys,
  options: DenoDirPathProviderOptions,
  deno_dir: Deferred<PathBuf>,
}

impl<TSys: EnvCacheDir + EnvHomeDir + EnvVar + EnvCurrentDir>
  DenoDirPathProvider<TSys>
{
  pub fn new(sys: TSys, options: DenoDirPathProviderOptions) -> Self {
    Self {
      sys,
      options,
      deno_dir: Default::default(),
    }
  }

  pub fn get_or_create(&self) -> Result<&PathBuf, DenoDirResolutionError> {
    self.deno_dir.get_or_try_init(|| {
      deno_cache_dir::resolve_deno_dir(
        &self.sys,
        self.options.maybe_custom_root.clone(),
      )
    })
  }
}

#[derive(Debug)]
pub struct NpmProcessStateOptions {
  pub node_modules_dir: Option<Cow<'static, str>>,
  pub is_byonm: bool,
}

#[derive(Debug, Default)]
pub struct WorkspaceFactoryOptions<
  TSys: EnvCacheDir + EnvHomeDir + EnvVar + EnvCurrentDir,
> {
  pub additional_config_file_names: &'static [&'static str],
  pub config_discovery: ConfigDiscoveryOption,
  pub deno_dir_path_provider: Option<DenoDirPathProviderRc<TSys>>,
  pub node_modules_dir: Option<NodeModulesDirMode>,
  pub no_npm: bool,
  /// The process sate if using ext/node and the current process was "forked".
  /// This value is found at `deno_lib::args::NPM_PROCESS_STATE`
  /// but in most scenarios this can probably just be `None`.
  pub npm_process_state: Option<NpmProcessStateOptions>,
  pub vendor: Option<bool>,
}

#[allow(clippy::disallowed_types)]
pub type WorkspaceFactoryRc<TSys> =
  crate::sync::MaybeArc<WorkspaceFactory<TSys>>;

pub struct WorkspaceFactory<
  TSys: EnvCacheDir
    + EnvHomeDir
    + EnvVar
    + EnvCurrentDir
    + FsCanonicalize
    + FsCreateDirAll
    + FsMetadata
    + FsOpen
    + FsRead
    + FsReadDir
    + FsRemoveFile
    + FsRename
    + SystemRandom
    + SystemTimeNow
    + ThreadSleep
    + std::fmt::Debug
    + MaybeSend
    + MaybeSync
    + Clone
    + 'static,
> {
  sys: TSys,
  deno_dir_path: DenoDirPathProviderRc<TSys>,
  global_http_cache: Deferred<GlobalHttpCacheRc<TSys>>,
  http_cache: Deferred<HttpCacheRc>,
  node_modules_dir_path: Deferred<Option<PathBuf>>,
  npm_cache_dir: Deferred<NpmCacheDirRc>,
  npmrc: Deferred<ResolvedNpmRcRc>,
  resolved_node_modules_dir: Deferred<NodeModulesDirMode>,
  specified_node_modules_dir: Deferred<Option<NodeModulesDirMode>>,
  workspace_directory: Deferred<WorkspaceDirectoryRc>,
  initial_cwd: PathBuf,
  options: WorkspaceFactoryOptions<TSys>,
}

impl<
    TSys: EnvCacheDir
      + EnvHomeDir
      + EnvVar
      + EnvCurrentDir
      + FsCanonicalize
      + FsCreateDirAll
      + FsMetadata
      + FsOpen
      + FsRead
      + FsReadDir
      + FsRemoveFile
      + FsRename
      + SystemRandom
      + SystemTimeNow
      + ThreadSleep
      + std::fmt::Debug
      + MaybeSend
      + MaybeSync
      + Clone
      + 'static,
  > WorkspaceFactory<TSys>
{
  pub fn new(
    sys: TSys,
    initial_cwd: PathBuf,
    mut options: WorkspaceFactoryOptions<TSys>,
  ) -> Self {
    Self {
      deno_dir_path: options.deno_dir_path_provider.take().unwrap_or_else(
        || {
          new_rc(DenoDirPathProvider::new(
            sys.clone(),
            DenoDirPathProviderOptions {
              maybe_custom_root: None,
            },
          ))
        },
      ),
      sys,
      global_http_cache: Default::default(),
      http_cache: Default::default(),
      node_modules_dir_path: Default::default(),
      npm_cache_dir: Default::default(),
      npmrc: Default::default(),
      resolved_node_modules_dir: Default::default(),
      specified_node_modules_dir: Default::default(),
      workspace_directory: Default::default(),
      initial_cwd,
      options,
    }
  }

  pub fn set_workspace_directory(
    &mut self,
    workspace_directory: WorkspaceDirectoryRc,
  ) {
    self.workspace_directory = Deferred::from_value(workspace_directory);
  }

  pub fn initial_cwd(&self) -> &PathBuf {
    &self.initial_cwd
  }

  pub fn no_npm(&self) -> bool {
    self.options.no_npm
  }

  pub fn resolved_node_modules_dir(
    &self,
  ) -> Result<NodeModulesDirMode, anyhow::Error> {
    self
      .resolved_node_modules_dir
      .get_or_try_init(|| {
        if let Some(specified) = self.specified_node_modules_dir()? {
          return Ok(specified);
        }
        let workspace = &self.workspace_directory()?.workspace;
        let node_modules_dir_path = self.node_modules_dir_path()?;
        let is_byonm = node_modules_dir_path.is_some()
          && workspace
            .config_folders()
            .values()
            .any(|f| f.pkg_json.is_some());
        if is_byonm {
          Ok(NodeModulesDirMode::Manual)
        } else {
          Ok(NodeModulesDirMode::None)
        }
      })
      .copied()
  }

  fn specified_node_modules_dir(
    &self,
  ) -> Result<Option<NodeModulesDirMode>, NodeModulesModeResolveError> {
    self
      .specified_node_modules_dir
      .get_or_try_init(|| match self.options.node_modules_dir {
        Some(flag) => Ok(Some(flag)),
        None => {
          if let Some(process_state) = &self.options.npm_process_state {
            if process_state.is_byonm {
              return Ok(Some(NodeModulesDirMode::Manual));
            }
          }

          let workspace = &self.workspace_directory()?.workspace;
          Ok(
            workspace.node_modules_dir()?.or(
              self
                .options
                .vendor
                .or_else(|| {
                  workspace.root_deno_json().and_then(|c| c.json.vendor)
                })
                .map(|vendor| {
                  if vendor {
                    NodeModulesDirMode::Auto
                  } else {
                    NodeModulesDirMode::None
                  }
                }),
            ),
          )
        }
      })
      .copied()
  }

  /// Resolves the path to use for a local node_modules folder.
  pub fn node_modules_dir_path(
    &self,
  ) -> Result<Option<&Path>, NodeModulesFolderResolveError> {
    fn resolve_from_root(root_folder: &FolderConfigs, cwd: &Path) -> PathBuf {
      root_folder
        .deno_json
        .as_ref()
        .map(|c| Cow::Owned(c.dir_path()))
        .or_else(|| {
          root_folder
            .pkg_json
            .as_ref()
            .map(|c| Cow::Borrowed(c.dir_path()))
        })
        .unwrap_or(Cow::Borrowed(cwd))
        .join("node_modules")
    }

    self
      .node_modules_dir_path
      .get_or_try_init(|| {
        let workspace = &self.workspace_directory()?.workspace;
        let root_folder = workspace.root_folder_configs();
        let use_node_modules_dir = self
          .resolved_node_modules_dir()?
          .map(|v| v.uses_node_modules_dir());
        let path = if use_node_modules_dir == Some(false) {
          return Ok(None);
        } else if let Some(process_state) = &self.options.npm_process_state {
          return Ok(
            process_state
              .node_modules_dir
              .as_ref()
              .map(|p| PathBuf::from(p.as_ref())),
          );
        } else if root_folder.pkg_json.is_some() {
          let node_modules_dir =
            resolve_from_root(root_folder, &self.initial_cwd);
          if let Ok(deno_dir) = self.deno_dir_path() {
            // `deno_dir.root` can be symlink in macOS
            if let Ok(root) =
              canonicalize_path_maybe_not_exists(&self.sys, deno_dir)
            {
              if node_modules_dir.starts_with(root) {
                // if the package.json is in deno_dir, then do not use node_modules
                // next to it as local node_modules dir
                return Ok(None);
              }
            }
          }
          node_modules_dir
        } else if use_node_modules_dir.is_none() {
          return Ok(None);
        } else {
          resolve_from_root(root_folder, &self.initial_cwd)
        };
        Ok(Some(canonicalize_path_maybe_not_exists(&self.sys, &path)?))
      })
      .map(|p| p.as_deref())
  }

  pub fn deno_dir_path(&self) -> Result<&PathBuf, DenoDirResolutionError> {
    self.deno_dir_path.get_or_create()
  }

  pub fn global_http_cache(
    &self,
  ) -> Result<&GlobalHttpCacheRc<TSys>, DenoDirResolutionError> {
    self.global_http_cache.get_or_try_init(|| {
      let global_cache_dir = self.deno_dir_path()?.join("remote");
      let global_http_cache = new_rc(deno_cache_dir::GlobalHttpCache::new(
        self.sys.clone(),
        global_cache_dir,
      ));
      Ok(global_http_cache)
    })
  }

  pub fn http_cache(&self) -> Result<&HttpCacheRc, HttpCacheCreateError> {
    self.http_cache.get_or_try_init(|| {
      let global_cache = self.global_http_cache()?.clone();
      match self.workspace_directory()?.workspace.vendor_dir_path() {
        Some(local_path) => {
          let local_cache = LocalHttpCache::new(
            local_path.clone(),
            global_cache,
            deno_cache_dir::GlobalToLocalCopy::Allow,
          );
          Ok(new_rc(local_cache))
        }
        None => Ok(global_cache),
      }
    })
  }

  pub fn npm_cache_dir(
    &self,
  ) -> Result<&NpmCacheDirRc, NpmCacheDirCreateError> {
    self.npm_cache_dir.get_or_try_init(|| {
      let npm_cache_dir = self.deno_dir_path()?.join("npm");
      Ok(new_rc(NpmCacheDir::new(
        &self.sys,
        npm_cache_dir,
        self.npmrc()?.get_all_known_registries_urls(),
      )))
    })
  }

  pub fn npmrc(&self) -> Result<&ResolvedNpmRcRc, NpmRcCreateError> {
    self.npmrc.get_or_try_init(|| {
      let (npmrc, _) = discover_npmrc_from_workspace(
        &self.sys,
        &self.workspace_directory()?.workspace,
      )?;
      Ok(new_rc(npmrc))
    })
  }

  pub fn workspace_directory(
    &self,
  ) -> Result<&WorkspaceDirectoryRc, WorkspaceDiscoverError> {
    self.workspace_directory.get_or_try_init(|| {
      let maybe_vendor_override = self.options.vendor.map(|v| match v {
        true => VendorEnablement::Enable {
          cwd: &self.initial_cwd,
        },
        false => VendorEnablement::Disable,
      });
      let resolve_workspace_discover_options = || {
        let discover_pkg_json = !self.options.no_npm
          && !self.has_flag_env_var("DENO_NO_PACKAGE_JSON");
        if !discover_pkg_json {
          log::debug!("package.json auto-discovery is disabled");
        }
        WorkspaceDiscoverOptions {
          deno_json_cache: None,
          pkg_json_cache: Some(&node_resolver::PackageJsonThreadLocalCache),
          workspace_cache: None,
          additional_config_file_names: self
            .options
            .additional_config_file_names,
          discover_pkg_json,
          maybe_vendor_override,
        }
      };
      let resolve_empty_options = || WorkspaceDirectoryEmptyOptions {
        root_dir: new_rc(
          deno_path_util::url_from_directory_path(&self.initial_cwd).unwrap(),
        ),
        use_vendor_dir: maybe_vendor_override
          .unwrap_or(VendorEnablement::Disable),
      };

      let dir = match &self.options.config_discovery {
        ConfigDiscoveryOption::DiscoverCwd => WorkspaceDirectory::discover(
          &self.sys,
          WorkspaceDiscoverStart::Paths(&[self.initial_cwd.clone()]),
          &resolve_workspace_discover_options(),
        )?,
        ConfigDiscoveryOption::Discover { start_paths } => {
          WorkspaceDirectory::discover(
            &self.sys,
            WorkspaceDiscoverStart::Paths(start_paths),
            &resolve_workspace_discover_options(),
          )?
        }
        ConfigDiscoveryOption::Path(path) => {
          let config_path = normalize_path(self.initial_cwd.join(path));
          WorkspaceDirectory::discover(
            &self.sys,
            WorkspaceDiscoverStart::ConfigFile(&config_path),
            &resolve_workspace_discover_options(),
          )?
        }
        ConfigDiscoveryOption::Disabled => {
          WorkspaceDirectory::empty(resolve_empty_options())
        }
      };
      Ok(new_rc(dir))
    })
  }

  fn has_flag_env_var(&self, name: &str) -> bool {
    let value = self.sys.env_var_os(name);
    match value {
      Some(value) => value == "1",
      None => false,
    }
  }
}

#[derive(Debug, Default)]
pub struct ResolverFactoryOptions {
  pub no_sloppy_imports_cache: bool,
  pub npm_system_info: NpmSystemInfo,
  pub specified_import_map: Option<Box<dyn SpecifiedImportMapProvider>>,
  pub unstable_sloppy_imports: bool,
}

pub struct ResolverFactory<
  TSys: EnvCacheDir
    + EnvCurrentDir
    + EnvHomeDir
    + EnvVar
    + FsCanonicalize
    + FsCreateDirAll
    + FsMetadata
    + FsOpen
    + FsRead
    + FsReadDir
    + FsRemoveFile
    + FsRename
    + ThreadSleep
    + SystemRandom
    + SystemTimeNow
    + std::fmt::Debug
    + MaybeSend
    + MaybeSync
    + Clone
    + 'static,
> {
  options: ResolverFactoryOptions,
  sys: TSys,
  deno_resolver: Deferred<DefaultDenoResolverRc<TSys>>,
  in_npm_package_checker: Deferred<DenoInNpmPackageChecker>,
  node_resolver: Deferred<
    NodeResolverRc<
      DenoInNpmPackageChecker,
      DenoIsBuiltInNodeModuleChecker,
      NpmResolver<TSys>,
      TSys,
    >,
  >,
  npm_req_resolver: Deferred<
    NpmReqResolverRc<
      DenoInNpmPackageChecker,
      DenoIsBuiltInNodeModuleChecker,
      NpmResolver<TSys>,
      TSys,
    >,
  >,
  npm_resolver: Deferred<NpmResolver<TSys>>,
  npm_resolution: NpmResolutionCellRc,
  pkg_json_resolver: Deferred<PackageJsonResolverRc<TSys>>,
  sloppy_imports_resolver:
    Deferred<Option<SloppyImportsResolverRc<SloppyImportsCachedFs<TSys>>>>,
  workspace_factory: WorkspaceFactoryRc<TSys>,
  workspace_resolver: Deferred<WorkspaceResolverRc>,
}

impl<
    TSys: EnvCacheDir
      + EnvCurrentDir
      + EnvHomeDir
      + EnvVar
      + FsCanonicalize
      + FsCreateDirAll
      + FsMetadata
      + FsOpen
      + FsRead
      + FsReadDir
      + FsRemoveFile
      + FsRename
      + ThreadSleep
      + SystemRandom
      + SystemTimeNow
      + std::fmt::Debug
      + MaybeSend
      + MaybeSync
      + Clone
      + 'static,
  > ResolverFactory<TSys>
{
  pub fn new(
    sys: TSys,
    workspace_factory: WorkspaceFactoryRc<TSys>,
    options: ResolverFactoryOptions,
  ) -> Self {
    Self {
      options,
      sys,
      deno_resolver: Default::default(),
      in_npm_package_checker: Default::default(),
      node_resolver: Default::default(),
      npm_req_resolver: Default::default(),
      npm_resolution: Default::default(),
      npm_resolver: Default::default(),
      pkg_json_resolver: Default::default(),
      sloppy_imports_resolver: Default::default(),
      workspace_factory,
      workspace_resolver: Default::default(),
    }
  }

  pub async fn deno_resolver(
    &self,
  ) -> Result<&DefaultDenoResolverRc<TSys>, anyhow::Error> {
    self
      .deno_resolver
      .get_or_try_init_async(async {
        Ok(new_rc(DenoResolver::new(DenoResolverOptions {
          in_npm_pkg_checker: self.in_npm_package_checker()?.clone(),
          node_and_req_resolver: if self.workspace_factory.no_npm() {
            None
          } else {
            Some(NodeAndNpmReqResolver {
              node_resolver: self.node_resolver()?.clone(),
              npm_req_resolver: self.npm_req_resolver()?.clone(),
            })
          },
          is_byonm: self.use_byonm()?,
          maybe_vendor_dir: self
            .workspace_factory
            .workspace_directory()?
            .workspace
            .vendor_dir_path(),
          sloppy_imports_resolver: self.sloppy_imports_resolver()?.cloned(),
          workspace_resolver: self.workspace_resolver().await?.clone(),
        })))
      })
      .await
  }

  pub fn in_npm_package_checker(
    &self,
  ) -> Result<&DenoInNpmPackageChecker, anyhow::Error> {
    self.in_npm_package_checker.get_or_try_init(|| {
      let options = match self.use_byonm()? {
        true => CreateInNpmPkgCheckerOptions::Byonm,
        false => CreateInNpmPkgCheckerOptions::Managed(
          ManagedInNpmPkgCheckerCreateOptions {
            root_cache_dir_url: self
              .workspace_factory
              .npm_cache_dir()?
              .root_dir_url(),
            maybe_node_modules_path: self
              .workspace_factory
              .node_modules_dir_path()?,
          },
        ),
      };
      Ok(DenoInNpmPackageChecker::new(options))
    })
  }

  pub fn node_resolver(
    &self,
  ) -> Result<
    &NodeResolverRc<
      DenoInNpmPackageChecker,
      DenoIsBuiltInNodeModuleChecker,
      NpmResolver<TSys>,
      TSys,
    >,
    anyhow::Error,
  > {
    self.node_resolver.get_or_try_init(|| {
      Ok(new_rc(NodeResolver::new(
        self.in_npm_package_checker()?.clone(),
        DenoIsBuiltInNodeModuleChecker,
        self.npm_resolver()?.clone(),
        self.pkg_json_resolver().clone(),
        self.sys.clone(),
        // todo(THIS PR): move to options
        ConditionsFromResolutionMode::default(),
      )))
    })
  }

  pub fn npm_resolution(&self) -> &NpmResolutionCellRc {
    &self.npm_resolution
  }

  pub fn npm_req_resolver(
    &self,
  ) -> Result<
    &NpmReqResolverRc<
      DenoInNpmPackageChecker,
      DenoIsBuiltInNodeModuleChecker,
      NpmResolver<TSys>,
      TSys,
    >,
    anyhow::Error,
  > {
    self.npm_req_resolver.get_or_try_init(|| {
      Ok(new_rc(NpmReqResolver::new(NpmReqResolverOptions {
        in_npm_pkg_checker: self.in_npm_package_checker()?.clone(),
        node_resolver: self.node_resolver()?.clone(),
        npm_resolver: self.npm_resolver()?.clone(),
        sys: self.sys.clone(),
      })))
    })
  }

  pub fn npm_resolver(&self) -> Result<&NpmResolver<TSys>, anyhow::Error> {
    self.npm_resolver.get_or_try_init(|| {
      Ok(NpmResolver::<TSys>::new::<TSys>(if self.use_byonm()? {
        NpmResolverCreateOptions::Byonm(ByonmNpmResolverCreateOptions {
          sys: self.sys.clone(),
          pkg_json_resolver: self.pkg_json_resolver().clone(),
          root_node_modules_dir: Some(
            match self.workspace_factory.node_modules_dir_path()? {
              Some(node_modules_path) => node_modules_path.to_path_buf(),
              // path needs to be canonicalized for node resolution
              // (node_modules_dir_path above is already canonicalized)
              None => canonicalize_path_maybe_not_exists(
                &self.sys,
                self.workspace_factory.initial_cwd(),
              )?
              .join("node_modules"),
            },
          ),
        })
      } else {
        NpmResolverCreateOptions::Managed(ManagedNpmResolverCreateOptions {
          sys: self.sys.clone(),
          npm_resolution: self.npm_resolution().clone(),
          npm_cache_dir: self.workspace_factory.npm_cache_dir()?.clone(),
          maybe_node_modules_path: self
            .workspace_factory
            .node_modules_dir_path()?
            .map(|p| p.to_path_buf()),
          npm_system_info: self.options.npm_system_info.clone(),
          npmrc: self.workspace_factory.npmrc()?.clone(),
        })
      }))
    })
  }

  pub fn pkg_json_resolver(&self) -> &PackageJsonResolverRc<TSys> {
    self
      .pkg_json_resolver
      .get_or_init(|| new_rc(PackageJsonResolver::new(self.sys.clone())))
  }

  pub fn sloppy_imports_resolver(
    &self,
  ) -> Result<
    Option<&SloppyImportsResolverRc<SloppyImportsCachedFs<TSys>>>,
    anyhow::Error,
  > {
    self
      .sloppy_imports_resolver
      .get_or_try_init(|| {
        let enabled = self.options.unstable_sloppy_imports
          || self
            .workspace_factory
            .workspace_directory()?
            .workspace
            .has_unstable("sloppy-imports");
        if enabled {
          Ok(Some(new_rc(SloppyImportsResolver::new(
            if self.options.no_sloppy_imports_cache {
              SloppyImportsCachedFs::new_without_stat_cache(self.sys.clone())
            } else {
              SloppyImportsCachedFs::new(self.sys.clone())
            },
          ))))
        } else {
          Ok(None)
        }
      })
      .map(|v| v.as_ref())
  }

  pub async fn workspace_resolver(
    &self,
  ) -> Result<&WorkspaceResolverRc, anyhow::Error> {
    self
      .workspace_resolver
      .get_or_try_init_async(async {
        let directory = self.workspace_factory.workspace_directory()?;
        let workspace = &directory.workspace;
        let specified_import_map = match &self.options.specified_import_map {
          Some(import_map) => import_map.get().await?,
          None => None,
        };
        let node_modules_dir_mode =
          self.workspace_factory.resolved_node_modules_dir()?;
        // todo(THIS PR): do not use Disabled for `deno publish`?
        let options = deno_config::workspace::CreateResolverOptions {
          pkg_json_dep_resolution: match node_modules_dir_mode {
            NodeModulesDirMode::Manual => PackageJsonDepResolution::Disabled,
            NodeModulesDirMode::Auto | NodeModulesDirMode::None => {
              // todo(dsherret): this should be disabled for auto?
              PackageJsonDepResolution::Enabled
            }
          },
          specified_import_map,
        };
        let resolver = workspace.create_resolver(&self.sys, options)?;
        if !resolver.diagnostics().is_empty() {
          // todo(dsherret): do not log this in this crate... that should be
          // a CLI responsibility
          log::warn!(
            "Import map diagnostics:\n{}",
            resolver
              .diagnostics()
              .iter()
              .map(|d| format!("  - {d}"))
              .collect::<Vec<_>>()
              .join("\n")
          );
        }
        Ok(new_rc(resolver))
      })
      .await
  }

  pub fn use_byonm(&self) -> Result<bool, anyhow::Error> {
    Ok(
      self.workspace_factory.resolved_node_modules_dir()?
        == NodeModulesDirMode::Manual,
    )
  }
}
