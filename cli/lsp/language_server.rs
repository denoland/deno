// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::MediaType;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceDiscoverOptions;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::unsync::spawn;
use deno_core::url;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::GraphKind;
use deno_graph::Resolution;
use deno_path_util::url_to_file_path;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_semver::jsr::JsrPackageReqReference;
use indexmap::Equivalent;
use indexmap::IndexSet;
use log::error;
use serde::Deserialize;
use serde_json::from_value;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::env;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tower_lsp::jsonrpc::Error as LspError;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::request::*;
use tower_lsp::lsp_types::*;

use super::analysis::fix_ts_import_changes;
use super::analysis::ts_changes_to_edit;
use super::analysis::CodeActionCollection;
use super::analysis::CodeActionData;
use super::analysis::TsResponseImportMapper;
use super::cache::LspCache;
use super::capabilities;
use super::client::Client;
use super::code_lens;
use super::completions;
use super::config::Config;
use super::config::UpdateImportsOnFileMoveEnabled;
use super::config::WorkspaceSettings;
use super::config::SETTINGS_SECTION;
use super::diagnostics;
use super::diagnostics::DiagnosticDataSpecifier;
use super::diagnostics::DiagnosticServerUpdateMessage;
use super::diagnostics::DiagnosticsServer;
use super::diagnostics::DiagnosticsState;
use super::documents::to_lsp_range;
use super::documents::AssetOrDocument;
use super::documents::Document;
use super::documents::Documents;
use super::documents::DocumentsFilter;
use super::documents::LanguageId;
use super::jsr::CliJsrSearchApi;
use super::logging::lsp_log;
use super::logging::lsp_warn;
use super::lsp_custom;
use super::lsp_custom::TaskDefinition;
use super::npm::CliNpmSearchApi;
use super::parent_process_checker;
use super::performance::Performance;
use super::refactor;
use super::registries::ModuleRegistry;
use super::resolver::LspResolver;
use super::testing;
use super::text;
use super::tsc;
use super::tsc::Assets;
use super::tsc::AssetsSnapshot;
use super::tsc::ChangeKind;
use super::tsc::GetCompletionDetailsArgs;
use super::tsc::TsServer;
use super::urls;
use super::urls::uri_to_url;
use super::urls::url_to_uri;
use crate::args::create_default_npmrc;
use crate::args::get_root_cert_store;
use crate::args::has_flag_env_var;
use crate::args::CaData;
use crate::args::CacheSetting;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::InternalFlags;
use crate::args::UnstableFmtOptions;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::graph_util;
use crate::http_util::HttpClientProvider;
use crate::lsp::config::ConfigWatchedFileType;
use crate::lsp::logging::init_log_file;
use crate::lsp::tsc::file_text_changes_to_workspace_edit;
use crate::lsp::urls::LspUrlKind;
use crate::tools::fmt::format_file;
use crate::tools::fmt::format_parsed_source;
use crate::tools::upgrade::check_for_upgrades_for_lsp;
use crate::tools::upgrade::upgrade_check_enabled;
use crate::util::fs::remove_dir_all_if_exists;
use crate::util::path::is_importable_ext;
use crate::util::path::to_percent_decoded_str;
use crate::util::sync::AsyncFlag;

struct LspRootCertStoreProvider(RootCertStore);

impl RootCertStoreProvider for LspRootCertStoreProvider {
  fn get_or_try_init(&self) -> Result<&RootCertStore, AnyError> {
    Ok(&self.0)
  }
}

#[derive(Debug, Clone)]
pub struct LanguageServer {
  client: Client,
  pub inner: Arc<tokio::sync::RwLock<Inner>>,
  /// This is used to block out standard request handling until the complete
  /// user configuration has been fetched. This is done in the `initialized`
  /// handler which normally may occur concurrently with those other requests.
  /// TODO(nayeemrmn): This wouldn't be necessary if LSP allowed
  /// `workspace/configuration` requests in the `initialize` handler. See:
  /// https://github.com/Microsoft/language-server-protocol/issues/567#issuecomment-2085131917
  init_flag: AsyncFlag,
  performance: Arc<Performance>,
  shutdown_flag: AsyncFlag,
}

/// Snapshot of the state used by TSC.
#[derive(Clone, Debug, Default)]
pub struct StateSnapshot {
  pub project_version: usize,
  pub assets: AssetsSnapshot,
  pub config: Arc<Config>,
  pub documents: Arc<Documents>,
  pub resolver: Arc<LspResolver>,
}

type LanguageServerTaskFn = Box<dyn FnOnce(LanguageServer) + Send + Sync>;

/// Used to queue tasks from inside of the language server lock that must be
/// commenced from outside of it. For example, queue a request to cache a module
/// after having loaded a config file which references it.
#[derive(Debug)]
struct LanguageServerTaskQueue {
  task_tx: UnboundedSender<LanguageServerTaskFn>,
  /// This is moved out to its own task after initializing.
  task_rx: Option<UnboundedReceiver<LanguageServerTaskFn>>,
}

impl Default for LanguageServerTaskQueue {
  fn default() -> Self {
    let (task_tx, task_rx) = unbounded_channel();
    Self {
      task_tx,
      task_rx: Some(task_rx),
    }
  }
}

impl LanguageServerTaskQueue {
  pub fn queue_task(&self, task_fn: LanguageServerTaskFn) -> bool {
    self.task_tx.send(task_fn).is_ok()
  }

  /// Panics if called more than once.
  fn start(&mut self, ls: LanguageServer) {
    let mut task_rx = self.task_rx.take().unwrap();
    spawn(async move {
      while let Some(task_fn) = task_rx.recv().await {
        task_fn(ls.clone());
      }
    });
  }
}

#[derive(Debug)]
pub struct Inner {
  /// Cached versions of "fixed" assets that can either be inlined in Rust or
  /// are part of the TypeScript snapshot and have to be fetched out.
  assets: Assets,
  cache: LspCache,
  /// The LSP client that this LSP server is connected to.
  pub client: Client,
  /// Configuration information.
  pub config: Config,
  diagnostics_state: Arc<diagnostics::DiagnosticsState>,
  diagnostics_server: diagnostics::DiagnosticsServer,
  /// The collection of documents that the server is currently handling, either
  /// on disk or "open" within the client.
  pub documents: Documents,
  http_client_provider: Arc<HttpClientProvider>,
  initial_cwd: PathBuf,
  jsr_search_api: CliJsrSearchApi,
  /// Handles module registries, which allow discovery of modules
  module_registry: ModuleRegistry,
  /// A lazily create "server" for handling test run requests.
  maybe_testing_server: Option<testing::TestServer>,
  pub npm_search_api: CliNpmSearchApi,
  project_version: usize,
  /// A collection of measurements which instrument that performance of the LSP.
  performance: Arc<Performance>,
  pub resolver: Arc<LspResolver>,
  task_queue: LanguageServerTaskQueue,
  /// A memoized version of fixable diagnostic codes retrieved from TypeScript.
  ts_fixable_diagnostics: Vec<String>,
  /// An abstraction that handles interactions with TypeScript.
  pub ts_server: Arc<TsServer>,
  /// A map of specifiers and URLs used to translate over the LSP.
  pub url_map: urls::LspUrlMap,
  workspace_files: IndexSet<ModuleSpecifier>,
  /// Set to `self.config.settings.enable_settings_hash()` after
  /// refreshing `self.workspace_files`.
  workspace_files_hash: u64,
}

impl LanguageServer {
  pub fn new(client: Client, shutdown_flag: AsyncFlag) -> Self {
    let performance = Arc::new(Performance::default());
    Self {
      client: client.clone(),
      inner: Arc::new(tokio::sync::RwLock::new(Inner::new(
        client,
        performance.clone(),
      ))),
      init_flag: Default::default(),
      performance,
      shutdown_flag,
    }
  }

  /// Similar to `deno install --entrypoint` on the command line, where modules will be cached
  /// in the Deno cache, including any of their dependencies.
  pub async fn cache(
    &self,
    specifiers: Vec<ModuleSpecifier>,
    referrer: ModuleSpecifier,
    force_global_cache: bool,
  ) -> LspResult<Option<Value>> {
    async fn create_graph_for_caching(
      cli_options: CliOptions,
      roots: Vec<ModuleSpecifier>,
      open_docs: Vec<Arc<Document>>,
    ) -> Result<(), AnyError> {
      let open_docs = open_docs
        .into_iter()
        .map(|d| (d.specifier().clone(), d))
        .collect::<HashMap<_, _>>();
      let cli_options = Arc::new(cli_options);
      let factory = CliFactory::from_cli_options(cli_options.clone());
      let module_graph_builder = factory.module_graph_builder().await?;
      let module_graph_creator = factory.module_graph_creator().await?;
      let mut inner_loader = module_graph_builder.create_graph_loader();
      let mut loader = crate::lsp::documents::OpenDocumentsGraphLoader {
        inner_loader: &mut inner_loader,
        open_docs: &open_docs,
      };
      let graph = module_graph_creator
        .create_graph_with_loader(GraphKind::All, roots.clone(), &mut loader)
        .await?;
      graph_util::graph_valid(
        &graph,
        factory.fs(),
        &roots,
        graph_util::GraphValidOptions {
          kind: GraphKind::All,
          check_js: false,
          exit_integrity_errors: false,
        },
      )?;

      // Update the lockfile on the file system with anything new
      // found after caching
      if let Some(lockfile) = cli_options.maybe_lockfile() {
        if let Err(err) = &lockfile.write_if_changed() {
          lsp_warn!("{:#}", err);
        }
      }

      Ok(())
    }

    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }

    // prepare the cache inside the lock
    let mark = self
      .performance
      .mark_with_args("lsp.cache", (&specifiers, &referrer));
    let prepare_cache_result = self.inner.write().await.prepare_cache(
      specifiers,
      referrer,
      force_global_cache,
    );

    match prepare_cache_result {
      Ok(result) => {
        // cache outside the lock
        let cli_options = result.cli_options;
        let roots = result.roots;
        let open_docs = result.open_docs;
        let handle = spawn(async move {
          create_graph_for_caching(cli_options, roots, open_docs).await
        });

        if let Err(err) = handle.await.unwrap() {
          lsp_warn!("Error caching: {:#}", err);
          self.client.show_message(MessageType::WARNING, err);
        }

        // now get the lock back to update with the new information
        self.inner.write().await.post_cache().await;
        self.performance.measure(mark);
      }
      Err(err) => {
        lsp_warn!("Error preparing caching: {:#}", err);
        self.client.show_message(MessageType::WARNING, err);
        return Err(LspError::internal_error());
      }
    }

    Ok(Some(json!(true)))
  }

  /// This request is only used by the lsp integration tests to
  /// coordinate the tests receiving the latest diagnostics.
  pub async fn latest_diagnostic_batch_index_request(
    &self,
  ) -> LspResult<Option<Value>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    Ok(
      self
        .inner
        .read()
        .await
        .diagnostics_server
        .latest_batch_index()
        .map(|v| v.into()),
    )
  }

  pub async fn performance_request(&self) -> LspResult<Option<Value>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    Ok(Some(self.inner.read().await.get_performance()))
  }

  pub async fn task_definitions(&self) -> LspResult<Vec<TaskDefinition>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.task_definitions()
  }

  pub async fn test_run_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.test_run_request(params).await
  }

  pub async fn test_run_cancel_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.test_run_cancel_request(params)
  }

  pub async fn virtual_text_document(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    match params.map(serde_json::from_value) {
      Some(Ok(params)) => Ok(Some(
        serde_json::to_value(
          self.inner.read().await.virtual_text_document(params)?,
        )
        .map_err(|err| {
          error!(
            "Failed to serialize virtual_text_document response: {:#}",
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
    let (folders, capable) = {
      let inner = self.inner.read().await;
      (
        inner.config.workspace_folders.clone(),
        inner.config.workspace_configuration_capable(),
      )
    };
    if capable {
      let mut scopes = Vec::with_capacity(folders.len() + 1);
      scopes.push(None);
      for (_, folder) in folders.as_ref() {
        scopes.push(Some(folder.uri.clone()));
      }
      let configs = self
        .client
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
        let mut folder_settings = Vec::with_capacity(folders.len());
        for (folder_uri, _) in folders.as_ref() {
          folder_settings.push((folder_uri.clone(), configs.next().unwrap()));
        }
        self
          .inner
          .write()
          .await
          .config
          .set_workspace_settings(unscoped, folder_settings);
      }
    }
  }
}

impl Inner {
  fn new(client: Client, performance: Arc<Performance>) -> Self {
    let cache = LspCache::default();
    let http_client_provider = Arc::new(HttpClientProvider::new(None, None));
    let module_registry = ModuleRegistry::new(
      cache.deno_dir().registries_folder_path(),
      http_client_provider.clone(),
    );
    let jsr_search_api =
      CliJsrSearchApi::new(module_registry.file_fetcher.clone());
    let npm_search_api =
      CliNpmSearchApi::new(module_registry.file_fetcher.clone());
    let documents = Documents::default();
    let config = Config::default();
    let ts_server = Arc::new(TsServer::new(performance.clone()));
    let diagnostics_state = Arc::new(DiagnosticsState::default());
    let diagnostics_server = DiagnosticsServer::new(
      client.clone(),
      performance.clone(),
      ts_server.clone(),
      diagnostics_state.clone(),
    );
    let assets = Assets::new(ts_server.clone());
    let initial_cwd = std::env::current_dir().unwrap_or_else(|_| {
      panic!("Could not resolve current working directory")
    });

    Self {
      assets,
      cache,
      client,
      config,
      diagnostics_state,
      diagnostics_server,
      documents,
      http_client_provider,
      initial_cwd: initial_cwd.clone(),
      jsr_search_api,
      project_version: 0,
      task_queue: Default::default(),
      maybe_testing_server: None,
      module_registry,
      npm_search_api,
      performance,
      resolver: Default::default(),
      ts_fixable_diagnostics: Default::default(),
      ts_server,
      url_map: Default::default(),
      workspace_files: Default::default(),
      workspace_files_hash: 0,
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
          .get_navigation_tree(
            self.snapshot(),
            specifier.clone(),
            asset_or_doc.scope().cloned(),
          )
          .await?;
        let navigation_tree = Arc::new(navigation_tree);
        match asset_or_doc {
          AssetOrDocument::Asset(_) => self
            .assets
            .cache_navigation_tree(specifier, navigation_tree.clone())?,
          AssetOrDocument::Document(doc) => {
            doc.cache_navigation_tree(navigation_tree.clone());
          }
        }
        navigation_tree
      };
    self.performance.measure(mark);
    Ok(navigation_tree)
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

  pub fn snapshot(&self) -> Arc<StateSnapshot> {
    Arc::new(StateSnapshot {
      project_version: self.project_version,
      assets: self.assets.snapshot(),
      config: Arc::new(self.config.clone()),
      documents: Arc::new(self.documents.clone()),
      resolver: self.resolver.snapshot(),
    })
  }

  pub async fn update_global_cache(&mut self) {
    let mark = self.performance.mark("lsp.update_global_cache");
    let maybe_cache = self.config.workspace_settings().cache.as_ref();
    let global_cache_url = maybe_cache.and_then(|cache_str| {
      if let Ok(url) = Url::from_file_path(cache_str) {
        Some(url)
      } else if let Some(root_uri) = self.config.root_uri() {
        root_uri.join(cache_str).inspect_err(|err| lsp_warn!("Failed to resolve custom cache path: {err}")).ok()
      } else {
        lsp_warn!(
          "The configured cache path \"{cache_str}\" is not resolvable outside of a workspace.",
        );
        None
      }
    });
    self.cache = LspCache::new(global_cache_url);
    let deno_dir = self.cache.deno_dir();
    let workspace_settings = self.config.workspace_settings();
    let maybe_root_path = self
      .config
      .root_uri()
      .and_then(|uri| url_to_file_path(uri).ok());
    let root_cert_store = get_root_cert_store(
      maybe_root_path,
      workspace_settings.certificate_stores.clone(),
      workspace_settings.tls_certificate.clone().map(CaData::File),
    )
    .inspect_err(|err| lsp_warn!("Failed to load root cert store: {err}"))
    .unwrap_or_else(|_| RootCertStore::empty());
    let root_cert_store_provider =
      Arc::new(LspRootCertStoreProvider(root_cert_store));
    self.http_client_provider = Arc::new(HttpClientProvider::new(
      Some(root_cert_store_provider),
      workspace_settings
        .unsafely_ignore_certificate_errors
        .clone(),
    ));
    self.module_registry = ModuleRegistry::new(
      deno_dir.registries_folder_path(),
      self.http_client_provider.clone(),
    );
    let workspace_settings = self.config.workspace_settings();
    for (registry, enabled) in workspace_settings.suggest.imports.hosts.iter() {
      if *enabled {
        lsp_log!("Enabling import suggestions for: {}", registry);
        self.module_registry.enable(registry).await;
      } else {
        self.module_registry.disable(registry);
      }
    }
    self.jsr_search_api =
      CliJsrSearchApi::new(self.module_registry.file_fetcher.clone());
    self.npm_search_api =
      CliNpmSearchApi::new(self.module_registry.file_fetcher.clone());
    self.performance.measure(mark);
  }

  pub fn update_cache(&mut self) {
    let mark = self.performance.mark("lsp.update_cache");
    self.cache.update_config(&self.config);
    self.url_map.set_cache(&self.cache);
    self.performance.measure(mark);
  }

  pub fn update_debug_flag(&self) {
    let internal_debug = self.config.workspace_settings().internal_debug;
    super::logging::set_lsp_debug_flag(internal_debug)
  }
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

    let capabilities = capabilities::server_capabilities(&params.capabilities);

    let version = format!(
      "{} ({}, {})",
      crate::version::DENO_VERSION_INFO.deno,
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
      let mut workspace_folders = vec![];
      if let Some(folders) = params.workspace_folders {
        workspace_folders = folders
          .into_iter()
          .map(|folder| {
            (
              self
                .url_map
                .uri_to_specifier(&folder.uri, LspUrlKind::Folder),
              folder,
            )
          })
          .collect();
      }
      // rootUri is deprecated by the LSP spec. If it's specified, merge it into
      // workspace_folders.
      #[allow(deprecated)]
      if let Some(root_uri) = params.root_uri {
        if !workspace_folders.iter().any(|(_, f)| f.uri == root_uri) {
          let root_url =
            self.url_map.uri_to_specifier(&root_uri, LspUrlKind::Folder);
          let name = root_url.path_segments().and_then(|s| s.last());
          let name = name.unwrap_or_default().to_string();
          workspace_folders.insert(
            0,
            (
              root_url,
              WorkspaceFolder {
                uri: root_uri,
                name,
              },
            ),
          );
        }
      }
      self.config.set_workspace_folders(workspace_folders);
      if let Some(options) = params.initialization_options {
        self.config.set_workspace_settings(
          WorkspaceSettings::from_initialization_options(options),
          vec![],
        );
      }
      self.config.set_client_capabilities(params.capabilities);
    }

    self.diagnostics_server.start();
    if let Err(e) = self
      .ts_server
      .start(self.config.internal_inspect().to_address())
    {
      lsp_warn!("{}", e);
      self.client.show_message(MessageType::ERROR, e);
      return Err(tower_lsp::jsonrpc::Error::internal_error());
    };

    self.update_debug_flag();

    if capabilities.code_action_provider.is_some() {
      let fixable_diagnostics = self
        .ts_server
        .get_supported_code_fixes(self.snapshot())
        .await?;
      self.ts_fixable_diagnostics = fixable_diagnostics;
    }
    self.assets.initialize(self.snapshot()).await;

    self.performance.measure(mark);
    Ok(InitializeResult {
      capabilities,
      server_info: Some(server_info),
      offset_encoding: None,
    })
  }

  fn walk_workspace(config: &Config) -> (IndexSet<ModuleSpecifier>, bool) {
    if !config.workspace_capable() {
      log::debug!("Skipped workspace walk due to client incapability.");
      return (Default::default(), false);
    }
    let mut workspace_files = IndexSet::default();
    let entry_limit = 1000;
    let mut pending = VecDeque::new();
    let mut entry_count = 0;
    let mut roots = config
      .workspace_folders
      .iter()
      .filter_map(|p| url_to_file_path(&p.0).ok())
      .collect::<Vec<_>>();
    roots.sort();
    let roots = roots
      .iter()
      .enumerate()
      .filter(|(i, root)| *i == 0 || !root.starts_with(&roots[i - 1]))
      .map(|(_, r)| r.clone())
      .collect::<Vec<_>>();
    let mut root_ancestors = BTreeSet::new();
    for root in roots {
      for ancestor in root.ancestors().skip(1) {
        if root_ancestors.insert(ancestor.to_path_buf()) {
          break;
        }
      }
      if let Ok(read_dir) = std::fs::read_dir(&root) {
        pending.push_back((root, read_dir));
      }
    }
    for root_ancestor in root_ancestors {
      for deno_json in ["deno.json", "deno.jsonc"] {
        let path = root_ancestor.join(deno_json);
        if path.exists() {
          if let Ok(specifier) = ModuleSpecifier::from_file_path(path) {
            workspace_files.insert(specifier);
          }
        }
      }
    }
    while let Some((parent_path, read_dir)) = pending.pop_front() {
      // Sort entries from each dir for consistency across operating systems.
      let mut dir_files = BTreeSet::new();
      let mut dir_subdirs = BTreeMap::new();
      for entry in read_dir {
        let Ok(entry) = entry else {
          continue;
        };
        if entry_count >= entry_limit {
          return (workspace_files, true);
        }
        entry_count += 1;
        let path = parent_path.join(entry.path());
        let Ok(specifier) = ModuleSpecifier::from_file_path(&path) else {
          continue;
        };
        let Ok(file_type) = entry.file_type() else {
          continue;
        };
        let Some(file_name) = path.file_name() else {
          continue;
        };
        if config.settings.specifier_enabled(&specifier) == Some(false) {
          continue;
        }
        if file_type.is_dir() {
          let dir_name = file_name.to_string_lossy().to_lowercase();
          // We ignore these directories by default because there is a
          // high likelihood they aren't relevant. Someone can opt-into
          // them by specifying one of them as an enabled path.
          if matches!(dir_name.as_str(), "vendor" | "node_modules" | ".git") {
            continue;
          }
          // ignore cargo target directories for anyone using Deno with Rust
          if dir_name == "target"
            && path
              .parent()
              .map(|p| p.join("Cargo.toml").exists())
              .unwrap_or(false)
          {
            continue;
          }
          if let Ok(read_dir) = std::fs::read_dir(&path) {
            dir_subdirs.insert(specifier, (path, read_dir));
          }
        } else if file_type.is_file()
          || file_type.is_symlink()
            && std::fs::metadata(&path)
              .ok()
              .map(|m| m.is_file())
              .unwrap_or(false)
        {
          if file_name.to_string_lossy().contains(".min.") {
            continue;
          }
          let media_type = MediaType::from_specifier(&specifier);
          match media_type {
            MediaType::JavaScript
            | MediaType::Jsx
            | MediaType::Mjs
            | MediaType::Cjs
            | MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Dts
            | MediaType::Dmts
            | MediaType::Dcts
            | MediaType::Json
            | MediaType::Tsx => {}
            MediaType::Wasm
            | MediaType::SourceMap
            | MediaType::TsBuildInfo
            | MediaType::Unknown => {
              if path.extension().and_then(|s| s.to_str()) != Some("jsonc") {
                continue;
              }
            }
          }
          dir_files.insert(specifier);
        }
      }
      workspace_files.extend(dir_files);
      pending.extend(dir_subdirs.into_values());
    }
    (workspace_files, false)
  }

  fn refresh_workspace_files(&mut self) {
    let enable_settings_hash = self.config.settings.enable_settings_hash();
    if self.workspace_files_hash == enable_settings_hash {
      return;
    }
    let (workspace_files, hit_limit) = Self::walk_workspace(&self.config);
    if hit_limit {
      let document_preload_limit =
        self.config.workspace_settings().document_preload_limit;
      if document_preload_limit == 0 {
        log::debug!("Skipped document preload.");
      } else {
        lsp_warn!(
          concat!(
            "Hit the language server document preload limit of {} file system entries. ",
            "You may want to use the \"deno.enablePaths\" configuration setting to only have Deno ",
            "partially enable a workspace or increase the limit via \"deno.documentPreloadLimit\". ",
            "In cases where Deno ends up using too much memory, you may want to lower the limit."
          ),
          document_preload_limit,
        );
      }
    }
    self.workspace_files = workspace_files;
    self.workspace_files_hash = enable_settings_hash;
  }

  async fn refresh_config_tree(&mut self) {
    let mut file_fetcher = FileFetcher::new(
      self.cache.global().clone(),
      CacheSetting::RespectHeaders,
      true,
      self.http_client_provider.clone(),
      Default::default(),
      None,
    );
    file_fetcher.set_download_log_level(super::logging::lsp_log_level());
    let file_fetcher = Arc::new(file_fetcher);
    self
      .config
      .tree
      .refresh(&self.config.settings, &self.workspace_files, &file_fetcher)
      .await;
    for config_file in self.config.tree.config_files() {
      (|| {
        let compiler_options = config_file.to_compiler_options().ok()?.options;
        let jsx_import_source = compiler_options.get("jsxImportSource")?;
        let jsx_import_source = jsx_import_source.as_str()?.to_string();
        let referrer = config_file.specifier.clone();
        let specifier = format!("{jsx_import_source}/jsx-runtime");
        self.task_queue.queue_task(Box::new(|ls: LanguageServer| {
          spawn(async move {
            let specifier = {
              let inner = ls.inner.read().await;
              let resolver = inner.resolver.as_graph_resolver(Some(&referrer));
              let Ok(specifier) = resolver.resolve(
                &specifier,
                &deno_graph::Range {
                  specifier: referrer.clone(),
                  start: deno_graph::Position::zeroed(),
                  end: deno_graph::Position::zeroed(),
                },
                deno_graph::source::ResolutionMode::Types,
              ) else {
                return;
              };
              specifier
            };
            if let Err(err) = ls.cache(vec![specifier], referrer, false).await {
              lsp_warn!("{:#}", err);
            }
          });
        }));
        Some(())
      })();
    }
  }

  async fn refresh_resolver(&mut self) {
    self.resolver = Arc::new(
      LspResolver::from_config(
        &self.config,
        &self.cache,
        Some(&self.http_client_provider),
      )
      .await,
    );
  }

  async fn refresh_documents_config(&mut self) {
    self.documents.update_config(
      &self.config,
      &self.resolver,
      &self.cache,
      &self.workspace_files,
    );

    // refresh the npm specifiers because it might have discovered
    // a @types/node package and now's a good time to do that anyway
    self.refresh_npm_specifiers().await;

    self.project_changed([], true);
  }

  async fn did_open(&mut self, params: DidOpenTextDocumentParams) {
    let mark = self.performance.mark_with_args("lsp.did_open", &params);
    let Some(scheme) = params.text_document.uri.scheme() else {
      return;
    };
    if scheme.as_str() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    let language_id =
      params
        .text_document
        .language_id
        .parse()
        .unwrap_or_else(|err| {
          error!("{:#}", err);
          LanguageId::Unknown
        });
    if language_id == LanguageId::Unknown {
      lsp_warn!(
        "Unsupported language id \"{}\" received for document \"{}\".",
        params.text_document.language_id,
        params.text_document.uri.as_str()
      );
    }
    let file_referrer = Some(uri_to_url(&params.text_document.uri))
      .filter(|s| self.documents.is_valid_file_referrer(s));
    let specifier = self
      .url_map
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
    let document = self.documents.open(
      specifier.clone(),
      params.text_document.version,
      params.text_document.language_id.parse().unwrap(),
      params.text_document.text.into(),
      file_referrer,
    );
    if document.is_diagnosable() {
      self.project_changed([(document.specifier(), ChangeKind::Opened)], false);
      self.refresh_npm_specifiers().await;
      self.diagnostics_server.invalidate(&[specifier]);
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    self.performance.measure(mark);
  }

  async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
    let mark = self.performance.mark_with_args("lsp.did_change", &params);
    let specifier = self
      .url_map
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
    match self.documents.change(
      &specifier,
      params.text_document.version,
      params.content_changes,
    ) {
      Ok(document) => {
        if document.is_diagnosable() {
          let old_scopes_with_node_specifier =
            self.documents.scopes_with_node_specifier().clone();
          self.refresh_npm_specifiers().await;
          let mut config_changed = false;
          if !self
            .documents
            .scopes_with_node_specifier()
            .equivalent(&old_scopes_with_node_specifier)
          {
            config_changed = true;
          }
          self.project_changed(
            [(document.specifier(), ChangeKind::Modified)],
            config_changed,
          );
          self.diagnostics_server.invalidate(&[specifier]);
          self.send_diagnostics_update();
          self.send_testing_update();
        }
      }
      Err(err) => error!("{:#}", err),
    }
    self.performance.measure(mark);
  }

  fn did_save(&mut self, params: DidSaveTextDocumentParams) {
    let _mark = self.performance.measure_scope("lsp.did_save");
    let specifier = self
      .url_map
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
    self.documents.save(&specifier);
    if !self
      .config
      .workspace_settings_for_specifier(&specifier)
      .cache_on_save
      || !self.config.specifier_enabled(&specifier)
      || !self.diagnostics_state.has_no_cache_diagnostics(&specifier)
    {
      return;
    }
    match url_to_file_path(&specifier) {
      Ok(path) if is_importable_ext(&path) => {}
      _ => return,
    }
    self.task_queue.queue_task(Box::new(|ls: LanguageServer| {
      spawn(async move {
        if let Err(err) = ls.cache(vec![], specifier.clone(), false).await {
          lsp_warn!("Failed to cache \"{}\" on save: {:#}", &specifier, err);
        }
      });
    }));
  }

  async fn refresh_npm_specifiers(&mut self) {
    let package_reqs = self.documents.npm_reqs_by_scope();
    let resolver = self.resolver.clone();
    // spawn due to the lsp's `Send` requirement
    spawn(async move { resolver.set_npm_reqs(&package_reqs).await })
      .await
      .ok();
  }

  async fn did_close(&mut self, params: DidCloseTextDocumentParams) {
    let mark = self.performance.mark_with_args("lsp.did_close", &params);
    let Some(scheme) = params.text_document.uri.scheme() else {
      return;
    };
    if scheme.as_str() == "deno" {
      // we can ignore virtual text documents closing, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    let specifier = self
      .url_map
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
    self.diagnostics_state.clear(&specifier);
    if self.is_diagnosable(&specifier) {
      self.refresh_npm_specifiers().await;
      self.diagnostics_server.invalidate(&[specifier.clone()]);
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    self.documents.close(&specifier);
    self.project_changed([(&specifier, ChangeKind::Closed)], false);
    self.performance.measure(mark);
  }

  async fn did_change_configuration(
    &mut self,
    params: DidChangeConfigurationParams,
  ) {
    if !self.config.workspace_configuration_capable() {
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
        self.config.set_workspace_settings(settings, vec![]);
      }
    };
    self.update_debug_flag();
    self.update_global_cache().await;
    self.refresh_workspace_files();
    self.refresh_config_tree().await;
    self.update_cache();
    self.refresh_resolver().await;
    self.refresh_documents_config().await;
    self.diagnostics_server.invalidate_all();
    self.send_diagnostics_update();
    self.send_testing_update();
  }

  async fn did_change_watched_files(
    &mut self,
    params: DidChangeWatchedFilesParams,
  ) {
    let mark = self
      .performance
      .mark_with_args("lsp.did_change_watched_files", &params);

    let changes = params
      .changes
      .into_iter()
      .map(|e| (self.url_map.uri_to_specifier(&e.uri, LspUrlKind::File), e))
      .collect::<Vec<_>>();
    if changes
      .iter()
      .any(|(s, _)| self.config.tree.is_watched_file(s))
    {
      let mut deno_config_changes = IndexSet::with_capacity(changes.len());
      deno_config_changes.extend(changes.iter().filter_map(|(s, e)| {
        self.config.tree.watched_file_type(s).and_then(|t| {
          let configuration_type = match t.1 {
            ConfigWatchedFileType::DenoJson => {
              lsp_custom::DenoConfigurationType::DenoJson
            }
            ConfigWatchedFileType::PackageJson => {
              lsp_custom::DenoConfigurationType::PackageJson
            }
            _ => return None,
          };
          Some(lsp_custom::DenoConfigurationChangeEvent {
            scope_uri: url_to_uri(t.0).ok()?,
            file_uri: e.uri.clone(),
            typ: lsp_custom::DenoConfigurationChangeType::from_file_change_type(
              e.typ,
            ),
            configuration_type,
          })
        })
      }));
      self.workspace_files_hash = 0;
      self.refresh_workspace_files();
      self.refresh_config_tree().await;
      self.update_cache();
      self.refresh_resolver().await;
      self.refresh_documents_config().await;
      self.project_changed(
        changes.iter().map(|(s, _)| (s, ChangeKind::Modified)),
        false,
      );
      self.ts_server.cleanup_semantic_cache(self.snapshot()).await;
      self.diagnostics_server.invalidate_all();
      self.send_diagnostics_update();
      self.send_testing_update();
      deno_config_changes.extend(changes.iter().filter_map(|(s, e)| {
        self.config.tree.watched_file_type(s).and_then(|t| {
          let configuration_type = match t.1 {
            ConfigWatchedFileType::DenoJson => {
              lsp_custom::DenoConfigurationType::DenoJson
            }
            ConfigWatchedFileType::PackageJson => {
              lsp_custom::DenoConfigurationType::PackageJson
            }
            _ => return None,
          };
          Some(lsp_custom::DenoConfigurationChangeEvent {
            scope_uri: url_to_uri(t.0).ok()?,
            file_uri: e.uri.clone(),
            typ: lsp_custom::DenoConfigurationChangeType::from_file_change_type(
              e.typ,
            ),
            configuration_type,
          })
        })
      }));
      if !deno_config_changes.is_empty() {
        self.client.send_did_change_deno_configuration_notification(
          lsp_custom::DidChangeDenoConfigurationNotificationParams {
            changes: deno_config_changes.into_iter().collect(),
          },
        );
      }
    }
    self.performance.measure(mark);
  }

  async fn document_symbol(
    &self,
    params: DocumentSymbolParams,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    let specifier = self
      .url_map
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
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
          "Error getting document symbols for \"{}\": {:#}",
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
    let file_referrer = Some(uri_to_url(&params.text_document.uri))
      .filter(|s| self.documents.is_valid_file_referrer(s));
    let mut specifier = self
      .url_map
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
    // skip formatting any files ignored by the config file
    if !self
      .config
      .tree
      .fmt_config_for_specifier(&specifier)
      .files
      .matches_specifier(&specifier)
    {
      return Ok(None);
    }
    let document = self
      .documents
      .get_or_load(&specifier, file_referrer.as_ref());
    let Some(document) = document else {
      return Ok(None);
    };
    // Detect vendored paths. Vendor file URLs will normalize to their remote
    // counterparts, but for formatting we want to favour the file URL.
    // TODO(nayeemrmn): Implement `Document::file_resource_path()` or similar.
    if specifier.scheme() != "file"
      && params.text_document.uri.scheme().map(|s| s.as_str()) == Some("file")
    {
      specifier = uri_to_url(&params.text_document.uri);
    }
    let file_path = url_to_file_path(&specifier).map_err(|err| {
      error!("{:#}", err);
      LspError::invalid_request()
    })?;
    let mark = self.performance.mark_with_args("lsp.formatting", &params);

    // spawn a blocking task to allow doing other work while this is occurring
    let text_edits = deno_core::unsync::spawn_blocking({
      let mut fmt_options = self
        .config
        .tree
        .fmt_config_for_specifier(&specifier)
        .options
        .clone();
      fmt_options.use_tabs = Some(!params.options.insert_spaces);
      fmt_options.indent_width = Some(params.options.tab_size as u8);
      let maybe_workspace = self
        .config
        .tree
        .data_for_specifier(&specifier)
        .map(|d| &d.member_dir.workspace);
      let unstable_options = UnstableFmtOptions {
        component: maybe_workspace
          .map(|w| w.has_unstable("fmt-component"))
          .unwrap_or(false),
      };
      let document = document.clone();
      move || {
        let format_result = match document.maybe_parsed_source() {
          Some(Ok(parsed_source)) => {
            format_parsed_source(parsed_source, &fmt_options)
          }
          Some(Err(err)) => Err(anyhow!("{:#}", err)),
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
            format_file(
              &file_path,
              document.content(),
              &fmt_options,
              &unstable_options,
              None,
            )
          }
        };
        match format_result {
          Ok(Some(new_text)) => Some(text::get_edits(
            document.content(),
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
      Ok(None)
    }
  }

  async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
    let specifier = self.url_map.uri_to_specifier(
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
    let file_referrer = asset_or_doc.file_referrer();
    let hover = if let Some((_, dep, range)) = asset_or_doc
      .get_maybe_dependency(&params.text_document_position_params.position)
    {
      let dep_doc = dep
        .get_code()
        .and_then(|s| self.documents.get_or_load(s, file_referrer));
      let dep_maybe_types_dependency =
        dep_doc.as_ref().map(|d| d.maybe_types_dependency());
      let value = match (dep.maybe_code.is_none(), dep.maybe_type.is_none(), &dep_maybe_types_dependency) {
        (false, false, None) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, file_referrer),
          self.resolution_to_hover_text(&dep.maybe_type, file_referrer),
        ),
        (false, false, Some(types_dep)) if !types_dep.is_none() => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n**Types**: {}\n**Import Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, file_referrer),
          self.resolution_to_hover_text(&dep.maybe_type, file_referrer),
          self.resolution_to_hover_text(types_dep, file_referrer),
        ),
        (false, false, Some(_)) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, file_referrer),
          self.resolution_to_hover_text(&dep.maybe_type, file_referrer),
        ),
        (false, true, Some(types_dep)) if !types_dep.is_none() => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, file_referrer),
          self.resolution_to_hover_text(types_dep, file_referrer),
        ),
        (false, true, _) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, file_referrer),
        ),
        (true, false, _) => format!(
          "**Resolved Dependency**\n\n**Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_type, file_referrer),
        ),
        (true, true, _) => unreachable!("{}", json!(params)),
      };
      let value = if let Some(docs) = self.module_registry.get_hover(&dep).await
      {
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
        .get_quick_info(
          self.snapshot(),
          specifier.clone(),
          position,
          asset_or_doc.scope().cloned(),
        )
        .await?;
      maybe_quick_info.map(|qi| qi.to_hover(line_index, self))
    };
    self.performance.measure(mark);
    Ok(hover)
  }

  fn resolution_to_hover_text(
    &self,
    resolution: &Resolution,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> String {
    match resolution {
      Resolution::Ok(resolved) => {
        let specifier = &resolved.specifier;
        let format = |scheme: &str, rest: &str| -> String {
          format!("{}&#8203;{}", scheme, rest).replace('@', "&#8203;@")
        };
        match specifier.scheme() {
          "data" => "_(a data url)_".to_string(),
          "blob" => "_(a blob url)_".to_string(),
          "file" => format(
            &specifier[..url::Position::AfterScheme],
            &to_percent_decoded_str(&specifier[url::Position::AfterScheme..]),
          ),
          _ => {
            let mut result = format(
              &specifier[..url::Position::AfterScheme],
              &specifier[url::Position::AfterScheme..],
            );
            if let Ok(jsr_req_ref) =
              JsrPackageReqReference::from_specifier(specifier)
            {
              if let Some(url) = self
                .resolver
                .jsr_to_resource_url(&jsr_req_ref, file_referrer)
              {
                result = format!("{result} (<{url}>)");
              }
            }
            result
          }
        }
      }
      Resolution::Err(_) => "_[errored]_".to_string(),
      Resolution::None => "_[missing]_".to_string(),
    }
  }

  async fn code_action(
    &self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    let specifier = self
      .url_map
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
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
    let mut code_actions = CodeActionCollection::default();
    if !fixable_diagnostics.is_empty() {
      let file_diagnostics = self
        .diagnostics_server
        .get_ts_diagnostics(&specifier, asset_or_doc.document_lsp_version());
      let mut includes_no_cache = false;
      for diagnostic in &fixable_diagnostics {
        match diagnostic.source.as_deref() {
          Some("deno-ts") => {
            let code = match diagnostic.code.as_ref().unwrap() {
              NumberOrString::String(code) => match code.parse() {
                Ok(c) => c,
                Err(e) => {
                  lsp_warn!("Invalid diagnostic code {code}: {e}");
                  continue;
                }
              },
              NumberOrString::Number(code) => *code,
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
                (&self
                  .config
                  .tree
                  .fmt_config_for_specifier(&specifier)
                  .options)
                  .into(),
                tsc::UserPreferences::from_config_for_specifier(
                  &self.config,
                  &specifier,
                ),
                asset_or_doc.scope().cloned(),
              )
              .await;
            for action in actions {
              code_actions
                .add_ts_fix_action(&specifier, &action, diagnostic, self)
                .map_err(|err| {
                  error!("Unable to convert fix: {:#}", err);
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
                == Some(NumberOrString::String("not-installed-jsr".to_string()))
              || diagnostic.code
                == Some(NumberOrString::String("not-installed-npm".to_string()))
            {
              includes_no_cache = true;
            }
            code_actions
              .add_deno_fix_action(&specifier, diagnostic)
              .map_err(|err| {
                error!("{:#}", err);
                LspError::internal_error()
              })?
          }
          Some("deno-lint") => code_actions
            .add_deno_lint_actions(
              &specifier,
              diagnostic,
              asset_or_doc.document().map(|d| d.text_info()),
              asset_or_doc
                .maybe_parsed_source()
                .and_then(|r| r.as_ref().ok()),
            )
            .map_err(|err| {
              error!("Unable to fix lint error: {:#}", err);
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
    }
    if let Some(document) = asset_or_doc.document() {
      code_actions
        .add_source_actions(document, &params.range, self)
        .await;
    }
    code_actions.set_preferred_fixes();
    all_actions.extend(code_actions.get_response());

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
          &specifier,
        )),
        params.context.trigger_kind,
        only,
        asset_or_doc.scope().cloned(),
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

    let code_action_disabled_capable =
      self.config.code_action_disabled_capable();
    let actions: Vec<CodeActionOrCommand> = all_actions.into_iter().filter(|ca| {
      code_action_disabled_capable
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
          error!("Unable to decode code action data: {:#}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let scope = self
        .get_asset_or_document(&code_action_data.specifier)
        .ok()
        .and_then(|d| d.scope().cloned());
      let combined_code_actions = self
        .ts_server
        .get_combined_code_fix(
          self.snapshot(),
          &code_action_data,
          (&self
            .config
            .tree
            .fmt_config_for_specifier(&code_action_data.specifier)
            .options)
            .into(),
          tsc::UserPreferences::from_config_for_specifier(
            &self.config,
            &code_action_data.specifier,
          ),
          scope,
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
          &self.get_ts_response_import_mapper(&code_action_data.specifier),
        )
        .map_err(|err| {
          error!("Unable to remap changes: {:#}", err);
          LspError::internal_error()
        })?
      } else {
        combined_code_actions.changes
      };
      let mut code_action = params;
      code_action.edit = ts_changes_to_edit(&changes, self).map_err(|err| {
        error!("Unable to convert changes to edits: {:#}", err);
        LspError::internal_error()
      })?;
      code_action
    } else if let Some(kind_suffix) = kind
      .as_str()
      .strip_prefix(CodeActionKind::REFACTOR.as_str())
    {
      let mut code_action = params;
      let action_data: refactor::RefactorCodeActionData = from_value(data)
        .map_err(|err| {
          error!("Unable to decode code action data: {:#}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let asset_or_doc = self.get_asset_or_document(&action_data.specifier)?;
      let line_index = asset_or_doc.line_index();
      let mut refactor_edit_info = self
        .ts_server
        .get_edits_for_refactor(
          self.snapshot(),
          action_data.specifier.clone(),
          (&self
            .config
            .tree
            .fmt_config_for_specifier(&action_data.specifier)
            .options)
            .into(),
          line_index.offset_tsc(action_data.range.start)?
            ..line_index.offset_tsc(action_data.range.end)?,
          action_data.refactor_name,
          action_data.action_name,
          Some(tsc::UserPreferences::from_config_for_specifier(
            &self.config,
            &action_data.specifier,
          )),
          asset_or_doc.scope().cloned(),
        )
        .await?;
      if kind_suffix == ".rewrite.function.returnType" {
        refactor_edit_info.edits = fix_ts_import_changes(
          &action_data.specifier,
          &refactor_edit_info.edits,
          &self.get_ts_response_import_mapper(&action_data.specifier),
        )
        .map_err(|err| {
          error!("Unable to remap changes: {:#}", err);
          LspError::internal_error()
        })?
      }
      code_action.edit = refactor_edit_info.to_workspace_edit(self)?;
      code_action
    } else {
      // The code action doesn't need to be resolved
      params
    };

    self.performance.measure(mark);
    Ok(result)
  }

  pub fn get_ts_response_import_mapper(
    &self,
    file_referrer: &ModuleSpecifier,
  ) -> TsResponseImportMapper {
    TsResponseImportMapper::new(
      &self.documents,
      self
        .config
        .tree
        .data_for_specifier(file_referrer)
        // todo(dsherret): this should probably just take the resolver itself
        // as the import map is an implementation detail
        .and_then(|d| d.resolver.maybe_import_map()),
      self.resolver.as_ref(),
      file_referrer,
    )
  }

  async fn code_lens(
    &self,
    params: CodeLensParams,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    let specifier = self
      .url_map
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
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
                "Error getting test code lenses for \"{}\": {:#}",
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
          error!("Error getting code lenses for \"{}\": {:#}", specifier, err);
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
        .map_err(|err| {
          error!(
            "Error getting ts code lenses for \"{:#}\": {:#}",
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
          error!("Error resolving code lens: {:#}", err);
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
    let specifier = self.url_map.uri_to_specifier(
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
        asset_or_doc.scope().cloned(),
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
    let specifier = self.url_map.uri_to_specifier(
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
      .await
      .map_err(|err| {
        lsp_warn!("Unable to find references: {err}");
        LspError::internal_error()
      })?;

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
        results.push(reference.entry.to_location(reference_line_index, self));
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
    let specifier = self.url_map.uri_to_specifier(
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
        asset_or_doc.scope().cloned(),
      )
      .await?;

    if let Some(definition) = maybe_definition {
      let results = definition.to_definition(line_index, self);
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
    let specifier = self.url_map.uri_to_specifier(
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
        asset_or_doc.scope().cloned(),
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
    let specifier = self.url_map.uri_to_specifier(
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
      let file_referrer = asset_or_doc.file_referrer().unwrap_or(&specifier);
      response = completions::get_import_completions(
        &specifier,
        &params.text_document_position.position,
        &self.config,
        &self.client,
        &self.module_registry,
        &self.jsr_search_api,
        &self.npm_search_api,
        &self.documents,
        self.resolver.as_ref(),
        self
          .config
          .tree
          .data_for_specifier(file_referrer)
          // todo(dsherret): this should probably just take the resolver itself
          // as the import map is an implementation detail
          .and_then(|d| d.resolver.maybe_import_map()),
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
      let scope = asset_or_doc.scope();
      let maybe_completion_info = self
        .ts_server
        .get_completions(
          self.snapshot(),
          specifier.clone(),
          position,
          tsc::GetCompletionsAtPositionOptions {
            user_preferences: tsc::UserPreferences::from_config_for_specifier(
              &self.config,
              &specifier,
            ),
            trigger_character,
            trigger_kind,
          },
          (&self
            .config
            .tree
            .fmt_config_for_specifier(&specifier)
            .options)
            .into(),
          scope.cloned(),
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
          error!("{:#}", err);
          LspError::invalid_params(
            "Could not decode data field of completion item.",
          )
        })?;
      if let Some(data) = &data.tsc {
        let specifier = &data.specifier;
        let scope = self
          .get_asset_or_document(specifier)
          .ok()
          .and_then(|d| d.scope().cloned());
        let result = self
          .ts_server
          .get_completion_details(
            self.snapshot(),
            GetCompletionDetailsArgs {
              format_code_settings: Some(
                (&self.config.tree.fmt_config_for_specifier(specifier).options)
                  .into(),
              ),
              preferences: Some(
                tsc::UserPreferences::from_config_for_specifier(
                  &self.config,
                  specifier,
                ),
              ),
              ..data.into()
            },
            scope,
          )
          .await;
        match result {
          Ok(maybe_completion_info) => {
            if let Some(completion_info) = maybe_completion_info {
              completion_info
                .as_completion_item(&params, data, specifier, self)
                .map_err(|err| {
                  error!(
                    "Failed to serialize virtual_text_document response: {:#}",
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
            error!("Unable to get completion info from TypeScript: {:#}", err);
            return Ok(params);
          }
        }
      } else if let Some(url) = data.documentation {
        CompletionItem {
          documentation: self.module_registry.get_documentation(&url).await,
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
    let specifier = self.url_map.uri_to_specifier(
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
      .await
      .map_err(|err| {
        lsp_warn!("{:#}", err);
        LspError::internal_error()
      })?;

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
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
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
      .get_outlining_spans(
        self.snapshot(),
        specifier,
        asset_or_doc.scope().cloned(),
      )
      .await?;

    let response = if !outlining_spans.is_empty() {
      Some(
        outlining_spans
          .iter()
          .map(|span| {
            span.to_folding_range(
              asset_or_doc.line_index(),
              asset_or_doc.text().as_bytes(),
              self.config.line_folding_only_capable(),
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
      .uri_to_specifier(&params.item.uri, LspUrlKind::File);
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
      .await
      .map_err(|err| {
        lsp_warn!("{:#}", err);
        LspError::internal_error()
      })?;

    let maybe_root_path_owned = self
      .config
      .root_uri()
      .and_then(|uri| url_to_file_path(uri).ok());
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
      .uri_to_specifier(&params.item.uri, LspUrlKind::File);
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
        asset_or_doc.scope().cloned(),
      )
      .await?;

    let maybe_root_path_owned = self
      .config
      .root_uri()
      .and_then(|uri| url_to_file_path(uri).ok());
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
    let specifier = self.url_map.uri_to_specifier(
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
        asset_or_doc.scope().cloned(),
      )
      .await?;

    let response = if let Some(one_or_many) = maybe_one_or_many {
      let maybe_root_path_owned = self
        .config
        .root_uri()
        .and_then(|uri| url_to_file_path(uri).ok());
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
    let specifier = self.url_map.uri_to_specifier(
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
      .await
      .map_err(|err| {
        lsp_warn!("{:#}", err);
        LspError::internal_error()
      })?;

    if let Some(locations) = maybe_locations {
      let rename_locations = tsc::RenameLocations { locations };
      let workspace_edits = rename_locations
        .into_workspace_edit(&params.new_name, self)
        .map_err(|err| {
          error!("Failed to get workspace edits: {:#}", err);
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
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
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
          asset_or_doc.scope().cloned(),
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
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier) {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.semantic_tokens_full", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    if let Some(tokens) = asset_or_doc.maybe_semantic_tokens() {
      let response = if !tokens.data.is_empty() {
        Some(SemanticTokensResult::Tokens(tokens.clone()))
      } else {
        None
      };
      self.performance.measure(mark);
      return Ok(response);
    }

    let line_index = asset_or_doc.line_index();

    let semantic_classification = self
      .ts_server
      .get_encoded_semantic_classifications(
        self.snapshot(),
        specifier,
        0..line_index.text_content_length_utf16().into(),
        asset_or_doc.scope().cloned(),
      )
      .await?;

    let semantic_tokens =
      semantic_classification.to_semantic_tokens(line_index)?;

    if let Some(doc) = asset_or_doc.document() {
      doc.cache_semantic_tokens_full(semantic_tokens.clone());
    }

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
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
    if !self.is_diagnosable(&specifier) {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark_with_args("lsp.semantic_tokens_range", &params);
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    if let Some(tokens) = asset_or_doc.maybe_semantic_tokens() {
      let tokens =
        super::semantic_tokens::tokens_within_range(&tokens, params.range);
      let response = if !tokens.data.is_empty() {
        Some(SemanticTokensRangeResult::Tokens(tokens))
      } else {
        None
      };
      self.performance.measure(mark);
      return Ok(response);
    }

    let line_index = asset_or_doc.line_index();

    let semantic_classification = self
      .ts_server
      .get_encoded_semantic_classifications(
        self.snapshot(),
        specifier,
        line_index.offset_tsc(params.range.start)?
          ..line_index.offset_tsc(params.range.end)?,
        asset_or_doc.scope().cloned(),
      )
      .await?;

    let semantic_tokens =
      semantic_classification.to_semantic_tokens(line_index)?;
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
    let specifier = self.url_map.uri_to_specifier(
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
        asset_or_doc.scope().cloned(),
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
      let old_specifier = self.url_map.uri_to_specifier(
        &Uri::from_str(&rename.old_uri).unwrap(),
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
      let format_code_settings = (&self
        .config
        .tree
        .fmt_config_for_specifier(&old_specifier)
        .options)
        .into();
      changes.extend(
        self
          .ts_server
          .get_edits_for_file_rename(
            self.snapshot(),
            old_specifier,
            self.url_map.uri_to_specifier(
              &Uri::from_str(&rename.new_uri).unwrap(),
              LspUrlKind::File,
            ),
            format_code_settings,
            tsc::UserPreferences {
              allow_text_changes_in_new_files: Some(true),
              ..Default::default()
            },
          )
          .await
          .map_err(|err| {
            lsp_warn!("{:#}", err);
            LspError::internal_error()
          })?,
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
      .await
      .map_err(|err| {
        error!("{:#}", err);
        LspError::invalid_request()
      })?;

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

  fn project_changed<'a>(
    &mut self,
    modified_scripts: impl IntoIterator<Item = (&'a ModuleSpecifier, ChangeKind)>,
    config_changed: bool,
  ) {
    self.project_version += 1; // increment before getting the snapshot
    self.ts_server.project_changed(
      self.snapshot(),
      modified_scripts,
      config_changed.then(|| {
        self
          .config
          .tree
          .data_by_scope()
          .iter()
          .map(|(s, d)| (s.clone(), d.ts_config.clone()))
          .collect()
      }),
    );
  }

  fn send_diagnostics_update(&self) {
    let snapshot = DiagnosticServerUpdateMessage {
      snapshot: self.snapshot(),
      url_map: self.url_map.clone(),
    };
    if let Err(err) = self.diagnostics_server.update(snapshot) {
      error!("Cannot update diagnostics: {:#}", err);
    }
  }

  /// Send a message to the testing server to look for any changes in tests and
  /// update the client.
  fn send_testing_update(&self) {
    if let Some(testing_server) = &self.maybe_testing_server {
      if let Err(err) = testing_server.update(self.snapshot()) {
        error!("Cannot update testing server: {:#}", err);
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
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    if params.command == "deno.cache" {
      #[derive(Default, Deserialize)]
      #[serde(rename_all = "camelCase")]
      struct Options {
        #[serde(default)]
        force_global_cache: bool,
      }
      #[derive(Deserialize)]
      struct Arguments(Vec<Url>, Url, #[serde(default)] Options);
      let Arguments(specifiers, referrer, options) =
        serde_json::from_value(json!(params.arguments))
          .map_err(|err| LspError::invalid_params(err.to_string()))?;
      self
        .cache(specifiers, referrer, options.force_global_cache)
        .await
    } else if params.command == "deno.reloadImportRegistries" {
      self.inner.write().await.reload_import_registries().await
    } else {
      Ok(None)
    }
  }

  async fn initialize(
    &self,
    params: InitializeParams,
  ) -> LspResult<InitializeResult> {
    self.inner.write().await.initialize(params).await
  }

  async fn initialized(&self, _: InitializedParams) {
    self.refresh_configuration().await;
    let (registrations, http_client) = {
      let mut inner = self.inner.write().await;
      let registrations = inner.initialized().await;
      inner.task_queue.start(self.clone());
      (registrations, inner.http_client_provider.clone())
    };
    self.init_flag.raise();

    for registration in registrations {
      if let Err(err) = self
        .client
        .when_outside_lsp_lock()
        .register_capability(vec![registration])
        .await
      {
        lsp_warn!("Client errored on capabilities.\n{:#}", err);
      }
    }

    if upgrade_check_enabled() {
      let client = self.client.clone();
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

    lsp_log!("Server ready.");
  }

  async fn shutdown(&self) -> LspResult<()> {
    self.shutdown_flag.raise();
    Ok(())
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.write().await.did_open(params).await;
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.write().await.did_change(params).await
  }

  async fn did_save(&self, params: DidSaveTextDocumentParams) {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.write().await.did_save(params);
  }

  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.write().await.did_close(params).await
  }

  async fn did_change_configuration(
    &self,
    params: DidChangeConfigurationParams,
  ) {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    let mark = self
      .performance
      .mark_with_args("lsp.did_change_configuration", &params);
    self.refresh_configuration().await;
    self
      .inner
      .write()
      .await
      .did_change_configuration(params)
      .await;
    self.performance.measure(mark);
  }

  async fn did_change_watched_files(
    &self,
    params: DidChangeWatchedFilesParams,
  ) {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self
      .inner
      .write()
      .await
      .did_change_watched_files(params)
      .await
  }

  async fn did_change_workspace_folders(
    &self,
    params: DidChangeWorkspaceFoldersParams,
  ) {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    let mark = self
      .performance
      .mark_with_args("lsp.did_change_workspace_folders", &params);
    self
      .inner
      .write()
      .await
      .pre_did_change_workspace_folders(params);
    self.refresh_configuration().await;
    self
      .inner
      .write()
      .await
      .post_did_change_workspace_folders()
      .await;
    self.performance.measure(mark);
  }

  async fn document_symbol(
    &self,
    params: DocumentSymbolParams,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.document_symbol(params).await
  }

  async fn formatting(
    &self,
    params: DocumentFormattingParams,
  ) -> LspResult<Option<Vec<TextEdit>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.formatting(params).await
  }

  async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.hover(params).await
  }

  async fn inlay_hint(
    &self,
    params: InlayHintParams,
  ) -> LspResult<Option<Vec<InlayHint>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.inlay_hint(params).await
  }

  async fn code_action(
    &self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.code_action(params).await
  }

  async fn code_action_resolve(
    &self,
    params: CodeAction,
  ) -> LspResult<CodeAction> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.code_action_resolve(params).await
  }

  async fn code_lens(
    &self,
    params: CodeLensParams,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.code_lens(params).await
  }

  async fn code_lens_resolve(&self, params: CodeLens) -> LspResult<CodeLens> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.code_lens_resolve(params).await
  }

  async fn document_highlight(
    &self,
    params: DocumentHighlightParams,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.document_highlight(params).await
  }

  async fn references(
    &self,
    params: ReferenceParams,
  ) -> LspResult<Option<Vec<Location>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.references(params).await
  }

  async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
  ) -> LspResult<Option<GotoDefinitionResponse>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.goto_definition(params).await
  }

  async fn goto_type_definition(
    &self,
    params: GotoTypeDefinitionParams,
  ) -> LspResult<Option<GotoTypeDefinitionResponse>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.goto_type_definition(params).await
  }

  async fn completion(
    &self,
    params: CompletionParams,
  ) -> LspResult<Option<CompletionResponse>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.completion(params).await
  }

  async fn completion_resolve(
    &self,
    params: CompletionItem,
  ) -> LspResult<CompletionItem> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.completion_resolve(params).await
  }

  async fn goto_implementation(
    &self,
    params: GotoImplementationParams,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.goto_implementation(params).await
  }

  async fn folding_range(
    &self,
    params: FoldingRangeParams,
  ) -> LspResult<Option<Vec<FoldingRange>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.folding_range(params).await
  }

  async fn incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.incoming_calls(params).await
  }

  async fn outgoing_calls(
    &self,
    params: CallHierarchyOutgoingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.outgoing_calls(params).await
  }

  async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams,
  ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.prepare_call_hierarchy(params).await
  }

  async fn rename(
    &self,
    params: RenameParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.rename(params).await
  }

  async fn selection_range(
    &self,
    params: SelectionRangeParams,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.selection_range(params).await
  }

  async fn semantic_tokens_full(
    &self,
    params: SemanticTokensParams,
  ) -> LspResult<Option<SemanticTokensResult>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.semantic_tokens_full(params).await
  }

  async fn semantic_tokens_range(
    &self,
    params: SemanticTokensRangeParams,
  ) -> LspResult<Option<SemanticTokensRangeResult>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.semantic_tokens_range(params).await
  }

  async fn signature_help(
    &self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.signature_help(params).await
  }

  async fn will_rename_files(
    &self,
    params: RenameFilesParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.will_rename_files(params).await
  }

  async fn symbol(
    &self,
    params: WorkspaceSymbolParams,
  ) -> LspResult<Option<Vec<SymbolInformation>>> {
    if !self.init_flag.is_raised() {
      self.init_flag.wait_raised().await;
    }
    self.inner.read().await.symbol(params).await
  }
}

struct PrepareCacheResult {
  cli_options: CliOptions,
  roots: Vec<ModuleSpecifier>,
  open_docs: Vec<Arc<Document>>,
}

// These are implementations of custom commands supported by the LSP
impl Inner {
  async fn initialized(&mut self) -> Vec<Registration> {
    let mut registrations = Vec::with_capacity(2);
    init_log_file(self.config.log_file());
    self.update_debug_flag();
    self.update_global_cache().await;
    self.refresh_workspace_files();
    self.refresh_config_tree().await;
    self.update_cache();
    self.refresh_resolver().await;
    self.refresh_documents_config().await;

    if self.config.did_change_watched_files_capable() {
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
    if self.config.will_rename_files_capable() {
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

    if self.config.testing_api_capable() {
      let test_server = testing::TestServer::new(
        self.client.clone(),
        self.performance.clone(),
        self.config.root_uri().cloned(),
      );
      self.maybe_testing_server = Some(test_server);
    }

    let mut config_events = vec![];
    for (scope_url, config_data) in self.config.tree.data_by_scope().iter() {
      let Ok(scope_uri) = url_to_uri(scope_url) else {
        continue;
      };
      if let Some(config_file) = config_data.maybe_deno_json() {
        if let Ok(file_uri) = url_to_uri(&config_file.specifier) {
          config_events.push(lsp_custom::DenoConfigurationChangeEvent {
            scope_uri: scope_uri.clone(),
            file_uri,
            typ: lsp_custom::DenoConfigurationChangeType::Added,
            configuration_type: lsp_custom::DenoConfigurationType::DenoJson,
          });
        }
      }
      if let Some(package_json) = config_data.maybe_pkg_json() {
        if let Ok(file_uri) = url_to_uri(&package_json.specifier()) {
          config_events.push(lsp_custom::DenoConfigurationChangeEvent {
            scope_uri,
            file_uri,
            typ: lsp_custom::DenoConfigurationChangeType::Added,
            configuration_type: lsp_custom::DenoConfigurationType::PackageJson,
          });
        }
      }
    }
    if !config_events.is_empty() {
      self.client.send_did_change_deno_configuration_notification(
        lsp_custom::DidChangeDenoConfigurationNotificationParams {
          changes: config_events,
        },
      );
    }
    registrations
  }

  fn prepare_cache(
    &mut self,
    specifiers: Vec<ModuleSpecifier>,
    referrer: ModuleSpecifier,
    force_global_cache: bool,
  ) -> Result<PrepareCacheResult, AnyError> {
    let config_data = self.config.tree.data_for_specifier(&referrer);
    let byonm = config_data.map(|d| d.byonm).unwrap_or(false);
    let mut roots = if !specifiers.is_empty() {
      specifiers
    } else {
      vec![referrer.clone()]
    };

    if byonm {
      roots.retain(|s| s.scheme() != "npm");
    } else if let Some(npm_reqs) = self
      .documents
      .npm_reqs_by_scope()
      .get(&config_data.map(|d| d.scope.as_ref().clone()))
    {
      // always include the npm packages since resolution of one npm package
      // might affect the resolution of other npm packages
      roots.extend(
        npm_reqs
          .iter()
          .map(|req| ModuleSpecifier::parse(&format!("npm:{}", req)).unwrap()),
      );
    }

    let workspace_settings = self.config.workspace_settings();
    let initial_cwd = config_data
      .and_then(|d| d.scope.to_file_path().ok())
      .unwrap_or_else(|| self.initial_cwd.clone());
    let workspace = match config_data {
      Some(d) => d.member_dir.clone(),
      None => Arc::new(WorkspaceDirectory::discover(
        deno_config::workspace::WorkspaceDiscoverStart::Paths(&[
          initial_cwd.clone()
        ]),
        &WorkspaceDiscoverOptions {
          fs: Default::default(), // use real fs,
          deno_json_cache: None,
          pkg_json_cache: None,
          workspace_cache: None,
          config_parse_options: deno_config::deno_json::ConfigParseOptions {
            include_task_comments: false,
          },
          additional_config_file_names: &[],
          discover_pkg_json: !has_flag_env_var("DENO_NO_PACKAGE_JSON"),
          maybe_vendor_override: if force_global_cache {
            Some(deno_config::workspace::VendorEnablement::Disable)
          } else {
            None
          },
        },
      )?),
    };
    let cli_options = CliOptions::new(
      Arc::new(Flags {
        internal: InternalFlags {
          cache_path: Some(self.cache.deno_dir().root.clone()),
          ..Default::default()
        },
        ca_stores: workspace_settings.certificate_stores.clone(),
        ca_data: workspace_settings.tls_certificate.clone().map(CaData::File),
        unsafely_ignore_certificate_errors: workspace_settings
          .unsafely_ignore_certificate_errors
          .clone(),
        import_map_path: config_data.and_then(|d| {
          d.import_map_from_settings
            .as_ref()
            .map(|url| url.to_string())
        }),
        // bit of a hack to force the lsp to cache the @types/node package
        type_check_mode: crate::args::TypeCheckMode::Local,
        permissions: crate::args::PermissionFlags {
          // allow remote import permissions in the lsp for now
          allow_import: Some(vec![]),
          ..Default::default()
        },
        ..Default::default()
      }),
      initial_cwd,
      config_data.and_then(|d| d.lockfile.clone()),
      config_data
        .and_then(|d| d.npmrc.clone())
        .unwrap_or_else(create_default_npmrc),
      workspace,
      force_global_cache,
    )?;

    let open_docs = self.documents.documents(DocumentsFilter::OpenDiagnosable);
    Ok(PrepareCacheResult {
      cli_options,
      open_docs,
      roots,
    })
  }

  async fn post_cache(&mut self) {
    self.resolver.did_cache();
    self.refresh_npm_specifiers().await;
    self.diagnostics_server.invalidate_all();
    self.project_changed([], true);
    self.ts_server.cleanup_semantic_cache(self.snapshot()).await;
    self.send_diagnostics_update();
    self.send_testing_update();
  }

  fn pre_did_change_workspace_folders(
    &mut self,
    params: DidChangeWorkspaceFoldersParams,
  ) {
    let mut workspace_folders = params
      .event
      .added
      .into_iter()
      .map(|folder| {
        (
          self
            .url_map
            .uri_to_specifier(&folder.uri, LspUrlKind::Folder),
          folder,
        )
      })
      .collect::<Vec<(ModuleSpecifier, WorkspaceFolder)>>();
    for (specifier, folder) in self.config.workspace_folders.as_ref() {
      if !params.event.removed.is_empty()
        && params.event.removed.iter().any(|f| f.uri == folder.uri)
      {
        continue;
      }
      workspace_folders.push((specifier.clone(), folder.clone()));
    }
    self.config.set_workspace_folders(workspace_folders);
  }

  async fn post_did_change_workspace_folders(&mut self) {
    self.refresh_workspace_files();
    self.refresh_config_tree().await;
    self.refresh_resolver().await;
    self.refresh_documents_config().await;
    self.diagnostics_server.invalidate_all();
    self.send_diagnostics_update();
    self.send_testing_update();
  }

  fn get_performance(&self) -> Value {
    let averages = self.performance.averages();
    json!({ "averages": averages })
  }

  async fn test_run_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    if let Some(testing_server) = &self.maybe_testing_server {
      match params.map(serde_json::from_value) {
        Some(Ok(params)) => {
          testing_server
            .run_request(params, self.config.workspace_settings().clone())
            .await
        }
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      }
    } else {
      Err(LspError::invalid_request())
    }
  }

  fn test_run_cancel_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    if let Some(testing_server) = &self.maybe_testing_server {
      match params.map(serde_json::from_value) {
        Some(Ok(params)) => testing_server.run_cancel_request(params),
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      }
    } else {
      Err(LspError::invalid_request())
    }
  }

  fn task_definitions(&self) -> LspResult<Vec<TaskDefinition>> {
    let mut result = vec![];
    for config_file in self.config.tree.config_files() {
      if let Some(tasks) = json!(&config_file.json.tasks).as_object() {
        for (name, value) in tasks {
          let Some(command) = value.as_str() else {
            continue;
          };
          result.push(TaskDefinition {
            name: name.clone(),
            command: command.to_string(),
            source_uri: url_to_uri(&config_file.specifier)
              .map_err(|_| LspError::internal_error())?,
          });
        }
      };
    }
    for package_json in self.config.tree.package_jsons() {
      if let Some(scripts) = &package_json.scripts {
        for (name, command) in scripts {
          result.push(TaskDefinition {
            name: name.clone(),
            command: command.clone(),
            source_uri: url_to_uri(&package_json.specifier())
              .map_err(|_| LspError::internal_error())?,
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
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
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
          error!("Failed to convert range to text_span: {:#}", err);
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
          &specifier,
        ),
        asset_or_doc.scope().cloned(),
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
    remove_dir_all_if_exists(&self.module_registry.location)
      .await
      .map_err(|err| {
        error!("Unable to remove registries cache: {:#}", err);
        LspError::internal_error()
      })?;
    self.module_registry.clear_cache();
    self.jsr_search_api.clear_cache();
    self.npm_search_api.clear_cache();
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
      .uri_to_specifier(&params.text_document.uri, LspUrlKind::File);
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
        serde_json::to_string_pretty(&workspace_settings)
          .inspect_err(|e| {
            dbg!(e);
          })
          .unwrap(),
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

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  #[test]
  fn test_walk_workspace() {
    let temp_dir = TempDir::new();
    temp_dir.create_dir_all("root1/vendor/");
    temp_dir.write("root1/vendor/mod.ts", ""); // no, vendor

    temp_dir.create_dir_all("root1/node_modules/");
    temp_dir.write("root1/node_modules/mod.ts", ""); // no, node_modules

    temp_dir.create_dir_all("root1/folder");
    temp_dir.create_dir_all("root1/target");
    temp_dir.create_dir_all("root1/node_modules");
    temp_dir.create_dir_all("root1/.git");
    temp_dir.create_dir_all("root1/file.ts"); // no, directory
    temp_dir.write("root1/mod0.ts", ""); // yes
    temp_dir.write("root1/mod1.js", ""); // yes
    temp_dir.write("root1/mod2.tsx", ""); // yes
    temp_dir.write("root1/mod3.d.ts", ""); // yes
    temp_dir.write("root1/mod4.jsx", ""); // yes
    temp_dir.write("root1/mod5.mjs", ""); // yes
    temp_dir.write("root1/mod6.mts", ""); // yes
    temp_dir.write("root1/mod7.d.mts", ""); // yes
    temp_dir.write("root1/mod8.json", ""); // yes
    temp_dir.write("root1/mod9.jsonc", ""); // yes
    temp_dir.write("root1/other.txt", ""); // no, text file
    temp_dir.write("root1/other.wasm", ""); // no, don't load wasm
    temp_dir.write("root1/Cargo.toml", ""); // no
    temp_dir.write("root1/folder/mod.ts", ""); // yes
    temp_dir.write("root1/folder/data.min.ts", ""); // no, minified file
    temp_dir.write("root1/.git/main.ts", ""); // no, .git folder
    temp_dir.write("root1/node_modules/main.ts", ""); // no, because it's in a node_modules folder
    temp_dir.write("root1/target/main.ts", ""); // no, because there is a Cargo.toml in the root directory

    temp_dir.create_dir_all("root2/folder");
    temp_dir.create_dir_all("root2/sub_folder");
    temp_dir.create_dir_all("root2/root2.1");
    temp_dir.write("root2/file1.ts", ""); // yes, enabled
    temp_dir.write("root2/file2.ts", ""); // no, not enabled
    temp_dir.write("root2/folder/main.ts", ""); // yes, enabled
    temp_dir.write("root2/folder/other.ts", ""); // no, disabled
    temp_dir.write("root2/sub_folder/a.js", ""); // no, not enabled
    temp_dir.write("root2/sub_folder/b.ts", ""); // no, not enabled
    temp_dir.write("root2/sub_folder/c.js", ""); // no, not enabled
    temp_dir.write("root2/root2.1/main.ts", ""); // yes, enabled as separate root

    temp_dir.create_dir_all("root3/");
    temp_dir.write("root3/mod.ts", ""); // no, not enabled

    temp_dir.create_dir_all("root4_parent/root4");
    temp_dir.write("root4_parent/deno.json", ""); // yes, enabled as deno.json above root
    temp_dir.write("root4_parent/root4/main.ts", ""); // yes, enabled

    let mut config = Config::new_with_roots(vec![
      temp_dir.url().join("root1/").unwrap(),
      temp_dir.url().join("root2/").unwrap(),
      temp_dir.url().join("root2/root2.1/").unwrap(),
      temp_dir.url().join("root3/").unwrap(),
      temp_dir.url().join("root4_parent/root4/").unwrap(),
    ]);
    config.set_client_capabilities(ClientCapabilities {
      workspace: Some(Default::default()),
      ..Default::default()
    });
    config.set_workspace_settings(
      Default::default(),
      vec![
        (
          temp_dir.url().join("root1/").unwrap(),
          WorkspaceSettings {
            enable: Some(true),
            ..Default::default()
          },
        ),
        (
          temp_dir.url().join("root2/").unwrap(),
          WorkspaceSettings {
            enable: Some(true),
            enable_paths: Some(vec![
              "file1.ts".to_string(),
              "folder".to_string(),
            ]),
            disable_paths: vec!["folder/other.ts".to_string()],
            ..Default::default()
          },
        ),
        (
          temp_dir.url().join("root2/root2.1/").unwrap(),
          WorkspaceSettings {
            enable: Some(true),
            ..Default::default()
          },
        ),
        (
          temp_dir.url().join("root3/").unwrap(),
          WorkspaceSettings {
            enable: Some(false),
            ..Default::default()
          },
        ),
        (
          temp_dir.url().join("root4_parent/root4/").unwrap(),
          WorkspaceSettings {
            enable: Some(true),
            ..Default::default()
          },
        ),
      ],
    );

    let (workspace_files, hit_limit) = Inner::walk_workspace(&config);
    assert!(!hit_limit);
    assert_eq!(
      json!(workspace_files),
      json!([
        temp_dir.url().join("root4_parent/deno.json").unwrap(),
        temp_dir.url().join("root1/mod0.ts").unwrap(),
        temp_dir.url().join("root1/mod1.js").unwrap(),
        temp_dir.url().join("root1/mod2.tsx").unwrap(),
        temp_dir.url().join("root1/mod3.d.ts").unwrap(),
        temp_dir.url().join("root1/mod4.jsx").unwrap(),
        temp_dir.url().join("root1/mod5.mjs").unwrap(),
        temp_dir.url().join("root1/mod6.mts").unwrap(),
        temp_dir.url().join("root1/mod7.d.mts").unwrap(),
        temp_dir.url().join("root1/mod8.json").unwrap(),
        temp_dir.url().join("root1/mod9.jsonc").unwrap(),
        temp_dir.url().join("root2/file1.ts").unwrap(),
        temp_dir.url().join("root4_parent/root4/main.ts").unwrap(),
        temp_dir.url().join("root1/folder/mod.ts").unwrap(),
        temp_dir.url().join("root2/folder/main.ts").unwrap(),
        temp_dir.url().join("root2/root2.1/main.ts").unwrap(),
      ])
    );
  }
}
