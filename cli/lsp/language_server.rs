// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::unsync::spawn;
use deno_core::ModuleSpecifier;
use deno_graph::GraphKind;
use deno_lockfile::Lockfile;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use import_map::ImportMap;
use indexmap::IndexSet;
use log::error;
use serde_json::from_value;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tower_lsp::jsonrpc::Error as LspError;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::request::*;
use tower_lsp::lsp_types::*;

use super::analysis::fix_ts_import_changes;
use super::analysis::ts_changes_to_edit;
use super::analysis::CodeActionCollection;
use super::analysis::CodeActionData;
use super::analysis::TsResponseImportMapper;
use super::cache;
use super::capabilities;
use super::client::Client;
use super::code_lens;
use super::completions;
use super::config::Config;
use super::config::ConfigSnapshot;
use super::config::UpdateImportsOnFileMoveEnabled;
use super::config::WorkspaceSettings;
use super::config::SETTINGS_SECTION;
use super::diagnostics;
use super::diagnostics::DiagnosticDataSpecifier;
use super::diagnostics::DiagnosticServerUpdateMessage;
use super::diagnostics::DiagnosticsServer;
use super::diagnostics::DiagnosticsState;
use super::documents::to_hover_text;
use super::documents::to_lsp_range;
use super::documents::AssetOrDocument;
use super::documents::Document;
use super::documents::Documents;
use super::documents::DocumentsFilter;
use super::documents::LanguageId;
use super::documents::UpdateDocumentConfigOptions;
use super::logging::lsp_log;
use super::logging::lsp_warn;
use super::lsp_custom;
use super::lsp_custom::TaskDefinition;
use super::npm::CliNpmSearchApi;
use super::parent_process_checker;
use super::performance::Performance;
use super::performance::PerformanceMark;
use super::refactor;
use super::registries::ModuleRegistry;
use super::testing;
use super::text;
use super::tsc;
use super::tsc::Assets;
use super::tsc::AssetsSnapshot;
use super::tsc::GetCompletionDetailsArgs;
use super::tsc::TsServer;
use super::urls;
use crate::args::get_root_cert_store;
use crate::args::package_json;
use crate::args::resolve_import_map_from_specifier;
use crate::args::CaData;
use crate::args::CacheSetting;
use crate::args::CliOptions;
use crate::args::ConfigFile;
use crate::args::Flags;
use crate::args::FmtOptions;
use crate::args::LintOptions;
use crate::args::TsConfig;
use crate::cache::DenoDir;
use crate::cache::FastInsecureHasher;
use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::cache::LocalLspHttpCache;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::graph_util;
use crate::http_util::HttpClient;
use crate::lsp::logging::init_log_file;
use crate::lsp::tsc::file_text_changes_to_workspace_edit;
use crate::lsp::urls::LspUrlKind;
use crate::npm::create_cli_npm_resolver_for_lsp;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverByonmCreateOptions;
use crate::npm::CliNpmResolverCreateOptions;
use crate::npm::CliNpmResolverManagedCreateOptions;
use crate::npm::CliNpmResolverManagedPackageJsonInstallerOption;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::tools::fmt::format_file;
use crate::tools::fmt::format_parsed_source;
use crate::tools::upgrade::check_for_upgrades_for_lsp;
use crate::tools::upgrade::upgrade_check_enabled;
use crate::util::fs::remove_dir_all_if_exists;
use crate::util::path::is_importable_ext;
use crate::util::path::specifier_to_file_path;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

struct LspRootCertStoreProvider(RootCertStore);

impl RootCertStoreProvider for LspRootCertStoreProvider {
  fn get_or_try_init(&self) -> Result<&RootCertStore, AnyError> {
    Ok(&self.0)
  }
}

#[derive(Debug)]
struct LspNpmServices {
  /// When this hash changes, the services need updating
  config_hash: LspNpmConfigHash,
  /// Npm's search api.
  search_api: CliNpmSearchApi,
  /// Node resolver.
  node_resolver: Option<Arc<NodeResolver>>,
  /// Resolver for npm packages.
  resolver: Option<Arc<dyn CliNpmResolver>>,
}

#[derive(Debug, PartialEq, Eq)]
struct LspNpmConfigHash(u64);

impl LspNpmConfigHash {
  pub fn from_inner(inner: &Inner) -> Self {
    let mut hasher = FastInsecureHasher::new();
    hasher.write_hashable(inner.config.maybe_node_modules_dir_path());
    hasher.write_hashable(&inner.maybe_global_cache_path);
    if let Some(lockfile) = inner.config.maybe_lockfile() {
      hasher.write_hashable(&*lockfile.lock());
    }
    Self(hasher.finish())
  }
}

#[derive(Debug, Clone)]
pub struct LanguageServer(Arc<tokio::sync::RwLock<Inner>>, CancellationToken);

#[derive(Debug)]
pub struct StateNpmSnapshot {
  pub node_resolver: Arc<NodeResolver>,
  pub npm_resolver: Arc<dyn CliNpmResolver>,
}

/// Snapshot of the state used by TSC.
#[derive(Debug)]
pub struct StateSnapshot {
  pub assets: AssetsSnapshot,
  pub cache_metadata: cache::CacheMetadata,
  pub config: Arc<ConfigSnapshot>,
  pub documents: Documents,
  pub maybe_import_map: Option<Arc<ImportMap>>,
  pub npm: Option<StateNpmSnapshot>,
}

#[derive(Debug)]
pub struct Inner {
  /// Cached versions of "fixed" assets that can either be inlined in Rust or
  /// are part of the TypeScript snapshot and have to be fetched out.
  assets: Assets,
  /// A representation of metadata associated with specifiers in the DENO_DIR
  /// which is used by the language server
  cache_metadata: cache::CacheMetadata,
  /// The LSP client that this LSP server is connected to.
  pub client: Client,
  /// Configuration information.
  pub config: Config,
  deps_http_cache: Arc<dyn HttpCache>,
  diagnostics_state: Arc<diagnostics::DiagnosticsState>,
  diagnostics_server: diagnostics::DiagnosticsServer,
  /// The collection of documents that the server is currently handling, either
  /// on disk or "open" within the client.
  pub documents: Documents,
  http_client: Arc<HttpClient>,
  /// Handles module registries, which allow discovery of modules
  module_registries: ModuleRegistry,
  /// The path to the module registries cache
  module_registries_location: PathBuf,
  /// An optional path to the DENO_DIR which has been specified in the client
  /// options.
  maybe_global_cache_path: Option<PathBuf>,
  /// An optional import map which is used to resolve modules.
  maybe_import_map: Option<Arc<ImportMap>>,
  /// The URL for the import map which is used to determine relative imports.
  maybe_import_map_uri: Option<Url>,
  /// An optional package.json configuration file.
  maybe_package_json: Option<PackageJson>,
  /// Configuration for formatter which has been taken from specified config file.
  fmt_options: FmtOptions,
  /// An optional configuration for linter which has been taken from specified config file.
  lint_options: LintOptions,
  /// A lazily create "server" for handling test run requests.
  maybe_testing_server: Option<testing::TestServer>,
  /// Services used for dealing with npm related functionality.
  npm: LspNpmServices,
  /// A collection of measurements which instrument that performance of the LSP.
  performance: Arc<Performance>,
  /// A memoized version of fixable diagnostic codes retrieved from TypeScript.
  ts_fixable_diagnostics: Vec<String>,
  /// An abstraction that handles interactions with TypeScript.
  pub ts_server: Arc<TsServer>,
  /// A map of specifiers and URLs used to translate over the LSP.
  pub url_map: urls::LspUrlMap,
}

impl LanguageServer {
  pub fn new(client: Client, token: CancellationToken) -> Self {
    Self(
      Arc::new(tokio::sync::RwLock::new(Inner::new(client))),
      token,
    )
  }

  /// Similar to `deno cache` on the command line, where modules will be cached
  /// in the Deno cache, including any of their dependencies.
  pub async fn cache_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    async fn create_graph_for_caching(
      cli_options: CliOptions,
      roots: Vec<ModuleSpecifier>,
      open_docs: Vec<Document>,
    ) -> Result<(), AnyError> {
      let open_docs = open_docs
        .into_iter()
        .map(|d| (d.specifier().clone(), d))
        .collect::<HashMap<_, _>>();
      let cli_options = Arc::new(cli_options);
      let factory = CliFactory::from_cli_options(cli_options.clone());
      let module_graph_builder = factory.module_graph_builder().await?;
      let mut inner_loader = module_graph_builder.create_graph_loader();
      let mut loader = crate::lsp::documents::OpenDocumentsGraphLoader {
        inner_loader: &mut inner_loader,
        open_docs: &open_docs,
        unstable_sloppy_imports: cli_options.unstable_sloppy_imports(),
      };
      let graph = module_graph_builder
        .create_graph_with_loader(GraphKind::All, roots.clone(), &mut loader)
        .await?;
      graph_util::graph_valid(
        &graph,
        factory.fs().as_ref(),
        &roots,
        graph_util::GraphValidOptions {
          is_vendoring: false,
          follow_type_only: true,
          check_js: false,
        },
      )?;

      // Update the lockfile on the file system with anything new
      // found after caching
      if let Some(lockfile) = cli_options.maybe_lockfile() {
        let lockfile = lockfile.lock();
        if let Err(err) = lockfile.write() {
          lsp_warn!("Error writing lockfile: {}", err);
        }
      }

      Ok(())
    }

    match params.map(serde_json::from_value) {
      Some(Ok(params)) => {
        // do as much as possible in a read, then do a write outside
        let maybe_prepare_cache_result = {
          let inner = self.0.read().await; // ensure dropped
          match inner.prepare_cache(params) {
            Ok(maybe_cache_result) => maybe_cache_result,
            Err(err) => {
              self
                .0
                .read()
                .await
                .client
                .show_message(MessageType::WARNING, err);
              return Err(LspError::internal_error());
            }
          }
        };
        if let Some(result) = maybe_prepare_cache_result {
          let cli_options = result.cli_options;
          let roots = result.roots;
          let open_docs = result.open_docs;
          let handle = spawn(async move {
            create_graph_for_caching(cli_options, roots, open_docs).await
          });
          if let Err(err) = handle.await.unwrap() {
            self
              .0
              .read()
              .await
              .client
              .show_message(MessageType::WARNING, err);
          }
          // do npm resolution in a write—we should have everything
          // cached by this point anyway
          self.0.write().await.refresh_npm_specifiers().await;
          // now refresh the data in a read
          self.0.read().await.post_cache(result.mark).await;
        }
        Ok(Some(json!(true)))
      }
      Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
      None => Err(LspError::invalid_params("Missing parameters")),
    }
  }

  /// This request is only used by the lsp integration tests to
  /// coordinate the tests receiving the latest diagnostics.
  pub async fn latest_diagnostic_batch_index_request(
    &self,
  ) -> LspResult<Option<Value>> {
    Ok(
      self
        .0
        .read()
        .await
        .diagnostics_server
        .latest_batch_index()
        .map(|v| v.into()),
    )
  }

  pub async fn performance_request(&self) -> LspResult<Option<Value>> {
    Ok(Some(self.0.read().await.get_performance()))
  }

  pub async fn reload_import_registries_request(
    &self,
  ) -> LspResult<Option<Value>> {
    self.0.write().await.reload_import_registries().await
  }

  pub async fn task_definitions(&self) -> LspResult<Vec<TaskDefinition>> {
    self.0.read().await.task_definitions()
  }

  pub async fn test_run_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    let inner = self.0.read().await;
    if let Some(testing_server) = &inner.maybe_testing_server {
      match params.map(serde_json::from_value) {
        Some(Ok(params)) => testing_server
          .run_request(params, inner.config.workspace_settings().clone()),
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      }
    } else {
      Err(LspError::invalid_request())
    }
  }

  pub async fn test_run_cancel_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    if let Some(testing_server) = &self.0.read().await.maybe_testing_server {
      match params.map(serde_json::from_value) {
        Some(Ok(params)) => testing_server.run_cancel_request(params),
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      }
    } else {
      Err(LspError::invalid_request())
    }
  }

  pub async fn virtual_text_document(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    match params.map(serde_json::from_value) {
      Some(Ok(params)) => Ok(Some(
        serde_json::to_value(
          self.0.read().await.virtual_text_document(params)?,
        )
        .map_err(|err| {
          error!(
            "Failed to serialize virtual_text_document response: {}",
            err
          );
          LspError::internal_error()
        })?,
      )),
      Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
      None => Err(LspError::invalid_params("Missing parameters")),
    }
  }

  pub async fn refresh_configuration(&self) {
    let (client, folders, capable) = {
      let ls = self.0.read().await;
      (
        ls.client.clone(),
        ls.config.workspace_folders.clone(),
        ls.config.client_capabilities.workspace_configuration,
      )
    };
    if capable {
      let mut scopes = Vec::with_capacity(folders.len() + 1);
      scopes.push(None);
      for (_, folder) in &folders {
        scopes.push(Some(folder.uri.clone()));
      }
      let configs = client
        .when_outside_lsp_lock()
        .workspace_configuration(scopes)
        .await;
      if let Ok(configs) = configs {
        if configs.len() != folders.len() + 1 {
          lsp_warn!("Incorrect number of configurations received.");
          return;
        }
        let mut configs = configs.into_iter();
        let unscoped = configs.next().unwrap();
        let mut by_workspace_folder = BTreeMap::new();
        for (folder_uri, _) in &folders {
          by_workspace_folder
            .insert(folder_uri.clone(), configs.next().unwrap());
        }
        let mut ls = self.0.write().await;
        ls.config
          .set_workspace_settings(unscoped, Some(by_workspace_folder));
      }
    }
  }
}

impl Inner {
  fn new(client: Client) -> Self {
    let dir = DenoDir::new(None).expect("could not access DENO_DIR");
    let module_registries_location = dir.registries_folder_path();
    let http_client = Arc::new(HttpClient::new(None, None));
    let module_registries = ModuleRegistry::new(
      module_registries_location.clone(),
      http_client.clone(),
    );
    let npm_search_api =
      CliNpmSearchApi::new(module_registries.file_fetcher.clone(), None);
    let location = dir.deps_folder_path();
    let deps_http_cache = Arc::new(GlobalHttpCache::new(
      location,
      crate::cache::RealDenoCacheEnv,
    ));
    let documents = Documents::new(deps_http_cache.clone());
    let cache_metadata = cache::CacheMetadata::new(deps_http_cache.clone());
    let performance = Arc::new(Performance::default());
    let ts_server =
      Arc::new(TsServer::new(performance.clone(), deps_http_cache.clone()));
    let config = Config::new();
    let diagnostics_state = Arc::new(DiagnosticsState::default());
    let diagnostics_server = DiagnosticsServer::new(
      client.clone(),
      performance.clone(),
      ts_server.clone(),
      diagnostics_state.clone(),
    );
    let assets = Assets::new(ts_server.clone());

    Self {
      assets,
      cache_metadata,
      client,
      config,
      deps_http_cache,
      diagnostics_state,
      diagnostics_server,
      documents,
      http_client,
      maybe_global_cache_path: None,
      maybe_import_map: None,
      maybe_import_map_uri: None,
      maybe_package_json: None,
      fmt_options: Default::default(),
      lint_options: Default::default(),
      maybe_testing_server: None,
      module_registries,
      module_registries_location,
      npm: LspNpmServices {
        config_hash: LspNpmConfigHash(0), // this will be updated in initialize
        search_api: npm_search_api,
        node_resolver: None,
        resolver: None,
      },
      performance,
      ts_fixable_diagnostics: Default::default(),
      ts_server,
      url_map: Default::default(),
    }
  }

  /// Searches assets and documents for the provided
  /// specifier erroring if it doesn't exist.
  pub fn get_asset_or_document(
    &self,
    specifier: &ModuleSpecifier,
  ) -> LspResult<AssetOrDocument> {
    self
      .get_maybe_asset_or_document(specifier)
      .map(Ok)
      .unwrap_or_else(|| {
        Err(LspError::invalid_params(format!(
          "Unable to find asset or document for: {specifier}"
        )))
      })
  }

  /// Searches assets and documents for the provided specifier.
  pub fn get_maybe_asset_or_document(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<AssetOrDocument> {
    if specifier.scheme() == "asset" {
      self.assets.get(specifier).map(AssetOrDocument::Asset)
    } else {
      self.documents.get(specifier).map(AssetOrDocument::Document)
    }
  }

  pub async fn get_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Arc<tsc::NavigationTree>, AnyError> {
    let mark = self.performance.mark_with_args(
      "lsp.get_navigation_tree",
      json!({ "specifier": specifier }),
    );
    let asset_or_doc = self.get_asset_or_document(specifier)?;
    let navigation_tree =
      if let Some(navigation_tree) = asset_or_doc.maybe_navigation_tree() {
        navigation_tree
      } else {
        let navigation_tree: tsc::NavigationTree = self
          .ts_server
          .get_navigation_tree(self.snapshot(), specifier.clone())
          .await?;
        let navigation_tree = Arc::new(navigation_tree);
        match asset_or_doc {
          AssetOrDocument::Asset(_) => self
            .assets
            .cache_navigation_tree(specifier, navigation_tree.clone())?,
          AssetOrDocument::Document(doc) => {
            self.documents.try_cache_navigation_tree(
              specifier,
              &doc.script_version(),
              navigation_tree.clone(),
            )?
          }
        }
        navigation_tree
      };
    self.performance.measure(mark);
    Ok(navigation_tree)
  }

  fn get_config_file(&self) -> Result<Option<ConfigFile>, AnyError> {
    let workspace_settings = self.config.workspace_settings();
    let maybe_config = &workspace_settings.config;
    if let Some(config_str) = maybe_config {
      if !config_str.is_empty() {
        lsp_log!("Setting Deno configuration from: \"{}\"", config_str);
        let config_url = if let Ok(url) = Url::from_file_path(config_str) {
          Ok(url)
        } else if let Some(root_uri) = self.config.root_uri() {
          root_uri.join(config_str).map_err(|_| {
            anyhow!("Bad file path for configuration file: \"{}\"", config_str)
          })
        } else {
          Err(anyhow!(
            "The path to the configuration file (\"{}\") is not resolvable.",
            config_str
          ))
        }?;
        lsp_log!("  Resolved configuration file: \"{}\"", config_url);

        let config_file = ConfigFile::from_specifier(config_url)?;
        return Ok(Some(config_file));
      }
    }

    // Auto-discover config

    // It is possible that root_uri is not set, for example when having a single
    // file open and not a workspace.  In those situations we can't
    // automatically discover the configuration
    if let Some(root_uri) = self.config.root_uri() {
      let root_path = specifier_to_file_path(root_uri)?;
      let mut checked = std::collections::HashSet::new();
      let maybe_config = ConfigFile::discover_from(&root_path, &mut checked)?;
      Ok(maybe_config.map(|c| {
        lsp_log!("  Auto-resolved configuration file: \"{}\"", c.specifier);
        c
      }))
    } else {
      Ok(None)
    }
  }

  fn get_package_json(
    &self,
    maybe_config_file: Option<&ConfigFile>,
  ) -> Result<Option<PackageJson>, AnyError> {
    if crate::args::has_flag_env_var("DENO_NO_PACKAGE_JSON") {
      return Ok(None);
    }

    // It is possible that root_uri is not set, for example when having a single
    // file open and not a workspace.  In those situations we can't
    // automatically discover the configuration
    if let Some(root_uri) = self.config.root_uri() {
      let root_path = specifier_to_file_path(root_uri)?;
      let maybe_package_json = package_json::discover_from(
        &root_path,
        maybe_config_file.and_then(|f| f.specifier.to_file_path().ok()),
      )?;
      Ok(maybe_package_json.map(|c| {
        lsp_log!("  Auto-resolved package.json: \"{}\"", c.specifier());
        c
      }))
    } else {
      Ok(None)
    }
  }

  fn is_diagnosable(&self, specifier: &ModuleSpecifier) -> bool {
    if specifier.scheme() == "asset" {
      matches!(
        MediaType::from_specifier(specifier),
        MediaType::JavaScript
          | MediaType::Jsx
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::TypeScript
          | MediaType::Tsx
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Dts
          | MediaType::Dmts
          | MediaType::Dcts
      )
    } else {
      self
        .documents
        .get(specifier)
        .map(|d| d.is_diagnosable())
        .unwrap_or(false)
    }
  }

  fn merge_user_tsconfig(
    &self,
    tsconfig: &mut TsConfig,
  ) -> Result<(), AnyError> {
    if let Some(config_file) = self.config.maybe_config_file() {
      let (value, maybe_ignored_options) = config_file.to_compiler_options()?;
      tsconfig.merge(&value);
      if let Some(ignored_options) = maybe_ignored_options {
        // TODO(@kitsonk) turn these into diagnostics that can be sent to the
        // client
        lsp_warn!("{}", ignored_options);
      }
    }

    Ok(())
  }

  pub fn snapshot(&self) -> Arc<StateSnapshot> {
    let maybe_state_npm_snapshot = self
      .npm
      .resolver
      .as_ref()
      .map(|resolver| resolver.clone_snapshotted())
      .map(|resolver| {
        let fs = Arc::new(deno_fs::RealFs);
        let node_resolver =
          Arc::new(NodeResolver::new(fs, resolver.clone().into_npm_resolver()));
        StateNpmSnapshot {
          node_resolver,
          npm_resolver: resolver,
        }
      });
    Arc::new(StateSnapshot {
      assets: self.assets.snapshot(),
      cache_metadata: self.cache_metadata.clone(),
      config: self.config.snapshot(),
      documents: self.documents.clone(),
      maybe_import_map: self.maybe_import_map.clone(),
      npm: maybe_state_npm_snapshot,
    })
  }

  pub async fn update_cache(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("lsp.update_cache");
    self.performance.measure(mark);
    let maybe_cache = &self.config.workspace_settings().cache;
    let maybe_global_cache_path = if let Some(cache_str) = maybe_cache {
      lsp_log!("Setting global cache path from: \"{}\"", cache_str);
      let cache_url = if let Ok(url) = Url::from_file_path(cache_str) {
        Ok(url)
      } else if let Some(root_uri) = self.config.root_uri() {
        let root_path = specifier_to_file_path(root_uri)?;
        let cache_path = root_path.join(cache_str);
        Url::from_file_path(cache_path).map_err(|_| {
          anyhow!("Bad file path for import path: {:?}", cache_str)
        })
      } else {
        Err(anyhow!(
          "The path to the cache path (\"{}\") is not resolvable.",
          cache_str
        ))
      }?;
      let cache_path = specifier_to_file_path(&cache_url)?;
      lsp_log!(
        "  Resolved global cache path: \"{}\"",
        cache_path.to_string_lossy()
      );
      Some(cache_path)
    } else {
      None
    };
    if self.maybe_global_cache_path != maybe_global_cache_path {
      self
        .set_new_global_cache_path(maybe_global_cache_path)
        .await?;
    }
    Ok(())
  }

  async fn recreate_http_client_and_dependents(
    &mut self,
  ) -> Result<(), AnyError> {
    self
      .set_new_global_cache_path(self.maybe_global_cache_path.clone())
      .await
  }

  /// Recreates the http client and all dependent structs.
  async fn set_new_global_cache_path(
    &mut self,
    new_cache_path: Option<PathBuf>,
  ) -> Result<(), AnyError> {
    let dir = DenoDir::new(new_cache_path.clone())?;
    let workspace_settings = self.config.workspace_settings();
    let maybe_root_path = self
      .config
      .root_uri()
      .and_then(|uri| specifier_to_file_path(uri).ok());
    let root_cert_store = get_root_cert_store(
      maybe_root_path,
      workspace_settings.certificate_stores.clone(),
      workspace_settings.tls_certificate.clone().map(CaData::File),
    )?;
    let root_cert_store_provider =
      Arc::new(LspRootCertStoreProvider(root_cert_store));
    let module_registries_location = dir.registries_folder_path();
    self.http_client = Arc::new(HttpClient::new(
      Some(root_cert_store_provider),
      workspace_settings
        .unsafely_ignore_certificate_errors
        .clone(),
    ));
    self.module_registries = ModuleRegistry::new(
      module_registries_location.clone(),
      self.http_client.clone(),
    );
    self.npm.search_api =
      CliNpmSearchApi::new(self.module_registries.file_fetcher.clone(), None);
    self.module_registries_location = module_registries_location;
    // update the cache path
    let global_cache = Arc::new(GlobalHttpCache::new(
      dir.deps_folder_path(),
      crate::cache::RealDenoCacheEnv,
    ));
    let maybe_local_cache =
      self.config.maybe_vendor_dir_path().map(|local_path| {
        Arc::new(LocalLspHttpCache::new(local_path, global_cache.clone()))
      });
    let cache: Arc<dyn HttpCache> = maybe_local_cache
      .clone()
      .map(|c| c as Arc<dyn HttpCache>)
      .unwrap_or(global_cache);
    self.deps_http_cache = cache.clone();
    self.documents.set_cache(cache.clone());
    self.cache_metadata.set_cache(cache);
    self.url_map.set_cache(maybe_local_cache);
    self.maybe_global_cache_path = new_cache_path;
    Ok(())
  }

  async fn recreate_npm_services_if_necessary(&mut self) {
    let deno_dir = match DenoDir::new(self.maybe_global_cache_path.clone()) {
      Ok(deno_dir) => deno_dir,
      Err(err) => {
        lsp_warn!("Error getting deno dir: {}", err);
        return;
      }
    };
    let config_hash = LspNpmConfigHash::from_inner(self);
    if config_hash == self.npm.config_hash {
      return; // no need to do anything
    }

    let npm_resolver = create_npm_resolver(
      &deno_dir,
      &self.http_client,
      self.config.maybe_config_file(),
      self.config.maybe_lockfile(),
      self.config.maybe_node_modules_dir_path().cloned(),
    )
    .await;
    self.npm.node_resolver = Some(Arc::new(NodeResolver::new(
      Arc::new(deno_fs::RealFs),
      npm_resolver.clone().into_npm_resolver(),
    )));
    self.npm.resolver = Some(npm_resolver);

    // update the hash
    self.npm.config_hash = config_hash;
  }

  pub async fn update_import_map(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("lsp.update_import_map");

    let maybe_import_map_url = self.resolve_import_map_specifier()?;
    if let Some(import_map_url) = maybe_import_map_url {
      if import_map_url.scheme() != "data" {
        lsp_log!("  Resolved import map: \"{}\"", import_map_url);
      }

      let import_map = self
        .fetch_import_map(&import_map_url, CacheSetting::RespectHeaders)
        .await?;
      self.maybe_import_map_uri = Some(import_map_url);
      self.maybe_import_map = Some(Arc::new(import_map));
    } else {
      self.maybe_import_map_uri = None;
      self.maybe_import_map = None;
    }
    self.performance.measure(mark);
    Ok(())
  }

  async fn fetch_import_map(
    &self,
    import_map_url: &ModuleSpecifier,
    cache_setting: CacheSetting,
  ) -> Result<ImportMap, AnyError> {
    resolve_import_map_from_specifier(
      import_map_url,
      self.config.maybe_config_file(),
      &self.create_file_fetcher(cache_setting),
    )
    .await
    .map_err(|err| {
      anyhow!(
        "Failed to load the import map at: {}. {:#}",
        import_map_url,
        err
      )
    })
  }

  fn create_file_fetcher(&self, cache_setting: CacheSetting) -> FileFetcher {
    let mut file_fetcher = FileFetcher::new(
      self.deps_http_cache.clone(),
      cache_setting,
      true,
      self.http_client.clone(),
      Default::default(),
      None,
    );
    file_fetcher.set_download_log_level(super::logging::lsp_log_level());
    file_fetcher
  }

  fn resolve_import_map_specifier(
    &self,
  ) -> Result<Option<ModuleSpecifier>, AnyError> {
    Ok(
      if let Some(import_map_str) = self
        .config
        .workspace_settings()
        .import_map
        .clone()
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
      {
        lsp_log!(
          "Setting import map from workspace settings: \"{}\"",
          import_map_str
        );
        if let Some(config_file) = self.config.maybe_config_file() {
          if let Some(import_map_path) = config_file.to_import_map_path() {
            lsp_log!("Warning: Import map \"{}\" configured in \"{}\" being ignored due to an import map being explicitly configured in workspace settings.", import_map_path, config_file.specifier);
          }
        }
        if let Ok(url) = Url::parse(&import_map_str) {
          Some(url)
        } else if let Some(root_uri) = self.config.root_uri() {
          let root_path = specifier_to_file_path(root_uri)?;
          let import_map_path = root_path.join(&import_map_str);
          let import_map_url =
            Url::from_file_path(import_map_path).map_err(|_| {
              anyhow!("Bad file path for import map: {}", import_map_str)
            })?;
          Some(import_map_url)
        } else {
          return Err(anyhow!(
            "The path to the import map (\"{}\") is not resolvable.",
            import_map_str
          ));
        }
      } else if let Some(config_file) = self.config.maybe_config_file() {
        if config_file.is_an_import_map() {
          lsp_log!(
            "Setting import map defined in configuration file: \"{}\"",
            config_file.specifier
          );
          let import_map_url = config_file.specifier.clone();
          Some(import_map_url)
        } else if let Some(import_map_path) = config_file.to_import_map_path() {
          lsp_log!(
            "Setting import map from configuration file: \"{}\"",
            import_map_path
          );
          let specifier = if let Ok(config_file_path) =
            config_file.specifier.to_file_path()
          {
            let import_map_file_path = config_file_path
              .parent()
              .ok_or_else(|| {
                anyhow!("Bad config file specifier: {}", config_file.specifier)
              })?
              .join(&import_map_path);
            ModuleSpecifier::from_file_path(import_map_file_path).unwrap()
          } else {
            deno_core::resolve_import(
              &import_map_path,
              config_file.specifier.as_str(),
            )?
          };
          Some(specifier)
        } else {
          None
        }
      } else {
        None
      },
    )
  }

  pub fn update_debug_flag(&self) {
    let internal_debug = self.config.workspace_settings().internal_debug;
    super::logging::set_lsp_debug_flag(internal_debug)
  }

  async fn update_registries(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("lsp.update_registries");
    self.recreate_http_client_and_dependents().await?;
    let workspace_settings = self.config.workspace_settings();
    for (registry, enabled) in workspace_settings.suggest.imports.hosts.iter() {
      if *enabled {
        lsp_log!("Enabling import suggestions for: {}", registry);
        self.module_registries.enable(registry).await?;
      } else {
        self.module_registries.disable(registry).await?;
      }
    }
    self.performance.measure(mark);
    Ok(())
  }

  async fn update_config_file(&mut self) -> Result<(), AnyError> {
    self.config.clear_config_file();
    self.fmt_options = Default::default();
    self.lint_options = Default::default();
    if let Some(config_file) = self.get_config_file()? {
      let lint_options = config_file
        .to_lint_config()
        .and_then(|maybe_lint_config| {
          LintOptions::resolve(maybe_lint_config, None)
        })
        .map_err(|err| {
          anyhow!("Unable to update lint configuration: {:?}", err)
        })?;
      let fmt_options = config_file
        .to_fmt_config()
        .and_then(|maybe_fmt_config| {
          FmtOptions::resolve(maybe_fmt_config, None)
        })
        .map_err(|err| {
          anyhow!("Unable to update formatter configuration: {:?}", err)
        })?;

      self.config.set_config_file(config_file);
      self.lint_options = lint_options;
      self.fmt_options = fmt_options;
      self.recreate_http_client_and_dependents().await?;
    }

    Ok(())
  }

  /// Updates the package.json. Always ensure this is done after updating
  /// the configuration file as the resolution of this depends on that.
  fn update_package_json(&mut self) -> Result<(), AnyError> {
    self.maybe_package_json = None;
    self.maybe_package_json =
      self.get_package_json(self.config.maybe_config_file())?;
    Ok(())
  }

  async fn update_tsconfig(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("lsp.update_tsconfig");
    let mut tsconfig = TsConfig::new(json!({
      "allowJs": true,
      "esModuleInterop": true,
      "experimentalDecorators": true,
      "isolatedModules": true,
      "jsx": "react",
      "lib": ["deno.ns", "deno.window"],
      "module": "esnext",
      "moduleDetection": "force",
      "noEmit": true,
      "resolveJsonModule": true,
      "strict": true,
      "target": "esnext",
      "useDefineForClassFields": true,
      // TODO(@kitsonk) remove for Deno 1.15
      "useUnknownInCatchVariables": false,
    }));
    let config = &self.config;
    let workspace_settings = config.workspace_settings();
    if workspace_settings.unstable {
      let unstable_libs = json!({
        "lib": ["deno.ns", "deno.window", "deno.unstable"]
      });
      tsconfig.merge(&unstable_libs);
    }
    if let Err(err) = self.merge_user_tsconfig(&mut tsconfig) {
      self.client.show_message(MessageType::WARNING, err);
    }
    let _ok = self.ts_server.configure(self.snapshot(), tsconfig).await?;
    self.performance.measure(mark);
    Ok(())
  }
}

async fn create_npm_resolver(
  deno_dir: &DenoDir,
  http_client: &Arc<HttpClient>,
  maybe_config_file: Option<&ConfigFile>,
  maybe_lockfile: Option<&Arc<Mutex<Lockfile>>>,
  maybe_node_modules_dir_path: Option<PathBuf>,
) -> Arc<dyn CliNpmResolver> {
  let is_byonm = std::env::var("DENO_UNSTABLE_BYONM").as_deref() == Ok("1")
    || maybe_config_file
      .as_ref()
      .map(|c| c.has_unstable("byonm"))
      .unwrap_or(false);
  create_cli_npm_resolver_for_lsp(if is_byonm {
    CliNpmResolverCreateOptions::Byonm(CliNpmResolverByonmCreateOptions {
      fs: Arc::new(deno_fs::RealFs),
      root_node_modules_dir: std::env::current_dir()
        .unwrap()
        .join("node_modules"),
    })
  } else {
    CliNpmResolverCreateOptions::Managed(CliNpmResolverManagedCreateOptions {
      http_client: http_client.clone(),
      snapshot: match maybe_lockfile {
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
      npm_global_cache_dir: deno_dir.npm_folder_path(),
      // Use an "only" cache setting in order to make the
      // user do an explicit "cache" command and prevent
      // the cache from being filled with lots of packages while
      // the user is typing.
      cache_setting: CacheSetting::Only,
      text_only_progress_bar: ProgressBar::new(ProgressBarStyle::TextOnly),
      maybe_node_modules_path: maybe_node_modules_dir_path,
      // do not install while resolving in the lsp—leave that to the cache command
      package_json_installer:
        CliNpmResolverManagedPackageJsonInstallerOption::NoInstall,
      npm_registry_url: crate::args::npm_registry_default_url().to_owned(),
      npm_system_info: NpmSystemInfo::default(),
    })
  })
  .await
}

// lspower::LanguageServer methods. This file's LanguageServer delegates to us.
impl Inner {
  async fn initialize(
    &mut self,
    params: InitializeParams,
  ) -> LspResult<InitializeResult> {
    lsp_log!("Starting Deno language server...");
    let mark = self.performance.mark_with_args("lsp.initialize", &params);

    // exit this process when the parent is lost
    if let Some(parent_pid) = params.process_id {
      parent_process_checker::start(parent_pid)
    }

    // TODO(nayeemrmn): This flag exists to avoid breaking the extension for the
    // 1.37.0 release. Eventually make this always true.
    // See https://github.com/denoland/deno/pull/20111#issuecomment-1705776794.
    let mut enable_builtin_commands = false;
    if let Some(value) = &params.initialization_options {
      if let Some(object) = value.as_object() {
        if let Some(value) = object.get("enableBuiltinCommands") {
          if value.as_bool() == Some(true) {
            enable_builtin_commands = true;
          }
        }
      }
    }

    let capabilities = capabilities::server_capabilities(
      &params.capabilities,
      enable_builtin_commands,
    );

    let version = format!(
      "{} ({}, {})",
      crate::version::deno(),
      env!("PROFILE"),
      env!("TARGET")
    );
    lsp_log!("  version: {}", version);
    if let Ok(path) = std::env::current_exe() {
      lsp_log!("  executable: {}", path.to_string_lossy());
    }

    let server_info = ServerInfo {
      name: "deno-language-server".to_string(),
      version: Some(version),
    };

    if let Some(client_info) = params.client_info {
      lsp_log!(
        "Connected to \"{}\" {}",
        client_info.name,
        client_info.version.unwrap_or_default(),
      );
    }

    {
      if let Some(options) = params.initialization_options {
        self.config.set_workspace_settings(
          WorkspaceSettings::from_initialization_options(options),
          None,
        );
      }
      if let Some(folders) = params.workspace_folders {
        self.config.workspace_folders = folders
          .into_iter()
          .map(|folder| {
            (
              self.url_map.normalize_url(&folder.uri, LspUrlKind::Folder),
              folder,
            )
          })
          .collect();
      }
      // rootUri is deprecated by the LSP spec. If it's specified, merge it into
      // workspace_folders.
      if let Some(root_uri) = params.root_uri {
        if !self
          .config
          .workspace_folders
          .iter()
          .any(|(_, f)| f.uri == root_uri)
        {
          let name = root_uri.path_segments().and_then(|s| s.last());
          let name = name.unwrap_or_default().to_string();
          self.config.workspace_folders.insert(
            0,
            (
              self.url_map.normalize_url(&root_uri, LspUrlKind::Folder),
              WorkspaceFolder {
                uri: root_uri,
                name,
              },
            ),
          );
        }
      }
      self.config.update_capabilities(&params.capabilities);
    }

    self
      .ts_server
      .start(self.config.internal_inspect().to_address());

    self.update_debug_flag();
    // Check to see if we need to change the cache path
    if let Err(err) = self.update_cache().await {
      self.client.show_message(MessageType::WARNING, err);
    }
    if let Err(err) = self.update_config_file().await {
      self.client.show_message(MessageType::WARNING, err);
    }
    if let Err(err) = self.update_package_json() {
      self.client.show_message(MessageType::WARNING, err);
    }
    if let Err(err) = self.update_tsconfig().await {
      self.client.show_message(MessageType::WARNING, err);
    }

    if capabilities.code_action_provider.is_some() {
      let fixable_diagnostics = self
        .ts_server
        .get_supported_code_fixes(self.snapshot())
        .await?;
      self.ts_fixable_diagnostics = fixable_diagnostics;
    }

    // Check to see if we need to setup the import map
    if let Err(err) = self.update_import_map().await {
      self.client.show_message(MessageType::WARNING, err);
    }
    // Check to see if we need to setup any module registries
    if let Err(err) = self.update_registries().await {
      self.client.show_message(MessageType::WARNING, err);
    }

    self.recreate_npm_services_if_necessary().await;
    self.assets.initialize(self.snapshot()).await;

    self.performance.measure(mark);
    Ok(InitializeResult {
      capabilities,
      server_info: Some(server_info),
      offset_encoding: None,
    })
  }

  async fn refresh_documents_config(&mut self) {
    self.documents.update_config(UpdateDocumentConfigOptions {
      enabled_paths: self.config.get_enabled_paths(),
      disabled_paths: self.config.get_disabled_paths(),
      document_preload_limit: self
        .config
        .workspace_settings()
        .document_preload_limit,
      maybe_import_map: self.maybe_import_map.clone(),
      maybe_config_file: self.config.maybe_config_file(),
      maybe_package_json: self.maybe_package_json.as_ref(),
      node_resolver: self.npm.node_resolver.clone(),
      npm_resolver: self.npm.resolver.clone(),
    });

    // refresh the npm specifiers because it might have discovered
    // a @types/node package and now's a good time to do that anyway
    self.refresh_npm_specifiers().await;
  }

  async fn shutdown(&self) -> LspResult<()> {
    Ok(())
  }

  async fn did_open(
    &mut self,
    specifier: &ModuleSpecifier,
    params: DidOpenTextDocumentParams,
  ) -> Document {
    let mark = self.performance.mark_with_args("lsp.did_open", &params);
    let language_id =
      params
        .text_document
        .language_id
        .parse()
        .unwrap_or_else(|err| {
          error!("{}", err);
          LanguageId::Unknown
        });
    if language_id == LanguageId::Unknown {
      lsp_warn!(
        "Unsupported language id \"{}\" received for document \"{}\".",
        params.text_document.language_id,
        params.text_document.uri
      );
    }
    let document = self.documents.open(
      specifier.clone(),
      params.text_document.version,
      params.text_document.language_id.parse().unwrap(),
      params.text_document.text.into(),
    );

    self.performance.measure(mark);
    document
  }

  async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
    let mark = self.performance.mark_with_args("lsp.did_change", &params);
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    match self.documents.change(
      &specifier,
      params.text_document.version,
      params.content_changes,
    ) {
      Ok(document) => {
        if document.is_diagnosable() {
          self.refresh_npm_specifiers().await;
          self
            .diagnostics_server
            .invalidate(&self.documents.dependents(&specifier));
          self.ts_server.increment_project_version();
          self.send_diagnostics_update();
          self.send_testing_update();
        }
      }
      Err(err) => error!("{}", err),
    }
    self.performance.measure(mark);
  }

  async fn refresh_npm_specifiers(&mut self) {
    let package_reqs = self.documents.npm_package_reqs();
    let npm_resolver = self.npm.resolver.clone();
    // spawn to avoid the LSP's Send requirements
    let handle = spawn(async move {
      if let Some(npm_resolver) =
        npm_resolver.as_ref().and_then(|r| r.as_managed())
      {
        npm_resolver.set_package_reqs(&package_reqs).await
      } else {
        Ok(())
      }
    });
    if let Err(err) = handle.await.unwrap() {
      lsp_warn!("Could not set npm package requirements. {:#}", err);
    }
  }

  async fn did_close(&mut self, params: DidCloseTextDocumentParams) {
    let mark = self.performance.mark_with_args("lsp.did_close", &params);
    self.diagnostics_state.clear(&params.text_document.uri);
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents closing, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if self.is_diagnosable(&specifier) {
      self.refresh_npm_specifiers().await;
      let mut specifiers = self.documents.dependents(&specifier);
      specifiers.push(specifier.clone());
      self.diagnostics_server.invalidate(&specifiers);
      self.ts_server.increment_project_version();
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    if let Err(err) = self.documents.close(&specifier) {
      error!("{}", err);
    }
    self.performance.measure(mark);
  }

  async fn did_change_configuration(
    &mut self,
    params: DidChangeConfigurationParams,
  ) {
    if !self.config.client_capabilities.workspace_configuration {
      let config = params.settings.as_object().map(|settings| {
        let deno =
          serde_json::to_value(settings.get(SETTINGS_SECTION)).unwrap();
        let javascript =
          serde_json::to_value(settings.get("javascript")).unwrap();
        let typescript =
          serde_json::to_value(settings.get("typescript")).unwrap();
        WorkspaceSettings::from_raw_settings(deno, javascript, typescript)
      });
      if let Some(settings) = config {
        self.config.set_workspace_settings(settings, None);
      }
    };

    self.update_debug_flag();
    if let Err(err) = self.update_cache().await {
      self.client.show_message(MessageType::WARNING, err);
    }
    if let Err(err) = self.update_registries().await {
      self.client.show_message(MessageType::WARNING, err);
    }
    if let Err(err) = self.update_config_file().await {
      self.client.show_message(MessageType::WARNING, err);
    }
    if let Err(err) = self.update_package_json() {
      self.client.show_message(MessageType::WARNING, err);
    }
    if let Err(err) = self.update_import_map().await {
      self.client.show_message(MessageType::WARNING, err);
    }
    if let Err(err) = self.update_tsconfig().await {
      self.client.show_message(MessageType::WARNING, err);
    }

    self.recreate_npm_services_if_necessary().await;
    self.refresh_documents_config().await;

    self.diagnostics_server.invalidate_all();
    self.ts_server.increment_project_version();
    self.send_diagnostics_update();
    self.send_testing_update();
  }

  async fn did_change_watched_files(
    &mut self,
    params: DidChangeWatchedFilesParams,
  ) {
    fn has_lockfile_content_changed(lockfile: &Lockfile) -> bool {
      match Lockfile::new(lockfile.filename.clone(), false) {
        Ok(new_lockfile) => {
          // only update if the lockfile has changed
          FastInsecureHasher::hash(lockfile)
            != FastInsecureHasher::hash(new_lockfile)
        }
        Err(err) => {
          lsp_warn!("Error loading lockfile: {:#}", err);
          false
        }
      }
    }

    fn has_config_changed(config: &Config, changes: &IndexSet<Url>) -> bool {
      // Check the canonicalized specifier here because file watcher
      // changes will be for the canonicalized path in vscode, but also check the
      // non-canonicalized specifier in order to please the tests and handle
      // a client that might send that instead.
      if config
        .maybe_config_file_canonicalized_specifier()
        .map(|s| changes.contains(s))
        .unwrap_or(false)
      {
        return true;
      }
      match config.maybe_config_file() {
        Some(file) => {
          if changes.contains(&file.specifier) {
            return true;
          }
        }
        None => {
          // check for auto-discovery
          if changes.iter().any(|url| {
            url.path().ends_with("/deno.json")
              || url.path().ends_with("/deno.jsonc")
          }) {
            return true;
          }
        }
      }

      // if the lockfile has changed, reload the config as well
      if let Some(lockfile) = config.maybe_lockfile() {
        let lockfile_matches = config
          .maybe_lockfile_canonicalized_specifier()
          .map(|s| changes.contains(s))
          .or_else(|| {
            ModuleSpecifier::from_file_path(&lockfile.lock().filename)
              .ok()
              .map(|s| changes.contains(&s))
          })
          .unwrap_or(false);
        lockfile_matches && has_lockfile_content_changed(&lockfile.lock())
      } else {
        // check for auto-discovery
        changes.iter().any(|url| url.path().ends_with("/deno.lock"))
      }
    }

    let mark = self
      .performance
      .mark_with_args("lsp.did_change_watched_files", &params);
    let mut touched = false;
    let changes: IndexSet<Url> = params
      .changes
      .iter()
      .map(|f| self.url_map.normalize_url(&f.uri, LspUrlKind::File))
      .collect();

    let mut config_changes = IndexSet::with_capacity(changes.len());

    // if the current deno.json has changed, we need to reload it
    if has_config_changed(&self.config, &changes) {
      // Check the 'current' config specifier from both before and after it's
      // updated. Check canonicalized and uncanonicalized variants for each.
      // If any are included in `changes`, send our custom notification for
      // `deno.json` changes: `deno/didChangeDenoConfigurationNotification`.
      let mut files_to_check = IndexSet::with_capacity(4);
      // Collect previous config specifiers.
      if let Some(url) = self.config.maybe_config_file().map(|c| &c.specifier) {
        files_to_check.insert(url.clone());
      }
      if let Some(url) = self.config.maybe_config_file_canonicalized_specifier()
      {
        files_to_check.insert(url.clone());
      }
      // Update config.
      if let Err(err) = self.update_config_file().await {
        self.client.show_message(MessageType::WARNING, err);
      }
      // Collect new config specifiers.
      if let Some(url) = self.config.maybe_config_file().map(|c| &c.specifier) {
        files_to_check.insert(url.clone());
      }
      if let Some(url) = self.config.maybe_config_file_canonicalized_specifier()
      {
        files_to_check.insert(url.clone());
      }
      config_changes.extend(
        params
          .changes
          .iter()
          .filter(|e| files_to_check.contains(&e.uri))
          .map(|e| lsp_custom::DenoConfigurationChangeEvent {
            file_event: e.clone(),
            configuration_type: lsp_custom::DenoConfigurationType::DenoJson,
          }),
      );
      if let Err(err) = self.update_tsconfig().await {
        self.client.show_message(MessageType::WARNING, err);
      }
      touched = true;
    }

    let has_package_json_changed = changes
      .iter()
      .any(|e| e.as_str().ends_with("/package.json"));

    if has_package_json_changed {
      let mut files_to_check = IndexSet::with_capacity(2);
      if let Some(package_json) = &self.maybe_package_json {
        files_to_check.insert(package_json.specifier());
      }
      if let Err(err) = self.update_package_json() {
        self.client.show_message(MessageType::WARNING, err);
      }
      if let Some(package_json) = &self.maybe_package_json {
        files_to_check.insert(package_json.specifier());
      }
      config_changes.extend(
        params
          .changes
          .iter()
          .filter(|e| files_to_check.contains(&e.uri))
          .map(|e| lsp_custom::DenoConfigurationChangeEvent {
            file_event: e.clone(),
            configuration_type: lsp_custom::DenoConfigurationType::PackageJson,
          }),
      );
      touched = true;
    }

    if !config_changes.is_empty() {
      self.client.send_did_change_deno_configuration_notification(
        lsp_custom::DidChangeDenoConfigurationNotificationParams {
          changes: config_changes.into_iter().collect(),
        },
      );
    }

    // if the current import map, or config file has changed, we need to
    // reload the import map
    let import_map_changed = self
      .maybe_import_map_uri
      .as_ref()
      .map(|uri| changes.contains(uri))
      .unwrap_or(false);
    if touched || import_map_changed {
      if let Err(err) = self.update_import_map().await {
        self.client.show_message(MessageType::WARNING, err);
      }
      touched = true;
    }

    if touched {
      self.recreate_npm_services_if_necessary().await;
      self.refresh_documents_config().await;
      self.diagnostics_server.invalidate_all();
      self.ts_server.restart(self.snapshot()).await;
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    self.performance.measure(mark);
  }

  fn did_change_workspace_folders(
    &mut self,
    params: DidChangeWorkspaceFoldersParams,
  ) {
    let mut workspace_folders = params
      .event
      .added
      .into_iter()
      .map(|folder| {
        (
          self.url_map.normalize_url(&folder.uri, LspUrlKind::Folder),
          folder,
        )
      })
      .collect::<Vec<(ModuleSpecifier, WorkspaceFolder)>>();
    for (specifier, folder) in &self.config.workspace_folders {
      if !params.event.removed.is_empty()
        && params.event.removed.iter().any(|f| f.uri == folder.uri)
      {
        continue;
      }
      workspace_folders.push((specifier.clone(), folder.clone()));
    }

    self.config.workspace_folders = workspace_folders;
  }

  async fn document_symbol(
    &self,
    params: DocumentSymbolParams,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.document_symbol", &params);
    let asset_or_document = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_document.line_index();

    let navigation_tree =
      self.get_navigation_tree(&specifier).await.map_err(|err| {
        error!(
          "Error getting document symbols for \"{}\": {}",
          specifier, err
        );
        LspError::internal_error()
      })?;

    let response = if let Some(child_items) = &navigation_tree.child_items {
      let mut document_symbols = Vec::<DocumentSymbol>::new();
      for item in child_items {
        item
          .collect_document_symbols(line_index.clone(), &mut document_symbols);
      }
      Some(DocumentSymbolResponse::Nested(document_symbols))
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn formatting(
    &self,
    params: DocumentFormattingParams,
  ) -> LspResult<Option<Vec<TextEdit>>> {
    let mut specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    // skip formatting any files ignored by the config file
    if !self.fmt_options.files.matches_specifier(&specifier) {
      return Ok(None);
    }
    let document = match self.documents.get(&specifier) {
      Some(doc) if doc.is_open() => doc,
      _ => return Ok(None),
    };
    // Detect vendored paths. Vendor file URLs will normalize to their remote
    // counterparts, but for formatting we want to favour the file URL.
    // TODO(nayeemrmn): Implement `Document::file_resource_path()` or similar.
    if specifier.scheme() != "file"
      && params.text_document.uri.scheme() == "file"
    {
      specifier = params.text_document.uri.clone();
    }
    let file_path = specifier_to_file_path(&specifier).map_err(|err| {
      error!("{}", err);
      LspError::invalid_request()
    })?;
    let mark = self.performance.mark_with_args("lsp.formatting", &params);

    // spawn a blocking task to allow doing other work while this is occurring
    let text_edits = deno_core::unsync::spawn_blocking({
      let fmt_options = self.fmt_options.options.clone();
      let document = document.clone();
      move || {
        let format_result = match document.maybe_parsed_source() {
          Some(Ok(parsed_source)) => {
            format_parsed_source(&parsed_source, &fmt_options)
          }
          Some(Err(err)) => Err(anyhow!("{}", err)),
          None => {
            // the file path is only used to determine what formatter should
            // be used to format the file, so give the filepath an extension
            // that matches what the user selected as the language
            let file_path = document
              .maybe_language_id()
              .and_then(|id| id.as_extension())
              .map(|ext| file_path.with_extension(ext))
              .unwrap_or(file_path);
            // it's not a js/ts file, so attempt to format its contents
            format_file(&file_path, &document.content(), &fmt_options)
          }
        };
        match format_result {
          Ok(Some(new_text)) => Some(text::get_edits(
            &document.content(),
            &new_text,
            document.line_index().as_ref(),
          )),
          Ok(None) => Some(Vec::new()),
          Err(err) => {
            lsp_warn!("Format error: {:#}", err);
            None
          }
        }
      }
    })
    .await
    .unwrap();

    self.performance.measure(mark);
    if let Some(text_edits) = text_edits {
      if text_edits.is_empty() {
        Ok(None)
      } else {
        Ok(Some(text_edits))
      }
    } else {
      self.client.show_message(MessageType::WARNING, format!("Unable to format \"{specifier}\". Likely due to unrecoverable syntax errors in the file."));
      Ok(None)
    }
  }

  async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position_params.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.hover", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let hover = if let Some((_, dep, range)) = asset_or_doc
      .get_maybe_dependency(&params.text_document_position_params.position)
    {
      let dep_maybe_types_dependency = dep
        .get_code()
        .and_then(|s| self.documents.get(s))
        .map(|d| d.maybe_types_dependency());
      let value = match (dep.maybe_code.is_none(), dep.maybe_type.is_none(), &dep_maybe_types_dependency) {
        (false, false, None) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          to_hover_text(&dep.maybe_code),
          to_hover_text(&dep.maybe_type)
        ),
        (false, false, Some(types_dep)) if !types_dep.is_none() => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n**Types**: {}\n**Import Types**: {}\n",
          to_hover_text(&dep.maybe_code),
          to_hover_text(&dep.maybe_type),
          to_hover_text(types_dep)
        ),
        (false, false, Some(_)) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          to_hover_text(&dep.maybe_code),
          to_hover_text(&dep.maybe_type)
        ),
        (false, true, Some(types_dep)) if !types_dep.is_none() => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          to_hover_text(&dep.maybe_code),
          to_hover_text(types_dep)
        ),
        (false, true, _) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n",
          to_hover_text(&dep.maybe_code)
        ),
        (true, false, _) => format!(
          "**Resolved Dependency**\n\n**Types**: {}\n",
          to_hover_text(&dep.maybe_type)
        ),
        (true, true, _) => unreachable!("{}", json!(params)),
      };
      let value =
        if let Some(docs) = self.module_registries.get_hover(&dep).await {
          format!("{value}\n\n---\n\n{docs}")
        } else {
          value
        };
      Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
          kind: MarkupKind::Markdown,
          value,
        }),
        range: Some(to_lsp_range(&range)),
      })
    } else {
      let line_index = asset_or_doc.line_index();
      let position =
        line_index.offset_tsc(params.text_document_position_params.position)?;
      let maybe_quick_info = self
        .ts_server
        .get_quick_info(self.snapshot(), specifier.clone(), position)
        .await?;
      maybe_quick_info.map(|qi| qi.to_hover(line_index, self))
    };
    self.performance.measure(mark);
    Ok(hover)
  }

  async fn code_action(
    &self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.code_action", &params);
    let mut all_actions = CodeActionResponse::new();
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    // QuickFix
    let fixable_diagnostics: Vec<&Diagnostic> = params
      .context
      .diagnostics
      .iter()
      .filter(|d| match &d.source {
        Some(source) => match source.as_str() {
          "deno-ts" => match &d.code {
            Some(NumberOrString::String(code)) => {
              self.ts_fixable_diagnostics.contains(code)
            }
            Some(NumberOrString::Number(code)) => {
              self.ts_fixable_diagnostics.contains(&code.to_string())
            }
            _ => false,
          },
          "deno-lint" => d.code.is_some(),
          "deno" => diagnostics::DenoDiagnostic::is_fixable(d),
          _ => false,
        },
        None => false,
      })
      .collect();
    if !fixable_diagnostics.is_empty() {
      let mut code_actions = CodeActionCollection::default();
      let file_diagnostics = self
        .diagnostics_server
        .get_ts_diagnostics(&specifier, asset_or_doc.document_lsp_version());
      let mut includes_no_cache = false;
      for diagnostic in &fixable_diagnostics {
        match diagnostic.source.as_deref() {
          Some("deno-ts") => {
            let code = match diagnostic.code.as_ref().unwrap() {
              NumberOrString::String(code) => code.to_string(),
              NumberOrString::Number(code) => code.to_string(),
            };
            let codes = vec![code];
            let actions = self
              .ts_server
              .get_code_fixes(
                self.snapshot(),
                specifier.clone(),
                line_index.offset_tsc(diagnostic.range.start)?
                  ..line_index.offset_tsc(diagnostic.range.end)?,
                codes,
                (&self.fmt_options.options).into(),
                tsc::UserPreferences::from_config_for_specifier(
                  &self.config,
                  &self.fmt_options.options,
                  &specifier,
                ),
              )
              .await;
            for action in actions {
              code_actions
                .add_ts_fix_action(&specifier, &action, diagnostic, self)
                .map_err(|err| {
                  error!("Unable to convert fix: {}", err);
                  LspError::internal_error()
                })?;
              if code_actions.is_fix_all_action(
                &action,
                diagnostic,
                &file_diagnostics,
              ) {
                code_actions
                  .add_ts_fix_all_action(&action, &specifier, diagnostic);
              }
            }
          }
          Some("deno") => {
            if diagnostic.code
              == Some(NumberOrString::String("no-cache".to_string()))
              || diagnostic.code
                == Some(NumberOrString::String("no-cache-npm".to_string()))
            {
              includes_no_cache = true;
            }
            code_actions
              .add_deno_fix_action(&specifier, diagnostic)
              .map_err(|err| {
                error!("{}", err);
                LspError::internal_error()
              })?
          }
          Some("deno-lint") => code_actions
            .add_deno_lint_ignore_action(
              &specifier,
              diagnostic,
              asset_or_doc.document().map(|d| d.text_info()),
              asset_or_doc.maybe_parsed_source().and_then(|r| r.ok()),
            )
            .map_err(|err| {
              error!("Unable to fix lint error: {}", err);
              LspError::internal_error()
            })?,
          _ => (),
        }
      }
      if includes_no_cache {
        let no_cache_diagnostics =
          self.diagnostics_state.no_cache_diagnostics(&specifier);
        let uncached_deps = no_cache_diagnostics
          .iter()
          .filter_map(|d| {
            let data = serde_json::from_value::<DiagnosticDataSpecifier>(
              d.data.clone().into(),
            )
            .ok()?;
            Some(data.specifier)
          })
          .collect::<HashSet<_>>();
        if uncached_deps.len() > 1 {
          code_actions
            .add_cache_all_action(&specifier, no_cache_diagnostics.to_owned());
        }
      }
      code_actions.set_preferred_fixes();
      all_actions.extend(code_actions.get_response());
    }

    // Refactor
    let only = params
      .context
      .only
      .as_ref()
      .and_then(|values| values.first().map(|v| v.as_str().to_owned()))
      .unwrap_or_default();
    let refactor_infos = self
      .ts_server
      .get_applicable_refactors(
        self.snapshot(),
        specifier.clone(),
        line_index.offset_tsc(params.range.start)?
          ..line_index.offset_tsc(params.range.end)?,
        Some(tsc::UserPreferences::from_config_for_specifier(
          &self.config,
          &self.fmt_options.options,
          &specifier,
        )),
        only,
      )
      .await?;
    let mut refactor_actions = Vec::<CodeAction>::new();
    for refactor_info in refactor_infos.iter() {
      refactor_actions
        .extend(refactor_info.to_code_actions(&specifier, &params.range));
    }
    all_actions.extend(
      refactor::prune_invalid_actions(refactor_actions, 5)
        .into_iter()
        .map(CodeActionOrCommand::CodeAction),
    );

    let code_action_disabled_support =
      self.config.client_capabilities.code_action_disabled_support;
    let actions: Vec<CodeActionOrCommand> = all_actions.into_iter().filter(|ca| {
      code_action_disabled_support
        || matches!(ca, CodeActionOrCommand::CodeAction(ca) if ca.disabled.is_none())
    }).collect();
    let response = if actions.is_empty() {
      None
    } else {
      Some(actions)
    };

    self.performance.measure(mark);
    Ok(response)
  }

  async fn code_action_resolve(
    &self,
    params: CodeAction,
  ) -> LspResult<CodeAction> {
    if params.kind.is_none() || params.data.is_none() {
      return Ok(params);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.code_action_resolve", &params);
    let kind = params.kind.clone().unwrap();
    let data = params.data.clone().unwrap();

    let result = if kind.as_str().starts_with(CodeActionKind::QUICKFIX.as_str())
    {
      let code_action_data: CodeActionData =
        from_value(data).map_err(|err| {
          error!("Unable to decode code action data: {}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let combined_code_actions = self
        .ts_server
        .get_combined_code_fix(
          self.snapshot(),
          &code_action_data,
          (&self.fmt_options.options).into(),
          tsc::UserPreferences::from_config_for_specifier(
            &self.config,
            &self.fmt_options.options,
            &code_action_data.specifier,
          ),
        )
        .await?;
      if combined_code_actions.commands.is_some() {
        error!("Deno does not support code actions with commands.");
        return Err(LspError::invalid_request());
      }

      let changes = if code_action_data.fix_id == "fixMissingImport" {
        fix_ts_import_changes(
          &code_action_data.specifier,
          &combined_code_actions.changes,
          &self.get_ts_response_import_mapper(),
        )
        .map_err(|err| {
          error!("Unable to remap changes: {}", err);
          LspError::internal_error()
        })?
      } else {
        combined_code_actions.changes
      };
      let mut code_action = params;
      code_action.edit = ts_changes_to_edit(&changes, self).map_err(|err| {
        error!("Unable to convert changes to edits: {}", err);
        LspError::internal_error()
      })?;
      code_action
    } else if kind.as_str().starts_with(CodeActionKind::REFACTOR.as_str()) {
      let mut code_action = params;
      let action_data: refactor::RefactorCodeActionData = from_value(data)
        .map_err(|err| {
          error!("Unable to decode code action data: {}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let asset_or_doc = self.get_asset_or_document(&action_data.specifier)?;
      let line_index = asset_or_doc.line_index();
      let refactor_edit_info = self
        .ts_server
        .get_edits_for_refactor(
          self.snapshot(),
          action_data.specifier.clone(),
          (&self.fmt_options.options).into(),
          line_index.offset_tsc(action_data.range.start)?
            ..line_index.offset_tsc(action_data.range.end)?,
          action_data.refactor_name,
          action_data.action_name,
          Some(tsc::UserPreferences::from_config_for_specifier(
            &self.config,
            &self.fmt_options.options,
            &action_data.specifier,
          )),
        )
        .await?;
      code_action.edit = refactor_edit_info.to_workspace_edit(self).await?;
      code_action
    } else {
      // The code action doesn't need to be resolved
      params
    };

    self.performance.measure(mark);
    Ok(result)
  }

  pub fn get_ts_response_import_mapper(&self) -> TsResponseImportMapper {
    TsResponseImportMapper::new(
      &self.documents,
      self.maybe_import_map.as_deref(),
      self.npm.node_resolver.as_deref(),
      self.npm.resolver.as_deref(),
    )
  }

  async fn code_lens(
    &self,
    params: CodeLensParams,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.code_lens", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let settings = self.config.workspace_settings_for_specifier(&specifier);
    let mut code_lenses = Vec::new();
    if settings.code_lens.test
      && self.config.specifier_enabled_for_test(&specifier)
    {
      if let Some(Ok(parsed_source)) = asset_or_doc.maybe_parsed_source() {
        code_lenses.extend(
          code_lens::collect_test(&specifier, parsed_source).map_err(
            |err| {
              error!(
                "Error getting test code lenses for \"{}\": {}",
                &specifier, err
              );
              LspError::internal_error()
            },
          )?,
        );
      }
    }
    if settings.code_lens.implementations || settings.code_lens.references {
      let navigation_tree =
        self.get_navigation_tree(&specifier).await.map_err(|err| {
          error!("Error getting code lenses for \"{}\": {}", specifier, err);
          LspError::internal_error()
        })?;
      let line_index = asset_or_doc.line_index();
      code_lenses.extend(
        code_lens::collect_tsc(
          &specifier,
          &settings.code_lens,
          line_index,
          &navigation_tree,
        )
        .await
        .map_err(|err| {
          error!(
            "Error getting ts code lenses for \"{}\": {}",
            &specifier, err
          );
          LspError::internal_error()
        })?,
      );
    }
    self.performance.measure(mark);

    if code_lenses.is_empty() {
      return Ok(None);
    }
    Ok(Some(code_lenses))
  }

  async fn code_lens_resolve(
    &self,
    code_lens: CodeLens,
  ) -> LspResult<CodeLens> {
    let mark = self
      .performance
      .mark_with_args("lsp.code_lens_resolve", &code_lens);
    let result = if code_lens.data.is_some() {
      code_lens::resolve_code_lens(code_lens, self)
        .await
        .map_err(|err| {
          error!("Error resolving code lens: {}", err);
          LspError::internal_error()
        })
    } else {
      Err(LspError::invalid_params(
        "Code lens is missing the \"data\" property.",
      ))
    };
    self.performance.measure(mark);
    result
  }

  async fn document_highlight(
    &self,
    params: DocumentHighlightParams,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position_params.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.document_highlight", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let files_to_search = vec![specifier.clone()];
    let maybe_document_highlights = self
      .ts_server
      .get_document_highlights(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.text_document_position_params.position)?,
        files_to_search,
      )
      .await?;

    if let Some(document_highlights) = maybe_document_highlights {
      let result = document_highlights
        .into_iter()
        .flat_map(|dh| dh.to_highlight(line_index.clone()))
        .collect();
      self.performance.measure(mark);
      Ok(Some(result))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn references(
    &self,
    params: ReferenceParams,
  ) -> LspResult<Option<Vec<Location>>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.references", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let maybe_referenced_symbols = self
      .ts_server
      .find_references(
        self.snapshot(),
        specifier.clone(),
        line_index.offset_tsc(params.text_document_position.position)?,
      )
      .await?;

    if let Some(symbols) = maybe_referenced_symbols {
      let mut results = Vec::new();
      for reference in symbols.iter().flat_map(|s| &s.references) {
        if !params.context.include_declaration && reference.is_definition {
          continue;
        }
        let reference_specifier =
          resolve_url(&reference.entry.document_span.file_name).unwrap();
        let reference_line_index = if reference_specifier == specifier {
          line_index.clone()
        } else {
          let asset_or_doc =
            self.get_asset_or_document(&reference_specifier)?;
          asset_or_doc.line_index()
        };
        results.push(
          reference
            .entry
            .to_location(reference_line_index, &self.url_map),
        );
      }

      self.performance.measure(mark);
      Ok(Some(results))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
  ) -> LspResult<Option<GotoDefinitionResponse>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position_params.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.goto_definition", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let maybe_definition = self
      .ts_server
      .get_definition(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.text_document_position_params.position)?,
      )
      .await?;

    if let Some(definition) = maybe_definition {
      let results = definition.to_definition(line_index, self).await;
      self.performance.measure(mark);
      Ok(results)
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn goto_type_definition(
    &self,
    params: GotoTypeDefinitionParams,
  ) -> LspResult<Option<GotoTypeDefinitionResponse>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position_params.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.goto_definition", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let maybe_definition_info = self
      .ts_server
      .get_type_definition(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.text_document_position_params.position)?,
      )
      .await?;

    let response = if let Some(definition_info) = maybe_definition_info {
      let mut location_links = Vec::new();
      for info in definition_info {
        if let Some(link) = info.document_span.to_link(line_index.clone(), self)
        {
          location_links.push(link);
        }
      }
      Some(GotoTypeDefinitionResponse::Link(location_links))
    } else {
      None
    };

    self.performance.measure(mark);
    Ok(response)
  }

  async fn completion(
    &self,
    params: CompletionParams,
  ) -> LspResult<Option<CompletionResponse>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position.text_document.uri,
      LspUrlKind::File,
    );
    let language_settings =
      self.config.language_settings_for_specifier(&specifier);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
      || !language_settings.map(|s| s.suggest.enabled).unwrap_or(true)
    {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.completion", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    // Import specifiers are something wholly internal to Deno, so for
    // completions, we will use internal logic and if there are completions
    // for imports, we will return those and not send a message into tsc, where
    // other completions come from.
    let mut response = None;
    if language_settings
      .map(|s| s.suggest.include_completions_for_import_statements)
      .unwrap_or(true)
    {
      response = completions::get_import_completions(
        &specifier,
        &params.text_document_position.position,
        &self.config.snapshot(),
        &self.client,
        &self.module_registries,
        &self.npm.search_api,
        &self.documents,
        self.maybe_import_map.clone(),
      )
      .await;
    }
    if response.is_none() {
      let line_index = asset_or_doc.line_index();
      let (trigger_character, trigger_kind) =
        if let Some(context) = &params.context {
          (
            context.trigger_character.clone(),
            Some(context.trigger_kind.into()),
          )
        } else {
          (None, None)
        };
      let position =
        line_index.offset_tsc(params.text_document_position.position)?;
      let maybe_completion_info = self
        .ts_server
        .get_completions(
          self.snapshot(),
          specifier.clone(),
          position,
          tsc::GetCompletionsAtPositionOptions {
            user_preferences: tsc::UserPreferences::from_config_for_specifier(
              &self.config,
              &self.fmt_options.options,
              &specifier,
            ),
            trigger_character,
            trigger_kind,
          },
          (&self.fmt_options.options).into(),
        )
        .await;

      if let Some(completions) = maybe_completion_info {
        response = Some(
          completions.as_completion_response(
            line_index,
            &self
              .config
              .language_settings_for_specifier(&specifier)
              .cloned()
              .unwrap_or_default()
              .suggest,
            &specifier,
            position,
            self,
          ),
        );
      }
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn completion_resolve(
    &self,
    params: CompletionItem,
  ) -> LspResult<CompletionItem> {
    let mark = self
      .performance
      .mark_with_args("lsp.completion_resolve", &params);
    let completion_item = if let Some(data) = &params.data {
      let data: completions::CompletionItemData =
        serde_json::from_value(data.clone()).map_err(|err| {
          error!("{}", err);
          LspError::invalid_params(
            "Could not decode data field of completion item.",
          )
        })?;
      if let Some(data) = &data.tsc {
        let specifier = &data.specifier;
        let result = self
          .ts_server
          .get_completion_details(
            self.snapshot(),
            GetCompletionDetailsArgs {
              format_code_settings: Some((&self.fmt_options.options).into()),
              preferences: Some(
                tsc::UserPreferences::from_config_for_specifier(
                  &self.config,
                  &self.fmt_options.options,
                  specifier,
                ),
              ),
              ..data.into()
            },
          )
          .await;
        match result {
          Ok(maybe_completion_info) => {
            if let Some(completion_info) = maybe_completion_info {
              completion_info
                .as_completion_item(&params, data, specifier, self)
                .map_err(|err| {
                  error!(
                    "Failed to serialize virtual_text_document response: {}",
                    err
                  );
                  LspError::internal_error()
                })?
            } else {
              error!(
                "Received an undefined response from tsc for completion details."
              );
              params
            }
          }
          Err(err) => {
            error!("Unable to get completion info from TypeScript: {}", err);
            return Ok(params);
          }
        }
      } else if let Some(url) = data.documentation {
        CompletionItem {
          documentation: self.module_registries.get_documentation(&url).await,
          data: None,
          ..params
        }
      } else {
        params
      }
    } else {
      params
    };
    self.performance.measure(mark);
    Ok(completion_item)
  }

  async fn goto_implementation(
    &self,
    params: GotoImplementationParams,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position_params.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.goto_implementation", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let maybe_implementations = self
      .ts_server
      .get_implementations(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.text_document_position_params.position)?,
      )
      .await?;

    let result = if let Some(implementations) = maybe_implementations {
      let mut links = Vec::new();
      for implementation in implementations {
        if let Some(link) = implementation.to_link(line_index.clone(), self) {
          links.push(link)
        }
      }
      Some(GotoDefinitionResponse::Link(links))
    } else {
      None
    };

    self.performance.measure(mark);
    Ok(result)
  }

  async fn folding_range(
    &self,
    params: FoldingRangeParams,
  ) -> LspResult<Option<Vec<FoldingRange>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.folding_range", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;

    let outlining_spans = self
      .ts_server
      .get_outlining_spans(self.snapshot(), specifier)
      .await?;

    let response = if !outlining_spans.is_empty() {
      Some(
        outlining_spans
          .iter()
          .map(|span| {
            span.to_folding_range(
              asset_or_doc.line_index(),
              asset_or_doc.text().as_bytes(),
              self.config.client_capabilities.line_folding_only,
            )
          })
          .collect::<Vec<FoldingRange>>(),
      )
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.item.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.incoming_calls", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let incoming_calls: Vec<tsc::CallHierarchyIncomingCall> = self
      .ts_server
      .provide_call_hierarchy_incoming_calls(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.item.selection_range.start)?,
      )
      .await?;

    let maybe_root_path_owned = self
      .config
      .root_uri()
      .and_then(|uri| specifier_to_file_path(uri).ok());
    let mut resolved_items = Vec::<CallHierarchyIncomingCall>::new();
    for item in incoming_calls.iter() {
      if let Some(resolved) = item.try_resolve_call_hierarchy_incoming_call(
        self,
        maybe_root_path_owned.as_deref(),
      ) {
        resolved_items.push(resolved);
      }
    }
    self.performance.measure(mark);
    Ok(Some(resolved_items))
  }

  async fn outgoing_calls(
    &self,
    params: CallHierarchyOutgoingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.item.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.outgoing_calls", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let outgoing_calls: Vec<tsc::CallHierarchyOutgoingCall> = self
      .ts_server
      .provide_call_hierarchy_outgoing_calls(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.item.selection_range.start)?,
      )
      .await?;

    let maybe_root_path_owned = self
      .config
      .root_uri()
      .and_then(|uri| specifier_to_file_path(uri).ok());
    let mut resolved_items = Vec::<CallHierarchyOutgoingCall>::new();
    for item in outgoing_calls.iter() {
      if let Some(resolved) = item.try_resolve_call_hierarchy_outgoing_call(
        line_index.clone(),
        self,
        maybe_root_path_owned.as_deref(),
      ) {
        resolved_items.push(resolved);
      }
    }
    self.performance.measure(mark);
    Ok(Some(resolved_items))
  }

  async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams,
  ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position_params.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.prepare_call_hierarchy", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let maybe_one_or_many = self
      .ts_server
      .prepare_call_hierarchy(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.text_document_position_params.position)?,
      )
      .await?;

    let response = if let Some(one_or_many) = maybe_one_or_many {
      let maybe_root_path_owned = self
        .config
        .root_uri()
        .and_then(|uri| specifier_to_file_path(uri).ok());
      let mut resolved_items = Vec::<CallHierarchyItem>::new();
      match one_or_many {
        tsc::OneOrMany::One(item) => {
          if let Some(resolved) = item.try_resolve_call_hierarchy_item(
            self,
            maybe_root_path_owned.as_deref(),
          ) {
            resolved_items.push(resolved)
          }
        }
        tsc::OneOrMany::Many(items) => {
          for item in items.iter() {
            if let Some(resolved) = item.try_resolve_call_hierarchy_item(
              self,
              maybe_root_path_owned.as_deref(),
            ) {
              resolved_items.push(resolved);
            }
          }
        }
      }
      Some(resolved_items)
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn rename(
    &self,
    params: RenameParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.rename", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let maybe_locations = self
      .ts_server
      .find_rename_locations(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.text_document_position.position)?,
      )
      .await?;

    if let Some(locations) = maybe_locations {
      let rename_locations = tsc::RenameLocations { locations };
      let workspace_edits = rename_locations
        .into_workspace_edit(&params.new_name, self)
        .await
        .map_err(|err| {
          error!("Failed to get workspace edits: {}", err);
          LspError::internal_error()
        })?;
      self.performance.measure(mark);
      Ok(Some(workspace_edits))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn selection_range(
    &self,
    params: SelectionRangeParams,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.selection_range", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let mut selection_ranges = Vec::<SelectionRange>::new();
    for position in params.positions {
      let selection_range: tsc::SelectionRange = self
        .ts_server
        .get_smart_selection_range(
          self.snapshot(),
          specifier.clone(),
          line_index.offset_tsc(position)?,
        )
        .await?;

      selection_ranges
        .push(selection_range.to_selection_range(line_index.clone()));
    }
    self.performance.measure(mark);
    Ok(Some(selection_ranges))
  }

  async fn semantic_tokens_full(
    &self,
    params: SemanticTokensParams,
  ) -> LspResult<Option<SemanticTokensResult>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier) {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.semantic_tokens_full", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let semantic_classification = self
      .ts_server
      .get_encoded_semantic_classifications(
        self.snapshot(),
        specifier,
        0..line_index.text_content_length_utf16().into(),
      )
      .await?;

    let semantic_tokens =
      semantic_classification.to_semantic_tokens(&asset_or_doc, line_index)?;
    let response = if !semantic_tokens.data.is_empty() {
      Some(SemanticTokensResult::Tokens(semantic_tokens))
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn semantic_tokens_range(
    &self,
    params: SemanticTokensRangeParams,
  ) -> LspResult<Option<SemanticTokensRangeResult>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier) {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.semantic_tokens_range", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let semantic_classification = self
      .ts_server
      .get_encoded_semantic_classifications(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.range.start)?
          ..line_index.offset_tsc(params.range.end)?,
      )
      .await?;

    let semantic_tokens =
      semantic_classification.to_semantic_tokens(&asset_or_doc, line_index)?;
    let response = if !semantic_tokens.data.is_empty() {
      Some(SemanticTokensRangeResult::Tokens(semantic_tokens))
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn signature_help(
    &self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    let specifier = self.url_map.normalize_url(
      &params.text_document_position_params.text_document.uri,
      LspUrlKind::File,
    );
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.signature_help", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let options = if let Some(context) = params.context {
      tsc::SignatureHelpItemsOptions {
        trigger_reason: Some(tsc::SignatureHelpTriggerReason {
          kind: context.trigger_kind.into(),
          trigger_character: context.trigger_character,
        }),
      }
    } else {
      tsc::SignatureHelpItemsOptions {
        trigger_reason: None,
      }
    };
    let maybe_signature_help_items: Option<tsc::SignatureHelpItems> = self
      .ts_server
      .get_signature_help_items(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.text_document_position_params.position)?,
        options,
      )
      .await?;

    if let Some(signature_help_items) = maybe_signature_help_items {
      let signature_help = signature_help_items.into_signature_help(self);
      self.performance.measure(mark);
      Ok(Some(signature_help))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn will_rename_files(
    &self,
    params: RenameFilesParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    let mut changes = vec![];
    for rename in params.files {
      let old_specifier = self.url_map.normalize_url(
        &resolve_url(&rename.old_uri).unwrap(),
        LspUrlKind::File,
      );
      let options = self
        .config
        .language_settings_for_specifier(&old_specifier)
        .map(|s| s.update_imports_on_file_move.clone())
        .unwrap_or_default();
      // Note that `Always` and `Prompt` are treated the same in the server, the
      // client will worry about that after receiving the edits.
      if options.enabled == UpdateImportsOnFileMoveEnabled::Never {
        continue;
      }
      changes.extend(
        self
          .ts_server
          .get_edits_for_file_rename(
            self.snapshot(),
            old_specifier,
            self.url_map.normalize_url(
              &resolve_url(&rename.new_uri).unwrap(),
              LspUrlKind::File,
            ),
            (&self.fmt_options.options).into(),
            tsc::UserPreferences {
              allow_text_changes_in_new_files: Some(true),
              ..Default::default()
            },
          )
          .await?,
      );
    }
    file_text_changes_to_workspace_edit(&changes, self)
  }

  async fn symbol(
    &self,
    params: WorkspaceSymbolParams,
  ) -> LspResult<Option<Vec<SymbolInformation>>> {
    let mark = self.performance.mark_with_args("lsp.symbol", &params);

    let navigate_to_items = self
      .ts_server
      .get_navigate_to_items(
        self.snapshot(),
        tsc::GetNavigateToItemsArgs {
          search: params.query,
          // this matches vscode's hard coded result count
          max_result_count: Some(256),
          file: None,
        },
      )
      .await?;

    let maybe_symbol_information = if navigate_to_items.is_empty() {
      None
    } else {
      let mut symbol_information = Vec::new();
      for item in navigate_to_items {
        if let Some(info) = item.to_symbol_information(self) {
          symbol_information.push(info);
        }
      }
      Some(symbol_information)
    };

    self.performance.measure(mark);
    Ok(maybe_symbol_information)
  }

  fn send_diagnostics_update(&self) {
    let snapshot = DiagnosticServerUpdateMessage {
      snapshot: self.snapshot(),
      config: self.config.snapshot(),
      lint_options: self.lint_options.clone(),
      url_map: self.url_map.clone(),
    };
    if let Err(err) = self.diagnostics_server.update(snapshot) {
      error!("Cannot update diagnostics: {}", err);
    }
  }

  /// Send a message to the testing server to look for any changes in tests and
  /// update the client.
  fn send_testing_update(&self) {
    if let Some(testing_server) = &self.maybe_testing_server {
      if let Err(err) = testing_server.update(self.snapshot()) {
        error!("Cannot update testing server: {}", err);
      }
    }
  }
}

#[tower_lsp::async_trait]
impl tower_lsp::LanguageServer for LanguageServer {
  async fn execute_command(
    &self,
    params: ExecuteCommandParams,
  ) -> LspResult<Option<Value>> {
    if params.command == "deno.cache" {
      let mut arguments = params.arguments.into_iter();
      let uris = serde_json::to_value(arguments.next()).unwrap();
      let uris: Vec<Url> = serde_json::from_value(uris)
        .map_err(|err| LspError::invalid_params(err.to_string()))?;
      let referrer = serde_json::to_value(arguments.next()).unwrap();
      let referrer: Url = serde_json::from_value(referrer)
        .map_err(|err| LspError::invalid_params(err.to_string()))?;
      self
        .cache_request(Some(
          serde_json::to_value(lsp_custom::CacheParams {
            referrer: TextDocumentIdentifier { uri: referrer },
            uris: uris
              .into_iter()
              .map(|uri| TextDocumentIdentifier { uri })
              .collect(),
          })
          .expect("well formed json"),
        ))
        .await
    } else if params.command == "deno.reloadImportRegistries" {
      self.0.write().await.reload_import_registries().await
    } else {
      Ok(None)
    }
  }

  async fn initialize(
    &self,
    params: InitializeParams,
  ) -> LspResult<InitializeResult> {
    let mut language_server = self.0.write().await;
    language_server.diagnostics_server.start();
    language_server.initialize(params).await
  }

  async fn initialized(&self, _: InitializedParams) {
    let mut registrations = Vec::with_capacity(2);
    let (client, http_client) = {
      let mut ls = self.0.write().await;
      if ls
        .config
        .client_capabilities
        .workspace_did_change_watched_files
      {
        // we are going to watch all the JSON files in the workspace, and the
        // notification handler will pick up any of the changes of those files we
        // are interested in.
        let options = DidChangeWatchedFilesRegistrationOptions {
          watchers: vec![FileSystemWatcher {
            glob_pattern: GlobPattern::String(
              "**/*.{json,jsonc,lock}".to_string(),
            ),
            kind: None,
          }],
        };
        registrations.push(Registration {
          id: "workspace/didChangeWatchedFiles".to_string(),
          method: "workspace/didChangeWatchedFiles".to_string(),
          register_options: Some(serde_json::to_value(options).unwrap()),
        });
      }
      if ls.config.client_capabilities.workspace_will_rename_files {
        let options = FileOperationRegistrationOptions {
          filters: vec![FileOperationFilter {
            scheme: Some("file".to_string()),
            pattern: FileOperationPattern {
              glob: "**/*".to_string(),
              matches: None,
              options: None,
            },
          }],
        };
        registrations.push(Registration {
          id: "workspace/willRenameFiles".to_string(),
          method: "workspace/willRenameFiles".to_string(),
          register_options: Some(serde_json::to_value(options).unwrap()),
        });
      }

      if ls.config.client_capabilities.testing_api {
        let test_server = testing::TestServer::new(
          ls.client.clone(),
          ls.performance.clone(),
          ls.config.root_uri().cloned(),
        );
        ls.maybe_testing_server = Some(test_server);
      }
      (ls.client.clone(), ls.http_client.clone())
    };

    for registration in registrations {
      if let Err(err) = client
        .when_outside_lsp_lock()
        .register_capability(vec![registration])
        .await
      {
        lsp_warn!("Client errored on capabilities.\n{:#}", err);
      }
    }

    self.refresh_configuration().await;

    {
      let mut ls = self.0.write().await;
      init_log_file(ls.config.log_file());
      if let Err(err) = ls.update_tsconfig().await {
        ls.client.show_message(MessageType::WARNING, err);
      }
      ls.refresh_documents_config().await;
      ls.diagnostics_server.invalidate_all();
      ls.send_diagnostics_update();
    }

    lsp_log!("Server ready.");

    if upgrade_check_enabled() {
      // spawn to avoid lsp send/sync requirement, but also just
      // to ensure this initialized method returns quickly
      spawn(async move {
        match check_for_upgrades_for_lsp(http_client).await {
          Ok(version_info) => {
            client.send_did_upgrade_check_notification(
              lsp_custom::DidUpgradeCheckNotificationParams {
                upgrade_available: version_info.map(|info| {
                  lsp_custom::UpgradeAvailable {
                    latest_version: info.latest_version,
                    is_canary: info.is_canary,
                  }
                }),
              },
            );
          }
          Err(err) => lsp_warn!("Failed to check for upgrades: {err}"),
        }
      });
    }
  }

  async fn shutdown(&self) -> LspResult<()> {
    self.1.cancel();
    self.0.write().await.shutdown().await
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }

    let mut inner = self.0.write().await;
    let specifier = inner
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    let document = inner.did_open(&specifier, params).await;
    if document.is_diagnosable() {
      inner.refresh_npm_specifiers().await;
      let specifiers = inner.documents.dependents(&specifier);
      inner.diagnostics_server.invalidate(&specifiers);
      inner.ts_server.increment_project_version();
      inner.send_diagnostics_update();
      inner.send_testing_update();
    }
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    self.0.write().await.did_change(params).await
  }

  async fn did_save(&self, params: DidSaveTextDocumentParams) {
    let uri = &params.text_document.uri;
    {
      let mut inner = self.0.write().await;
      let specifier = inner.url_map.normalize_url(uri, LspUrlKind::File);
      inner.documents.save(&specifier);
      if !inner
        .config
        .workspace_settings_for_specifier(&specifier)
        .cache_on_save
        || !inner.config.specifier_enabled(&specifier)
        || !inner.diagnostics_state.has_no_cache_diagnostics(&specifier)
      {
        return;
      }
      match specifier_to_file_path(&specifier) {
        Ok(path) if is_importable_ext(&path) => {}
        _ => return,
      }
    }
    if let Err(err) = self
      .cache_request(Some(
        serde_json::to_value(lsp_custom::CacheParams {
          referrer: TextDocumentIdentifier { uri: uri.clone() },
          uris: vec![TextDocumentIdentifier { uri: uri.clone() }],
        })
        .unwrap(),
      ))
      .await
    {
      lsp_warn!("Failed to cache \"{}\" on save: {}", uri.to_string(), err);
    }
  }

  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    self.0.write().await.did_close(params).await
  }

  async fn did_change_configuration(
    &self,
    params: DidChangeConfigurationParams,
  ) {
    let mark = {
      let inner = self.0.read().await;
      inner
        .performance
        .mark_with_args("lsp.did_change_configuration", &params)
    };

    self.refresh_configuration().await;

    let mut inner = self.0.write().await;
    inner.did_change_configuration(params).await;
    inner.performance.measure(mark);
  }

  async fn did_change_watched_files(
    &self,
    params: DidChangeWatchedFilesParams,
  ) {
    self.0.write().await.did_change_watched_files(params).await
  }

  async fn did_change_workspace_folders(
    &self,
    params: DidChangeWorkspaceFoldersParams,
  ) {
    let (performance, mark) = {
      let mut ls = self.0.write().await;
      let mark = ls
        .performance
        .mark_with_args("lsp.did_change_workspace_folders", &params);
      ls.did_change_workspace_folders(params);
      (ls.performance.clone(), mark)
    };

    self.refresh_configuration().await;
    {
      let mut ls = self.0.write().await;
      ls.refresh_documents_config().await;
      ls.diagnostics_server.invalidate_all();
      ls.ts_server.increment_project_version();
      ls.send_diagnostics_update();
    }
    performance.measure(mark);
  }

  async fn document_symbol(
    &self,
    params: DocumentSymbolParams,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    self.0.read().await.document_symbol(params).await
  }

  async fn formatting(
    &self,
    params: DocumentFormattingParams,
  ) -> LspResult<Option<Vec<TextEdit>>> {
    self.0.read().await.formatting(params).await
  }

  async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
    self.0.read().await.hover(params).await
  }

  async fn inlay_hint(
    &self,
    params: InlayHintParams,
  ) -> LspResult<Option<Vec<InlayHint>>> {
    self.0.read().await.inlay_hint(params).await
  }

  async fn code_action(
    &self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    self.0.read().await.code_action(params).await
  }

  async fn code_action_resolve(
    &self,
    params: CodeAction,
  ) -> LspResult<CodeAction> {
    self.0.read().await.code_action_resolve(params).await
  }

  async fn code_lens(
    &self,
    params: CodeLensParams,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    self.0.read().await.code_lens(params).await
  }

  async fn code_lens_resolve(&self, params: CodeLens) -> LspResult<CodeLens> {
    self.0.read().await.code_lens_resolve(params).await
  }

  async fn document_highlight(
    &self,
    params: DocumentHighlightParams,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    self.0.read().await.document_highlight(params).await
  }

  async fn references(
    &self,
    params: ReferenceParams,
  ) -> LspResult<Option<Vec<Location>>> {
    self.0.read().await.references(params).await
  }

  async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
  ) -> LspResult<Option<GotoDefinitionResponse>> {
    self.0.read().await.goto_definition(params).await
  }

  async fn goto_type_definition(
    &self,
    params: GotoTypeDefinitionParams,
  ) -> LspResult<Option<GotoTypeDefinitionResponse>> {
    self.0.read().await.goto_type_definition(params).await
  }

  async fn completion(
    &self,
    params: CompletionParams,
  ) -> LspResult<Option<CompletionResponse>> {
    self.0.read().await.completion(params).await
  }

  async fn completion_resolve(
    &self,
    params: CompletionItem,
  ) -> LspResult<CompletionItem> {
    self.0.read().await.completion_resolve(params).await
  }

  async fn goto_implementation(
    &self,
    params: GotoImplementationParams,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    self.0.read().await.goto_implementation(params).await
  }

  async fn folding_range(
    &self,
    params: FoldingRangeParams,
  ) -> LspResult<Option<Vec<FoldingRange>>> {
    self.0.read().await.folding_range(params).await
  }

  async fn incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
    self.0.read().await.incoming_calls(params).await
  }

  async fn outgoing_calls(
    &self,
    params: CallHierarchyOutgoingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
    self.0.read().await.outgoing_calls(params).await
  }

  async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams,
  ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
    self.0.read().await.prepare_call_hierarchy(params).await
  }

  async fn rename(
    &self,
    params: RenameParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    self.0.read().await.rename(params).await
  }

  async fn selection_range(
    &self,
    params: SelectionRangeParams,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    self.0.read().await.selection_range(params).await
  }

  async fn semantic_tokens_full(
    &self,
    params: SemanticTokensParams,
  ) -> LspResult<Option<SemanticTokensResult>> {
    self.0.read().await.semantic_tokens_full(params).await
  }

  async fn semantic_tokens_range(
    &self,
    params: SemanticTokensRangeParams,
  ) -> LspResult<Option<SemanticTokensRangeResult>> {
    self.0.read().await.semantic_tokens_range(params).await
  }

  async fn signature_help(
    &self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    self.0.read().await.signature_help(params).await
  }

  async fn will_rename_files(
    &self,
    params: RenameFilesParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    self.0.read().await.will_rename_files(params).await
  }

  async fn symbol(
    &self,
    params: WorkspaceSymbolParams,
  ) -> LspResult<Option<Vec<SymbolInformation>>> {
    self.0.read().await.symbol(params).await
  }
}

struct PrepareCacheResult {
  cli_options: CliOptions,
  roots: Vec<ModuleSpecifier>,
  open_docs: Vec<Document>,
  mark: PerformanceMark,
}

// These are implementations of custom commands supported by the LSP
impl Inner {
  fn prepare_cache(
    &self,
    params: lsp_custom::CacheParams,
  ) -> Result<Option<PrepareCacheResult>, AnyError> {
    let referrer = self
      .url_map
      .normalize_url(&params.referrer.uri, LspUrlKind::File);
    if !self.is_diagnosable(&referrer) {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.cache", &params);
    let roots = if !params.uris.is_empty() {
      params
        .uris
        .iter()
        .map(|t| self.url_map.normalize_url(&t.uri, LspUrlKind::File))
        .collect()
    } else {
      vec![referrer]
    };

    let workspace_settings = self.config.workspace_settings();
    let mut cli_options = CliOptions::new(
      Flags {
        cache_path: self.maybe_global_cache_path.clone(),
        ca_stores: workspace_settings.certificate_stores.clone(),
        ca_data: workspace_settings.tls_certificate.clone().map(CaData::File),
        unsafely_ignore_certificate_errors: workspace_settings
          .unsafely_ignore_certificate_errors
          .clone(),
        node_modules_dir: Some(
          self.config.maybe_node_modules_dir_path().is_some(),
        ),
        // bit of a hack to force the lsp to cache the @types/node package
        type_check_mode: crate::args::TypeCheckMode::Local,
        ..Default::default()
      },
      std::env::current_dir().with_context(|| "Failed getting cwd.")?,
      self.config.maybe_config_file().cloned(),
      self.config.maybe_lockfile().cloned(),
      self.maybe_package_json.clone(),
    )?;
    cli_options.set_import_map_specifier(self.maybe_import_map_uri.clone());

    let open_docs = self.documents.documents(DocumentsFilter::OpenDiagnosable);
    Ok(Some(PrepareCacheResult {
      cli_options,
      open_docs,
      roots,
      mark,
    }))
  }

  async fn post_cache(&self, mark: PerformanceMark) {
    // Now that we have dependencies loaded, we need to re-analyze all the files.
    // For that we're invalidating all the existing diagnostics and restarting
    // the language server for TypeScript (as it might hold to some stale
    // documents).
    self.diagnostics_server.invalidate_all();
    self.ts_server.restart(self.snapshot()).await;
    self.send_diagnostics_update();
    self.send_testing_update();

    self.performance.measure(mark);
  }

  fn get_performance(&self) -> Value {
    let averages = self.performance.averages();
    json!({ "averages": averages })
  }

  fn task_definitions(&self) -> LspResult<Vec<TaskDefinition>> {
    let mut result = vec![];
    if let Some(config_file) = self.config.maybe_config_file() {
      if let Some(tasks) = json!(&config_file.json.tasks).as_object() {
        for (name, value) in tasks {
          let Some(command) = value.as_str() else {
            continue;
          };
          result.push(TaskDefinition {
            name: name.clone(),
            command: command.to_string(),
            source_uri: config_file.specifier.clone(),
          });
        }
      };
    }
    if let Some(package_json) = &self.maybe_package_json {
      if let Some(scripts) = &package_json.scripts {
        for (name, command) in scripts {
          result.push(TaskDefinition {
            name: name.clone(),
            command: command.clone(),
            source_uri: package_json.specifier(),
          });
        }
      }
    }
    result.sort_by_key(|d| d.name.clone());
    Ok(result)
  }

  async fn inlay_hint(
    &self,
    params: InlayHintParams,
  ) -> LspResult<Option<Vec<InlayHint>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
      || !self.config.enabled_inlay_hints_for_specifier(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.inlay_hint", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let text_span =
      tsc::TextSpan::from_range(&params.range, line_index.clone()).map_err(
        |err| {
          error!("Failed to convert range to text_span: {}", err);
          LspError::internal_error()
        },
      )?;
    let maybe_inlay_hints = self
      .ts_server
      .provide_inlay_hints(
        self.snapshot(),
        specifier.clone(),
        text_span,
        tsc::UserPreferences::from_config_for_specifier(
          &self.config,
          &self.fmt_options.options,
          &specifier,
        ),
      )
      .await?;
    let maybe_inlay_hints = maybe_inlay_hints.map(|hints| {
      hints
        .iter()
        .map(|hint| hint.to_lsp(line_index.clone()))
        .collect()
    });
    self.performance.measure(mark);
    Ok(maybe_inlay_hints)
  }

  async fn reload_import_registries(&mut self) -> LspResult<Option<Value>> {
    remove_dir_all_if_exists(&self.module_registries_location)
      .await
      .map_err(|err| {
        error!("Unable to remove registries cache: {}", err);
        LspError::internal_error()
      })?;
    self.update_registries().await.map_err(|err| {
      error!("Unable to update registries: {}", err);
      LspError::internal_error()
    })?;
    Ok(Some(json!(true)))
  }

  fn virtual_text_document(
    &self,
    params: lsp_custom::VirtualTextDocumentParams,
  ) -> LspResult<Option<String>> {
    let mark = self
      .performance
      .mark_with_args("lsp.virtual_text_document", &params);
    let specifier = self
      .url_map
      .normalize_url(&params.text_document.uri, LspUrlKind::File);
    let contents = if specifier.scheme() == "deno"
      && specifier.path() == "/status.md"
    {
      let mut contents = String::new();
      let mut documents_specifiers = self
        .documents
        .documents(DocumentsFilter::All)
        .into_iter()
        .map(|d| d.specifier().clone())
        .collect::<Vec<_>>();
      documents_specifiers.sort();
      let measures = self.performance.to_vec();
      let workspace_settings = self.config.workspace_settings();

      write!(
        contents,
        r#"# Deno Language Server Status

## Workspace Settings

```json
{}
```

## Workspace Details

  - <details><summary>Documents in memory: {}</summary>

    - {}

  </details>

  - <details><summary>Performance measures: {}</summary>

    - {}

  </details>
"#,
        serde_json::to_string_pretty(&workspace_settings).unwrap(),
        documents_specifiers.len(),
        documents_specifiers
          .into_iter()
          .map(|s| s.to_string())
          .collect::<Vec<String>>()
          .join("\n    - "),
        measures.len(),
        measures
          .iter()
          .map(|m| m.to_string())
          .collect::<Vec<String>>()
          .join("\n    - ")
      )
      .unwrap();

      contents
        .push_str("\n## Performance (last 3 000 entries)\n\n|Name|Count|Duration|\n|---|---|---|\n");
      let mut averages = self.performance.averages_as_f64();
      averages.sort_by(|a, b| a.0.cmp(&b.0));
      for (name, count, average_duration) in averages {
        writeln!(contents, "|{}|{}|{}ms|", name, count, average_duration)
          .unwrap();
      }

      contents.push_str(
        "\n## Performance (total)\n\n|Name|Count|Duration|\n|---|---|---|\n",
      );
      let mut measurements_by_type = self.performance.measurements_by_type();
      measurements_by_type.sort_by(|a, b| a.0.cmp(&b.0));
      for (name, total_count, total_duration) in measurements_by_type {
        writeln!(
          contents,
          "|{}|{}|{:.3}ms|",
          name, total_count, total_duration
        )
        .unwrap();
      }

      Some(contents)
    } else {
      let asset_or_doc = self.get_maybe_asset_or_document(&specifier);
      if let Some(asset_or_doc) = asset_or_doc {
        Some(asset_or_doc.text().to_string())
      } else {
        error!("The source was not found: {}", specifier);
        None
      }
    };
    self.performance.measure(mark);
    Ok(contents)
  }
}
