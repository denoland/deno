// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use boxed_error::Boxed;
use deno_cache_dir::DenoDirResolutionError;
use deno_cache_dir::GlobalHttpCacheRc;
use deno_cache_dir::GlobalOrLocalHttpCache;
use deno_cache_dir::LocalHttpCache;
use deno_cache_dir::npm::NpmCacheDir;
use deno_config::deno_json::MinimumDependencyAgeConfig;
use deno_config::deno_json::NewestDependencyDate;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::workspace::FolderConfigs;
use deno_config::workspace::VendorEnablement;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceDirectoryEmptyOptions;
use deno_config::workspace::WorkspaceDirectoryRc;
use deno_config::workspace::WorkspaceDiscoverError;
use deno_config::workspace::WorkspaceDiscoverOptions;
use deno_config::workspace::WorkspaceDiscoverStart;
use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use deno_maybe_sync::new_rc;
pub use deno_npm::NpmSystemInfo;
use deno_npm::resolution::NpmVersionResolver;
use deno_path_util::fs::canonicalize_path_maybe_not_exists;
use futures::future::FutureExt;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolver;
use node_resolver::NodeResolverOptions;
use node_resolver::NodeResolverRc;
use node_resolver::PackageJsonResolver;
use node_resolver::PackageJsonResolverRc;
use node_resolver::analyze::CjsModuleExportAnalyzerRc;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::analyze::NodeCodeTranslatorRc;
use node_resolver::cache::NodeResolutionSys;
use thiserror::Error;
use url::Url;

use crate::DefaultRawDenoResolverRc;
use crate::DenoResolverOptions;
use crate::NodeAndNpmResolvers;
use crate::NpmCacheDirRc;
use crate::RawDenoResolver;
use crate::WorkspaceResolverRc;
use crate::cache::DenoDir;
use crate::cache::DenoDirOptions;
use crate::cache::DenoDirProvider;
use crate::cache::DenoDirProviderRc;
use crate::cache::DenoDirSys;
use crate::cache::EmitCache;
use crate::cache::EmitCacheRc;
use crate::cjs::CjsTracker;
use crate::cjs::CjsTrackerRc;
use crate::cjs::IsCjsResolutionMode;
use crate::cjs::analyzer::DenoCjsCodeAnalyzer;
use crate::cjs::analyzer::NodeAnalysisCacheRc;
use crate::cjs::analyzer::NullNodeAnalysisCache;
use crate::deno_json::CompilerOptionsOverrides;
use crate::deno_json::CompilerOptionsResolver;
use crate::deno_json::CompilerOptionsResolverRc;
use crate::import_map::WorkspaceExternalImportMapLoader;
use crate::import_map::WorkspaceExternalImportMapLoaderRc;
use crate::loader::AllowJsonImports;
use crate::loader::DenoNpmModuleLoaderRc;
use crate::loader::NpmModuleLoader;
use crate::lockfile::LockfileLock;
use crate::lockfile::LockfileLockRc;
use crate::npm::ByonmNpmResolverCreateOptions;
use crate::npm::CreateInNpmPkgCheckerOptions;
use crate::npm::DenoInNpmPackageChecker;
use crate::npm::NpmReqResolver;
use crate::npm::NpmReqResolverOptions;
use crate::npm::NpmReqResolverRc;
use crate::npm::NpmResolver;
use crate::npm::NpmResolverCreateOptions;
use crate::npm::managed::ManagedInNpmPkgCheckerCreateOptions;
use crate::npm::managed::ManagedNpmResolverCreateOptions;
use crate::npm::managed::NpmResolutionCellRc;
use crate::npmrc::NpmRcDiscoverError;
use crate::npmrc::ResolvedNpmRcRc;
use crate::npmrc::discover_npmrc_from_workspace;
use crate::workspace::FsCacheOptions;
use crate::workspace::PackageJsonDepResolution;
use crate::workspace::SloppyImportsOptions;
use crate::workspace::WorkspaceNpmLinkPackagesRc;
use crate::workspace::WorkspaceResolver;

// todo(https://github.com/rust-lang/rust/issues/109737): remove once_cell after get_or_try_init is stabilized
#[cfg(feature = "sync")]
type Deferred<T> = once_cell::sync::OnceCell<T>;
#[cfg(not(feature = "sync"))]
type Deferred<T> = once_cell::unsync::OnceCell<T>;

pub type DenoCjsModuleExportAnalyzerRc<TSys> = CjsModuleExportAnalyzerRc<
  DenoCjsCodeAnalyzer<TSys>,
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  NpmResolver<TSys>,
  TSys,
>;
pub type DenoNodeCodeTranslatorRc<TSys> = NodeCodeTranslatorRc<
  DenoCjsCodeAnalyzer<TSys>,
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  NpmResolver<TSys>,
  TSys,
>;
#[allow(clippy::disallowed_types)]
pub type NpmVersionResolverRc = deno_maybe_sync::MaybeArc<NpmVersionResolver>;
#[cfg(feature = "graph")]
#[allow(clippy::disallowed_types)]
pub type JsrVersionResolverRc =
  deno_maybe_sync::MaybeArc<deno_graph::packages::JsrVersionResolver>;

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

/// Resolves the JSR registry URL to use for the given system.
pub fn resolve_jsr_url(sys: &impl sys_traits::EnvVar) -> Url {
  let env_var_name = "JSR_URL";
  if let Ok(registry_url) = sys.env_var(env_var_name) {
    // ensure there is a trailing slash for the directory
    let registry_url = format!("{}/", registry_url.trim_end_matches('/'));
    match Url::parse(&registry_url) {
      Ok(url) => {
        return url;
      }
      Err(err) => {
        log::debug!("Invalid {} environment variable: {:#}", env_var_name, err,);
      }
    }
  }

  Url::parse("https://jsr.io/").unwrap()
}

#[async_trait::async_trait(?Send)]
pub trait SpecifiedImportMapProvider:
  std::fmt::Debug + MaybeSend + MaybeSync
{
  async fn get(
    &self,
  ) -> Result<Option<crate::workspace::SpecifiedImportMap>, anyhow::Error>;
}

#[derive(Debug)]
pub struct NpmProcessStateOptions {
  pub node_modules_dir: Option<Cow<'static, str>>,
  pub is_byonm: bool,
}

#[derive(Debug, Default)]
pub struct WorkspaceFactoryOptions {
  pub additional_config_file_names: &'static [&'static str],
  pub config_discovery: ConfigDiscoveryOption,
  pub is_package_manager_subcommand: bool,
  pub frozen_lockfile: Option<bool>,
  pub lock_arg: Option<PathBuf>,
  /// Whether to skip writing to the lockfile.
  pub lockfile_skip_write: bool,
  pub maybe_custom_deno_dir_root: Option<PathBuf>,
  pub node_modules_dir: Option<NodeModulesDirMode>,
  pub no_lock: bool,
  pub no_npm: bool,
  /// The process state if using ext/node and the current process was "forked".
  /// This value is found at `deno_lib::args::NPM_PROCESS_STATE`
  /// but in most scenarios this can probably just be `None`.
  pub npm_process_state: Option<NpmProcessStateOptions>,
  /// Override the path to the root node_modules directory.
  pub root_node_modules_dir_override: Option<PathBuf>,
  pub vendor: Option<bool>,
}

#[allow(clippy::disallowed_types)]
pub type WorkspaceFactoryRc<TSys> =
  deno_maybe_sync::MaybeArc<WorkspaceFactory<TSys>>;

#[sys_traits::auto_impl]
pub trait WorkspaceFactorySys:
  DenoDirSys
  + crate::lockfile::LockfileSys
  + crate::npm::NpmResolverSys
  + deno_cache_dir::GlobalHttpCacheSys
  + deno_cache_dir::LocalHttpCacheSys
  + crate::loader::NpmModuleLoaderSys
  + sys_traits::SystemTimeNow
{
}

pub struct WorkspaceFactory<TSys: WorkspaceFactorySys> {
  sys: TSys,
  deno_dir_provider: Deferred<DenoDirProviderRc<TSys>>,
  emit_cache: Deferred<EmitCacheRc<TSys>>,
  global_http_cache: Deferred<GlobalHttpCacheRc<TSys>>,
  http_cache: Deferred<GlobalOrLocalHttpCache<TSys>>,
  jsr_url: Deferred<Url>,
  lockfile: async_once_cell::OnceCell<Option<LockfileLockRc<TSys>>>,
  node_modules_dir_path: Deferred<Option<PathBuf>>,
  npm_cache_dir: Deferred<NpmCacheDirRc>,
  npmrc: Deferred<(ResolvedNpmRcRc, Option<PathBuf>)>,
  node_modules_dir_mode: Deferred<NodeModulesDirMode>,
  workspace_directory: Deferred<WorkspaceDirectoryRc>,
  workspace_external_import_map_loader:
    Deferred<WorkspaceExternalImportMapLoaderRc<TSys>>,
  workspace_npm_link_packages: Deferred<WorkspaceNpmLinkPackagesRc>,
  initial_cwd: PathBuf,
  options: WorkspaceFactoryOptions,
}

impl<TSys: WorkspaceFactorySys> WorkspaceFactory<TSys> {
  pub fn new(
    sys: TSys,
    initial_cwd: PathBuf,
    options: WorkspaceFactoryOptions,
  ) -> Self {
    Self {
      sys,
      deno_dir_provider: Default::default(),
      emit_cache: Default::default(),
      global_http_cache: Default::default(),
      http_cache: Default::default(),
      jsr_url: Default::default(),
      lockfile: Default::default(),
      node_modules_dir_path: Default::default(),
      npm_cache_dir: Default::default(),
      npmrc: Default::default(),
      node_modules_dir_mode: Default::default(),
      workspace_directory: Default::default(),
      workspace_external_import_map_loader: Default::default(),
      workspace_npm_link_packages: Default::default(),
      initial_cwd,
      options,
    }
  }

  pub fn set_workspace_directory(
    &mut self,
    workspace_directory: WorkspaceDirectoryRc,
  ) {
    self.workspace_directory = Deferred::from(workspace_directory);
  }

  pub fn deno_dir_provider(&self) -> &DenoDirProviderRc<TSys> {
    self.deno_dir_provider.get_or_init(|| {
      new_rc(DenoDirProvider::new(
        self.sys.clone(),
        DenoDirOptions {
          maybe_custom_root: self.options.maybe_custom_deno_dir_root.clone(),
        },
      ))
    })
  }

  pub fn emit_cache(&self) -> Result<&EmitCacheRc<TSys>, anyhow::Error> {
    self.emit_cache.get_or_try_init(|| {
      Ok(new_rc(EmitCache::new(
        &self.sys,
        self.deno_dir()?.gen_cache.clone(),
        #[cfg(feature = "deno_ast")]
        Cow::Borrowed(deno_ast::VERSION),
        #[cfg(not(feature = "deno_ast"))]
        Cow::Borrowed(env!("CARGO_PKG_VERSION")),
      )))
    })
  }

  pub fn jsr_url(&self) -> &Url {
    self.jsr_url.get_or_init(|| resolve_jsr_url(&self.sys))
  }

  pub fn initial_cwd(&self) -> &PathBuf {
    &self.initial_cwd
  }

  pub fn no_npm(&self) -> bool {
    self.options.no_npm
  }

  pub fn node_modules_dir_mode(
    &self,
  ) -> Result<NodeModulesDirMode, anyhow::Error> {
    self
      .node_modules_dir_mode
      .get_or_try_init(|| {
        let raw_resolve = || -> Result<_, anyhow::Error> {
          if let Some(process_state) = &self.options.npm_process_state {
            if process_state.is_byonm {
              return Ok(NodeModulesDirMode::Manual);
            }
            if process_state.node_modules_dir.is_some() {
              return Ok(NodeModulesDirMode::Auto);
            } else {
              return Ok(NodeModulesDirMode::None);
            }
          }
          if let Some(flag) = self.options.node_modules_dir {
            return Ok(flag);
          }
          let workspace = &self.workspace_directory()?.workspace;
          if let Some(mode) = workspace.node_modules_dir()? {
            return Ok(mode);
          }

          let workspace = &self.workspace_directory()?.workspace;

          if let Some(pkg_json) = workspace.root_pkg_json() {
            if let Ok(deno_dir) = self.deno_dir() {
              let deno_dir = &deno_dir.root;
              // `deno_dir` can be symlink in macOS or on the CI
              if let Ok(deno_dir) =
                canonicalize_path_maybe_not_exists(&self.sys, deno_dir)
                && pkg_json.path.starts_with(deno_dir)
              {
                // if the package.json is in deno_dir, then do not use node_modules
                // next to it as local node_modules dir
                return Ok(NodeModulesDirMode::None);
              }
            }

            Ok(NodeModulesDirMode::Manual)
          } else if workspace.vendor_dir_path().is_some() {
            Ok(NodeModulesDirMode::Auto)
          } else {
            // use the global cache
            Ok(NodeModulesDirMode::None)
          }
        };

        let mode = raw_resolve()?;
        if mode == NodeModulesDirMode::Manual
          && self.options.is_package_manager_subcommand
        {
          // force using the managed resolver for package management
          // sub commands so that it sets up the node_modules directory
          Ok(NodeModulesDirMode::Auto)
        } else {
          Ok(mode)
        }
      })
      .copied()
  }

  /// Resolves the path to use for a local node_modules folder.
  pub fn node_modules_dir_path(&self) -> Result<Option<&Path>, anyhow::Error> {
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
        if let Some(path) = &self.options.root_node_modules_dir_override {
          return Ok(Some(path.clone()));
        }
        if let Some(process_state) = &self.options.npm_process_state {
          return Ok(
            process_state
              .node_modules_dir
              .as_ref()
              .map(|p| PathBuf::from(p.as_ref())),
          );
        }

        let mode = self.node_modules_dir_mode()?;
        let workspace = &self.workspace_directory()?.workspace;
        let root_folder = workspace.root_folder_configs();
        if !mode.uses_node_modules_dir() {
          return Ok(None);
        }

        let node_modules_dir =
          resolve_from_root(root_folder, &self.initial_cwd);

        Ok(Some(canonicalize_path_maybe_not_exists(
          &self.sys,
          &node_modules_dir,
        )?))
      })
      .map(|p| p.as_deref())
  }

  pub fn deno_dir(&self) -> Result<&DenoDir<TSys>, DenoDirResolutionError> {
    self.deno_dir_provider().get_or_create()
  }

  pub fn global_http_cache(
    &self,
  ) -> Result<&GlobalHttpCacheRc<TSys>, DenoDirResolutionError> {
    self.global_http_cache.get_or_try_init(|| {
      let global_cache_dir = self.deno_dir()?.remote_folder_path();
      let global_http_cache = new_rc(deno_cache_dir::GlobalHttpCache::new(
        self.sys.clone(),
        global_cache_dir,
      ));
      Ok(global_http_cache)
    })
  }

  pub fn http_cache(
    &self,
  ) -> Result<&deno_cache_dir::GlobalOrLocalHttpCache<TSys>, HttpCacheCreateError>
  {
    self.http_cache.get_or_try_init(|| {
      let global_cache = self.global_http_cache()?.clone();
      match self.workspace_directory()?.workspace.vendor_dir_path() {
        Some(local_path) => {
          let local_cache = LocalHttpCache::new(
            local_path.clone(),
            global_cache,
            deno_cache_dir::GlobalToLocalCopy::Allow,
            self.jsr_url().clone(),
          );
          Ok(new_rc(local_cache).into())
        }
        None => Ok(global_cache.into()),
      }
    })
  }

  pub async fn maybe_lockfile(
    &self,
    npm_package_info_provider: &dyn deno_lockfile::NpmPackageInfoProvider,
  ) -> Result<Option<&LockfileLockRc<TSys>>, anyhow::Error> {
    self
      .lockfile
      .get_or_try_init(async move {
        let workspace_directory = self.workspace_directory()?;
        let maybe_external_import_map =
          self.workspace_external_import_map_loader()?.get_or_load()?;

        let maybe_lock_file = LockfileLock::discover(
          self.sys().clone(),
          crate::lockfile::LockfileFlags {
            no_lock: self.options.no_lock,
            frozen_lockfile: self.options.frozen_lockfile,
            lock: self.options.lock_arg.as_ref().map(|path| {
              #[cfg(not(target_arch = "wasm32"))]
              debug_assert!(path.is_absolute());
              path.clone()
            }),
            skip_write: self.options.lockfile_skip_write,
            no_config: matches!(
              self.options.config_discovery,
              ConfigDiscoveryOption::Disabled
            ),
            no_npm: self.options.no_npm,
          },
          &workspace_directory.workspace,
          maybe_external_import_map.as_ref().map(|v| &v.value),
          npm_package_info_provider,
        )
        .await?
        .map(deno_maybe_sync::new_rc);

        Ok(maybe_lock_file)
      })
      .await
      .map(|c| c.as_ref())
  }

  pub fn npm_cache_dir(
    &self,
  ) -> Result<&NpmCacheDirRc, NpmCacheDirCreateError> {
    self.npm_cache_dir.get_or_try_init(|| {
      let npm_cache_dir = self.deno_dir()?.npm_folder_path();
      Ok(new_rc(NpmCacheDir::new(
        &self.sys,
        npm_cache_dir,
        self.npmrc()?.get_all_known_registries_urls(),
      )))
    })
  }

  pub fn npmrc(&self) -> Result<&ResolvedNpmRcRc, NpmRcCreateError> {
    self.npmrc_with_path().map(|(npmrc, _)| npmrc)
  }

  pub fn npmrc_with_path(
    &self,
  ) -> Result<&(ResolvedNpmRcRc, Option<PathBuf>), NpmRcCreateError> {
    self.npmrc.get_or_try_init(|| {
      let (npmrc, path) = discover_npmrc_from_workspace(
        &self.sys,
        &self.workspace_directory()?.workspace,
      )?;
      Ok((new_rc(npmrc), path))
    })
  }

  pub fn sys(&self) -> &TSys {
    &self.sys
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
          WorkspaceDiscoverStart::Paths(std::slice::from_ref(
            &self.initial_cwd,
          )),
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
          #[cfg(not(target_arch = "wasm32"))]
          debug_assert!(path.is_absolute());
          WorkspaceDirectory::discover(
            &self.sys,
            WorkspaceDiscoverStart::ConfigFile(path),
            &resolve_workspace_discover_options(),
          )?
        }
        ConfigDiscoveryOption::Disabled => {
          WorkspaceDirectory::empty(resolve_empty_options())
        }
      };
      Ok(dir)
    })
  }

  pub fn workspace_external_import_map_loader(
    &self,
  ) -> Result<&WorkspaceExternalImportMapLoaderRc<TSys>, WorkspaceDiscoverError>
  {
    self
      .workspace_external_import_map_loader
      .get_or_try_init(|| {
        Ok(new_rc(WorkspaceExternalImportMapLoader::new(
          self.sys().clone(),
          self.workspace_directory()?.workspace.clone(),
        )))
      })
  }

  pub fn workspace_npm_link_packages(
    &self,
  ) -> Result<&WorkspaceNpmLinkPackagesRc, anyhow::Error> {
    self
      .workspace_npm_link_packages
      .get_or_try_init(|| {
        let workspace_dir = self.workspace_directory()?;
        let npm_packages = WorkspaceNpmLinkPackagesRc::from_workspace(
          workspace_dir.workspace.as_ref(),
        );
        if !npm_packages.0.is_empty() && !matches!(self.node_modules_dir_mode()?, NodeModulesDirMode::Auto | NodeModulesDirMode::Manual) {
          bail!("Linking npm packages requires using a node_modules directory. Ensure you have a package.json or set the \"nodeModulesDir\" option to \"auto\" or \"manual\" in your workspace root deno.json.")
        } else {
          Ok(npm_packages)
        }
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

#[derive(Default)]
pub struct ResolverFactoryOptions {
  pub compiler_options_overrides: CompilerOptionsOverrides,
  pub is_cjs_resolution_mode: IsCjsResolutionMode,
  /// Prevents installing packages newer than the specified date.
  pub newest_dependency_date: Option<NewestDependencyDate>,
  pub node_analysis_cache: Option<NodeAnalysisCacheRc>,
  pub node_code_translator_mode: node_resolver::analyze::NodeCodeTranslatorMode,
  pub node_resolver_options: NodeResolverOptions,
  pub node_resolution_cache: Option<node_resolver::NodeResolutionCacheRc>,
  pub npm_system_info: NpmSystemInfo,
  pub package_json_cache: Option<node_resolver::PackageJsonCacheRc>,
  pub package_json_dep_resolution: Option<PackageJsonDepResolution>,
  pub specified_import_map: Option<Box<dyn SpecifiedImportMapProvider>>,
  /// Whether to resolve bare node builtins (ex. "path" as "node:path").
  pub bare_node_builtins: bool,
  pub unstable_sloppy_imports: bool,
  #[cfg(feature = "graph")]
  pub on_mapped_resolution_diagnostic:
    Option<crate::graph::OnMappedResolutionDiagnosticFn>,
  pub allow_json_imports: AllowJsonImports,
  /// Modules loaded via --require flag that should always be treated as CommonJS
  pub require_modules: Vec<Url>,
}

pub struct ResolverFactory<TSys: WorkspaceFactorySys> {
  options: ResolverFactoryOptions,
  sys: NodeResolutionSys<TSys>,
  cjs_module_export_analyzer: Deferred<DenoCjsModuleExportAnalyzerRc<TSys>>,
  cjs_tracker: Deferred<CjsTrackerRc<DenoInNpmPackageChecker, TSys>>,
  compiler_options_resolver: Deferred<CompilerOptionsResolverRc>,
  #[cfg(feature = "graph")]
  deno_resolver:
    async_once_cell::OnceCell<crate::graph::DefaultDenoResolverRc<TSys>>,
  #[cfg(feature = "deno_ast")]
  emitter: Deferred<crate::emit::EmitterRc<DenoInNpmPackageChecker, TSys>>,
  #[cfg(feature = "graph")]
  found_package_json_dep_flag: crate::graph::FoundPackageJsonDepFlagRc,
  in_npm_package_checker: Deferred<DenoInNpmPackageChecker>,
  #[cfg(feature = "graph")]
  jsr_version_resolver: Deferred<JsrVersionResolverRc>,
  minimum_dependency_age: Deferred<MinimumDependencyAgeConfig>,
  node_code_translator: Deferred<DenoNodeCodeTranslatorRc<TSys>>,
  node_resolver: Deferred<
    NodeResolverRc<
      DenoInNpmPackageChecker,
      DenoIsBuiltInNodeModuleChecker,
      NpmResolver<TSys>,
      TSys,
    >,
  >,
  npm_module_loader: Deferred<DenoNpmModuleLoaderRc<TSys>>,
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
  npm_version_resolver: Deferred<NpmVersionResolverRc>,
  #[cfg(feature = "deno_ast")]
  parsed_source_cache: crate::cache::ParsedSourceCacheRc,
  pkg_json_resolver: Deferred<PackageJsonResolverRc<TSys>>,
  #[cfg(all(feature = "graph", feature = "deno_ast"))]
  module_loader: Deferred<crate::loader::ModuleLoaderRc<TSys>>,
  raw_deno_resolver: async_once_cell::OnceCell<DefaultRawDenoResolverRc<TSys>>,
  workspace_factory: WorkspaceFactoryRc<TSys>,
  workspace_resolver: async_once_cell::OnceCell<WorkspaceResolverRc<TSys>>,
}

impl<TSys: WorkspaceFactorySys> ResolverFactory<TSys> {
  pub fn new(
    workspace_factory: WorkspaceFactoryRc<TSys>,
    options: ResolverFactoryOptions,
  ) -> Self {
    Self {
      sys: NodeResolutionSys::new(
        workspace_factory.sys.clone(),
        options.node_resolution_cache.clone(),
      ),
      cjs_module_export_analyzer: Default::default(),
      cjs_tracker: Default::default(),
      compiler_options_resolver: Default::default(),
      raw_deno_resolver: Default::default(),
      #[cfg(feature = "graph")]
      deno_resolver: Default::default(),
      #[cfg(feature = "deno_ast")]
      emitter: Default::default(),
      #[cfg(feature = "graph")]
      found_package_json_dep_flag: Default::default(),
      in_npm_package_checker: Default::default(),
      #[cfg(feature = "graph")]
      jsr_version_resolver: Default::default(),
      minimum_dependency_age: Default::default(),
      node_code_translator: Default::default(),
      node_resolver: Default::default(),
      npm_module_loader: Default::default(),
      npm_req_resolver: Default::default(),
      npm_resolution: Default::default(),
      npm_resolver: Default::default(),
      npm_version_resolver: Default::default(),
      #[cfg(feature = "deno_ast")]
      parsed_source_cache: Default::default(),
      pkg_json_resolver: Default::default(),
      #[cfg(all(feature = "graph", feature = "deno_ast"))]
      module_loader: Default::default(),
      workspace_factory,
      workspace_resolver: Default::default(),
      options,
    }
  }

  pub async fn raw_deno_resolver(
    &self,
  ) -> Result<&DefaultRawDenoResolverRc<TSys>, anyhow::Error> {
    self
      .raw_deno_resolver
      .get_or_try_init(
        async {
          Ok(new_rc(RawDenoResolver::new(DenoResolverOptions {
            in_npm_pkg_checker: self.in_npm_package_checker()?.clone(),
            node_and_req_resolver: if self.workspace_factory.no_npm() {
              None
            } else {
              Some(NodeAndNpmResolvers {
                node_resolver: self.node_resolver()?.clone(),
                npm_resolver: self.npm_resolver()?.clone(),
                npm_req_resolver: self.npm_req_resolver()?.clone(),
              })
            },
            bare_node_builtins: self.bare_node_builtins()?,
            is_byonm: self.use_byonm()?,
            maybe_vendor_dir: self
              .workspace_factory
              .workspace_directory()?
              .workspace
              .vendor_dir_path(),
            workspace_resolver: self.workspace_resolver().await?.clone(),
          })))
        }
        // boxed to prevent the futures getting big and exploding the stack
        .boxed_local(),
      )
      .await
  }

  pub fn cjs_module_export_analyzer(
    &self,
  ) -> Result<&DenoCjsModuleExportAnalyzerRc<TSys>, anyhow::Error> {
    self.cjs_module_export_analyzer.get_or_try_init(|| {
      let code_analyzer = DenoCjsCodeAnalyzer::new(
        self
          .options
          .node_analysis_cache
          .clone()
          .unwrap_or_else(|| new_rc(NullNodeAnalysisCache)),
        self.cjs_tracker()?.clone(),
        #[cfg(feature = "deno_ast")]
        new_rc(crate::cjs::analyzer::DenoAstModuleExportAnalyzer::new(
          self.parsed_source_cache().clone(),
        )),
        #[cfg(not(feature = "deno_ast"))]
        new_rc(crate::cjs::analyzer::NotImplementedModuleExportAnalyzer),
        self.workspace_factory.sys().clone(),
      );
      Ok(new_rc(
        node_resolver::analyze::CjsModuleExportAnalyzer::new(
          code_analyzer,
          self.in_npm_package_checker()?.clone(),
          self.node_resolver()?.clone(),
          self.npm_resolver()?.clone(),
          self.pkg_json_resolver().clone(),
          self.workspace_factory.sys().clone(),
        ),
      ))
    })
  }

  pub fn cjs_tracker(
    &self,
  ) -> Result<&CjsTrackerRc<DenoInNpmPackageChecker, TSys>, anyhow::Error> {
    self.cjs_tracker.get_or_try_init(|| {
      Ok(new_rc(CjsTracker::new(
        self.in_npm_package_checker()?.clone(),
        self.pkg_json_resolver().clone(),
        self.options.is_cjs_resolution_mode,
        self.options.require_modules.clone(),
      )))
    })
  }

  pub fn compiler_options_resolver(
    &self,
  ) -> Result<&CompilerOptionsResolverRc, anyhow::Error> {
    self.compiler_options_resolver.get_or_try_init(|| {
      Ok(new_rc(CompilerOptionsResolver::new(
        &self.sys,
        &self.workspace_factory.workspace_directory()?.workspace,
        self.node_resolver()?,
        &self.workspace_factory.options.config_discovery,
        &self.options.compiler_options_overrides,
      )))
    })
  }

  #[cfg(feature = "deno_ast")]
  pub fn emitter(
    &self,
  ) -> Result<
    &crate::emit::EmitterRc<DenoInNpmPackageChecker, TSys>,
    anyhow::Error,
  > {
    self.emitter.get_or_try_init(|| {
      Ok(new_rc(crate::emit::Emitter::new(
        self.cjs_tracker()?.clone(),
        self.workspace_factory.emit_cache()?.clone(),
        self.parsed_source_cache().clone(),
        self.compiler_options_resolver()?.clone(),
      )))
    })
  }

  #[cfg(feature = "graph")]
  pub fn found_package_json_dep_flag(
    &self,
  ) -> &crate::graph::FoundPackageJsonDepFlagRc {
    &self.found_package_json_dep_flag
  }

  #[cfg(feature = "graph")]
  pub async fn deno_resolver(
    &self,
  ) -> Result<&crate::graph::DefaultDenoResolverRc<TSys>, anyhow::Error> {
    self
      .deno_resolver
      .get_or_try_init(async {
        Ok(new_rc(crate::graph::DenoResolver::new(
          self.raw_deno_resolver().await?.clone(),
          self.workspace_factory.sys.clone(),
          self.found_package_json_dep_flag.clone(),
          self.options.on_mapped_resolution_diagnostic.clone(),
        )))
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

  #[cfg(feature = "graph")]
  pub fn jsr_version_resolver(
    &self,
  ) -> Result<&JsrVersionResolverRc, anyhow::Error> {
    self.jsr_version_resolver.get_or_try_init(|| {
      let minimum_dependency_age_config =
        self.minimum_dependency_age_config()?;
      Ok(new_rc(deno_graph::packages::JsrVersionResolver {
        newest_dependency_date_options:
          deno_graph::packages::NewestDependencyDateOptions {
            date: minimum_dependency_age_config
              .age
              .as_ref()
              .and_then(|d| d.into_option())
              .map(deno_graph::packages::NewestDependencyDate),
            exclude_jsr_pkgs: minimum_dependency_age_config
              .exclude
              .iter()
              .filter_map(|v| v.strip_prefix("jsr:"))
              .map(|v| v.into())
              .collect(),
          },
      }))
    })
  }

  /// The newest allowed dependency date.
  pub fn minimum_dependency_age_config(
    &self,
  ) -> Result<&MinimumDependencyAgeConfig, anyhow::Error> {
    self.minimum_dependency_age.get_or_try_init(|| {
      let config = if let Some(date) = self.options.newest_dependency_date {
        MinimumDependencyAgeConfig {
          age: Some(date),
          exclude: Vec::new(),
        }
      } else {
        let workspace_factory = self.workspace_factory();
        let workspace = &workspace_factory.workspace_directory()?.workspace;
        workspace.minimum_dependency_age(workspace_factory.sys())?
      };
      if let Some(newest_dependency_date) =
        config.age.and_then(|d| d.into_option())
      {
        log::debug!("Newest dependency date: {}", newest_dependency_date);
      }
      Ok(config)
    })
  }

  pub fn node_code_translator(
    &self,
  ) -> Result<&DenoNodeCodeTranslatorRc<TSys>, anyhow::Error> {
    self.node_code_translator.get_or_try_init(|| {
      Ok(new_rc(NodeCodeTranslator::new(
        self.cjs_module_export_analyzer()?.clone(),
        self.options.node_code_translator_mode,
      )))
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
        self.options.node_resolver_options.clone(),
      )))
    })
  }

  pub fn npm_module_loader(
    &self,
  ) -> Result<&DenoNpmModuleLoaderRc<TSys>, anyhow::Error> {
    self.npm_module_loader.get_or_try_init(|| {
      Ok(new_rc(NpmModuleLoader::new(
        self.cjs_tracker()?.clone(),
        self.node_code_translator()?.clone(),
        self.workspace_factory.sys.clone(),
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
        sys: self.workspace_factory.sys.clone(),
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
                &self.workspace_factory.sys,
                self.workspace_factory.initial_cwd(),
              )?
              .join("node_modules"),
            },
          ),
        })
      } else {
        NpmResolverCreateOptions::Managed(ManagedNpmResolverCreateOptions {
          sys: self.workspace_factory.sys.clone(),
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

  pub fn npm_version_resolver(
    &self,
  ) -> Result<&NpmVersionResolverRc, anyhow::Error> {
    self.npm_version_resolver.get_or_try_init(|| {
      let minimum_dependency_age_config =
        self.minimum_dependency_age_config()?;
      Ok(new_rc(NpmVersionResolver {
        types_node_version_req: None,
        newest_dependency_date_options:
          deno_npm::resolution::NewestDependencyDateOptions {
            date: minimum_dependency_age_config
              .age
              .as_ref()
              .and_then(|d| d.into_option())
              .map(deno_npm::resolution::NewestDependencyDate),
            exclude: minimum_dependency_age_config
              .exclude
              .iter()
              .filter_map(|v| v.strip_prefix("npm:"))
              .map(|v| v.into())
              .collect(),
          },
        link_packages: self
          .workspace_factory
          .workspace_npm_link_packages()?
          .0
          .clone(),
      }))
    })
  }

  #[cfg(feature = "deno_ast")]
  pub fn parsed_source_cache(&self) -> &crate::cache::ParsedSourceCacheRc {
    &self.parsed_source_cache
  }

  pub fn pkg_json_resolver(&self) -> &PackageJsonResolverRc<TSys> {
    self.pkg_json_resolver.get_or_init(|| {
      new_rc(PackageJsonResolver::new(
        self.workspace_factory.sys.clone(),
        self.options.package_json_cache.clone(),
      ))
    })
  }

  #[cfg(all(feature = "graph", feature = "deno_ast"))]
  pub fn module_loader(
    &self,
  ) -> Result<&crate::loader::ModuleLoaderRc<TSys>, anyhow::Error> {
    self.module_loader.get_or_try_init(|| {
      let cjs_tracker = self.cjs_tracker()?;
      Ok(new_rc(crate::loader::ModuleLoader::new(
        cjs_tracker.clone(),
        self.emitter()?.clone(),
        self.in_npm_package_checker()?.clone(),
        self.node_code_translator()?.clone(),
        self.npm_module_loader()?.clone(),
        self.parsed_source_cache.clone(),
        self.workspace_factory.sys.clone(),
        self.options.allow_json_imports,
      )))
    })
  }

  pub fn workspace_factory(&self) -> &WorkspaceFactoryRc<TSys> {
    &self.workspace_factory
  }

  pub async fn workspace_resolver(
    &self,
  ) -> Result<&WorkspaceResolverRc<TSys>, anyhow::Error> {
    self
      .workspace_resolver
      .get_or_try_init(
        async {
          let directory = self.workspace_factory.workspace_directory()?;
          let workspace = &directory.workspace;
          let specified_import_map = match &self.options.specified_import_map {
            Some(import_map) => import_map.get().await?,
            None => None,
          };
          let options = crate::workspace::CreateResolverOptions {
            pkg_json_dep_resolution: match self
              .options
              .package_json_dep_resolution
            {
              Some(value) => value,
              None => {
                match self.workspace_factory.node_modules_dir_mode()? {
                  NodeModulesDirMode::Manual => {
                    PackageJsonDepResolution::Disabled
                  }
                  NodeModulesDirMode::Auto | NodeModulesDirMode::None => {
                    // todo(dsherret): should this be disabled for auto?
                    PackageJsonDepResolution::Enabled
                  }
                }
              }
            },
            specified_import_map,
            sloppy_imports_options: if self.options.unstable_sloppy_imports
              || workspace.has_unstable("sloppy-imports")
            {
              SloppyImportsOptions::Enabled
            } else {
              SloppyImportsOptions::Unspecified
            },
            fs_cache_options: FsCacheOptions::Enabled,
          };
          let resolver = WorkspaceResolver::from_workspace(
            workspace,
            self.workspace_factory.sys.clone(),
            options,
          )?;
          resolver.set_compiler_options_resolver(
            self.compiler_options_resolver()?.clone(),
          );
          if !resolver.diagnostics().is_empty() {
            // todo(dsherret): do not log this in this crate... that should be
            // a CLI responsibility
            log::warn!(
              "Resolver diagnostics:\n{}",
              resolver
                .diagnostics()
                .iter()
                .map(|d| format!("  - {d}"))
                .collect::<Vec<_>>()
                .join("\n")
            );
          }
          Ok(new_rc(resolver))
        }
        // boxed to prevent the futures getting big and exploding the stack
        .boxed_local(),
      )
      .await
  }

  pub fn bare_node_builtins(&self) -> Result<bool, anyhow::Error> {
    Ok(
      self.options.bare_node_builtins
        || self
          .workspace_factory
          .workspace_directory()?
          .workspace
          .has_unstable("bare-node-builtins"),
    )
  }

  pub fn npm_system_info(&self) -> &NpmSystemInfo {
    &self.options.npm_system_info
  }

  pub fn use_byonm(&self) -> Result<bool, anyhow::Error> {
    Ok(
      self.workspace_factory.node_modules_dir_mode()?
        == NodeModulesDirMode::Manual,
    )
  }
}
