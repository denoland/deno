// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;
use deno_ast::MediaType;
use deno_cache_dir::npm::NpmCacheDir;
use deno_cache_dir::HttpCache;
use deno_config::workspace::JsxImportSourceConfig;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_graph::GraphImport;
use deno_graph::ModuleSpecifier;
use deno_graph::Range;
use deno_npm::NpmSystemInfo;
use deno_npm_cache::TarballCache;
use deno_path_util::url_to_file_path;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::npm::managed::ManagedInNpmPkgCheckerCreateOptions;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::npm::CreateInNpmPkgCheckerOptions;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmReqResolverOptions;
use deno_resolver::npmrc::create_default_npmrc;
use deno_resolver::workspace::PackageJsonDepResolution;
use deno_resolver::workspace::WorkspaceResolver;
use deno_resolver::DenoResolverOptions;
use deno_resolver::NodeAndNpmReqResolver;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use node_resolver::cache::NodeResolutionSys;
use node_resolver::cache::NodeResolutionThreadLocalCache;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::PackageJsonThreadLocalCache;
use node_resolver::ResolutionMode;

use super::cache::LspCache;
use super::jsr::JsrCacheResolver;
use crate::args::CliLockfile;
use crate::args::LifecycleScriptsConfig;
use crate::args::NpmCachingStrategy;
use crate::args::NpmInstallDepsProvider;
use crate::factory::Deferred;
use crate::graph_util::to_node_resolution_kind;
use crate::graph_util::to_node_resolution_mode;
use crate::graph_util::CliJsrUrlProvider;
use crate::http_util::HttpClientProvider;
use crate::lsp::config::Config;
use crate::lsp::config::ConfigData;
use crate::lsp::logging::lsp_warn;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::installer::NpmInstaller;
use crate::npm::installer::NpmResolutionInstaller;
use crate::npm::CliByonmNpmResolverCreateOptions;
use crate::npm::CliManagedNpmResolver;
use crate::npm::CliManagedNpmResolverCreateOptions;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverCreateOptions;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::npm::NpmResolutionInitializer;
use crate::resolver::CliDenoResolver;
use crate::resolver::CliIsCjsResolver;
use crate::resolver::CliNpmGraphResolver;
use crate::resolver::CliNpmReqResolver;
use crate::resolver::CliResolver;
use crate::resolver::FoundPackageJsonDepFlag;
use crate::sys::CliSys;
use crate::tsc::into_specifier_and_media_type;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

#[derive(Debug, Clone)]
struct LspScopeResolver {
  resolver: Arc<CliResolver>,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  is_cjs_resolver: Arc<CliIsCjsResolver>,
  jsr_resolver: Option<Arc<JsrCacheResolver>>,
  npm_graph_resolver: Arc<CliNpmGraphResolver>,
  npm_installer: Option<Arc<NpmInstaller>>,
  npm_resolution: Arc<NpmResolutionCell>,
  npm_resolver: Option<CliNpmResolver>,
  node_resolver: Option<Arc<CliNodeResolver>>,
  npm_pkg_req_resolver: Option<Arc<CliNpmReqResolver>>,
  pkg_json_resolver: Arc<CliPackageJsonResolver>,
  redirect_resolver: Option<Arc<RedirectResolver>>,
  graph_imports: Arc<IndexMap<ModuleSpecifier, GraphImport>>,
  dep_info: Arc<Mutex<Arc<ScopeDepInfo>>>,
  package_json_deps_by_resolution: Arc<IndexMap<ModuleSpecifier, String>>,
  config_data: Option<Arc<ConfigData>>,
}

impl Default for LspScopeResolver {
  fn default() -> Self {
    let factory = ResolverFactory::new(None);
    Self {
      resolver: factory.cli_resolver().clone(),
      in_npm_pkg_checker: factory.in_npm_pkg_checker().clone(),
      is_cjs_resolver: factory.is_cjs_resolver().clone(),
      jsr_resolver: None,
      npm_graph_resolver: factory.npm_graph_resolver().clone(),
      npm_installer: None,
      npm_resolver: None,
      node_resolver: None,
      npm_resolution: factory.services.npm_resolution.clone(),
      npm_pkg_req_resolver: None,
      pkg_json_resolver: factory.pkg_json_resolver().clone(),
      redirect_resolver: None,
      graph_imports: Default::default(),
      dep_info: Default::default(),
      package_json_deps_by_resolution: Default::default(),
      config_data: None,
    }
  }
}

impl LspScopeResolver {
  async fn from_config_data(
    config_data: Option<&Arc<ConfigData>>,
    cache: &LspCache,
    http_client_provider: Option<&Arc<HttpClientProvider>>,
  ) -> Self {
    let mut factory = ResolverFactory::new(config_data);
    if let Some(http_client_provider) = http_client_provider {
      factory.init_npm_resolver(http_client_provider, cache).await;
    }
    let in_npm_pkg_checker = factory.in_npm_pkg_checker().clone();
    let npm_resolver = factory.npm_resolver().cloned();
    let npm_installer = factory.npm_installer().cloned();
    let node_resolver = factory.node_resolver().cloned();
    let npm_pkg_req_resolver = factory.npm_pkg_req_resolver().cloned();
    let cli_resolver = factory.cli_resolver().clone();
    let pkg_json_resolver = factory.pkg_json_resolver().clone();
    let jsr_resolver = Some(Arc::new(JsrCacheResolver::new(
      cache.for_specifier(config_data.map(|d| d.scope.as_ref())),
      config_data.map(|d| d.as_ref()),
    )));
    let redirect_resolver = Some(Arc::new(RedirectResolver::new(
      cache.for_specifier(config_data.map(|d| d.scope.as_ref())),
      config_data.and_then(|d| d.lockfile.clone()),
    )));
    let npm_graph_resolver = factory.npm_graph_resolver();
    let maybe_jsx_import_source_config =
      config_data.and_then(|d| d.maybe_jsx_import_source_config());
    let graph_imports = config_data
      .and_then(|d| d.member_dir.to_compiler_option_types().ok())
      .map(|imports| {
        Arc::new(
          imports
            .into_iter()
            .map(|(referrer, imports)| {
              let resolver = SingleReferrerGraphResolver {
                valid_referrer: &referrer,
                module_resolution_mode: ResolutionMode::Import,
                cli_resolver: &cli_resolver,
                jsx_import_source_config: maybe_jsx_import_source_config
                  .as_ref(),
              };
              let graph_import = GraphImport::new(
                &referrer,
                imports,
                &CliJsrUrlProvider,
                Some(&resolver),
                Some(npm_graph_resolver.as_ref()),
              );
              (referrer, graph_import)
            })
            .collect(),
        )
      })
      .unwrap_or_default();
    let package_json_deps_by_resolution = (|| {
      let npm_pkg_req_resolver = npm_pkg_req_resolver.as_ref()?;
      let package_json = config_data?.maybe_pkg_json()?;
      let referrer = package_json.specifier();
      let dependencies = package_json.dependencies.as_ref()?;
      let result = dependencies
        .iter()
        .flat_map(|(name, _)| {
          let mut deps = Vec::with_capacity(2);
          let Some(req_ref) =
            NpmPackageReqReference::from_str(&format!("npm:{name}")).ok()
          else {
            return vec![];
          };
          for kind in [NodeResolutionKind::Types, NodeResolutionKind::Execution]
          {
            let Some(req) = npm_pkg_req_resolver
              .resolve_req_reference(
                &req_ref,
                &referrer,
                // todo(dsherret): this is wrong because it doesn't consider CJS referrers
                ResolutionMode::Import,
                kind,
              )
              .ok()
            else {
              continue;
            };

            let Some(url) = req.into_url().ok() else {
              continue;
            };
            let specifier = into_specifier_and_media_type(Some(url)).0;
            deps.push((specifier, name.clone()))
          }
          deps
        })
        .collect();
      Some(result)
    })();
    let package_json_deps_by_resolution =
      Arc::new(package_json_deps_by_resolution.unwrap_or_default());
    Self {
      resolver: cli_resolver,
      in_npm_pkg_checker,
      is_cjs_resolver: factory.is_cjs_resolver().clone(),
      jsr_resolver,
      npm_graph_resolver: factory.npm_graph_resolver().clone(),
      npm_pkg_req_resolver,
      npm_resolver,
      npm_installer,
      npm_resolution: factory.services.npm_resolution.clone(),
      node_resolver,
      pkg_json_resolver,
      redirect_resolver,
      graph_imports,
      dep_info: Default::default(),
      package_json_deps_by_resolution,
      config_data: config_data.cloned(),
    }
  }

  fn snapshot(&self) -> Arc<Self> {
    // create a copy of the resolution and then re-initialize the npm resolver from that
    // todo(dsherret): this is pretty terrible... we should improve this. It should
    // be possible to just change the npm_resolution on the new factory then access
    // another method to create a new npm resolver
    let mut factory = ResolverFactory::new(self.config_data.as_ref());
    factory
      .services
      .npm_resolution
      .set_snapshot(self.npm_resolution.snapshot());
    let npm_resolver = self.npm_resolver.as_ref();
    if let Some(npm_resolver) = &npm_resolver {
      factory.set_npm_resolver(CliNpmResolver::new::<CliSys>(
        match npm_resolver {
          CliNpmResolver::Byonm(byonm_npm_resolver) => {
            CliNpmResolverCreateOptions::Byonm(
              CliByonmNpmResolverCreateOptions {
                root_node_modules_dir: byonm_npm_resolver
                  .root_node_modules_path()
                  .map(|p| p.to_path_buf()),
                sys: factory.node_resolution_sys.clone(),
                pkg_json_resolver: self.pkg_json_resolver.clone(),
              },
            )
          }
          CliNpmResolver::Managed(managed_npm_resolver) => {
            CliNpmResolverCreateOptions::Managed({
              let sys = CliSys::default();
              let npmrc = self
                .config_data
                .as_ref()
                .and_then(|d| d.npmrc.clone())
                .unwrap_or_else(|| Arc::new(create_default_npmrc(&sys)));
              let npm_cache_dir = Arc::new(NpmCacheDir::new(
                &sys,
                managed_npm_resolver.global_cache_root_path().to_path_buf(),
                npmrc.get_all_known_registries_urls(),
              ));
              CliManagedNpmResolverCreateOptions {
                sys,
                npm_cache_dir,
                maybe_node_modules_path: managed_npm_resolver
                  .root_node_modules_path()
                  .map(|p| p.to_path_buf()),
                npmrc,
                npm_resolution: factory.services.npm_resolution.clone(),
                npm_system_info: NpmSystemInfo::default(),
              }
            })
          }
        },
      ));
    }

    Arc::new(Self {
      resolver: factory.cli_resolver().clone(),
      in_npm_pkg_checker: factory.in_npm_pkg_checker().clone(),
      is_cjs_resolver: factory.is_cjs_resolver().clone(),
      jsr_resolver: self.jsr_resolver.clone(),
      npm_graph_resolver: factory.npm_graph_resolver().clone(),
      // npm installer isn't necessary for a snapshot
      npm_installer: None,
      npm_pkg_req_resolver: factory.npm_pkg_req_resolver().cloned(),
      npm_resolution: factory.services.npm_resolution.clone(),
      npm_resolver: factory.npm_resolver().cloned(),
      node_resolver: factory.node_resolver().cloned(),
      redirect_resolver: self.redirect_resolver.clone(),
      pkg_json_resolver: factory.pkg_json_resolver().clone(),
      graph_imports: self.graph_imports.clone(),
      dep_info: self.dep_info.clone(),
      package_json_deps_by_resolution: self
        .package_json_deps_by_resolution
        .clone(),
      config_data: self.config_data.clone(),
    })
  }
}

#[derive(Debug, Default, Clone)]
pub struct LspResolver {
  unscoped: Arc<LspScopeResolver>,
  by_scope: BTreeMap<ModuleSpecifier, Arc<LspScopeResolver>>,
}

impl LspResolver {
  pub async fn from_config(
    config: &Config,
    cache: &LspCache,
    http_client_provider: Option<&Arc<HttpClientProvider>>,
  ) -> Self {
    let mut by_scope = BTreeMap::new();
    for (scope, config_data) in config.tree.data_by_scope().as_ref() {
      by_scope.insert(
        scope.clone(),
        Arc::new(
          LspScopeResolver::from_config_data(
            Some(config_data),
            cache,
            http_client_provider,
          )
          .await,
        ),
      );
    }
    Self {
      unscoped: Arc::new(
        LspScopeResolver::from_config_data(None, cache, http_client_provider)
          .await,
      ),
      by_scope,
    }
  }

  pub fn snapshot(&self) -> Arc<Self> {
    Arc::new(Self {
      unscoped: self.unscoped.snapshot(),
      by_scope: self
        .by_scope
        .iter()
        .map(|(s, r)| (s.clone(), r.snapshot()))
        .collect(),
    })
  }

  pub fn did_cache(&self) {
    for resolver in
      std::iter::once(&self.unscoped).chain(self.by_scope.values())
    {
      resolver.jsr_resolver.as_ref().inspect(|r| r.did_cache());
      resolver
        .redirect_resolver
        .as_ref()
        .inspect(|r| r.did_cache());
    }
  }

  pub async fn set_dep_info_by_scope(
    &self,
    dep_info_by_scope: &Arc<
      BTreeMap<Option<ModuleSpecifier>, Arc<ScopeDepInfo>>,
    >,
  ) {
    for (scope, resolver) in [(None, &self.unscoped)]
      .into_iter()
      .chain(self.by_scope.iter().map(|(s, r)| (Some(s), r)))
    {
      let dep_info = dep_info_by_scope.get(&scope.cloned());
      if let Some(dep_info) = dep_info {
        *resolver.dep_info.lock() = dep_info.clone();
      }
      if let Some(npm_installer) = resolver.npm_installer.as_ref() {
        let reqs = dep_info
          .map(|i| i.npm_reqs.iter().cloned().collect::<Vec<_>>())
          .unwrap_or_default();
        if let Err(err) = npm_installer.set_package_reqs(&reqs).await {
          lsp_warn!("Could not set npm package requirements: {:#}", err);
        }
      }
    }
  }

  pub fn as_cli_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> &CliResolver {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.resolver.as_ref()
  }

  pub fn as_graph_npm_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> &Arc<CliNpmGraphResolver> {
    let resolver = self.get_scope_resolver(file_referrer);
    &resolver.npm_graph_resolver
  }

  pub fn as_is_cjs_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> &CliIsCjsResolver {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.is_cjs_resolver.as_ref()
  }

  pub fn as_config_data(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<&Arc<ConfigData>> {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.config_data.as_ref()
  }

  pub fn in_npm_pkg_checker(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> &DenoInNpmPackageChecker {
    let resolver = self.get_scope_resolver(file_referrer);
    &resolver.in_npm_pkg_checker
  }

  pub fn maybe_managed_npm_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<&CliManagedNpmResolver> {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.npm_resolver.as_ref().and_then(|r| r.as_managed())
  }

  pub fn pkg_json_resolver(
    &self,
    referrer: &ModuleSpecifier,
  ) -> &Arc<CliPackageJsonResolver> {
    let resolver = self.get_scope_resolver(Some(referrer));
    &resolver.pkg_json_resolver
  }

  pub fn graph_imports_by_referrer(
    &self,
    file_referrer: &ModuleSpecifier,
  ) -> IndexMap<&ModuleSpecifier, Vec<&ModuleSpecifier>> {
    let resolver = self.get_scope_resolver(Some(file_referrer));
    resolver
      .graph_imports
      .iter()
      .map(|(s, i)| {
        (
          s,
          i.dependencies
            .values()
            .flat_map(|d| d.get_type().or_else(|| d.get_code()))
            .collect(),
        )
      })
      .collect()
  }

  pub fn jsr_to_resource_url(
    &self,
    req_ref: &JsrPackageReqReference,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<ModuleSpecifier> {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.jsr_resolver.as_ref()?.jsr_to_resource_url(req_ref)
  }

  pub fn jsr_lookup_export_for_path(
    &self,
    nv: &PackageNv,
    path: &str,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<String> {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver
      .jsr_resolver
      .as_ref()?
      .lookup_export_for_path(nv, path)
  }

  pub fn jsr_lookup_req_for_nv(
    &self,
    nv: &PackageNv,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<PackageReq> {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.jsr_resolver.as_ref()?.lookup_req_for_nv(nv)
  }

  pub fn npm_to_file_url(
    &self,
    req_ref: &NpmPackageReqReference,
    referrer: &ModuleSpecifier,
    resolution_mode: ResolutionMode,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let resolver = self.get_scope_resolver(file_referrer);
    let npm_pkg_req_resolver = resolver.npm_pkg_req_resolver.as_ref()?;
    Some(into_specifier_and_media_type(Some(
      npm_pkg_req_resolver
        .resolve_req_reference(
          req_ref,
          referrer,
          resolution_mode,
          NodeResolutionKind::Types,
        )
        .ok()?
        .into_url()
        .ok()?,
    )))
  }

  pub fn file_url_to_package_json_dep(
    &self,
    specifier: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<String> {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver
      .package_json_deps_by_resolution
      .get(specifier)
      .cloned()
  }

  pub fn deno_types_to_code_resolution(
    &self,
    specifier: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<ModuleSpecifier> {
    let resolver = self.get_scope_resolver(file_referrer);
    let dep_info = resolver.dep_info.lock().clone();
    dep_info
      .deno_types_to_code_resolutions
      .get(specifier)
      .cloned()
  }

  pub fn in_node_modules(&self, specifier: &ModuleSpecifier) -> bool {
    fn has_node_modules_dir(specifier: &ModuleSpecifier) -> bool {
      // consider any /node_modules/ directory as being in the node_modules
      // folder for the LSP because it's pretty complicated to deal with multiple scopes
      specifier.scheme() == "file"
        && specifier
          .path()
          .to_ascii_lowercase()
          .contains("/node_modules/")
    }

    if let Some(node_resolver) =
      &self.get_scope_resolver(Some(specifier)).node_resolver
    {
      if node_resolver.in_npm_package(specifier) {
        return true;
      }
    }

    has_node_modules_dir(specifier)
  }

  pub fn resolve_redirects(
    &self,
    specifier: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<ModuleSpecifier> {
    let resolver = self.get_scope_resolver(file_referrer);
    let Some(redirect_resolver) = resolver.redirect_resolver.as_ref() else {
      return Some(specifier.clone());
    };
    redirect_resolver.resolve(specifier)
  }

  pub fn redirect_chain_headers(
    &self,
    specifier: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Vec<(ModuleSpecifier, Arc<HashMap<String, String>>)> {
    let resolver = self.get_scope_resolver(file_referrer);
    let Some(redirect_resolver) = resolver.redirect_resolver.as_ref() else {
      return vec![];
    };
    redirect_resolver
      .chain(specifier)
      .into_iter()
      .map(|(s, e)| (s, e.headers.clone()))
      .collect()
  }

  fn get_scope_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> &LspScopeResolver {
    let Some(file_referrer) = file_referrer else {
      return self.unscoped.as_ref();
    };
    self
      .by_scope
      .values()
      .rfind(|r| {
        r.config_data
          .as_ref()
          .map(|d| d.scope_contains_specifier(file_referrer))
          .unwrap_or(false)
      })
      .map(|r| r.as_ref())
      .unwrap_or(self.unscoped.as_ref())
  }
}

#[derive(Debug, Default, Clone)]
pub struct ScopeDepInfo {
  pub deno_types_to_code_resolutions: HashMap<ModuleSpecifier, ModuleSpecifier>,
  pub npm_reqs: BTreeSet<PackageReq>,
  pub has_node_specifier: bool,
}

#[derive(Default)]
struct ResolverFactoryServices {
  cli_resolver: Deferred<Arc<CliResolver>>,
  found_pkg_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
  in_npm_pkg_checker: Deferred<DenoInNpmPackageChecker>,
  is_cjs_resolver: Deferred<Arc<CliIsCjsResolver>>,
  node_resolver: Deferred<Option<Arc<CliNodeResolver>>>,
  npm_graph_resolver: Deferred<Arc<CliNpmGraphResolver>>,
  npm_installer: Option<Arc<NpmInstaller>>,
  npm_pkg_req_resolver: Deferred<Option<Arc<CliNpmReqResolver>>>,
  npm_resolver: Option<CliNpmResolver>,
  npm_resolution: Arc<NpmResolutionCell>,
}

struct ResolverFactory<'a> {
  config_data: Option<&'a Arc<ConfigData>>,
  pkg_json_resolver: Arc<CliPackageJsonResolver>,
  node_resolution_sys: NodeResolutionSys<CliSys>,
  sys: CliSys,
  services: ResolverFactoryServices,
}

impl<'a> ResolverFactory<'a> {
  pub fn new(config_data: Option<&'a Arc<ConfigData>>) -> Self {
    let sys = CliSys::default();
    let pkg_json_resolver = Arc::new(CliPackageJsonResolver::new(
      sys.clone(),
      // this should be ok because we handle clearing this cache often in the LSP
      Some(Arc::new(PackageJsonThreadLocalCache)),
    ));
    Self {
      config_data,
      pkg_json_resolver,
      node_resolution_sys: NodeResolutionSys::new(
        sys.clone(),
        Some(Arc::new(NodeResolutionThreadLocalCache)),
      ),
      sys,
      services: Default::default(),
    }
  }

  // todo(dsherret): probably this method could be removed in the future
  // and instead just `npm_resolution_initializer.ensure_initialized()` could
  // be called. The reason this exists is because creating the npm resolvers
  // used to be async.
  async fn init_npm_resolver(
    &mut self,
    http_client_provider: &Arc<HttpClientProvider>,
    cache: &LspCache,
  ) {
    let enable_byonm = self.config_data.map(|d| d.byonm).unwrap_or(false);
    let sys = CliSys::default();
    let options = if enable_byonm {
      CliNpmResolverCreateOptions::Byonm(CliByonmNpmResolverCreateOptions {
        sys: self.node_resolution_sys.clone(),
        pkg_json_resolver: self.pkg_json_resolver.clone(),
        root_node_modules_dir: self.config_data.and_then(|config_data| {
          config_data.node_modules_dir.clone().or_else(|| {
            url_to_file_path(&config_data.scope)
              .ok()
              .map(|p| p.join("node_modules/"))
          })
        }),
      })
    } else {
      let npmrc = self
        .config_data
        .and_then(|d| d.npmrc.clone())
        .unwrap_or_else(|| Arc::new(create_default_npmrc(&sys)));
      let npm_cache_dir = Arc::new(NpmCacheDir::new(
        &sys,
        cache.deno_dir().npm_folder_path(),
        npmrc.get_all_known_registries_urls(),
      ));
      let npm_cache = Arc::new(CliNpmCache::new(
        npm_cache_dir.clone(),
        sys.clone(),
        // Use an "only" cache setting in order to make the
        // user do an explicit "cache" command and prevent
        // the cache from being filled with lots of packages while
        // the user is typing.
        deno_npm_cache::NpmCacheSetting::Only,
        npmrc.clone(),
      ));
      let pb = ProgressBar::new(ProgressBarStyle::TextOnly);
      let npm_client = Arc::new(CliNpmCacheHttpClient::new(
        http_client_provider.clone(),
        pb.clone(),
      ));
      let registry_info_provider = Arc::new(CliNpmRegistryInfoProvider::new(
        npm_cache.clone(),
        npm_client.clone(),
        npmrc.clone(),
      ));
      let npm_resolution_initializer = Arc::new(NpmResolutionInitializer::new(
        registry_info_provider.clone(),
        self.services.npm_resolution.clone(),
        match self.config_data.and_then(|d| d.lockfile.as_ref()) {
          Some(lockfile) => {
            CliNpmResolverManagedSnapshotOption::ResolveFromLockfile(
              lockfile.clone(),
            )
          }
          None => CliNpmResolverManagedSnapshotOption::Specified(None),
        },
      ));
      // Don't provide the lockfile. We don't want these resolvers
      // updating it. Only the cache request should update the lockfile.
      let maybe_lockfile: Option<Arc<CliLockfile>> = None;
      let maybe_node_modules_path =
        self.config_data.and_then(|d| d.node_modules_dir.clone());
      let tarball_cache = Arc::new(TarballCache::new(
        npm_cache.clone(),
        npm_client.clone(),
        sys.clone(),
        npmrc.clone(),
      ));
      let npm_resolution_installer = Arc::new(NpmResolutionInstaller::new(
        registry_info_provider,
        self.services.npm_resolution.clone(),
        maybe_lockfile.clone(),
      ));
      let npm_installer = Arc::new(NpmInstaller::new(
        npm_cache.clone(),
        Arc::new(NpmInstallDepsProvider::empty()),
        self.services.npm_resolution.clone(),
        npm_resolution_initializer.clone(),
        npm_resolution_installer,
        &pb,
        sys.clone(),
        tarball_cache.clone(),
        maybe_lockfile,
        maybe_node_modules_path.clone(),
        LifecycleScriptsConfig::default(),
        NpmSystemInfo::default(),
      ));
      self.set_npm_installer(npm_installer);
      if let Err(err) = npm_resolution_initializer.ensure_initialized().await {
        log::warn!("failed to initialize npm resolution: {}", err);
      }

      CliNpmResolverCreateOptions::Managed(CliManagedNpmResolverCreateOptions {
        sys: CliSys::default(),
        npm_cache_dir,
        maybe_node_modules_path,
        npmrc,
        npm_resolution: self.services.npm_resolution.clone(),
        npm_system_info: NpmSystemInfo::default(),
      })
    };
    self.set_npm_resolver(CliNpmResolver::new(options));
  }

  pub fn set_npm_installer(&mut self, npm_installer: Arc<NpmInstaller>) {
    self.services.npm_installer = Some(npm_installer);
  }

  pub fn set_npm_resolver(&mut self, npm_resolver: CliNpmResolver) {
    self.services.npm_resolver = Some(npm_resolver);
  }

  pub fn npm_resolver(&self) -> Option<&CliNpmResolver> {
    self.services.npm_resolver.as_ref()
  }

  pub fn cli_resolver(&self) -> &Arc<CliResolver> {
    self.services.cli_resolver.get_or_init(|| {
      let npm_req_resolver = self.npm_pkg_req_resolver().cloned();
      let deno_resolver = Arc::new(CliDenoResolver::new(DenoResolverOptions {
        in_npm_pkg_checker: self.in_npm_pkg_checker().clone(),
        node_and_req_resolver: match (self.node_resolver(), npm_req_resolver) {
          (Some(node_resolver), Some(npm_req_resolver)) => {
            Some(NodeAndNpmReqResolver {
              node_resolver: node_resolver.clone(),
              npm_req_resolver,
            })
          }
          _ => None,
        },
        workspace_resolver: self
          .config_data
          .map(|d| d.resolver.clone())
          .unwrap_or_else(|| {
            Arc::new(WorkspaceResolver::new_raw(
              // this is fine because this is only used before initialization
              Arc::new(ModuleSpecifier::parse("file:///").unwrap()),
              None,
              Vec::new(),
              Vec::new(),
              PackageJsonDepResolution::Disabled,
              Default::default(),
              Default::default(),
              Default::default(),
              Default::default(),
              self.sys.clone(),
            ))
          }),
        is_byonm: self.config_data.map(|d| d.byonm).unwrap_or(false),
        maybe_vendor_dir: self.config_data.and_then(|d| d.vendor_dir.as_ref()),
      }));
      Arc::new(CliResolver::new(
        deno_resolver,
        self.services.found_pkg_json_dep_flag.clone(),
      ))
    })
  }

  pub fn npm_installer(&self) -> Option<&Arc<NpmInstaller>> {
    self.services.npm_installer.as_ref()
  }

  pub fn npm_graph_resolver(&self) -> &Arc<CliNpmGraphResolver> {
    self.services.npm_graph_resolver.get_or_init(|| {
      Arc::new(CliNpmGraphResolver::new(
        None,
        self.services.found_pkg_json_dep_flag.clone(),
        self
          .config_data
          .is_some_and(|d| d.unstable.contains("bare-node-builtins")),
        NpmCachingStrategy::Eager,
      ))
    })
  }

  pub fn pkg_json_resolver(&self) -> &Arc<CliPackageJsonResolver> {
    &self.pkg_json_resolver
  }

  pub fn in_npm_pkg_checker(&self) -> &DenoInNpmPackageChecker {
    self.services.in_npm_pkg_checker.get_or_init(|| {
      DenoInNpmPackageChecker::new(match &self.services.npm_resolver {
        Some(CliNpmResolver::Byonm(_)) | None => {
          CreateInNpmPkgCheckerOptions::Byonm
        }
        Some(CliNpmResolver::Managed(m)) => {
          CreateInNpmPkgCheckerOptions::Managed(
            ManagedInNpmPkgCheckerCreateOptions {
              root_cache_dir_url: m.global_cache_root_url(),
              maybe_node_modules_path: m.root_node_modules_path(),
            },
          )
        }
      })
    })
  }

  pub fn is_cjs_resolver(&self) -> &Arc<CliIsCjsResolver> {
    self.services.is_cjs_resolver.get_or_init(|| {
      Arc::new(CliIsCjsResolver::new(
        self.in_npm_pkg_checker().clone(),
        self.pkg_json_resolver().clone(),
        if self
          .config_data
          .is_some_and(|d| d.unstable.contains("detect-cjs"))
        {
          IsCjsResolutionMode::ImplicitTypeCommonJs
        } else {
          IsCjsResolutionMode::ExplicitTypeCommonJs
        },
      ))
    })
  }

  pub fn node_resolver(&self) -> Option<&Arc<CliNodeResolver>> {
    self
      .services
      .node_resolver
      .get_or_init(|| {
        let npm_resolver = self.services.npm_resolver.as_ref()?;
        Some(Arc::new(CliNodeResolver::new(
          self.in_npm_pkg_checker().clone(),
          DenoIsBuiltInNodeModuleChecker,
          npm_resolver.clone(),
          self.pkg_json_resolver.clone(),
          self.node_resolution_sys.clone(),
          node_resolver::ConditionsFromResolutionMode::default(),
        )))
      })
      .as_ref()
  }

  pub fn npm_pkg_req_resolver(&self) -> Option<&Arc<CliNpmReqResolver>> {
    self
      .services
      .npm_pkg_req_resolver
      .get_or_init(|| {
        let node_resolver = self.node_resolver()?;
        let npm_resolver = self.npm_resolver()?;
        Some(Arc::new(CliNpmReqResolver::new(NpmReqResolverOptions {
          in_npm_pkg_checker: self.in_npm_pkg_checker().clone(),
          node_resolver: node_resolver.clone(),
          npm_resolver: npm_resolver.clone(),
          sys: self.sys.clone(),
        })))
      })
      .as_ref()
  }
}

#[derive(Debug, Eq, PartialEq)]
struct RedirectEntry {
  headers: Arc<HashMap<String, String>>,
  target: Url,
  destination: Option<Url>,
}

type GetHeadersFn =
  Box<dyn Fn(&Url) -> Option<HashMap<String, String>> + Send + Sync>;

struct RedirectResolver {
  get_headers: GetHeadersFn,
  entries: DashMap<Url, Option<Arc<RedirectEntry>>>,
}

impl std::fmt::Debug for RedirectResolver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("RedirectResolver")
      .field("get_headers", &"Box(|_| { ... })")
      .field("entries", &self.entries)
      .finish()
  }
}

#[derive(Debug)]
pub struct SingleReferrerGraphResolver<'a> {
  pub valid_referrer: &'a ModuleSpecifier,
  pub module_resolution_mode: ResolutionMode,
  pub cli_resolver: &'a CliResolver,
  pub jsx_import_source_config: Option<&'a JsxImportSourceConfig>,
}

impl<'a> deno_graph::source::Resolver for SingleReferrerGraphResolver<'a> {
  fn default_jsx_import_source(
    &self,
    _referrer: &ModuleSpecifier,
  ) -> Option<String> {
    self
      .jsx_import_source_config
      .and_then(|c| c.import_source.as_ref().map(|s| s.specifier.clone()))
  }

  fn default_jsx_import_source_types(
    &self,
    _referrer: &ModuleSpecifier,
  ) -> Option<String> {
    self
      .jsx_import_source_config
      .and_then(|c| c.import_source_types.as_ref().map(|s| s.specifier.clone()))
  }

  fn jsx_import_source_module(&self, _referrer: &ModuleSpecifier) -> &str {
    self
      .jsx_import_source_config
      .map(|c| c.module.as_str())
      .unwrap_or(deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE)
  }

  fn resolve(
    &self,
    specifier_text: &str,
    referrer_range: &Range,
    resolution_kind: deno_graph::source::ResolutionKind,
  ) -> Result<ModuleSpecifier, deno_graph::source::ResolveError> {
    // this resolver assumes it will only be used with a single referrer
    // with the provided referrer kind
    debug_assert_eq!(referrer_range.specifier, *self.valid_referrer);
    self.cli_resolver.resolve(
      specifier_text,
      &referrer_range.specifier,
      referrer_range.range.start,
      referrer_range
        .resolution_mode
        .map(to_node_resolution_mode)
        .unwrap_or(self.module_resolution_mode),
      to_node_resolution_kind(resolution_kind),
    )
  }
}

impl RedirectResolver {
  fn new(
    cache: Arc<dyn HttpCache>,
    lockfile: Option<Arc<CliLockfile>>,
  ) -> Self {
    let entries = DashMap::new();
    if let Some(lockfile) = lockfile {
      for (source, destination) in &lockfile.lock().content.redirects {
        let Ok(source) = ModuleSpecifier::parse(source) else {
          continue;
        };
        let Ok(destination) = ModuleSpecifier::parse(destination) else {
          continue;
        };
        entries.insert(
          source,
          Some(Arc::new(RedirectEntry {
            headers: Default::default(),
            target: destination.clone(),
            destination: Some(destination.clone()),
          })),
        );
        entries.insert(destination, None);
      }
    }
    Self {
      get_headers: Box::new(move |specifier| {
        let cache_key = cache.cache_item_key(specifier).ok()?;
        cache.read_headers(&cache_key).ok().flatten()
      }),
      entries,
    }
  }

  #[cfg(test)]
  fn mock(get_headers: GetHeadersFn) -> Self {
    Self {
      get_headers,
      entries: Default::default(),
    }
  }

  fn resolve(&self, specifier: &Url) -> Option<Url> {
    if !matches!(specifier.scheme(), "http" | "https") {
      return Some(specifier.clone());
    }
    let mut current = specifier.clone();
    let mut chain = vec![];
    let destination = loop {
      if let Some(maybe_entry) = self.entries.get(&current) {
        break match maybe_entry.as_ref() {
          Some(entry) => entry.destination.clone(),
          None => Some(current),
        };
      }
      let Some(headers) = (self.get_headers)(&current) else {
        break None;
      };
      let headers = Arc::new(headers);
      if let Some(location) = headers.get("location") {
        if chain.len() > 10 {
          break None;
        }
        let Ok(target) = specifier.join(location) else {
          break None;
        };
        chain.push((
          current.clone(),
          RedirectEntry {
            headers,
            target: target.clone(),
            destination: None,
          },
        ));
        current = target;
      } else {
        self.entries.insert(current.clone(), None);
        break Some(current);
      }
    };
    for (specifier, mut entry) in chain {
      entry.destination.clone_from(&destination);
      self.entries.insert(specifier, Some(Arc::new(entry)));
    }
    destination
  }

  fn chain(&self, specifier: &Url) -> Vec<(Url, Arc<RedirectEntry>)> {
    self.resolve(specifier);
    let mut result = vec![];
    let mut seen = HashSet::new();
    let mut current = Cow::Borrowed(specifier);
    loop {
      let Some(maybe_entry) = self.entries.get(&current) else {
        break;
      };
      let Some(entry) = maybe_entry.as_ref() else {
        break;
      };
      result.push((current.as_ref().clone(), entry.clone()));
      seen.insert(current.as_ref().clone());
      if seen.contains(&entry.target) {
        break;
      }
      current = Cow::Owned(entry.target.clone())
    }
    result
  }

  fn did_cache(&self) {
    self.entries.retain(|_, entry| entry.is_some());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_redirect_resolver() {
    let redirect_resolver =
      RedirectResolver::mock(Box::new(|specifier| match specifier.as_str() {
        "https://foo/redirect_2.js" => Some(
          [("location".to_string(), "./redirect_1.js".to_string())]
            .into_iter()
            .collect(),
        ),
        "https://foo/redirect_1.js" => Some(
          [("location".to_string(), "./file.js".to_string())]
            .into_iter()
            .collect(),
        ),
        "https://foo/file.js" => Some([].into_iter().collect()),
        _ => None,
      }));
    assert_eq!(
      redirect_resolver.resolve(&Url::parse("https://foo/file.js").unwrap()),
      Some(Url::parse("https://foo/file.js").unwrap())
    );
    assert_eq!(
      redirect_resolver
        .resolve(&Url::parse("https://foo/redirect_1.js").unwrap()),
      Some(Url::parse("https://foo/file.js").unwrap())
    );
    assert_eq!(
      redirect_resolver
        .resolve(&Url::parse("https://foo/redirect_2.js").unwrap()),
      Some(Url::parse("https://foo/file.js").unwrap())
    );
    assert_eq!(
      redirect_resolver.resolve(&Url::parse("https://foo/unknown").unwrap()),
      None
    );
    assert_eq!(
      redirect_resolver
        .chain(&Url::parse("https://foo/redirect_2.js").unwrap()),
      vec![
        (
          Url::parse("https://foo/redirect_2.js").unwrap(),
          Arc::new(RedirectEntry {
            headers: Arc::new(
              [("location".to_string(), "./redirect_1.js".to_string())]
                .into_iter()
                .collect()
            ),
            target: Url::parse("https://foo/redirect_1.js").unwrap(),
            destination: Some(Url::parse("https://foo/file.js").unwrap()),
          })
        ),
        (
          Url::parse("https://foo/redirect_1.js").unwrap(),
          Arc::new(RedirectEntry {
            headers: Arc::new(
              [("location".to_string(), "./file.js".to_string())]
                .into_iter()
                .collect()
            ),
            target: Url::parse("https://foo/file.js").unwrap(),
            destination: Some(Url::parse("https://foo/file.js").unwrap()),
          })
        ),
      ]
    );
  }
}
