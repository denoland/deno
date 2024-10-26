// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use dashmap::DashMap;
use deno_ast::MediaType;
use deno_cache_dir::HttpCache;
use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::WorkspaceResolver;
use deno_core::url::Url;
use deno_graph::source::Resolver;
use deno_graph::GraphImport;
use deno_graph::ModuleSpecifier;
use deno_npm::NpmSystemInfo;
use deno_path_util::url_to_file_path;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_node::PackageJson;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::NodeResolution;
use node_resolver::NodeResolutionMode;
use node_resolver::NpmResolver;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use super::cache::LspCache;
use super::jsr::JsrCacheResolver;
use crate::args::create_default_npmrc;
use crate::args::CacheSetting;
use crate::args::CliLockfile;
use crate::args::NpmInstallDepsProvider;
use crate::graph_util::CliJsrUrlProvider;
use crate::http_util::HttpClientProvider;
use crate::lsp::config::Config;
use crate::lsp::config::ConfigData;
use crate::lsp::logging::lsp_warn;
use crate::npm::create_cli_npm_resolver_for_lsp;
use crate::npm::CliByonmNpmResolverCreateOptions;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverCreateOptions;
use crate::npm::CliNpmResolverManagedCreateOptions;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::npm::ManagedCliNpmResolver;
use crate::resolver::CjsResolutionStore;
use crate::resolver::CliDenoResolverFs;
use crate::resolver::CliGraphResolver;
use crate::resolver::CliGraphResolverOptions;
use crate::resolver::CliNodeResolver;
use crate::resolver::WorkerCliNpmGraphResolver;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

#[derive(Debug, Clone)]
struct LspScopeResolver {
  graph_resolver: Arc<CliGraphResolver>,
  jsr_resolver: Option<Arc<JsrCacheResolver>>,
  npm_resolver: Option<Arc<dyn CliNpmResolver>>,
  node_resolver: Option<Arc<CliNodeResolver>>,
  redirect_resolver: Option<Arc<RedirectResolver>>,
  graph_imports: Arc<IndexMap<ModuleSpecifier, GraphImport>>,
  config_data: Option<Arc<ConfigData>>,
}

impl Default for LspScopeResolver {
  fn default() -> Self {
    Self {
      graph_resolver: create_graph_resolver(None, None, None),
      jsr_resolver: None,
      npm_resolver: None,
      node_resolver: None,
      redirect_resolver: None,
      graph_imports: Default::default(),
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
    let mut npm_resolver = None;
    let mut node_resolver = None;
    if let Some(http_client) = http_client_provider {
      npm_resolver = create_npm_resolver(
        config_data.map(|d| d.as_ref()),
        cache,
        http_client,
      )
      .await;
      node_resolver = create_node_resolver(npm_resolver.as_ref());
    }
    let graph_resolver = create_graph_resolver(
      config_data.map(|d| d.as_ref()),
      npm_resolver.as_ref(),
      node_resolver.as_ref(),
    );
    let jsr_resolver = Some(Arc::new(JsrCacheResolver::new(
      cache.for_specifier(config_data.map(|d| d.scope.as_ref())),
      config_data.map(|d| d.as_ref()),
    )));
    let redirect_resolver = Some(Arc::new(RedirectResolver::new(
      cache.for_specifier(config_data.map(|d| d.scope.as_ref())),
      config_data.and_then(|d| d.lockfile.clone()),
    )));
    let npm_graph_resolver = graph_resolver.create_graph_npm_resolver();
    let graph_imports = config_data
      .and_then(|d| d.member_dir.workspace.to_compiler_option_types().ok())
      .map(|imports| {
        Arc::new(
          imports
            .into_iter()
            .map(|(referrer, imports)| {
              let graph_import = GraphImport::new(
                &referrer,
                imports,
                &CliJsrUrlProvider,
                Some(graph_resolver.as_ref()),
                Some(&npm_graph_resolver),
              );
              (referrer, graph_import)
            })
            .collect(),
        )
      })
      .unwrap_or_default();
    Self {
      graph_resolver,
      jsr_resolver,
      npm_resolver,
      node_resolver,
      redirect_resolver,
      graph_imports,
      config_data: config_data.cloned(),
    }
  }

  fn snapshot(&self) -> Arc<Self> {
    let npm_resolver =
      self.npm_resolver.as_ref().map(|r| r.clone_snapshotted());
    let node_resolver = create_node_resolver(npm_resolver.as_ref());
    let graph_resolver = create_graph_resolver(
      self.config_data.as_deref(),
      npm_resolver.as_ref(),
      node_resolver.as_ref(),
    );
    Arc::new(Self {
      graph_resolver,
      jsr_resolver: self.jsr_resolver.clone(),
      npm_resolver,
      node_resolver,
      redirect_resolver: self.redirect_resolver.clone(),
      graph_imports: self.graph_imports.clone(),
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

  pub async fn set_npm_reqs(
    &self,
    reqs: &BTreeMap<Option<ModuleSpecifier>, BTreeSet<PackageReq>>,
  ) {
    for (scope, resolver) in [(None, &self.unscoped)]
      .into_iter()
      .chain(self.by_scope.iter().map(|(s, r)| (Some(s), r)))
    {
      if let Some(npm_resolver) = resolver.npm_resolver.as_ref() {
        if let Some(npm_resolver) = npm_resolver.as_managed() {
          let reqs = reqs
            .get(&scope.cloned())
            .map(|reqs| reqs.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
          if let Err(err) = npm_resolver.set_package_reqs(&reqs).await {
            lsp_warn!("Could not set npm package requirements: {:#}", err);
          }
        }
      }
    }
  }

  pub fn as_graph_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> &dyn Resolver {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.graph_resolver.as_ref()
  }

  pub fn create_graph_npm_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> WorkerCliNpmGraphResolver {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.graph_resolver.create_graph_npm_resolver()
  }

  pub fn maybe_managed_npm_resolver(
    &self,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<&ManagedCliNpmResolver> {
    let resolver = self.get_scope_resolver(file_referrer);
    resolver.npm_resolver.as_ref().and_then(|r| r.as_managed())
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
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let resolver = self.get_scope_resolver(file_referrer);
    let node_resolver = resolver.node_resolver.as_ref()?;
    Some(NodeResolution::into_specifier_and_media_type(Some(
      node_resolver
        .resolve_req_reference(req_ref, referrer, NodeResolutionMode::Types)
        .ok()?,
    )))
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

    let global_npm_resolver = self
      .get_scope_resolver(Some(specifier))
      .npm_resolver
      .as_ref()
      .and_then(|npm_resolver| npm_resolver.as_managed())
      .filter(|r| r.root_node_modules_path().is_none());
    if let Some(npm_resolver) = &global_npm_resolver {
      if npm_resolver.in_npm_package(specifier) {
        return true;
      }
    }

    has_node_modules_dir(specifier)
  }

  pub fn node_media_type(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<MediaType> {
    let resolver = self.get_scope_resolver(Some(specifier));
    let node_resolver = resolver.node_resolver.as_ref()?;
    let resolution = node_resolver
      .url_to_node_resolution(specifier.clone())
      .ok()?;
    Some(NodeResolution::into_specifier_and_media_type(Some(resolution)).1)
  }

  pub fn is_bare_package_json_dep(
    &self,
    specifier_text: &str,
    referrer: &ModuleSpecifier,
  ) -> bool {
    let resolver = self.get_scope_resolver(Some(referrer));
    let Some(node_resolver) = resolver.node_resolver.as_ref() else {
      return false;
    };
    node_resolver
      .resolve_if_for_npm_pkg(
        specifier_text,
        referrer,
        NodeResolutionMode::Types,
      )
      .ok()
      .flatten()
      .is_some()
  }

  pub fn get_closest_package_json(
    &self,
    referrer: &ModuleSpecifier,
  ) -> Result<Option<Arc<PackageJson>>, ClosestPkgJsonError> {
    let resolver = self.get_scope_resolver(Some(referrer));
    let Some(node_resolver) = resolver.node_resolver.as_ref() else {
      return Ok(None);
    };
    node_resolver.get_closest_package_json(referrer)
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

async fn create_npm_resolver(
  config_data: Option<&ConfigData>,
  cache: &LspCache,
  http_client_provider: &Arc<HttpClientProvider>,
) -> Option<Arc<dyn CliNpmResolver>> {
  let enable_byonm = config_data.map(|d| d.byonm).unwrap_or(false);
  let options = if enable_byonm {
    CliNpmResolverCreateOptions::Byonm(CliByonmNpmResolverCreateOptions {
      fs: CliDenoResolverFs(Arc::new(deno_fs::RealFs)),
      root_node_modules_dir: config_data.and_then(|config_data| {
        config_data.node_modules_dir.clone().or_else(|| {
          url_to_file_path(&config_data.scope)
            .ok()
            .map(|p| p.join("node_modules/"))
        })
      }),
    })
  } else {
    CliNpmResolverCreateOptions::Managed(CliNpmResolverManagedCreateOptions {
      http_client_provider: http_client_provider.clone(),
      snapshot: match config_data.and_then(|d| d.lockfile.as_ref()) {
        Some(lockfile) => {
          CliNpmResolverManagedSnapshotOption::ResolveFromLockfile(
            lockfile.clone(),
          )
        }
        None => CliNpmResolverManagedSnapshotOption::Specified(None),
      },
      // Don't provide the lockfile. We don't want these resolvers
      // updating it. Only the cache request should update the lockfile.
      maybe_lockfile: None,
      fs: Arc::new(deno_fs::RealFs),
      npm_global_cache_dir: cache.deno_dir().npm_folder_path(),
      // Use an "only" cache setting in order to make the
      // user do an explicit "cache" command and prevent
      // the cache from being filled with lots of packages while
      // the user is typing.
      cache_setting: CacheSetting::Only,
      text_only_progress_bar: ProgressBar::new(ProgressBarStyle::TextOnly),
      maybe_node_modules_path: config_data
        .and_then(|d| d.node_modules_dir.clone()),
      // only used for top level install, so we can ignore this
      npm_install_deps_provider: Arc::new(NpmInstallDepsProvider::empty()),
      npmrc: config_data
        .and_then(|d| d.npmrc.clone())
        .unwrap_or_else(create_default_npmrc),
      npm_system_info: NpmSystemInfo::default(),
      lifecycle_scripts: Default::default(),
    })
  };
  Some(create_cli_npm_resolver_for_lsp(options).await)
}

fn create_node_resolver(
  npm_resolver: Option<&Arc<dyn CliNpmResolver>>,
) -> Option<Arc<CliNodeResolver>> {
  use once_cell::sync::Lazy;

  // it's not ideal to share this across all scopes and to
  // never clear it, but it's fine for the time being
  static CJS_RESOLUTIONS: Lazy<Arc<CjsResolutionStore>> =
    Lazy::new(Default::default);

  let npm_resolver = npm_resolver?;
  let fs = Arc::new(deno_fs::RealFs);
  let node_resolver_inner = Arc::new(NodeResolver::new(
    deno_runtime::deno_node::DenoFsNodeResolverEnv::new(fs.clone()),
    npm_resolver.clone().into_npm_resolver(),
  ));
  Some(Arc::new(CliNodeResolver::new(
    CJS_RESOLUTIONS.clone(),
    fs,
    node_resolver_inner,
    npm_resolver.clone(),
  )))
}

fn create_graph_resolver(
  config_data: Option<&ConfigData>,
  npm_resolver: Option<&Arc<dyn CliNpmResolver>>,
  node_resolver: Option<&Arc<CliNodeResolver>>,
) -> Arc<CliGraphResolver> {
  let workspace = config_data.map(|d| &d.member_dir.workspace);
  Arc::new(CliGraphResolver::new(CliGraphResolverOptions {
    node_resolver: node_resolver.cloned(),
    npm_resolver: npm_resolver.cloned(),
    workspace_resolver: config_data.map(|d| d.resolver.clone()).unwrap_or_else(
      || {
        Arc::new(WorkspaceResolver::new_raw(
          // this is fine because this is only used before initialization
          Arc::new(ModuleSpecifier::parse("file:///").unwrap()),
          None,
          Vec::new(),
          Vec::new(),
          PackageJsonDepResolution::Disabled,
        ))
      },
    ),
    maybe_jsx_import_source_config: workspace.and_then(|workspace| {
      workspace.to_maybe_jsx_import_source_config().ok().flatten()
    }),
    maybe_vendor_dir: config_data.and_then(|d| d.vendor_dir.as_ref()),
    bare_node_builtins_enabled: workspace
      .is_some_and(|workspace| workspace.has_unstable("bare-node-builtins")),
    sloppy_imports_resolver: config_data
      .and_then(|d| d.sloppy_imports_resolver.clone()),
  }))
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
        let Ok(target) =
          deno_core::resolve_import(location, specifier.as_str())
        else {
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
