// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_npm::resolution::PackageIdNotFoundError;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm_cache::NpmCache;
use deno_npm_cache::NpmCacheHttpClient;
use deno_npm_cache::NpmCacheSetting;
use deno_npm_cache::RegistryInfoProvider;
use deno_npm_cache::TarballCache;
use deno_resolver::factory::ResolverFactory;
use deno_resolver::factory::WorkspaceFactory;
use deno_resolver::factory::WorkspaceFactorySys;
use deno_resolver::lockfile::LockfileLock;
use deno_resolver::lockfile::LockfileNpmPackageInfoApiAdapter;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageKind;
use deno_semver::package::PackageReq;
use futures::FutureExt;

use crate::LifecycleScriptsConfig;
use crate::NpmInstaller;
use crate::NpmInstallerOptions;
use crate::Reporter;
use crate::graph::NpmCachingStrategy;
use crate::graph::NpmDenoGraphResolver;
use crate::initializer::NpmResolutionInitializer;
use crate::initializer::NpmResolverManagedSnapshotOption;
use crate::lifecycle_scripts::LifecycleScriptsExecutor;
use crate::package_json::NpmInstallDepsProvider;
use crate::resolution::HasJsExecutionStartedFlagRc;
use crate::resolution::NpmResolutionInstaller;

// todo(https://github.com/rust-lang/rust/issues/109737): remove once_cell after get_or_try_init is stabilized
type Deferred<T> = once_cell::sync::OnceCell<T>;

#[sys_traits::auto_impl]
pub trait NpmInstallerFactorySys:
  crate::NpmInstallerSys + WorkspaceFactorySys
{
}

type ResolveNpmResolutionSnapshotFn = Box<
  dyn Fn() -> Result<
    Option<ValidSerializedNpmResolutionSnapshot>,
    PackageIdNotFoundError,
  >,
>;

pub struct NpmInstallerFactoryOptions {
  pub cache_setting: NpmCacheSetting,
  pub caching_strategy: NpmCachingStrategy,
  pub lifecycle_scripts_config: LifecycleScriptsConfig,
  /// Resolves the npm resolution snapshot from the environment.
  pub resolve_npm_resolution_snapshot: ResolveNpmResolutionSnapshotFn,
}

pub trait InstallReporter:
  deno_npm::resolution::Reporter
  + deno_graph::source::Reporter
  + deno_npm_cache::TarballCacheReporter
  + crate::InstallProgressReporter
{
}

impl<
  T: deno_npm::resolution::Reporter
    + deno_graph::source::Reporter
    + deno_npm_cache::TarballCacheReporter
    + crate::InstallProgressReporter,
> InstallReporter for T
{
}

pub struct NpmInstallerFactory<
  TNpmCacheHttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: NpmInstallerFactorySys,
> {
  resolver_factory: Arc<ResolverFactory<TSys>>,
  has_js_execution_started_flag: HasJsExecutionStartedFlagRc,
  http_client: Arc<TNpmCacheHttpClient>,
  lifecycle_scripts_config: Deferred<Arc<LifecycleScriptsConfig>>,
  lifecycle_scripts_executor: Arc<dyn LifecycleScriptsExecutor>,
  reporter: TReporter,
  lockfile_npm_package_info_provider:
    Deferred<LockfileNpmPackageInfoApiAdapter>,
  npm_cache: Deferred<Arc<NpmCache<TSys>>>,
  npm_deno_graph_resolver: async_once_cell::OnceCell<
    Arc<NpmDenoGraphResolver<TNpmCacheHttpClient, TSys>>,
  >,
  npm_installer:
    async_once_cell::OnceCell<Arc<NpmInstaller<TNpmCacheHttpClient, TSys>>>,
  npm_resolution_initializer:
    async_once_cell::OnceCell<Arc<NpmResolutionInitializer<TSys>>>,
  npm_resolution_installer: async_once_cell::OnceCell<
    Arc<NpmResolutionInstaller<TNpmCacheHttpClient, TSys>>,
  >,
  registry_info_provider:
    Deferred<Arc<RegistryInfoProvider<TNpmCacheHttpClient, TSys>>>,
  tarball_cache: Deferred<Arc<TarballCache<TNpmCacheHttpClient, TSys>>>,
  options: NpmInstallerFactoryOptions,
  install_reporter: Option<Arc<dyn InstallReporter + 'static>>,
}

impl<
  TNpmCacheHttpClient: NpmCacheHttpClient,
  TReporter: Reporter,
  TSys: NpmInstallerFactorySys,
> NpmInstallerFactory<TNpmCacheHttpClient, TReporter, TSys>
{
  pub fn new(
    resolver_factory: Arc<ResolverFactory<TSys>>,
    http_client: Arc<TNpmCacheHttpClient>,
    lifecycle_scripts_executor: Arc<dyn LifecycleScriptsExecutor>,
    reporter: TReporter,
    install_reporter: Option<Arc<dyn InstallReporter + 'static>>,
    options: NpmInstallerFactoryOptions,
  ) -> Self {
    Self {
      resolver_factory,
      has_js_execution_started_flag: Default::default(),
      http_client,
      lifecycle_scripts_config: Default::default(),
      lifecycle_scripts_executor,
      reporter,
      lockfile_npm_package_info_provider: Default::default(),
      npm_cache: Default::default(),
      npm_deno_graph_resolver: Default::default(),
      npm_installer: Default::default(),
      npm_resolution_initializer: Default::default(),
      npm_resolution_installer: Default::default(),
      registry_info_provider: Default::default(),
      tarball_cache: Default::default(),
      install_reporter,
      options,
    }
  }

  pub fn has_js_execution_started_flag(&self) -> &HasJsExecutionStartedFlagRc {
    &self.has_js_execution_started_flag
  }

  pub fn http_client(&self) -> &Arc<TNpmCacheHttpClient> {
    &self.http_client
  }

  pub async fn initialize_npm_resolution_if_managed(
    &self,
  ) -> Result<(), anyhow::Error> {
    let npm_resolver = self.resolver_factory().npm_resolver()?;
    if npm_resolver.is_managed() {
      self
        .npm_resolution_initializer()
        .await?
        .ensure_initialized()
        .await?;
    }
    Ok(())
  }

  pub fn lifecycle_scripts_config(
    &self,
  ) -> Result<&Arc<LifecycleScriptsConfig>, anyhow::Error> {
    use crate::PackagesAllowedScripts;

    fn jsr_deps_to_reqs(deps: Vec<JsrDepPackageReq>) -> Vec<PackageReq> {
      deps
        .into_iter()
        .filter_map(|p| {
          if p.kind == PackageKind::Npm {
            Some(p.req)
          } else {
            None
          }
        })
        .collect::<Vec<_>>()
    }

    self.lifecycle_scripts_config.get_or_try_init(|| {
      let workspace_factory = self.workspace_factory();
      let workspace = &workspace_factory.workspace_directory()?.workspace;
      let allow_scripts = workspace.allow_scripts()?;
      let args = &self.options.lifecycle_scripts_config;
      Ok(Arc::new(LifecycleScriptsConfig {
        allowed: match &args.allowed {
          PackagesAllowedScripts::All => PackagesAllowedScripts::All,
          PackagesAllowedScripts::Some(package_reqs) => {
            PackagesAllowedScripts::Some(package_reqs.clone())
          }
          PackagesAllowedScripts::None => match allow_scripts.allow {
            deno_config::deno_json::AllowScriptsValueConfig::All => {
              PackagesAllowedScripts::All
            }
            deno_config::deno_json::AllowScriptsValueConfig::Limited(deps) => {
              let reqs = jsr_deps_to_reqs(deps);
              if reqs.is_empty() {
                PackagesAllowedScripts::None
              } else {
                PackagesAllowedScripts::Some(reqs)
              }
            }
          },
        },
        denied: match &args.allowed {
          PackagesAllowedScripts::All | PackagesAllowedScripts::Some(_) => {
            vec![]
          }
          PackagesAllowedScripts::None => jsr_deps_to_reqs(allow_scripts.deny),
        },
        initial_cwd: args.initial_cwd.clone(),
        root_dir: args.root_dir.clone(),
        explicit_install: args.explicit_install,
      }))
    })
  }

  pub fn lockfile_npm_package_info_provider(
    &self,
  ) -> Result<&LockfileNpmPackageInfoApiAdapter, anyhow::Error> {
    self.lockfile_npm_package_info_provider.get_or_try_init(|| {
      Ok(LockfileNpmPackageInfoApiAdapter::new(
        self.registry_info_provider()?.clone(),
        self
          .workspace_factory()
          .workspace_npm_link_packages()?
          .clone(),
      ))
    })
  }

  pub async fn maybe_lockfile(
    &self,
  ) -> Result<Option<&Arc<LockfileLock<TSys>>>, anyhow::Error> {
    let workspace_factory = self.workspace_factory();
    let package_info_provider = self.lockfile_npm_package_info_provider()?;
    workspace_factory
      .maybe_lockfile(package_info_provider)
      .await
  }

  pub fn npm_cache(&self) -> Result<&Arc<NpmCache<TSys>>, anyhow::Error> {
    self.npm_cache.get_or_try_init(|| {
      let workspace_factory = self.workspace_factory();
      Ok(Arc::new(NpmCache::new(
        workspace_factory.npm_cache_dir()?.clone(),
        workspace_factory.sys().clone(),
        self.options.cache_setting.clone(),
        workspace_factory.npmrc()?.clone(),
      )))
    })
  }

  pub async fn npm_deno_graph_resolver(
    &self,
  ) -> Result<
    &Arc<NpmDenoGraphResolver<TNpmCacheHttpClient, TSys>>,
    anyhow::Error,
  > {
    self
      .npm_deno_graph_resolver
      .get_or_try_init(
        async {
          Ok(Arc::new(NpmDenoGraphResolver::new(
            self.npm_installer_if_managed().await?.cloned(),
            self
              .resolver_factory()
              .found_package_json_dep_flag()
              .clone(),
            self.options.caching_strategy,
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub async fn npm_resolution_initializer(
    &self,
  ) -> Result<&Arc<NpmResolutionInitializer<TSys>>, anyhow::Error> {
    self
      .npm_resolution_initializer
      .get_or_try_init(async move {
        let workspace_factory = self.workspace_factory();
        Ok(Arc::new(NpmResolutionInitializer::new(
          self.resolver_factory.npm_resolution().clone(),
          workspace_factory.workspace_npm_link_packages()?.clone(),
          match (self.options.resolve_npm_resolution_snapshot)()? {
            Some(snapshot) => {
              NpmResolverManagedSnapshotOption::Specified(Some(snapshot))
            }
            None => match self.maybe_lockfile().await? {
              Some(lockfile) => {
                NpmResolverManagedSnapshotOption::ResolveFromLockfile(
                  lockfile.clone(),
                )
              }
              None => NpmResolverManagedSnapshotOption::Specified(None),
            },
          },
        )))
      })
      .await
  }

  pub async fn npm_resolution_installer(
    &self,
  ) -> Result<
    &Arc<NpmResolutionInstaller<TNpmCacheHttpClient, TSys>>,
    anyhow::Error,
  > {
    self
      .npm_resolution_installer
      .get_or_try_init(async move {
        Ok(Arc::new(NpmResolutionInstaller::new(
          self.has_js_execution_started_flag.clone(),
          self.resolver_factory.npm_version_resolver()?.clone(),
          self.registry_info_provider()?.clone(),
          self
            .install_reporter
            .as_ref()
            .map(|r| r.clone() as Arc<dyn deno_npm::resolution::Reporter>),
          self.resolver_factory.npm_resolution().clone(),
          self.maybe_lockfile().await?.cloned(),
        )))
      })
      .await
  }

  pub async fn npm_installer_if_managed(
    &self,
  ) -> Result<
    Option<&Arc<NpmInstaller<TNpmCacheHttpClient, TSys>>>,
    anyhow::Error,
  > {
    if self.resolver_factory().use_byonm()? || self.workspace_factory().no_npm()
    {
      Ok(None)
    } else {
      Ok(Some(self.npm_installer().await?))
    }
  }

  pub async fn npm_installer(
    &self,
  ) -> Result<&Arc<NpmInstaller<TNpmCacheHttpClient, TSys>>, anyhow::Error> {
    self
      .npm_installer
      .get_or_try_init(
        async move {
          let workspace_factory = self.workspace_factory();
          let npm_cache = self.npm_cache()?;
          let registry_info_provider = self.registry_info_provider()?;
          let workspace_npm_link_packages =
            workspace_factory.workspace_npm_link_packages()?;
          Ok(Arc::new(NpmInstaller::new(
            self.install_reporter.clone(),
            self.lifecycle_scripts_executor.clone(),
            npm_cache.clone(),
            Arc::new(NpmInstallDepsProvider::from_workspace(
              &workspace_factory.workspace_directory()?.workspace,
            )),
            registry_info_provider.clone(),
            self.resolver_factory.npm_resolution().clone(),
            self.npm_resolution_initializer().await?.clone(),
            self.npm_resolution_installer().await?.clone(),
            &self.reporter,
            workspace_factory.sys().clone(),
            self.tarball_cache()?.clone(),
            NpmInstallerOptions {
              maybe_lockfile: self.maybe_lockfile().await?.cloned(),
              maybe_node_modules_path: workspace_factory
                .node_modules_dir_path()?
                .map(|p| p.to_path_buf()),
              lifecycle_scripts: self.lifecycle_scripts_config()?.clone(),
              system_info: self.resolver_factory.npm_system_info().clone(),
              workspace_link_packages: workspace_npm_link_packages.clone(),
            },
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub fn registry_info_provider(
    &self,
  ) -> Result<
    &Arc<RegistryInfoProvider<TNpmCacheHttpClient, TSys>>,
    anyhow::Error,
  > {
    self.registry_info_provider.get_or_try_init(|| {
      Ok(Arc::new(RegistryInfoProvider::new(
        self.npm_cache()?.clone(),
        self.http_client().clone(),
        self.workspace_factory().npmrc()?.clone(),
      )))
    })
  }

  pub fn tarball_cache(
    &self,
  ) -> Result<&Arc<TarballCache<TNpmCacheHttpClient, TSys>>, anyhow::Error> {
    self.tarball_cache.get_or_try_init(|| {
      let workspace_factory = self.workspace_factory();
      Ok(Arc::new(TarballCache::new(
        self.npm_cache()?.clone(),
        self.http_client.clone(),
        workspace_factory.sys().clone(),
        workspace_factory.npmrc()?.clone(),
        self
          .install_reporter
          .as_ref()
          .map(|r| r.clone() as Arc<dyn deno_npm_cache::TarballCacheReporter>),
      )))
    })
  }

  pub fn resolver_factory(&self) -> &Arc<ResolverFactory<TSys>> {
    &self.resolver_factory
  }

  pub fn workspace_factory(&self) -> &Arc<WorkspaceFactory<TSys>> {
    self.resolver_factory.workspace_factory()
  }
}
