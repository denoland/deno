// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;
use deno_ast::MediaType;
use deno_cache_dir::HttpCache;
use deno_cache_dir::npm::NpmCacheDir;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_graph::ModuleSpecifier;
use deno_graph::Range;
use deno_npm::NpmSystemInfo;
use deno_npm::resolution::NpmVersionResolver;
use deno_npm_cache::TarballCache;
use deno_npm_installer::LifecycleScriptsConfig;
use deno_npm_installer::initializer::NpmResolutionInitializer;
use deno_npm_installer::initializer::NpmResolverManagedSnapshotOption;
use deno_npm_installer::lifecycle_scripts::NullLifecycleScriptsExecutor;
use deno_npm_installer::package_json::NpmInstallDepsProvider;
use deno_npm_installer::resolution::NpmResolutionInstaller;
use deno_path_util::url_to_file_path;
use deno_resolver::DenoResolverOptions;
use deno_resolver::NodeAndNpmResolvers;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::deno_json::CompilerOptionsResolver;
use deno_resolver::deno_json::JsxImportSourceConfig;
use deno_resolver::graph::FoundPackageJsonDepFlag;
use deno_resolver::npm::CreateInNpmPkgCheckerOptions;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmReqResolverOptions;
use deno_resolver::npm::managed::ManagedInNpmPkgCheckerCreateOptions;
use deno_resolver::npm::managed::ManagedNpmResolverCreateOptions;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::npmrc::create_default_npmrc;
use deno_resolver::workspace::CreateResolverOptions;
use deno_resolver::workspace::FsCacheOptions;
use deno_resolver::workspace::PackageJsonDepResolution;
use deno_resolver::workspace::SloppyImportsOptions;
use deno_resolver::workspace::WorkspaceNpmLinkPackagesRc;
use deno_resolver::workspace::WorkspaceResolver;
use deno_runtime::tokio_util::create_basic_runtime;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolverOptions;
use node_resolver::PackageJson;
use node_resolver::PackageJsonThreadLocalCache;
use node_resolver::ResolutionMode;
use node_resolver::cache::NodeResolutionSys;
use node_resolver::cache::NodeResolutionThreadLocalCache;
use once_cell::sync::Lazy;

use super::cache::LspCache;
use super::documents::DocumentModule;
use super::jsr::JsrCacheResolver;
use crate::args::CliLockfile;
use crate::factory::Deferred;
use crate::http_util::HttpClientProvider;
use crate::lsp::config::Config;
use crate::lsp::config::ConfigData;
use crate::lsp::logging::lsp_warn;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::CliByonmNpmResolverCreateOptions;
use crate::npm::CliManagedNpmResolver;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmInstaller;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverCreateOptions;
use crate::resolver::CliIsCjsResolver;
use crate::resolver::CliNpmReqResolver;
use crate::resolver::CliResolver;
use crate::resolver::on_resolve_diagnostic;
use crate::sys::CliSys;
use crate::tsc::into_specifier_and_media_type;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

#[derive(Debug, Clone)]
pub struct LspScopedResolver {
  resolver: Arc<CliResolver>,
  workspace_resolver: Arc<WorkspaceResolver<CliSys>>,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  is_cjs_resolver: Arc<CliIsCjsResolver>,
  jsr_resolver: Option<Arc<JsrCacheResolver>>,
  npm_installer: Option<Arc<CliNpmInstaller>>,
  npm_installer_reqs: Arc<Mutex<BTreeSet<PackageReq>>>,
  npm_resolution: Arc<NpmResolutionCell>,
  npm_resolver: Option<CliNpmResolver>,
  node_resolver: Option<Arc<CliNodeResolver>>,
  npm_pkg_req_resolver: Option<Arc<CliNpmReqResolver>>,
  pkg_json_resolver: Arc<CliPackageJsonResolver>,
  redirect_resolver: Option<Arc<RedirectResolver>>,
  dep_info: Arc<Mutex<Arc<ScopeDepInfo>>>,
  configured_dep_resolutions: Arc<ConfiguredDepResolutions>,
  config_data: Option<Arc<ConfigData>>,
}

impl Default for LspScopedResolver {
  fn default() -> Self {
    let factory = ResolverFactory::new(None);
    Self {
      resolver: factory.cli_resolver().clone(),
      workspace_resolver: factory.workspace_resolver().clone(),
      in_npm_pkg_checker: factory.in_npm_pkg_checker().clone(),
      is_cjs_resolver: factory.is_cjs_resolver().clone(),
      jsr_resolver: None,
      npm_installer: None,
      npm_installer_reqs: Default::default(),
      npm_resolver: None,
      node_resolver: None,
      npm_resolution: factory.services.npm_resolution.clone(),
      npm_pkg_req_resolver: None,
      pkg_json_resolver: factory.pkg_json_resolver().clone(),
      redirect_resolver: None,
      dep_info: Default::default(),
      configured_dep_resolutions: Default::default(),
      config_data: None,
    }
  }
}

impl LspScopedResolver {
  async fn from_config_data(
    config_data: Option<&Arc<ConfigData>>,
    cache: &LspCache,
    http_client_provider: Option<&Arc<HttpClientProvider>>,
  ) -> Self {
    let mut factory = ResolverFactory::new(config_data);
    let workspace_resolver = factory.workspace_resolver().clone();
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
      &workspace_resolver,
    )));
    let redirect_resolver = Some(Arc::new(RedirectResolver::new(
      cache.for_specifier(config_data.map(|d| d.scope.as_ref())),
      config_data.and_then(|d| d.lockfile.clone()),
    )));
    let configured_dep_resolutions = (|| {
      let npm_pkg_req_resolver = npm_pkg_req_resolver.as_ref()?;
      Some(Arc::new(ConfiguredDepResolutions::new(
        workspace_resolver.clone(),
        config_data.and_then(|d| d.maybe_pkg_json().map(|p| p.as_ref())),
        npm_pkg_req_resolver,
        &pkg_json_resolver,
      )))
    })()
    .unwrap_or_default();
    Self {
      resolver: cli_resolver,
      workspace_resolver,
      in_npm_pkg_checker,
      is_cjs_resolver: factory.is_cjs_resolver().clone(),
      jsr_resolver,
      npm_pkg_req_resolver,
      npm_resolver,
      npm_installer,
      npm_installer_reqs: Default::default(),
      npm_resolution: factory.services.npm_resolution.clone(),
      node_resolver,
      pkg_json_resolver,
      redirect_resolver,
      dep_info: Default::default(),
      configured_dep_resolutions,
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
              ManagedNpmResolverCreateOptions {
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
      workspace_resolver: factory.workspace_resolver().clone(),
      in_npm_pkg_checker: factory.in_npm_pkg_checker().clone(),
      is_cjs_resolver: factory.is_cjs_resolver().clone(),
      jsr_resolver: self.jsr_resolver.clone(),
      npm_installer: self.npm_installer.clone(),
      npm_installer_reqs: self.npm_installer_reqs.clone(),
      npm_pkg_req_resolver: factory.npm_pkg_req_resolver().cloned(),
      npm_resolution: factory.services.npm_resolution.clone(),
      npm_resolver: factory.npm_resolver().cloned(),
      node_resolver: factory.node_resolver().cloned(),
      redirect_resolver: self.redirect_resolver.clone(),
      pkg_json_resolver: factory.pkg_json_resolver().clone(),
      dep_info: self.dep_info.clone(),
      configured_dep_resolutions: self.configured_dep_resolutions.clone(),
      config_data: self.config_data.clone(),
    })
  }

  pub fn as_in_npm_pkg_checker(&self) -> &DenoInNpmPackageChecker {
    &self.in_npm_pkg_checker
  }

  pub fn as_cli_resolver(&self) -> &CliResolver {
    self.resolver.as_ref()
  }

  pub fn as_workspace_resolver(&self) -> &Arc<WorkspaceResolver<CliSys>> {
    &self.workspace_resolver
  }

  pub fn as_is_cjs_resolver(&self) -> &CliIsCjsResolver {
    self.is_cjs_resolver.as_ref()
  }

  pub fn as_node_resolver(&self) -> Option<&Arc<CliNodeResolver>> {
    self.node_resolver.as_ref()
  }

  pub fn as_maybe_managed_npm_resolver(
    &self,
  ) -> Option<&Arc<CliManagedNpmResolver>> {
    self.npm_resolver.as_ref().and_then(|r| r.as_managed())
  }

  pub fn as_pkg_json_resolver(&self) -> &Arc<CliPackageJsonResolver> {
    &self.pkg_json_resolver
  }

  pub fn jsr_to_resource_url(
    &self,
    req_ref: &JsrPackageReqReference,
  ) -> Option<ModuleSpecifier> {
    self.jsr_resolver.as_ref()?.jsr_to_resource_url(req_ref)
  }

  pub fn jsr_lookup_bare_specifier_for_workspace_file(
    &self,
    specifier: &Url,
  ) -> Option<String> {
    self
      .jsr_resolver
      .as_ref()?
      .lookup_bare_specifier_for_workspace_file(specifier)
  }

  pub fn jsr_lookup_export_for_path(
    &self,
    nv: &PackageNv,
    path: &str,
  ) -> Option<String> {
    self.jsr_resolver.as_ref()?.lookup_export_for_path(nv, path)
  }

  pub fn jsr_lookup_req_for_nv(&self, nv: &PackageNv) -> Option<PackageReq> {
    self.jsr_resolver.as_ref()?.lookup_req_for_nv(nv)
  }

  pub fn npm_to_file_url(
    &self,
    req_ref: &NpmPackageReqReference,
    referrer: &ModuleSpecifier,
    resolution_kind: NodeResolutionKind,
    resolution_mode: ResolutionMode,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let npm_pkg_req_resolver = self.npm_pkg_req_resolver.as_ref()?;
    self.add_npm_reqs(vec![req_ref.req().clone()]);
    Some(into_specifier_and_media_type(Some(
      npm_pkg_req_resolver
        .resolve_req_reference(
          req_ref,
          referrer,
          resolution_mode,
          resolution_kind,
        )
        .ok()?
        .into_url()
        .ok()?,
    )))
  }

  pub fn resource_url_to_configured_dep_key(
    &self,
    specifier: &Url,
    referrer: &Url,
  ) -> Option<String> {
    self
      .configured_dep_resolutions
      .dep_key_from_resolution(specifier, referrer)
  }

  pub fn npm_reqs(&self) -> BTreeSet<PackageReq> {
    self.npm_installer_reqs.lock().clone()
  }

  pub fn deno_types_to_code_resolution(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let dep_info = self.dep_info.lock();
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

    if let Some(node_resolver) = &self.node_resolver
      && node_resolver.in_npm_package(specifier)
    {
      return true;
    }

    has_node_modules_dir(specifier)
  }

  pub fn resolve_redirects(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let Some(redirect_resolver) = self.redirect_resolver.as_ref() else {
      return Some(specifier.clone());
    };
    redirect_resolver.resolve(specifier)
  }

  pub fn redirect_chain_headers(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Vec<(ModuleSpecifier, Arc<HashMap<String, String>>)> {
    let Some(redirect_resolver) = self.redirect_resolver.as_ref() else {
      return vec![];
    };
    redirect_resolver
      .chain(specifier)
      .into_iter()
      .map(|(s, e)| (s, e.headers.clone()))
      .collect()
  }

  pub fn refresh_npm_reqs(&self) {
    let Some(npm_installer) = self.npm_installer.as_ref().cloned() else {
      return;
    };
    let npm_installer_reqs = self.npm_installer_reqs.lock();
    let reqs = npm_installer_reqs.iter().cloned().collect::<Vec<_>>();
    if let Err(err) = ADD_NPM_REQS_THREAD.add_npm_reqs(npm_installer, reqs) {
      lsp_warn!("Could not refresh npm package requirements: {:#}", err);
    }
  }

  pub fn add_npm_reqs(&self, reqs: Vec<PackageReq>) {
    let Some(npm_installer) = self.npm_installer.as_ref().cloned() else {
      return;
    };
    let mut npm_installer_reqs = self.npm_installer_reqs.lock();
    let old_reqs_count = npm_installer_reqs.len();
    npm_installer_reqs.extend(reqs.clone());
    if npm_installer_reqs.len() == old_reqs_count {
      return;
    }
    if let Err(err) = ADD_NPM_REQS_THREAD.add_npm_reqs(npm_installer, reqs) {
      lsp_warn!("Could not add npm package requirements: {:#}", err);
    }
  }
}

#[derive(Debug, Default, Clone)]
pub struct LspResolver {
  unscoped: Arc<LspScopedResolver>,
  by_scope: BTreeMap<Arc<Url>, Arc<LspScopedResolver>>,
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
          LspScopedResolver::from_config_data(
            Some(config_data),
            cache,
            http_client_provider,
          )
          .await,
        ),
      );
    }
    let unscoped = Arc::new(
      LspScopedResolver::from_config_data(None, cache, http_client_provider)
        .await,
    );
    for resolver in std::iter::once(&unscoped).chain(by_scope.values()) {
      if resolver.npm_installer.is_none() {
        continue;
      }
      let Some(lockfile) = resolver
        .config_data
        .as_ref()
        .and_then(|d| d.lockfile.as_ref())
      else {
        continue;
      };
      let npm_reqs = lockfile
        .lock()
        .content
        .packages
        .specifiers
        .keys()
        .filter(|r| r.kind == deno_semver::package::PackageKind::Npm)
        .map(|r| r.req.clone())
        .collect::<Vec<_>>();
      resolver.add_npm_reqs(npm_reqs);
    }
    Self { unscoped, by_scope }
  }

  pub fn set_compiler_options_resolver(
    &self,
    value: &Arc<CompilerOptionsResolver>,
  ) {
    for resolver in
      std::iter::once(&self.unscoped).chain(self.by_scope.values())
    {
      resolver
        .workspace_resolver
        .set_compiler_options_resolver(value.clone());
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
      resolver.refresh_npm_reqs();
    }
  }

  pub fn did_create_module(&self, module: &DocumentModule) {
    let resolver = self.get_scoped_resolver(module.scope.as_deref());
    let npm_reqs = module
      .dependencies
      .values()
      .flat_map(|d| [d.get_code(), d.get_type()])
      .flatten()
      .chain(
        module
          .types_dependency
          .iter()
          .flat_map(|d| d.dependency.maybe_specifier()),
      )
      .flat_map(|s| NpmPackageReqReference::from_specifier(s).ok())
      .map(|r| r.into_inner().req)
      .collect::<Vec<_>>();
    resolver.add_npm_reqs(npm_reqs);
  }

  pub fn set_dep_info_by_scope(
    &self,
    dep_info_by_scope: &Arc<BTreeMap<Option<Arc<Url>>, Arc<ScopeDepInfo>>>,
  ) {
    for (scope, resolver) in [(None, &self.unscoped)]
      .into_iter()
      .chain(self.by_scope.iter().map(|(s, r)| (Some(s), r)))
    {
      let dep_info = dep_info_by_scope
        .get(&scope.cloned())
        .cloned()
        .unwrap_or_default();
      {
        let mut resolver_dep_info = resolver.dep_info.lock();
        *resolver_dep_info = dep_info.clone();
      }
    }
  }

  pub fn in_node_modules(&self, specifier: &ModuleSpecifier) -> bool {
    self
      .get_scoped_resolver(Some(specifier))
      .in_node_modules(specifier)
  }

  pub fn get_scoped_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> &LspScopedResolver {
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
  pub has_node_specifier: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum ConfiguredDepKind {
  ImportMap { key: String, value: Url },
  PackageJson,
}

#[derive(Debug, Default)]
struct ConfiguredDepResolutions {
  workspace_resolver: Option<Arc<WorkspaceResolver<CliSys>>>,
  deps_by_resolution: IndexMap<ModuleSpecifier, (String, ConfiguredDepKind)>,
}

impl ConfiguredDepResolutions {
  fn new(
    workspace_resolver: Arc<WorkspaceResolver<CliSys>>,
    package_json: Option<&PackageJson>,
    npm_pkg_req_resolver: &CliNpmReqResolver,
    pkg_json_resolver: &CliPackageJsonResolver,
  ) -> Self {
    let mut result = Self::default();
    let insert_export_resolutions =
      |key_prefix: &str,
       dep_req_str: &str,
       dep_package_json: &PackageJson,
       referrer,
       dep_kind: &ConfiguredDepKind,
       result: &mut Self| {
        let export_keys = dep_package_json
          .exports
          .as_ref()
          .into_iter()
          .flat_map(|e| e.keys());
        for export_key in export_keys {
          let Some(export_name) = export_key.strip_prefix("./") else {
            continue;
          };
          // Wildcards are not supported here.
          if export_name.chars().filter(|c| *c == '*').count() == 1 {
            continue;
          }
          let Some(req_ref) = NpmPackageReqReference::from_str(&format!(
            "npm:{dep_req_str}/{export_name}"
          ))
          .ok() else {
            continue;
          };
          for kind in [NodeResolutionKind::Types, NodeResolutionKind::Execution]
          {
            let Some(url_or_path) = npm_pkg_req_resolver
              .resolve_req_reference(
                &req_ref,
                referrer,
                // todo(dsherret): this is wrong because it doesn't consider CJS referrers
                ResolutionMode::Import,
                kind,
              )
              .ok()
            else {
              continue;
            };
            let Some(file_url) = url_or_path.into_url().ok() else {
              continue;
            };
            result.deps_by_resolution.insert(
              file_url,
              (format!("{key_prefix}/{export_name}"), dep_kind.clone()),
            );
          }
        }
      };
    if let Some(import_map) = workspace_resolver.maybe_import_map() {
      let referrer = import_map.base_url();
      for entry in import_map.imports().entries().chain(
        import_map
          .scopes()
          .flat_map(|scope| scope.imports.entries()),
      ) {
        let Some(value) = entry.value else {
          continue;
        };
        let Ok(req_ref) = NpmPackageReqReference::from_specifier(value) else {
          continue;
        };
        let dep_kind = ConfiguredDepKind::ImportMap {
          key: entry.key.to_string(),
          value: value.clone(),
        };
        let mut dep_package_json = None;
        for kind in [NodeResolutionKind::Types, NodeResolutionKind::Execution] {
          let Some(file_url) = npm_pkg_req_resolver
            .resolve_req_reference(
              &req_ref,
              referrer,
              // todo(dsherret): this is wrong because it doesn't consider CJS referrers
              ResolutionMode::Import,
              kind,
            )
            .ok()
            .and_then(|u| u.into_url().ok())
          else {
            continue;
          };
          if dep_package_json.is_none() {
            dep_package_json = (|| {
              let path = url_to_file_path(&file_url).ok()?;
              pkg_json_resolver.get_closest_package_json(&path).ok()?
            })();
          }
          if !entry.key.ends_with('/') {
            result.deps_by_resolution.insert(
              file_url,
              (
                entry.key.to_string(),
                ConfiguredDepKind::ImportMap {
                  key: entry.key.to_string(),
                  value: value.clone(),
                },
              ),
            );
          }
        }
        if let Some(key_prefix) = entry.key.strip_suffix('/')
          && req_ref.sub_path().is_none()
          && let Some(dep_package_json) = &dep_package_json
        {
          insert_export_resolutions(
            key_prefix,
            &req_ref.req().to_string(),
            dep_package_json,
            referrer,
            &dep_kind,
            &mut result,
          );
        }
      }
    }
    if let Some(package_json) = package_json {
      let referrer = package_json.specifier();
      let Some(dependencies) = package_json.dependencies.as_ref() else {
        return Self::default();
      };
      for name in dependencies.keys() {
        let Some(req_ref) =
          NpmPackageReqReference::from_str(&format!("npm:{name}")).ok()
        else {
          continue;
        };
        let mut dep_package_json = None;
        for kind in [NodeResolutionKind::Types, NodeResolutionKind::Execution] {
          let Ok(req) = npm_pkg_req_resolver.resolve_req_reference(
            &req_ref,
            &referrer,
            // todo(dsherret): this is wrong because it doesn't consider CJS referrers
            ResolutionMode::Import,
            kind,
          ) else {
            continue;
          };
          let Some(file_url) = req.into_url().ok() else {
            continue;
          };
          if dep_package_json.is_none() {
            dep_package_json = (|| {
              let path = url_to_file_path(&file_url).ok()?;
              pkg_json_resolver.get_closest_package_json(&path).ok()?
            })();
          }
          result
            .deps_by_resolution
            .insert(file_url, (name.clone(), ConfiguredDepKind::PackageJson));
        }
        if let Some(dep_package_json) = &dep_package_json {
          insert_export_resolutions(
            name,
            name,
            dep_package_json,
            &referrer,
            &ConfiguredDepKind::PackageJson,
            &mut result,
          );
        }
      }
    }
    result.workspace_resolver = Some(workspace_resolver);
    result
  }

  fn dep_key_from_resolution(
    &self,
    resolution: &Url,
    referrer: &Url,
  ) -> Option<String> {
    self
      .deps_by_resolution
      .get(resolution)
      .and_then(|(dep_key, kind)| match kind {
        // Ensure the mapping this entry came from is valid for this referrer.
        ConfiguredDepKind::ImportMap { key, value } => self
          .workspace_resolver
          .as_ref()?
          .maybe_import_map()?
          .resolve(key, referrer)
          .is_ok_and(|s| &s == value)
          .then(|| dep_key.clone()),
        ConfiguredDepKind::PackageJson => Some(dep_key.clone()),
      })
  }
}

#[derive(Default)]
struct ResolverFactoryServices {
  cli_resolver: Deferred<Arc<CliResolver>>,
  workspace_resolver: Deferred<Arc<WorkspaceResolver<CliSys>>>,
  found_pkg_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
  in_npm_pkg_checker: Deferred<DenoInNpmPackageChecker>,
  is_cjs_resolver: Deferred<Arc<CliIsCjsResolver>>,
  node_resolver: Deferred<Option<Arc<CliNodeResolver>>>,
  npm_installer: Option<Arc<CliNpmInstaller>>,
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
      let link_packages: WorkspaceNpmLinkPackagesRc = self
        .config_data
        .as_ref()
        .filter(|c| c.node_modules_dir.is_some()) // requires a node_modules dir
        .map(|d| {
          WorkspaceNpmLinkPackagesRc::from_workspace(&d.member_dir.workspace)
        })
        .unwrap_or_default();
      let npm_resolution_initializer = Arc::new(NpmResolutionInitializer::new(
        self.services.npm_resolution.clone(),
        link_packages.clone(),
        match self.config_data.and_then(|d| d.lockfile.as_ref()) {
          Some(lockfile) => {
            NpmResolverManagedSnapshotOption::ResolveFromLockfile(
              lockfile.clone(),
            )
          }
          None => NpmResolverManagedSnapshotOption::Specified(None),
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
        None,
      ));
      let npm_version_resolver = Arc::new(NpmVersionResolver {
        types_node_version_req: None,
        link_packages: link_packages.0.clone(),
        newest_dependency_date_options: Default::default(),
      });
      let npm_resolution_installer = Arc::new(NpmResolutionInstaller::new(
        Default::default(),
        npm_version_resolver,
        registry_info_provider.clone(),
        None,
        self.services.npm_resolution.clone(),
        maybe_lockfile.clone(),
      ));
      let npm_installer = Arc::new(CliNpmInstaller::new(
        None,
        Arc::new(NullLifecycleScriptsExecutor),
        npm_cache.clone(),
        Arc::new(NpmInstallDepsProvider::empty()),
        registry_info_provider.clone(),
        self.services.npm_resolution.clone(),
        npm_resolution_initializer.clone(),
        npm_resolution_installer,
        &pb,
        sys.clone(),
        tarball_cache.clone(),
        deno_npm_installer::NpmInstallerOptions {
          maybe_lockfile,
          maybe_node_modules_path: maybe_node_modules_path.clone(),
          lifecycle_scripts: Arc::new(LifecycleScriptsConfig::default()),
          system_info: NpmSystemInfo::default(),
          workspace_link_packages: link_packages,
        },
      ));
      self.set_npm_installer(npm_installer);
      if let Err(err) = npm_resolution_initializer.ensure_initialized().await {
        log::warn!("failed to initialize npm resolution: {}", err);
      }

      CliNpmResolverCreateOptions::Managed(ManagedNpmResolverCreateOptions {
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

  pub fn set_npm_installer(&mut self, npm_installer: Arc<CliNpmInstaller>) {
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
      let deno_resolver =
        Arc::new(deno_resolver::RawDenoResolver::new(DenoResolverOptions {
          in_npm_pkg_checker: self.in_npm_pkg_checker().clone(),
          node_and_req_resolver: match (
            self.node_resolver(),
            npm_req_resolver,
            self.npm_resolver(),
          ) {
            (
              Some(node_resolver),
              Some(npm_req_resolver),
              Some(npm_resolver),
            ) => Some(NodeAndNpmResolvers {
              node_resolver: node_resolver.clone(),
              npm_resolver: npm_resolver.clone(),
              npm_req_resolver,
            }),
            _ => None,
          },
          workspace_resolver: self.workspace_resolver().clone(),
          bare_node_builtins: self
            .config_data
            .is_some_and(|d| d.unstable.contains("bare-node-builtins")),
          is_byonm: self.config_data.map(|d| d.byonm).unwrap_or(false),
          maybe_vendor_dir: self
            .config_data
            .and_then(|d| d.vendor_dir.as_ref()),
        }));
      Arc::new(CliResolver::new(
        deno_resolver,
        CliSys::default(),
        self.services.found_pkg_json_dep_flag.clone(),
        Some(Arc::new(on_resolve_diagnostic)),
      ))
    })
  }

  pub fn workspace_resolver(&self) -> &Arc<WorkspaceResolver<CliSys>> {
    self.services.workspace_resolver.get_or_init(|| {
      let workspace_resolver = self
        .config_data
        .map(|d| {
          let unstable_sloppy_imports =
            std::env::var("DENO_UNSTABLE_SLOPPY_IMPORTS").is_ok()
              || d.unstable.contains("sloppy-imports");
          let pkg_json_dep_resolution = if d.byonm {
            PackageJsonDepResolution::Disabled
          } else {
            // todo(dsherret): this should be false for nodeModulesDir: true
            PackageJsonDepResolution::Enabled
          };
          WorkspaceResolver::from_workspace(
            &d.member_dir.workspace,
            CliSys::default(),
            CreateResolverOptions {
              pkg_json_dep_resolution,
              specified_import_map: d.specified_import_map.clone(),
              sloppy_imports_options: if unstable_sloppy_imports {
                SloppyImportsOptions::Enabled
              } else {
                SloppyImportsOptions::Unspecified
              },
              fs_cache_options: FsCacheOptions::Disabled,
            },
          )
          .inspect_err(|err| {
            lsp_warn!(
              "Failed to load resolver: {err}", // will contain the specifier
            );
          })
          .ok()
          .unwrap_or_else(|| {
            // create a dummy resolver
            WorkspaceResolver::new_raw(
              d.scope.clone(),
              None,
              d.member_dir.workspace.resolver_jsr_pkgs().collect(),
              d.member_dir.workspace.package_jsons().cloned().collect(),
              pkg_json_dep_resolution,
              Default::default(),
              Default::default(),
              CliSys::default(),
            )
          })
        })
        .unwrap_or_else(|| {
          WorkspaceResolver::new_raw(
            // this is fine because this is only used before initialization
            Arc::new(ModuleSpecifier::parse("file:///").unwrap()),
            None,
            Vec::new(),
            Vec::new(),
            PackageJsonDepResolution::Disabled,
            Default::default(),
            Default::default(),
            self.sys.clone(),
          )
        });
      let diagnostics = workspace_resolver.diagnostics();
      if !diagnostics.is_empty() {
        lsp_warn!(
          "Workspace resolver diagnostics ({}):\n{}",
          self
            .config_data
            .map(|d| d.scope.as_str())
            .unwrap_or("null scope"),
          diagnostics
            .iter()
            .map(|d| format!("  - {d}"))
            .collect::<Vec<_>>()
            .join("\n")
        );
      }
      Arc::new(workspace_resolver)
    })
  }

  pub fn npm_installer(&self) -> Option<&Arc<CliNpmInstaller>> {
    self.services.npm_installer.as_ref()
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
          NodeResolverOptions {
            conditions: Default::default(),
            typescript_version: Some(
              deno_semver::Version::parse_standard(
                deno_lib::version::DENO_VERSION_INFO.typescript,
              )
              .unwrap(),
            ),
            bundle_mode: false, // will change if we add support for moduleResolution bundler
            is_browser_platform: false,
          },
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

impl deno_graph::source::Resolver for SingleReferrerGraphResolver<'_> {
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
    self
      .cli_resolver
      .resolve(
        specifier_text,
        &referrer_range.specifier,
        referrer_range.range.start,
        referrer_range
          .resolution_mode
          .map(node_resolver::ResolutionMode::from_deno_graph)
          .unwrap_or(self.module_resolution_mode),
        node_resolver::NodeResolutionKind::from_deno_graph(resolution_kind),
      )
      .map_err(|err| err.into_deno_graph_error())
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

type AddNpmReqsRequest = (
  Arc<CliNpmInstaller>,
  Vec<PackageReq>,
  std::sync::mpsc::Sender<Result<(), JsErrorBox>>,
);

#[derive(Debug)]
struct AddNpmReqsThread {
  join_handle: Option<std::thread::JoinHandle<()>>,
  request_tx: Option<tokio::sync::mpsc::UnboundedSender<AddNpmReqsRequest>>,
}

impl AddNpmReqsThread {
  pub fn create() -> Self {
    let (request_tx, mut request_rx) =
      tokio::sync::mpsc::unbounded_channel::<AddNpmReqsRequest>();
    let join_handle = std::thread::spawn(move || {
      create_basic_runtime().block_on(async move {
        while let Some((npm_installer, reqs, response_tx)) =
          request_rx.recv().await
        {
          deno_core::unsync::spawn(async move {
            let result = npm_installer.add_package_reqs_no_cache(&reqs).await;
            response_tx.send(result).unwrap();
          });
        }
      });
    });
    Self {
      join_handle: Some(join_handle),
      request_tx: Some(request_tx),
    }
  }

  pub fn add_npm_reqs(
    &self,
    npm_installer: Arc<CliNpmInstaller>,
    reqs: Vec<PackageReq>,
  ) -> Result<(), JsErrorBox> {
    let request_tx = self.request_tx.as_ref().unwrap();
    let (response_tx, response_rx) = std::sync::mpsc::channel();
    let _ = request_tx.send((npm_installer, reqs, response_tx));
    response_rx.recv().unwrap()
  }
}

impl Drop for AddNpmReqsThread {
  fn drop(&mut self) {
    drop(self.request_tx.take());
    self.join_handle.take().unwrap().join().unwrap();
  }
}

static ADD_NPM_REQS_THREAD: Lazy<AddNpmReqsThread> =
  Lazy::new(AddNpmReqsThread::create);

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
