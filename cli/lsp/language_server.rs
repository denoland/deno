// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::env;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::unsync::spawn;
use deno_core::url;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::CheckJsOption;
use deno_graph::GraphKind;
use deno_graph::Resolution;
use deno_lib::args::get_root_cert_store;
use deno_lib::args::CaData;
use deno_lib::version::DENO_VERSION_INFO;
use deno_path_util::url_to_file_path;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_semver::jsr::JsrPackageReqReference;
use indexmap::Equivalent;
use indexmap::IndexMap;
use indexmap::IndexSet;
use log::error;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use serde::Deserialize;
use serde_json::from_value;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
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
use super::cache::LspCache;
use super::capabilities;
use super::capabilities::semantic_tokens_registration_options;
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
use super::documents::Document;
use super::documents::DocumentModule;
use super::documents::DocumentModules;
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
use super::tsc::ChangeKind;
use super::tsc::TsServer;
use super::urls::uri_to_file_path;
use super::urls::uri_to_url;
use super::urls::url_to_uri;
use crate::args::Flags;
use crate::args::InternalFlags;
use crate::args::UnstableFmtOptions;
use crate::factory::CliFactory;
use crate::file_fetcher::CliFileFetcher;
use crate::graph_util;
use crate::http_util::HttpClientProvider;
use crate::lsp::config::ConfigWatchedFileType;
use crate::lsp::logging::init_log_file;
use crate::lsp::tsc::file_text_changes_to_workspace_edit;
use crate::sys::CliSys;
use crate::tools::fmt::format_file;
use crate::tools::fmt::format_parsed_source;
use crate::tools::upgrade::check_for_upgrades_for_lsp;
use crate::tools::upgrade::upgrade_check_enabled;
use crate::util::fs::remove_dir_all_if_exists;
use crate::util::path::to_percent_decoded_str;
use crate::util::sync::AsyncFlag;

struct LspRootCertStoreProvider(RootCertStore);

impl RootCertStoreProvider for LspRootCertStoreProvider {
  fn get_or_try_init(&self) -> Result<&RootCertStore, deno_error::JsErrorBox> {
    Ok(&self.0)
  }
}

/// Used as a filtering option for `Inner::get_document()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enabled {
  /// Return Ok(None) if not enabled.
  Filter,
  /// Allow even if not enabled.
  Ignore,
}

/// Used as a filtering option for `Inner::get_document()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exists {
  /// Return Err(LspError::InvalidParams(...)) if non-existent.
  Enforce,
  /// Return Ok(None) if non-existent.
  Filter,
}

/// Used as a filtering option for `Inner::get_document()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diagnosable {
  /// Return Ok(None) if not diagnosable.
  Filter,
  /// Allow even if not diagnosable.
  Ignore,
}

pub fn to_lsp_range(referrer: &deno_graph::Range) -> lsp_types::Range {
  lsp_types::Range {
    start: lsp_types::Position {
      line: referrer.range.start.line as u32,
      character: referrer.range.start.character as u32,
    },
    end: lsp_types::Position {
      line: referrer.range.end.line as u32,
      character: referrer.range.end.character as u32,
    },
  }
}

#[derive(Debug, Clone)]
pub struct LanguageServer {
  client: Client,
  pub inner: Rc<tokio::sync::RwLock<Inner>>,
  /// This is used to block out standard request handling until the complete
  /// user configuration has been fetched. This is done in the `initialized`
  /// handler which normally may occur concurrently with those other requests.
  /// TODO(nayeemrmn): This wouldn't be necessary if LSP allowed
  /// `workspace/configuration` requests in the `initialize` handler. See:
  /// https://github.com/Microsoft/language-server-protocol/issues/567#issuecomment-2085131917
  init_flag: AsyncFlag,
  performance: Arc<Performance>,
}

/// Snapshot of the state used by TSC.
#[derive(Clone, Debug, Default)]
pub struct StateSnapshot {
  pub project_version: usize,
  pub config: Arc<Config>,
  pub document_modules: DocumentModules,
  pub resolver: Arc<LspResolver>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum ProjectScopesChange {
  None,
  OpenNotebooks,
  Config,
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

pub struct Inner {
  pub cache: LspCache,
  /// The LSP client that this LSP server is connected to.
  pub client: Client,
  /// Configuration information.
  pub config: Config,
  diagnostics_state: Arc<diagnostics::DiagnosticsState>,
  diagnostics_server: diagnostics::DiagnosticsServer,
  /// The collection of documents that the server is currently handling, either
  /// on disk or "open" within the client.
  pub document_modules: DocumentModules,
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
  registered_semantic_tokens_capabilities: bool,
  pub resolver: Arc<LspResolver>,
  task_queue: LanguageServerTaskQueue,
  ts_fixable_diagnostics: tokio::sync::OnceCell<Vec<String>>,
  pub ts_server: Arc<TsServer>,
  workspace_files: Arc<IndexSet<PathBuf>>,
  /// Set to `self.config.settings.enable_settings_hash()` after
  /// refreshing `self.workspace_files`.
  workspace_files_hash: u64,
  registry_provider:
    Arc<dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync>,
  _tracing: Option<super::trace::TracingGuard>,
}

impl std::fmt::Debug for Inner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Inner")
      .field("cache", &self.cache)
      .field("client", &self.client)
      .field("config", &self.config)
      .field("diagnostics_state", &self.diagnostics_state)
      .field("diagnostics_server", &self.diagnostics_server)
      .field("document_modules", &self.document_modules)
      .field("http_client_provider", &self.http_client_provider)
      .field("initial_cwd", &self.initial_cwd)
      .field("jsr_search_api", &self.jsr_search_api)
      .field("module_registry", &self.module_registry)
      .field("maybe_testing_server", &self.maybe_testing_server)
      .field("npm_search_api", &self.npm_search_api)
      .field("project_version", &self.project_version)
      .field("performance", &self.performance)
      .field(
        "registered_semantic_tokens_capabilities",
        &self.registered_semantic_tokens_capabilities,
      )
      .field("resolver", &self.resolver)
      .field("task_queue", &self.task_queue)
      .field("ts_fixable_diagnostics", &self.ts_fixable_diagnostics)
      .field("ts_server", &self.ts_server)
      .field("workspace_files", &self.workspace_files)
      .field("workspace_files_hash", &self.workspace_files_hash)
      .field("_tracing", &self._tracing)
      .finish()
  }
}

impl LanguageServer {
  pub fn new(
    client: Client,
    registry_provider: Arc<
      dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
    >,
  ) -> Self {
    let performance = Arc::new(Performance::default());
    Self {
      client: client.clone(),
      inner: Rc::new(tokio::sync::RwLock::new(Inner::new(
        client,
        performance.clone(),
        registry_provider,
      ))),
      init_flag: Default::default(),
      performance,
    }
  }

  /// Similar to `deno install --entrypoint` on the command line, where modules will be cached
  /// in the Deno cache, including any of their dependencies.
  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub async fn cache(
    &self,
    specifiers: Vec<ModuleSpecifier>,
    referrer: ModuleSpecifier,
    force_global_cache: bool,
  ) -> LspResult<Option<Value>> {
    async fn create_graph_for_caching(
      factory: CliFactory,
      roots: Vec<ModuleSpecifier>,
      open_modules: Vec<Arc<DocumentModule>>,
    ) -> Result<(), AnyError> {
      let open_modules = open_modules
        .into_iter()
        .map(|m| (m.specifier.clone(), m))
        .collect::<HashMap<_, _>>();
      let module_graph_builder = factory.module_graph_builder().await?;
      let module_graph_creator = factory.module_graph_creator().await?;
      let mut inner_loader = module_graph_builder.create_graph_loader();
      let mut loader = crate::lsp::documents::OpenDocumentsGraphLoader {
        inner_loader: &mut inner_loader,
        open_modules: &open_modules,
      };
      let graph = module_graph_creator
        .create_graph_with_loader(
          GraphKind::All,
          roots.clone(),
          &mut loader,
          graph_util::NpmCachingStrategy::Eager,
        )
        .await?;
      graph_util::graph_valid(
        &graph,
        &CliSys::default(),
        &roots,
        graph_util::GraphValidOptions {
          kind: GraphKind::All,
          check_js: CheckJsOption::False,
          exit_integrity_errors: false,
          allow_unknown_media_types: true,
        },
      )?;

      // Update the lockfile on the file system with anything new
      // found after caching
      if let Ok(Some(lockfile)) = factory.maybe_lockfile().await {
        if let Err(err) = &lockfile.write_if_changed() {
          lsp_warn!("{:#}", err);
        }
      }

      Ok(())
    }

    self.init_flag.wait_raised().await;

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
        let cli_factory = result.cli_factory;
        let roots = result.roots;
        let open_modules = result.open_modules;
        let handle = spawn(async move {
          create_graph_for_caching(cli_factory, roots, open_modules).await
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
    _token: CancellationToken,
  ) -> LspResult<Option<Value>> {
    self.init_flag.wait_raised().await;
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

  pub async fn performance_request(
    &self,
    _token: CancellationToken,
  ) -> LspResult<Option<Value>> {
    self.init_flag.wait_raised().await;
    Ok(Some(self.inner.read().await.get_performance()))
  }

  pub async fn task_definitions(
    &self,
    _token: CancellationToken,
  ) -> LspResult<Vec<TaskDefinition>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.task_definitions()
  }

  pub async fn test_run_request(
    &self,
    params: Option<Value>,
    _token: CancellationToken,
  ) -> LspResult<Option<Value>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.test_run_request(params).await
  }

  pub async fn test_run_cancel_request(
    &self,
    params: Option<Value>,
    _token: CancellationToken,
  ) -> LspResult<Option<Value>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.test_run_cancel_request(params)
  }

  pub async fn virtual_text_document(
    &self,
    params: Option<Value>,
    _token: CancellationToken,
  ) -> LspResult<Option<Value>> {
    self.init_flag.wait_raised().await;
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
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
  fn new(
    client: Client,
    performance: Arc<Performance>,
    registry_provider: Arc<
      dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
    >,
  ) -> Self {
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
    let config = Config::default();
    let ts_server = Arc::new(TsServer::new(performance.clone()));
    let diagnostics_state = Arc::new(DiagnosticsState::default());
    let diagnostics_server = DiagnosticsServer::new(
      client.clone(),
      performance.clone(),
      ts_server.clone(),
      diagnostics_state.clone(),
    );
    let initial_cwd = std::env::current_dir().unwrap_or_else(|_| {
      panic!("Could not resolve current working directory")
    });

    Self {
      cache,
      client,
      config,
      diagnostics_state,
      diagnostics_server,
      document_modules: Default::default(),
      http_client_provider,
      initial_cwd: initial_cwd.clone(),
      jsr_search_api,
      project_version: 0,
      task_queue: Default::default(),
      maybe_testing_server: None,
      module_registry,
      npm_search_api,
      performance,
      registered_semantic_tokens_capabilities: false,
      resolver: Default::default(),
      ts_fixable_diagnostics: Default::default(),
      ts_server,
      workspace_files: Default::default(),
      workspace_files_hash: 0,
      _tracing: Default::default(),
      registry_provider,
    }
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub fn get_document(
    &self,
    uri: &Uri,
    enabled: Enabled,
    exists: Exists,
    diagnosable: Diagnosable,
  ) -> LspResult<Option<Document>> {
    match enabled {
      Enabled::Filter => {
        if !self.config.uri_enabled(uri) {
          return Ok(None);
        }
      }
      Enabled::Ignore => {}
    }
    let Some(document) = self.document_modules.documents.get(uri) else {
      match exists {
        Exists::Enforce
          if !uri.scheme().is_some_and(|s| s.eq_lowercase("deno")) =>
        {
          return Err(LspError::invalid_params(format!(
            "Unable to find document for: {}",
            uri.as_str()
          )));
        }
        Exists::Filter => {
          return Ok(None);
        }
        _ => return Ok(None),
      }
    };
    match diagnosable {
      Diagnosable::Filter => {
        if !document.is_diagnosable() {
          return Ok(None);
        }
      }
      Diagnosable::Ignore => {}
    }
    Ok(Some(document))
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub fn get_primary_module(
    &self,
    document: &Document,
  ) -> LspResult<Option<Arc<DocumentModule>>> {
    let Some(module) = self.document_modules.primary_module(document) else {
      if document
        .uri()
        .scheme()
        .is_some_and(|s| s.eq_lowercase("deno"))
      {
        return Ok(None);
      }
      lsp_warn!(
        "Unable to get module for document: {}",
        document.uri().as_str(),
      );
      return Err(LspError::internal_error());
    };
    Ok(Some(module))
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub async fn get_navigation_tree(
    &self,
    module: &DocumentModule,
    token: &CancellationToken,
  ) -> LspResult<Arc<tsc::NavigationTree>> {
    let mark = self.performance.mark_with_args(
      "lsp.get_navigation_tree",
      json!({ "uri": &module.specifier }),
    );
    let result = module
      .navigation_tree
      .get_or_try_init(|| async {
        self
          .ts_server
          .get_navigation_tree(
            self.snapshot(),
            &module.specifier,
            module.scope.as_ref(),
            module.notebook_uri.as_ref(),
            token,
          )
          .await
          .map(Arc::new)
          .map_err(|err| {
            if token.is_cancelled() {
              LspError::request_cancelled()
            } else {
              error!(
                "Error getting navigation tree for \"{}\": {:#}",
                &module.specifier, err,
              );
              LspError::internal_error()
            }
          })
      })
      .await
      .map(Clone::clone);
    self.performance.measure(mark);
    result
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub fn snapshot(&self) -> Arc<StateSnapshot> {
    Arc::new(StateSnapshot {
      project_version: self.project_version,
      config: Arc::new(self.config.clone()),
      document_modules: self.document_modules.clone(),
      resolver: self.resolver.snapshot(),
    })
  }

  pub async fn ts_fixable_diagnostics(&self) -> &Vec<String> {
    self
      .ts_fixable_diagnostics
      .get_or_init(|| async {
        self
          .ts_server
          .get_supported_code_fixes(self.snapshot())
          .await
          .unwrap()
      })
      .await
  }

  pub fn update_tracing(&mut self) {
    let tracing =
      self
        .config
        .workspace_settings()
        .tracing
        .clone()
        .or_else(|| {
          std::env::var("DENO_LSP_TRACE").ok().map(|_| {
            super::trace::TracingConfig {
              enable: true,
              ..Default::default()
            }
            .into()
          })
        });
    self
      .ts_server
      .set_tracing_enabled(tracing.as_ref().is_some_and(|t| t.enabled()));
    self._tracing = tracing.and_then(|conf| {
      if !conf.enabled() {
        return None;
      }
      lsp_log!("Initializing tracing subscriber: {:#?}", conf);
      let config = conf.into();
      super::trace::init_tracing_subscriber(&config)
        .inspect_err(|e| {
          lsp_warn!("Error initializing tracing subscriber: {e:#}");
        })
        .ok()
    });
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub async fn update_global_cache(&mut self) {
    let mark = self.performance.mark("lsp.update_global_cache");
    let maybe_cache = self.config.workspace_settings().cache.as_ref();
    let global_cache_url = maybe_cache.and_then(|cache_str| {
      if let Ok(url) = Url::from_file_path(cache_str) {
        Some(url)
      } else if let Some(root_uri) = self.config.root_url() {
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
      .root_url()
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub fn update_cache(&mut self) {
    let mark = self.performance.mark("lsp.update_cache");
    self.cache.update_config(&self.config);
    self.performance.measure(mark);
  }

  pub fn update_debug_flag(&self) {
    let internal_debug = self.config.workspace_settings().internal_debug;
    super::logging::set_lsp_debug_flag(internal_debug)
  }

  pub fn check_semantic_tokens_capabilities(&mut self) {
    if self.registered_semantic_tokens_capabilities {
      return;
    }
    if !self
      .config
      .client_capabilities
      .text_document
      .as_ref()
      .and_then(|t| t.semantic_tokens.as_ref())
      .and_then(|s| s.dynamic_registration)
      .unwrap_or_default()
    {
      return;
    }
    let exists_enabled_document = self
      .document_modules
      .documents
      .open_docs()
      .any(|doc| doc.is_diagnosable() && self.config.uri_enabled(&doc.uri));
    if !exists_enabled_document {
      return;
    }
    self.task_queue.queue_task(Box::new(|ls| {
      spawn(async move {
        let register_options =
          serde_json::to_value(semantic_tokens_registration_options()).unwrap();
        ls.client.when_outside_lsp_lock().register_capability(vec![Registration {
          id: "textDocument/semanticTokens".to_string(),
          method: "textDocument/semanticTokens".to_string(),
          register_options: Some(register_options.clone()),
        }]).await.inspect_err(|err| {
          lsp_warn!("Couldn't register capability for \"textDocument/semanticTokens\": {err}");
        }).ok();
      });
    }));
    self.registered_semantic_tokens_capabilities = true;
  }
}

// lspower::LanguageServer methods. This file's LanguageServer delegates to us.
impl Inner {
  fn initialize(
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
      DENO_VERSION_INFO.deno,
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
            let mut url = uri_to_url(&folder.uri);
            if !url.path().ends_with('/') {
              if let Ok(mut path_segments) = url.path_segments_mut() {
                path_segments.push("");
              }
            }
            (Arc::new(url), folder)
          })
          .collect();
      }
      // rootUri is deprecated by the LSP spec. If it's specified, merge it into
      // workspace_folders.
      #[allow(deprecated)]
      if let Some(root_uri) = params.root_uri {
        if !workspace_folders.iter().any(|(_, f)| f.uri == root_uri) {
          let mut root_url = uri_to_url(&root_uri);
          let name = root_url
            .path_segments()
            .and_then(|s| s.last())
            .unwrap_or_default()
            .to_string();
          if !root_url.path().ends_with('/') {
            if let Ok(mut path_segments) = root_url.path_segments_mut() {
              path_segments.push("");
            }
          }
          workspace_folders.insert(
            0,
            (
              Arc::new(root_url),
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
    self
      .ts_server
      .set_inspector_server_addr(self.config.internal_inspect().to_address());

    self.update_tracing();
    self.update_debug_flag();

    if capabilities.semantic_tokens_provider.is_some() {
      self.registered_semantic_tokens_capabilities = true;
    }

    self.performance.measure(mark);
    Ok(InitializeResult {
      capabilities,
      server_info: Some(server_info),
      offset_encoding: None,
    })
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  fn walk_workspace(config: &Config) -> (IndexSet<PathBuf>, bool) {
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
      .filter_map(|(_, f)| uri_to_file_path(&f.uri).ok())
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
          workspace_files.insert(path);
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
        let Ok(file_type) = entry.file_type() else {
          continue;
        };
        let Some(file_name) = path.file_name() else {
          continue;
        };
        if config.settings.path_enabled(&path) == Some(false) {
          continue;
        }
        if file_type.is_dir() {
          let dir_name = file_name.to_string_lossy().to_lowercase();
          // We ignore these directories by default because there is a
          // high likelihood they aren't relevant. Someone can opt-into
          // them by specifying one of them as an enabled path.
          if matches!(
            dir_name.as_str(),
            "vendor" | "coverage" | "node_modules" | ".git"
          ) {
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
            dir_subdirs.insert(path, read_dir);
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
          let media_type = MediaType::from_path(&path);
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
            | MediaType::Css
            | MediaType::Html
            | MediaType::Sql
            | MediaType::Unknown => {
              if path.extension().and_then(|s| s.to_str()) != Some("jsonc") {
                continue;
              }
            }
          }
          dir_files.insert(path);
        }
      }
      workspace_files.extend(dir_files);
      pending.extend(dir_subdirs.into_iter());
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
    self.workspace_files = Arc::new(workspace_files);
    self.workspace_files_hash = enable_settings_hash;
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn refresh_config_tree(&mut self) {
    let file_fetcher = CliFileFetcher::new(
      self.cache.global().clone(),
      self.http_client_provider.clone(),
      CliSys::default(),
      Default::default(),
      None,
      true,
      CacheSetting::RespectHeaders,
      super::logging::lsp_log_level(),
    );
    let file_fetcher = Arc::new(file_fetcher);
    self
      .config
      .tree
      .refresh(
        &self.config.settings,
        &self.workspace_files,
        &file_fetcher,
        self.cache.deno_dir(),
        &self.registry_provider,
      )
      .await;
    self
      .client
      .send_did_refresh_deno_configuration_tree_notification(
        self.config.tree.to_did_refresh_params(),
      );
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
              let scoped_resolver =
                inner.resolver.get_scoped_resolver(Some(&referrer));
              let resolver = scoped_resolver.as_cli_resolver();
              let Ok(specifier) = resolver.resolve(
                &specifier,
                &referrer,
                deno_graph::Position::zeroed(),
                ResolutionMode::Import,
                NodeResolutionKind::Types,
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn refresh_documents_config(&mut self) {
    self.document_modules.update_config(
      &self.config,
      &self.resolver,
      &self.cache,
      &self.workspace_files,
    );

    // refresh the npm specifiers because it might have discovered
    // a @types/node package and now's a good time to do that anyway
    self.refresh_dep_info().await;

    self.project_changed([], ProjectScopesChange::Config);
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn did_open(&mut self, params: DidOpenTextDocumentParams) {
    let mark = self.performance.mark_with_args("lsp.did_open", &params);
    // `deno:` documents are read-only and should only be handled as server
    // documents.
    if params
      .text_document
      .uri
      .scheme()
      .is_some_and(|s| s.eq_lowercase("deno"))
    {
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
    let document = self.document_modules.open_document(
      params.text_document.uri,
      params.text_document.version,
      language_id,
      params.text_document.text.into(),
      None,
    );
    if document.is_diagnosable() {
      self.check_semantic_tokens_capabilities();
      self.refresh_dep_info().await;
      self.project_changed(
        self
          .document_modules
          .primary_specifier(&Document::Open(document.clone()))
          .map(|s| (s, ChangeKind::Opened)),
        ProjectScopesChange::None,
      );
      self.diagnostics_server.invalidate(&[document.uri.as_ref()]);
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    self.performance.measure(mark);
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
    let mark = self.performance.mark_with_args("lsp.did_change", &params);
    // `deno:` documents are read-only and should only be handled as server
    // documents.
    if params
      .text_document
      .uri
      .scheme()
      .is_some_and(|s| s.eq_lowercase("deno"))
    {
      return;
    }
    let document = match self.document_modules.change_document(
      &params.text_document.uri,
      params.text_document.version,
      params.content_changes,
    ) {
      Ok(doc) => doc,
      Err(err) => {
        error!("{:#}", err);
        return;
      }
    };
    if document.is_diagnosable() {
      let old_scopes_with_node_specifier =
        self.document_modules.scopes_with_node_specifier();
      self.refresh_dep_info().await;
      let mut config_changed = false;
      if !self
        .document_modules
        .scopes_with_node_specifier()
        .equivalent(&old_scopes_with_node_specifier)
      {
        config_changed = true;
      }
      self.project_changed(
        self
          .document_modules
          .primary_specifier(&Document::Open(document.clone()))
          .map(|s| (s, ChangeKind::Modified)),
        if config_changed {
          ProjectScopesChange::Config
        } else {
          ProjectScopesChange::None
        },
      );
      self.diagnostics_server.invalidate(&[&document.uri]);
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    self.performance.measure(mark);
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  fn did_save(&mut self, params: DidSaveTextDocumentParams) {
    let _mark = self.performance.measure_scope("lsp.did_save");
    let Ok(Some(document)) = self
      .get_document(
        &params.text_document.uri,
        Enabled::Filter,
        Exists::Enforce,
        Diagnosable::Filter,
      )
      .inspect_err(|err| {
        lsp_warn!("{:#}", err);
      })
    else {
      return;
    };
    if !self
      .config
      .workspace_settings_for_uri(document.uri())
      .cache_on_save
      || !self.config.uri_enabled(document.uri())
      || !self
        .diagnostics_state
        .has_no_cache_diagnostics(document.uri())
    {
      return;
    }
    let Ok(Some(module)) =
      self.get_primary_module(&document).inspect_err(|err| {
        lsp_warn!("{:#}", err);
      })
    else {
      return;
    };
    if module.specifier.scheme() != "file"
      || module.specifier.as_str().contains("/node_modules/")
    {
      return;
    }
    self.task_queue.queue_task(Box::new(|ls: LanguageServer| {
      spawn(async move {
        if let Err(err) = ls
          .cache(vec![], module.specifier.as_ref().clone(), false)
          .await
        {
          lsp_warn!(
            "Failed to cache \"{}\" on save: {:#}",
            &module.specifier,
            err
          );
        }
      });
    }));
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn refresh_dep_info(&mut self) {
    let dep_info_by_scope = self.document_modules.dep_info_by_scope();
    self
      .resolver
      .set_dep_info_by_scope(&dep_info_by_scope)
      .await;
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn did_close(&mut self, params: DidCloseTextDocumentParams) {
    let mark = self.performance.mark_with_args("lsp.did_close", &params);
    // `deno:` documents are read-only and should only be handled as server
    // documents.
    if params
      .text_document
      .uri
      .scheme()
      .is_some_and(|s| s.eq_lowercase("deno"))
    {
      return;
    }
    self.diagnostics_state.clear(&params.text_document.uri);
    let document = match self
      .document_modules
      .close_document(&params.text_document.uri)
    {
      Ok(doc) => doc,
      Err(err) => {
        error!("{:#}", err);
        return;
      }
    };
    if document.is_diagnosable() {
      self.refresh_dep_info().await;
      let changed_specifier = self
        .document_modules
        .primary_specifier(&Document::Open(document.clone()))
        .map(|s| (s, ChangeKind::Closed));
      // Invalidate the weak references of `document` before calling
      // `self.project_changed()` so its module entries will be dropped.
      drop(document);
      self.project_changed(changed_specifier, ProjectScopesChange::None);
      self
        .diagnostics_server
        .invalidate(&[&params.text_document.uri]);
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    self.performance.measure(mark);
  }

  async fn notebook_did_open(&mut self, params: DidOpenNotebookDocumentParams) {
    let _mark = self.performance.measure_scope("lsp.notebook_did_open");
    let documents = self.document_modules.open_notebook_document(
      params.notebook_document.uri,
      params.cell_text_documents,
    );
    let diagnosable_documents = documents
      .iter()
      .filter(|d| d.is_diagnosable())
      .collect::<Vec<_>>();
    if !diagnosable_documents.is_empty() {
      self.check_semantic_tokens_capabilities();
      self.refresh_dep_info().await;
      self.project_changed(
        diagnosable_documents
          .iter()
          .flat_map(|d| {
            let specifier = self
              .document_modules
              .primary_specifier(&Document::Open((*d).clone()))?;
            Some((specifier, ChangeKind::Closed))
          })
          .collect::<Vec<_>>(),
        ProjectScopesChange::OpenNotebooks,
      );
      self.diagnostics_server.invalidate(
        &diagnosable_documents
          .iter()
          .map(|d| d.uri.as_ref())
          .collect::<Vec<_>>(),
      );
      self.send_diagnostics_update();
    }
  }

  async fn notebook_did_change(
    &mut self,
    params: DidChangeNotebookDocumentParams,
  ) {
    let _mark = self.performance.measure_scope("lsp.notebook_did_change");
    let Some(cells) = params.change.cells else {
      return;
    };
    let documents = self.document_modules.change_notebook_document(
      &params.notebook_document.uri,
      cells.structure,
      cells.text_content,
    );
    let diagnosable_documents = documents
      .iter()
      .filter(|(d, _)| d.is_diagnosable())
      .collect::<Vec<_>>();
    if !diagnosable_documents.is_empty() {
      let old_scopes_with_node_specifier =
        self.document_modules.scopes_with_node_specifier();
      self.refresh_dep_info().await;
      let mut config_changed = false;
      if !self
        .document_modules
        .scopes_with_node_specifier()
        .equivalent(&old_scopes_with_node_specifier)
      {
        config_changed = true;
      }
      self.project_changed(
        diagnosable_documents
          .iter()
          .flat_map(|(d, k)| {
            let specifier = self
              .document_modules
              .primary_specifier(&Document::Open(d.clone()))?;
            Some((specifier, *k))
          })
          .collect::<Vec<_>>(),
        if config_changed {
          ProjectScopesChange::Config
        } else {
          ProjectScopesChange::None
        },
      );
      self.diagnostics_server.invalidate(
        &diagnosable_documents
          .iter()
          .map(|(d, _)| d.uri.as_ref())
          .collect::<Vec<_>>(),
      );
      self.send_diagnostics_update();
    }
  }

  fn notebook_did_save(&mut self, params: DidSaveNotebookDocumentParams) {
    let _mark = self.performance.measure_scope("lsp.notebook_did_save");
    let Some(cell_uris) = self
      .document_modules
      .documents
      .cells_by_notebook_uri()
      .get(&params.notebook_document.uri)
      .cloned()
    else {
      lsp_warn!(
        "The URI \"{}\" does not refer to an open notebook document.",
        params.notebook_document.uri.as_str()
      );
      return;
    };
    for cell_uri in cell_uris {
      self.did_save(DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier {
          uri: cell_uri.as_ref().clone(),
        },
        text: None,
      });
    }
  }

  async fn notebook_did_close(
    &mut self,
    params: DidCloseNotebookDocumentParams,
  ) {
    let _mark = self.performance.measure_scope("lsp.notebook_did_close");
    let documents = self
      .document_modules
      .close_notebook_document(&params.notebook_document.uri);
    let diagnosable_documents = documents
      .iter()
      .filter(|d| d.is_diagnosable())
      .collect::<Vec<_>>();
    if !diagnosable_documents.is_empty() {
      self.refresh_dep_info().await;
      self.project_changed(
        diagnosable_documents
          .iter()
          .flat_map(|d| {
            let specifier = self
              .document_modules
              .primary_specifier(&Document::Open((*d).clone()))?;
            Some((specifier, ChangeKind::Closed))
          })
          .collect::<Vec<_>>(),
        ProjectScopesChange::OpenNotebooks,
      );
      self.diagnostics_server.invalidate(
        &diagnosable_documents
          .iter()
          .map(|d| d.uri.as_ref())
          .collect::<Vec<_>>(),
      );
      self.send_diagnostics_update();
    }
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
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
    // TODO(nathanwhit): allow updating after startup, needs work to set thread local collector on tsc thread
    // self.update_tracing();
    self.check_semantic_tokens_capabilities();
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip(self)))]
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
      .map(|e| (uri_to_url(&e.uri), e))
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
        changes
          .iter()
          .filter_map(|(_, e)| {
            let document = self.document_modules.documents.inspect(&e.uri)?;
            if !document.is_diagnosable() {
              return None;
            }
            let specifier =
              self.document_modules.primary_specifier(&document)?;
            Some((specifier, ChangeKind::Modified))
          })
          .collect::<Vec<_>>(),
        ProjectScopesChange::None,
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn document_symbol(
    &self,
    params: DocumentSymbolParams,
    token: &CancellationToken,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    let mark = self
      .performance
      .mark_with_args("lsp.document_symbol", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let navigation_tree = self.get_navigation_tree(&module, token).await?;
    let response = if let Some(child_items) = &navigation_tree.child_items {
      let mut document_symbols = Vec::<DocumentSymbol>::new();
      for item in child_items {
        if token.is_cancelled() {
          return Err(LspError::request_cancelled());
        }
        item.collect_document_symbols(
          module.line_index.clone(),
          &mut document_symbols,
        );
      }
      Some(DocumentSymbolResponse::Nested(document_symbols))
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn formatting(
    &self,
    params: DocumentFormattingParams,
    _token: &CancellationToken,
  ) -> LspResult<Option<Vec<TextEdit>>> {
    let mark = self.performance.mark_with_args("lsp.formatting", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Ignore,
      Exists::Enforce,
      Diagnosable::Ignore,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let fmt_config =
      self.config.tree.fmt_config_for_specifier(&module.specifier);
    // Untitled files are exempt from enabled-checks because they tend not to
    // have meaningful paths, and they won't be auto-formatted on save anyway.
    let is_untitled = params
      .text_document
      .uri
      .scheme()
      .is_some_and(|s| s.eq_lowercase("untitled"));
    if !is_untitled && !fmt_config.files.matches_specifier(&module.specifier) {
      return Ok(None);
    }
    let file_path = url_to_file_path(&module.specifier)
      .or_else(|_| uri_to_file_path(document.uri()))
      .map_err(|err| {
        lsp_warn!("{:#}", err);
        LspError::invalid_request()
      })?;

    // spawn a blocking task to allow doing other work while this is occurring
    let text_edits = deno_core::unsync::spawn_blocking({
      let mut fmt_options = fmt_config.options.clone();
      let config_data = self.config.tree.data_for_specifier(&module.specifier);
      #[allow(clippy::nonminimal_bool)] // clippy's suggestion is more confusing
      if !config_data.is_some_and(|d| d.maybe_deno_json().is_some()) {
        fmt_options.use_tabs = Some(!params.options.insert_spaces);
        fmt_options.indent_width = Some(params.options.tab_size as u8);
      }
      let unstable_options = UnstableFmtOptions {
        component: config_data
          .map(|d| d.unstable.contains("fmt-component"))
          .unwrap_or(false),
        sql: config_data
          .map(|d| d.unstable.contains("fmt-sql"))
          .unwrap_or(false),
      };
      move || {
        let format_result = match &module
          .open_data
          .as_ref()
          .and_then(|d| d.parsed_source.as_ref())
        {
          Some(Ok(parsed_source)) => {
            format_parsed_source(parsed_source, &fmt_options)
          }
          Some(Err(err)) => Err(anyhow!("{:#}", err)),
          None => {
            let ext = document.open().and_then(|d| {
              d.language_id.as_extension().map(|s| s.to_string())
            });
            format_file(
              &file_path,
              &document.text(),
              &fmt_options,
              &unstable_options,
              ext,
            )
          }
        };
        match format_result {
          Ok(Some(new_text)) => Some(text::get_edits(
            &document.text(),
            &new_text,
            &module.line_index,
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn hover(
    &self,
    params: HoverParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Hover>> {
    let mark = self.performance.mark_with_args("lsp.hover", &params);
    let Some(document) = self.get_document(
      &params.text_document_position_params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let hover = if let Some((_, dep, range)) = module
      .dependency_at_position(&params.text_document_position_params.position)
    {
      let dep_module = dep.get_code().and_then(|s| {
        self
          .document_modules
          .inspect_module_for_specifier(s, module.scope.as_deref())
      });
      let dep_types_dependency = dep_module.as_ref().map(|m| {
        m.types_dependency
          .as_ref()
          .map(|d| &d.dependency)
          .unwrap_or(&Resolution::None)
      });
      let value = match (dep.maybe_code.is_none(), dep.maybe_type.is_none(), &dep_types_dependency) {
        (false, false, None) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, module.scope.as_deref()),
          self.resolution_to_hover_text(&dep.maybe_type, module.scope.as_deref()),
        ),
        (false, false, Some(types_dep)) if !types_dep.is_none() => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n**Types**: {}\n**Import Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, module.scope.as_deref()),
          self.resolution_to_hover_text(&dep.maybe_type, module.scope.as_deref()),
          self.resolution_to_hover_text(types_dep, module.scope.as_deref()),
        ),
        (false, false, Some(_)) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, module.scope.as_deref()),
          self.resolution_to_hover_text(&dep.maybe_type, module.scope.as_deref()),
        ),
        (false, true, Some(types_dep)) if !types_dep.is_none() => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, module.scope.as_deref()),
          self.resolution_to_hover_text(types_dep, module.scope.as_deref()),
        ),
        (false, true, _) => format!(
          "**Resolved Dependency**\n\n**Code**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_code, module.scope.as_deref()),
        ),
        (true, false, _) => format!(
          "**Resolved Dependency**\n\n**Types**: {}\n",
          self.resolution_to_hover_text(&dep.maybe_type, module.scope.as_deref()),
        ),
        (true, true, _) => unreachable!("{}", json!(params)),
      };
      let value = if let Some(docs) = self.module_registry.get_hover(dep).await
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
        range: Some(to_lsp_range(range)),
      })
    } else {
      let position = module
        .line_index
        .offset_tsc(params.text_document_position_params.position)?;
      let maybe_quick_info = self
        .ts_server
        .get_quick_info(
          self.snapshot(),
          &module.specifier,
          position,
          module.scope.as_ref(),
          module.notebook_uri.as_ref(),
          token,
        )
        .await
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            error!("Unable to get quick info from TypeScript: {:#}", err);
            LspError::internal_error()
          }
        })?;
      maybe_quick_info.map(|qi| qi.to_hover(&module, self))
    };
    self.performance.measure(mark);
    Ok(hover)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]

  fn resolution_to_hover_text(
    &self,
    resolution: &Resolution,
    scope: Option<&Url>,
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
              let scoped_resolver = self.resolver.get_scoped_resolver(scope);
              if let Some(url) =
                scoped_resolver.jsr_to_resource_url(&jsr_req_ref)
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn code_action(
    &self,
    params: CodeActionParams,
    token: &CancellationToken,
  ) -> LspResult<Option<CodeActionResponse>> {
    let mark = self.performance.mark_with_args("lsp.code_action", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };

    let mut all_actions = CodeActionResponse::new();

    // QuickFix
    let ts_fixable_diagnosics = self.ts_fixable_diagnostics().await;
    let fixable_diagnostics: Vec<&Diagnostic> = params
      .context
      .diagnostics
      .iter()
      .filter(|d| match &d.source {
        Some(source) => match source.as_str() {
          "deno-ts" => match &d.code {
            Some(NumberOrString::String(code)) => {
              ts_fixable_diagnosics.contains(code)
            }
            Some(NumberOrString::Number(code)) => {
              ts_fixable_diagnosics.contains(&code.to_string())
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
        .get_ts_diagnostics(document.uri(), document.open().map(|d| d.version));
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
                &module.specifier,
                module.line_index.offset_tsc(diagnostic.range.start)?
                  ..module.line_index.offset_tsc(diagnostic.range.end)?,
                codes,
                (&self
                  .config
                  .tree
                  .fmt_config_for_specifier(&module.specifier)
                  .options)
                  .into(),
                tsc::UserPreferences::from_config_for_specifier(
                  &self.config,
                  &module.specifier,
                ),
                module.scope.as_ref(),
                module.notebook_uri.as_ref(),
                token,
              )
              .await
              .unwrap_or_else(|err| {
                // sometimes tsc reports errors when retrieving code actions
                // because they don't reflect the current state of the document
                // so we will log them to the output, but we won't send an error
                // message back to the client.
                if !token.is_cancelled() {
                  error!(
                    "Unable to get code actions from TypeScript: {:#}",
                    err
                  );
                }
                vec![]
              });
            for action in actions {
              if token.is_cancelled() {
                return Err(LspError::request_cancelled());
              }
              code_actions
                .add_ts_fix_action(&action, diagnostic, &module, self)
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
                  .add_ts_fix_all_action(&action, &module, diagnostic);
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
              .add_deno_fix_action(
                document.uri(),
                &module.specifier,
                diagnostic,
              )
              .map_err(|err| {
                error!("{:#}", err);
                LspError::internal_error()
              })?
          }
          Some("deno-lint") => code_actions
            .add_deno_lint_actions(document.uri(), &module, diagnostic)
            .map_err(|err| {
              error!("Unable to fix lint error: {:#}", err);
              LspError::internal_error()
            })?,
          _ => (),
        }
      }
      if includes_no_cache {
        let no_cache_diagnostics =
          self.diagnostics_state.no_cache_diagnostics(document.uri());
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
          code_actions.add_cache_all_action(
            &module.specifier,
            no_cache_diagnostics.to_owned(),
          );
        }
      }
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
        &module.specifier,
        module.line_index.offset_tsc(params.range.start)?
          ..module.line_index.offset_tsc(params.range.end)?,
        Some(tsc::UserPreferences::from_config_for_specifier(
          &self.config,
          &module.specifier,
        )),
        params.context.trigger_kind,
        only,
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          error!("Unable to get refactor info from TypeScript: {:#}", err);
          LspError::internal_error()
        }
      })?;
    let refactor_actions = refactor_infos
      .into_iter()
      .map(|refactor_info| {
        refactor_info
          .to_code_actions(document.uri(), &params.range, token)
          .map_err(|err| {
            if token.is_cancelled() {
              LspError::request_cancelled()
            } else {
              error!("Unable to convert refactor info: {:#}", err);
              LspError::internal_error()
            }
          })
      })
      .collect::<Result<Vec<_>, _>>()?
      .into_iter()
      .flatten()
      .collect();
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn code_action_resolve(
    &self,
    params: CodeAction,
    token: &CancellationToken,
  ) -> LspResult<CodeAction> {
    let mark = self
      .performance
      .mark_with_args("lsp.code_action_resolve", &params);
    let (Some(kind), Some(data)) = (params.kind.clone(), params.data.clone())
    else {
      return Ok(params);
    };
    let result = if kind.as_str().starts_with(CodeActionKind::QUICKFIX.as_str())
    {
      let code_action_data: CodeActionData =
        from_value(data).map_err(|err| {
          error!("Unable to decode code action data: {:#}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let Some(document) = self.get_document(
        &code_action_data.uri,
        Enabled::Filter,
        Exists::Enforce,
        Diagnosable::Filter,
      )?
      else {
        return Ok(params);
      };
      let Some(module) = self.get_primary_module(&document)? else {
        return Ok(params);
      };
      let combined_code_actions = self
        .ts_server
        .get_combined_code_fix(
          self.snapshot(),
          &module.specifier,
          &code_action_data.fix_id,
          (&self
            .config
            .tree
            .fmt_config_for_specifier(&module.specifier)
            .options)
            .into(),
          tsc::UserPreferences::from_config_for_specifier(
            &self.config,
            &module.specifier,
          ),
          module.scope.as_ref(),
          module.notebook_uri.as_ref(),
          token,
        )
        .await
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            error!("Unable to get combined fix from TypeScript: {:#}", err);
            LspError::internal_error()
          }
        })?;
      if combined_code_actions.commands.is_some() {
        error!("Deno does not support code actions with commands.");
        return Err(LspError::invalid_request());
      }

      let changes = if code_action_data.fix_id == "fixMissingImport" {
        fix_ts_import_changes(
          &combined_code_actions.changes,
          &module,
          self,
          token,
        )
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            error!("Unable to fix import changes: {:#}", err);
            LspError::internal_error()
          }
        })?
      } else {
        combined_code_actions.changes
      };
      let mut code_action = params;
      code_action.edit =
        ts_changes_to_edit(&changes, &module, self).map_err(|err| {
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
      let Some(document) = self.get_document(
        &action_data.uri,
        Enabled::Filter,
        Exists::Enforce,
        Diagnosable::Filter,
      )?
      else {
        return Ok(code_action);
      };
      let Some(module) = self.get_primary_module(&document)? else {
        return Ok(code_action);
      };
      let refactor_edit_info = self
        .ts_server
        .get_edits_for_refactor(
          self.snapshot(),
          &module.specifier,
          (&self
            .config
            .tree
            .fmt_config_for_specifier(&module.specifier)
            .options)
            .into(),
          module.line_index.offset_tsc(action_data.range.start)?
            ..module.line_index.offset_tsc(action_data.range.end)?,
          action_data.refactor_name.clone(),
          action_data.action_name.clone(),
          Some(tsc::UserPreferences::from_config_for_specifier(
            &self.config,
            &module.specifier,
          )),
          module.scope.as_ref(),
          module.notebook_uri.as_ref(),
          token,
        )
        .await
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            error!(
              "Unable to get refactor edit info from TypeScript: {:#}",
              err
            );
            LspError::invalid_request()
          }
        });
      match refactor_edit_info {
        Ok(mut refactor_edit_info) => {
          if kind_suffix == ".rewrite.function.returnType"
            || kind_suffix == ".move.newFile"
          {
            refactor_edit_info.edits = fix_ts_import_changes(
              &refactor_edit_info.edits,
              &module,
              self,
              token,
            )
            .map_err(|err| {
              if token.is_cancelled() {
                LspError::request_cancelled()
              } else {
                error!("Unable to fix import changes: {:#}", err);
                LspError::internal_error()
              }
            })?
          }
          code_action.edit =
            refactor_edit_info.to_workspace_edit(&module, self, token)?;
        }
        Err(err) => {
          if token.is_cancelled() {
            return Err(LspError::request_cancelled());
          } else {
            lsp_warn!("Unable to get refactor edit info from TypeScript: {:#}\nCode action data: {:#}", err, json!(&action_data));
          }
        }
      }
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
    module: &DocumentModule,
  ) -> TsResponseImportMapper {
    TsResponseImportMapper::new(
      &self.document_modules,
      module.scope.clone(),
      self
        .config
        .tree
        .data_for_specifier(&module.specifier)
        // todo(dsherret): this should probably just take the resolver itself
        // as the import map is an implementation detail
        .and_then(|d| d.resolver.maybe_import_map()),
      &self.resolver,
      &self.ts_server.specifier_map,
    )
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn code_lens(
    &self,
    params: CodeLensParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    let mark = self.performance.mark_with_args("lsp.code_lens", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let settings = self
      .config
      .workspace_settings_for_specifier(&module.specifier);
    let mut code_lenses = Vec::new();
    if settings.code_lens.test
      && self.config.specifier_enabled_for_test(&module.specifier)
    {
      if let Some(Ok(parsed_source)) = &module
        .open_data
        .as_ref()
        .and_then(|d| d.parsed_source.as_ref())
      {
        code_lenses.extend(
          code_lens::collect_test(&module.specifier, parsed_source, token)
            .map_err(|err| {
              if token.is_cancelled() {
                LspError::request_cancelled()
              } else {
                error!(
                  "Error getting test code lenses for \"{}\": {:#}",
                  &module.specifier, err
                );
                LspError::internal_error()
              }
            })?,
        );
      }
    }
    if settings.code_lens.implementations || settings.code_lens.references {
      let navigation_tree = self.get_navigation_tree(&module, token).await?;
      code_lenses.extend(
        code_lens::collect_tsc(
          document.uri(),
          &settings.code_lens,
          module.line_index.clone(),
          &navigation_tree,
          token,
        )
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            error!(
              "Error getting ts code lenses for \"{:#}\": {:#}",
              &module.specifier, err
            );
            LspError::internal_error()
          }
        })?,
      );
    }
    self.performance.measure(mark);

    if code_lenses.is_empty() {
      return Ok(None);
    }
    Ok(Some(code_lenses))
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn code_lens_resolve(
    &self,
    code_lens: CodeLens,
    token: &CancellationToken,
  ) -> LspResult<CodeLens> {
    let mark = self
      .performance
      .mark_with_args("lsp.code_lens_resolve", &code_lens);
    let result = if code_lens.data.is_some() {
      code_lens::resolve_code_lens(code_lens, self, token)
        .await
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            error!("Unable to get resolved code lens: {:#}", err);
            LspError::internal_error()
          }
        })
    } else {
      Err(LspError::invalid_params(
        "Code lens is missing the \"data\" property.",
      ))
    };
    self.performance.measure(mark);
    result
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn document_highlight(
    &self,
    params: DocumentHighlightParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    let mark = self
      .performance
      .mark_with_args("lsp.document_highlight", &params);
    let Some(document) = self.get_document(
      &params.text_document_position_params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let maybe_document_highlights = self
      .ts_server
      .get_document_highlights(
        self.snapshot(),
        &module.specifier,
        module
          .line_index
          .offset_tsc(params.text_document_position_params.position)?,
        vec![module.specifier.as_ref().clone()],
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          error!(
            "Unable to get document highlights from TypeScript: {:#}",
            err
          );
          LspError::internal_error()
        }
      })?;

    let document_highlights = maybe_document_highlights
      .map(|document_highlights| {
        document_highlights
          .into_iter()
          .map(|dh| {
            dh.to_highlight(module.line_index.clone(), token)
              .map_err(|err| {
                if token.is_cancelled() {
                  LspError::request_cancelled()
                } else {
                  error!("Unable to convert document highlights: {:#}", err);
                  LspError::internal_error()
                }
              })
          })
          .collect::<Result<Vec<_>, _>>()
          .map(|s| s.into_iter().flatten().collect())
      })
      .transpose()?;
    self.performance.measure(mark);
    Ok(document_highlights)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub async fn references(
    &self,
    params: ReferenceParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<Location>>> {
    let mark = self.performance.mark_with_args("lsp.references", &params);
    let Some(document) = self.get_document(
      &params.text_document_position.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let mut locations = IndexSet::new();
    for (scope, module) in self
      .document_modules
      .inspect_or_temp_modules_by_scope(&document)
    {
      if token.is_cancelled() {
        return Err(LspError::request_cancelled());
      }
      let symbols = self
        .ts_server
        .find_references(
          self.snapshot(),
          &module.specifier,
          module
            .line_index
            .offset_tsc(params.text_document_position.position)?,
          scope.as_ref(),
          // TODO(nayeemrmn): Support notebook scopes here.
          None,
          token,
        )
        .await
        .inspect_err(|err| {
          if !err.to_string().contains("Could not find source file") {
            lsp_warn!(
              "Unable to get references from TypeScript: {:#}\nScope: {}",
              err,
              module.scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
            );
          }
        })
        .unwrap_or_default();
      for reference in symbols.iter().flatten().flat_map(|s| &s.references) {
        if token.is_cancelled() {
          return Err(LspError::request_cancelled());
        }
        if !params.context.include_declaration && reference.is_definition {
          continue;
        }
        let Some(location) = reference.entry.to_location(&module, self) else {
          continue;
        };
        locations.insert(location);
      }
    }
    let locations = if locations.is_empty() {
      None
    } else {
      Some(locations.into_iter().collect())
    };

    self.performance.measure(mark);
    Ok(locations)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
    token: &CancellationToken,
  ) -> LspResult<Option<GotoDefinitionResponse>> {
    let mark = self
      .performance
      .mark_with_args("lsp.goto_definition", &params);
    let Some(document) = self.get_document(
      &params.text_document_position_params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let maybe_definition = self
      .ts_server
      .get_definition(
        self.snapshot(),
        &module.specifier,
        module
          .line_index
          .offset_tsc(params.text_document_position_params.position)?,
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          error!("Unable to get definition info from TypeScript: {:#}", err);
          LspError::internal_error()
        }
      })?;

    if let Some(definition) = maybe_definition {
      let results =
        definition
          .to_definition(&module, self, token)
          .map_err(|err| {
            if token.is_cancelled() {
              LspError::request_cancelled()
            } else {
              error!("Unable to convert definition info: {:#}", err);
              LspError::internal_error()
            }
          })?;
      self.performance.measure(mark);
      Ok(results)
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn goto_type_definition(
    &self,
    params: GotoTypeDefinitionParams,
    token: &CancellationToken,
  ) -> LspResult<Option<GotoTypeDefinitionResponse>> {
    let mark = self
      .performance
      .mark_with_args("lsp.goto_definition", &params);
    let Some(document) = self.get_document(
      &params.text_document_position_params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let maybe_definition_info = self
      .ts_server
      .get_type_definition(
        self.snapshot(),
        &module.specifier,
        module
          .line_index
          .offset_tsc(params.text_document_position_params.position)?,
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          error!(
            "Unable to get type definition info from TypeScript: {:#}",
            err
          );
          LspError::internal_error()
        }
      })?;

    let response = if let Some(definition_info) = maybe_definition_info {
      let mut location_links = Vec::new();
      for info in definition_info {
        if token.is_cancelled() {
          return Err(LspError::request_cancelled());
        }
        if let Some(link) = info.document_span.to_link(&module, self) {
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn completion(
    &self,
    params: CompletionParams,
    token: &CancellationToken,
  ) -> LspResult<Option<CompletionResponse>> {
    let mark = self.performance.mark_with_args("lsp.completion", &params);
    let Some(document) = self.get_document(
      &params.text_document_position.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let language_settings = self
      .config
      .language_settings_for_specifier(&module.specifier);
    if !language_settings.map(|s| s.suggest.enabled).unwrap_or(true) {
      return Ok(None);
    }

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
        &module,
        &params.text_document_position.position,
        &self.config,
        &self.client,
        &self.module_registry,
        &self.jsr_search_api,
        &self.npm_search_api,
        &self.document_modules,
        self.resolver.as_ref(),
        self
          .config
          .tree
          .data_for_specifier(&module.specifier)
          // todo(dsherret): this should probably just take the resolver itself
          // as the import map is an implementation detail
          .and_then(|d| d.resolver.maybe_import_map()),
      )
      .await;
    }
    if response.is_none() {
      let (trigger_character, trigger_kind) =
        if let Some(context) = &params.context {
          (
            context.trigger_character.clone(),
            Some(context.trigger_kind.into()),
          )
        } else {
          (None, None)
        };
      let position = module
        .line_index
        .offset_tsc(params.text_document_position.position)?;
      let maybe_completion_info = self
        .ts_server
        .get_completions(
          self.snapshot(),
          &module.specifier,
          position,
          tsc::GetCompletionsAtPositionOptions {
            user_preferences: tsc::UserPreferences::from_config_for_specifier(
              &self.config,
              &module.specifier,
            ),
            trigger_character,
            trigger_kind,
          },
          (&self
            .config
            .tree
            .fmt_config_for_specifier(&module.specifier)
            .options)
            .into(),
          module.scope.as_ref(),
          module.notebook_uri.as_ref(),
          token,
        )
        .await
        .unwrap_or_else(|err| {
          if !token.is_cancelled() {
            error!("Unable to get completion info from TypeScript: {:#}", err);
          }
          None
        });

      if let Some(completions) = maybe_completion_info {
        response = Some(
          completions
            .as_completion_response(
              module.line_index.clone(),
              &self
                .config
                .language_settings_for_specifier(&module.specifier)
                .cloned()
                .unwrap_or_default()
                .suggest,
              &module,
              position,
              self,
              token,
            )
            .map_err(|err| {
              if token.is_cancelled() {
                LspError::request_cancelled()
              } else {
                error!("Unable to convert completion info: {:#}", err);
                LspError::internal_error()
              }
            })?,
        );
      }
    };
    self.performance.measure(mark);
    Ok(response)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn completion_resolve(
    &self,
    params: CompletionItem,
    token: &CancellationToken,
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
        let Some(document) = self.get_document(
          &data.uri,
          Enabled::Filter,
          Exists::Enforce,
          Diagnosable::Filter,
        )?
        else {
          return Ok(params);
        };
        let Some(module) = self.get_primary_module(&document)? else {
          return Ok(params);
        };
        let result = self
          .ts_server
          .get_completion_details(
            self.snapshot(),
            &module.specifier,
            data.position,
            data.name.clone(),
            Some(
              (&self
                .config
                .tree
                .fmt_config_for_specifier(&module.specifier)
                .options)
                .into(),
            ),
            data.source.clone(),
            Some(tsc::UserPreferences::from_config_for_specifier(
              &self.config,
              &module.specifier,
            )),
            data.data.clone(),
            module.scope.as_ref(),
            module.notebook_uri.as_ref(),
            token,
          )
          .await;
        match result {
          Ok(maybe_completion_info) => {
            if let Some(completion_info) = maybe_completion_info {
              completion_info
                .as_completion_item(&params, data, &module, self)
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
            if !token.is_cancelled() {
              error!(
                "Unable to get completion info from TypeScript: {:#}",
                err
              );
            }
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub async fn goto_implementation(
    &self,
    params: GotoImplementationParams,
    token: &CancellationToken,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    let mark = self
      .performance
      .mark_with_args("lsp.goto_implementation", &params);
    let Some(document) = self.get_document(
      &params.text_document_position_params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let mut implementations_with_modules = IndexMap::new();
    for (scope, module) in self
      .document_modules
      .inspect_or_temp_modules_by_scope(&document)
    {
      if token.is_cancelled() {
        return Err(LspError::request_cancelled());
      }
      let maybe_implementations = self
        .ts_server
        .get_implementations(
          self.snapshot(),
          &module.specifier,
          module
            .line_index
            .offset_tsc(params.text_document_position_params.position)?,
          scope.as_ref(),
          module.notebook_uri.as_ref(),
          token,
        )
        .await
        .inspect_err(|err| {
          if !err.to_string().contains("Could not find source file") {
            lsp_warn!(
              "Unable to get implementation locations from TypeScript: {:#}\nScope: {}",
              err,
              module.scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
            );
          }
        })
        .unwrap_or_default();
      implementations_with_modules.extend(
        maybe_implementations
          .into_iter()
          .flatten()
          .map(|i| (i, module.clone())),
      );
    }
    let links = implementations_with_modules
      .iter()
      .flat_map(|(i, module)| {
        if token.is_cancelled() {
          return Some(Err(LspError::request_cancelled()));
        }
        Some(Ok(i.to_link(module, self)?))
      })
      .collect::<Result<Vec<_>, _>>()?;
    let result = if links.is_empty() {
      None
    } else {
      Some(GotoDefinitionResponse::Link(links.into_iter().collect()))
    };

    self.performance.measure(mark);
    Ok(result)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn folding_range(
    &self,
    params: FoldingRangeParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<FoldingRange>>> {
    let mark = self
      .performance
      .mark_with_args("lsp.folding_range", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let outlining_spans = self
      .ts_server
      .get_outlining_spans(
        self.snapshot(),
        &module.specifier,
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          lsp_warn!("Unable to get outlining spans from TypeScript: {:#}", err);
          LspError::invalid_request()
        }
      })?;
    let response = if !outlining_spans.is_empty() {
      Some(
        outlining_spans
          .iter()
          .map(|span| {
            if token.is_cancelled() {
              return Err(LspError::request_cancelled());
            }
            Ok(span.to_folding_range(
              module.line_index.clone(),
              module.text.as_bytes(),
              self.config.line_folding_only_capable(),
            ))
          })
          .collect::<Result<Vec<_>, _>>()?,
      )
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
    let mark = self
      .performance
      .mark_with_args("lsp.incoming_calls", &params);
    let Some(document) = self.get_document(
      &params.item.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let mut incoming_calls_with_modules = IndexMap::new();
    for (scope, module) in self
      .document_modules
      .inspect_or_temp_modules_by_scope(&document)
    {
      if token.is_cancelled() {
        return Err(LspError::request_cancelled());
      }
      let calls = self
        .ts_server
        .provide_call_hierarchy_incoming_calls(
          self.snapshot(),
          &module.specifier,
          module
            .line_index
            .offset_tsc(params.item.selection_range.start)?,
          scope.as_ref(),
          module.notebook_uri.as_ref(),
          token,
        )
        .await
        .inspect_err(|err| {
          if !err.to_string().contains("Could not find source file") {
            lsp_warn!(
              "Unable to get incoming calls from TypeScript: {:#}\nScope: {}",
              err,
              module.scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
            );
          }
        })
        .unwrap_or_default();
      incoming_calls_with_modules
        .extend(calls.into_iter().map(|c| (c, module.clone())));
    }
    let root_path = self
      .config
      .root_url()
      .and_then(|s| url_to_file_path(s).ok());
    let resolved_items = incoming_calls_with_modules
      .iter()
      .flat_map(|(c, module)| {
        if token.is_cancelled() {
          return Some(Err(LspError::request_cancelled()));
        }
        Some(Ok(c.try_resolve_call_hierarchy_incoming_call(
          module,
          self,
          root_path.as_deref(),
        )?))
      })
      .collect::<Result<Vec<_>, _>>()?;
    self.performance.measure(mark);
    Ok(Some(resolved_items))
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn outgoing_calls(
    &self,
    params: CallHierarchyOutgoingCallsParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
    let mark = self
      .performance
      .mark_with_args("lsp.outgoing_calls", &params);
    let Some(document) = self.get_document(
      &params.item.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let outgoing_calls: Vec<tsc::CallHierarchyOutgoingCall> = self
      .ts_server
      .provide_call_hierarchy_outgoing_calls(
        self.snapshot(),
        &module.specifier,
        module
          .line_index
          .offset_tsc(params.item.selection_range.start)?,
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          lsp_warn!("Unable to get outgoing calls from TypeScript: {:#}", err);
          LspError::invalid_request()
        }
      })?;
    let root_path = self
      .config
      .root_url()
      .and_then(|s| url_to_file_path(s).ok());
    let resolved_items = outgoing_calls
      .iter()
      .flat_map(|c| {
        if token.is_cancelled() {
          return Some(Err(LspError::request_cancelled()));
        }
        Some(Ok(c.try_resolve_call_hierarchy_outgoing_call(
          &module,
          self,
          root_path.as_deref(),
        )?))
      })
      .collect::<Result<_, _>>()?;
    self.performance.measure(mark);
    Ok(Some(resolved_items))
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
    let mark = self
      .performance
      .mark_with_args("lsp.prepare_call_hierarchy", &params);
    let Some(document) = self.get_document(
      &params.text_document_position_params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let maybe_one_or_many = self
      .ts_server
      .prepare_call_hierarchy(
        self.snapshot(),
        &module.specifier,
        module
          .line_index
          .offset_tsc(params.text_document_position_params.position)?,
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          lsp_warn!("Unable to get call hierarchy from TypeScript: {:#}", err);
          LspError::invalid_request()
        }
      })?;

    let response = if let Some(one_or_many) = maybe_one_or_many {
      let root_path = self
        .config
        .root_url()
        .and_then(|s| url_to_file_path(s).ok());
      let mut resolved_items = Vec::<CallHierarchyItem>::new();
      match one_or_many {
        tsc::OneOrMany::One(item) => {
          if let Some(resolved) = item.try_resolve_call_hierarchy_item(
            &module,
            self,
            root_path.as_deref(),
          ) {
            resolved_items.push(resolved)
          }
        }
        tsc::OneOrMany::Many(items) => {
          for item in items.iter() {
            if token.is_cancelled() {
              return Err(LspError::request_cancelled());
            }
            if let Some(resolved) = item.try_resolve_call_hierarchy_item(
              &module,
              self,
              root_path.as_deref(),
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn rename(
    &self,
    params: RenameParams,
    token: &CancellationToken,
  ) -> LspResult<Option<WorkspaceEdit>> {
    let mark = self.performance.mark_with_args("lsp.rename", &params);
    let Some(document) = self.get_document(
      &params.text_document_position.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let mut locations_with_modules = IndexMap::new();
    for (scope, module) in self
      .document_modules
      .inspect_or_temp_modules_by_scope(&document)
    {
      if token.is_cancelled() {
        return Err(LspError::request_cancelled());
      }
      let maybe_locations = self
        .ts_server
        .find_rename_locations(
          self.snapshot(),
          &module.specifier,
          module
            .line_index
            .offset_tsc(params.text_document_position.position)?,
          tsc::UserPreferences::from_config_for_specifier(
            &self.config,
            &module.specifier,
          ),
          scope.as_ref(),
          // TODO(nayeemrmn): Support notebook scopes here.
          None,
          token,
        )
        .await
        .inspect_err(|err| {
          if !err.to_string().contains("Could not find source file") {
            lsp_warn!(
              "Unable to get rename locations from TypeScript: {:#}\nScope: {}",
              err,
              scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
            );
          }
        })
        .unwrap_or_default();
      locations_with_modules.extend(
        maybe_locations
          .into_iter()
          .flatten()
          .map(|l| (l, module.clone())),
      );
    }
    let result = if locations_with_modules.is_empty() {
      None
    } else {
      let workspace_edits = tsc::RenameLocation::collect_into_workspace_edit(
        locations_with_modules,
        &params.new_name,
        self,
        token,
      )
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          lsp_warn!("Unable to covert rename locations: {:#}", err);
          LspError::internal_error()
        }
      })?;
      Some(workspace_edits)
    };
    self.performance.measure(mark);
    Ok(result)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn selection_range(
    &self,
    params: SelectionRangeParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    let mark = self
      .performance
      .mark_with_args("lsp.selection_range", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let mut selection_ranges = Vec::<SelectionRange>::new();
    for &position in &params.positions {
      if token.is_cancelled() {
        return Err(LspError::request_cancelled());
      }
      let selection_range = self
        .ts_server
        .get_smart_selection_range(
          self.snapshot(),
          &module.specifier,
          module.line_index.offset_tsc(position)?,
          module.scope.as_ref(),
          module.notebook_uri.as_ref(),
          token,
        )
        .await
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            lsp_warn!(
              "Unable to get selection ranges from TypeScript: {:#}",
              err
            );
            LspError::invalid_request()
          }
        })?;

      selection_ranges
        .push(selection_range.to_selection_range(module.line_index.clone()));
    }
    self.performance.measure(mark);
    Ok(Some(selection_ranges))
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn semantic_tokens_full(
    &self,
    params: SemanticTokensParams,
    token: &CancellationToken,
  ) -> LspResult<Option<SemanticTokensResult>> {
    let mark = self
      .performance
      .mark_with_args("lsp.semantic_tokens_full", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Ignore,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    let semantic_tokens = module
      .semantic_tokens_full
      .get_or_try_init(|| async {
        let semantic_classification = self
          .ts_server
          .get_encoded_semantic_classifications(
            self.snapshot(),
            &module.specifier,
            0..module.line_index.text_content_length_utf16().into(),
            module.scope.as_ref(),
            module.notebook_uri.as_ref(),
            token,
          )
          .await
          .map_err(|err| {
            if token.is_cancelled() {
              LspError::request_cancelled()
            } else {
              lsp_warn!(
                "Unable to get semantic classifications from TypeScript: {:#}",
                err
              );
              LspError::invalid_request()
            }
          })?;
        semantic_classification
          .to_semantic_tokens(module.line_index.clone(), token)
      })
      .await?
      .clone();
    let response = if !semantic_tokens.data.is_empty() {
      Some(SemanticTokensResult::Tokens(semantic_tokens))
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn semantic_tokens_range(
    &self,
    params: SemanticTokensRangeParams,
    token: &CancellationToken,
  ) -> LspResult<Option<SemanticTokensRangeResult>> {
    let mark = self
      .performance
      .mark_with_args("lsp.semantic_tokens_range", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Ignore,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    if let Some(tokens) = module.semantic_tokens_full.get() {
      let tokens =
        super::semantic_tokens::tokens_within_range(tokens, params.range);
      let response = if !tokens.data.is_empty() {
        Some(SemanticTokensRangeResult::Tokens(tokens))
      } else {
        None
      };
      self.performance.measure(mark);
      return Ok(response);
    }
    let semantic_classification = self
      .ts_server
      .get_encoded_semantic_classifications(
        self.snapshot(),
        &module.specifier,
        module.line_index.offset_tsc(params.range.start)?
          ..module.line_index.offset_tsc(params.range.end)?,
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          lsp_warn!(
            "Unable to get semantic classifications from TypeScript: {:#}",
            err
          );
          LspError::invalid_request()
        }
      })?;
    let semantic_tokens = semantic_classification
      .to_semantic_tokens(module.line_index.clone(), token)?;
    let response = if !semantic_tokens.data.is_empty() {
      Some(SemanticTokensRangeResult::Tokens(semantic_tokens))
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn signature_help(
    &self,
    params: SignatureHelpParams,
    token: &CancellationToken,
  ) -> LspResult<Option<SignatureHelp>> {
    let mark = self
      .performance
      .mark_with_args("lsp.signature_help", &params);
    let Some(document) = self.get_document(
      &params.text_document_position_params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
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
        &module.specifier,
        module
          .line_index
          .offset_tsc(params.text_document_position_params.position)?,
        options,
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          lsp_warn!(
            "Unable to get signature help items from TypeScript: {:#}",
            err
          );
          LspError::invalid_request()
        }
      })?;
    let result = if let Some(signature_help_items) = maybe_signature_help_items
    {
      let signature_help = signature_help_items
        .into_signature_help(&module, self, token)
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            lsp_warn!("Unable to convert signature help items: {:#}", err);
            LspError::internal_error()
          }
        })?;
      Some(signature_help)
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(result)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn will_rename_files(
    &self,
    params: RenameFilesParams,
    token: &CancellationToken,
  ) -> LspResult<Option<WorkspaceEdit>> {
    if !self.ts_server.is_started() {
      return Ok(None);
    }
    let mut changes_with_modules = IndexMap::new();
    for rename in params.files {
      let Some(document) = self.get_document(
        &Uri::from_str(&rename.old_uri).unwrap(),
        Enabled::Ignore,
        Exists::Filter,
        Diagnosable::Ignore,
      )?
      else {
        continue;
      };
      for (scope, module) in self
        .document_modules
        .inspect_or_temp_modules_by_scope(&document)
      {
        if token.is_cancelled() {
          return Err(LspError::request_cancelled());
        }
        let options = self
          .config
          .language_settings_for_specifier(&module.specifier)
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
          .fmt_config_for_specifier(&module.specifier)
          .options)
          .into();
        let changes = self
        .ts_server
        .get_edits_for_file_rename(
          self.snapshot(),
          &module.specifier,
          &uri_to_url(&Uri::from_str(&rename.new_uri).unwrap()),
          format_code_settings,
          tsc::UserPreferences {
            allow_text_changes_in_new_files: Some(true),
            ..Default::default()
          },
          scope.as_ref(),
          // TODO(nayeemrmn): Support notebook scopes here.
          None,
          token,
        )
        .await
        .map_err(|err| {
          if token.is_cancelled() {
            LspError::request_cancelled()
          } else {
            lsp_warn!(
              "Unable to get edits for file rename from TypeScript: {:#}\nScope: {}",
              err,
              scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
            );
            LspError::internal_error()
          }
        })?;
        changes_with_modules
          .extend(changes.into_iter().map(|c| (c, module.clone())));
      }
    }
    file_text_changes_to_workspace_edit(&changes_with_modules, self, token)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn symbol(
    &self,
    params: WorkspaceSymbolParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<SymbolInformation>>> {
    if !self.ts_server.is_started() {
      return Ok(None);
    }

    let mark = self.performance.mark_with_args("lsp.symbol", &params);
    let mut items_with_scopes = IndexMap::new();
    for scope in self.document_modules.scopes() {
      if token.is_cancelled() {
        return Err(LspError::request_cancelled());
      }
      let items = self
        .ts_server
        .get_navigate_to_items(
          self.snapshot(),
          params.query.clone(),
          // this matches vscode's hard coded result count
          Some(256),
          None,
          scope.as_ref(),
          // TODO(nayeemrmn): Support notebook scopes here.
          None,
          token,
        )
        .await
        .inspect_err(|err| {
          lsp_warn!(
            "Unable to get signature help items from TypeScript: {:#}\nScope: {}",
            err,
            scope.as_ref().map(|s| s.as_str()).unwrap_or("null"),
          );
        })
        .unwrap_or_default();
      items_with_scopes.extend(items.into_iter().map(|i| (i, scope.clone())));
    }
    let symbol_information = items_with_scopes
      .into_iter()
      .flat_map(|(item, scope)| {
        if token.is_cancelled() {
          return Some(Err(LspError::request_cancelled()));
        }
        Some(Ok(item.to_symbol_information(scope.as_deref(), self)?))
      })
      .collect::<Result<Vec<_>, _>>()?;
    let symbol_information = if symbol_information.is_empty() {
      None
    } else {
      Some(symbol_information)
    };
    self.performance.measure(mark);
    Ok(symbol_information)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  fn project_changed<'a>(
    &mut self,
    changed_specifiers: impl IntoIterator<Item = (Arc<Url>, ChangeKind)>,
    scopes_change: ProjectScopesChange,
  ) {
    self.project_version += 1; // increment before getting the snapshot
    let changed_specifiers = changed_specifiers.into_iter().collect::<Vec<_>>();
    self.ts_server.project_changed(
      self.snapshot(),
      changed_specifiers.iter().map(|(u, k)| (u.as_ref(), *k)),
      matches!(scopes_change, ProjectScopesChange::Config).then(|| {
        self
          .config
          .tree
          .data_by_scope()
          .iter()
          .map(|(s, d)| (s.clone(), d.ts_config.clone()))
          .collect()
      }),
      matches!(
        scopes_change,
        ProjectScopesChange::OpenNotebooks | ProjectScopesChange::Config
      )
      .then(|| {
        self
          .document_modules
          .documents
          .cells_by_notebook_uri()
          .keys()
          .map(|u| {
            (
              u.clone(),
              self.document_modules.primary_scope(u).flatten().cloned(),
            )
          })
          .collect()
      }),
    );
    self.document_modules.remove_expired_modules();
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  fn send_diagnostics_update(&self) {
    let snapshot = DiagnosticServerUpdateMessage {
      snapshot: self.snapshot(),
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

#[tower_lsp::async_trait(?Send)]
impl tower_lsp::LanguageServer for LanguageServer {
  async fn execute_command(
    &self,
    params: ExecuteCommandParams,
    _token: CancellationToken,
  ) -> LspResult<Option<Value>> {
    self.init_flag.wait_raised().await;
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
    self.inner.write().await.initialize(params)
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
    Ok(())
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    self.init_flag.wait_raised().await;
    self.inner.write().await.did_open(params).await;
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    self.init_flag.wait_raised().await;
    self.inner.write().await.did_change(params).await;
  }

  async fn did_save(&self, params: DidSaveTextDocumentParams) {
    self.init_flag.wait_raised().await;
    self.inner.write().await.did_save(params);
  }

  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    self.init_flag.wait_raised().await;
    self.inner.write().await.did_close(params).await;
  }

  async fn notebook_did_open(&self, params: DidOpenNotebookDocumentParams) {
    self.init_flag.wait_raised().await;
    self.inner.write().await.notebook_did_open(params).await
  }

  async fn notebook_did_change(&self, params: DidChangeNotebookDocumentParams) {
    self.init_flag.wait_raised().await;
    self.inner.write().await.notebook_did_change(params).await
  }

  async fn notebook_did_save(&self, params: DidSaveNotebookDocumentParams) {
    self.init_flag.wait_raised().await;
    self.inner.write().await.notebook_did_save(params)
  }

  async fn notebook_did_close(&self, params: DidCloseNotebookDocumentParams) {
    self.init_flag.wait_raised().await;
    self.inner.write().await.notebook_did_close(params).await
  }

  async fn did_change_configuration(
    &self,
    params: DidChangeConfigurationParams,
  ) {
    self.init_flag.wait_raised().await;
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
    self.init_flag.wait_raised().await;
    self
      .inner
      .write()
      .await
      .did_change_watched_files(params)
      .await;
  }

  async fn did_change_workspace_folders(
    &self,
    params: DidChangeWorkspaceFoldersParams,
  ) {
    self.init_flag.wait_raised().await;
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
    token: CancellationToken,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .document_symbol(params, &token)
      .await
  }

  async fn formatting(
    &self,
    params: DocumentFormattingParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<TextEdit>>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.formatting(params, &token).await
  }

  async fn hover(
    &self,
    params: HoverParams,
    token: CancellationToken,
  ) -> LspResult<Option<Hover>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.hover(params, &token).await
  }

  async fn inlay_hint(
    &self,
    params: InlayHintParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<InlayHint>>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.inlay_hint(params, &token).await
  }

  async fn code_action(
    &self,
    params: CodeActionParams,
    token: CancellationToken,
  ) -> LspResult<Option<CodeActionResponse>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.code_action(params, &token).await
  }

  async fn code_action_resolve(
    &self,
    params: CodeAction,
    token: CancellationToken,
  ) -> LspResult<CodeAction> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .code_action_resolve(params, &token)
      .await
  }

  async fn code_lens(
    &self,
    params: CodeLensParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.code_lens(params, &token).await
  }

  async fn code_lens_resolve(
    &self,
    params: CodeLens,
    token: CancellationToken,
  ) -> LspResult<CodeLens> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .code_lens_resolve(params, &token)
      .await
  }

  async fn document_highlight(
    &self,
    params: DocumentHighlightParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .document_highlight(params, &token)
      .await
  }

  async fn references(
    &self,
    params: ReferenceParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<Location>>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.references(params, &token).await
  }

  async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
    token: CancellationToken,
  ) -> LspResult<Option<GotoDefinitionResponse>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .goto_definition(params, &token)
      .await
  }

  async fn goto_type_definition(
    &self,
    params: GotoTypeDefinitionParams,
    token: CancellationToken,
  ) -> LspResult<Option<GotoTypeDefinitionResponse>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .goto_type_definition(params, &token)
      .await
  }

  async fn completion(
    &self,
    params: CompletionParams,
    token: CancellationToken,
  ) -> LspResult<Option<CompletionResponse>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.completion(params, &token).await
  }

  async fn completion_resolve(
    &self,
    params: CompletionItem,
    token: CancellationToken,
  ) -> LspResult<CompletionItem> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .completion_resolve(params, &token)
      .await
  }

  async fn goto_implementation(
    &self,
    params: GotoImplementationParams,
    token: CancellationToken,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .goto_implementation(params, &token)
      .await
  }

  async fn folding_range(
    &self,
    params: FoldingRangeParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<FoldingRange>>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.folding_range(params, &token).await
  }

  async fn incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.incoming_calls(params, &token).await
  }

  async fn outgoing_calls(
    &self,
    params: CallHierarchyOutgoingCallsParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.outgoing_calls(params, &token).await
  }

  async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .prepare_call_hierarchy(params, &token)
      .await
  }

  async fn rename(
    &self,
    params: RenameParams,
    token: CancellationToken,
  ) -> LspResult<Option<WorkspaceEdit>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.rename(params, &token).await
  }

  async fn selection_range(
    &self,
    params: SelectionRangeParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .selection_range(params, &token)
      .await
  }

  async fn semantic_tokens_full(
    &self,
    params: SemanticTokensParams,
    token: CancellationToken,
  ) -> LspResult<Option<SemanticTokensResult>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .semantic_tokens_full(params, &token)
      .await
  }

  async fn semantic_tokens_range(
    &self,
    params: SemanticTokensRangeParams,
    token: CancellationToken,
  ) -> LspResult<Option<SemanticTokensRangeResult>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .semantic_tokens_range(params, &token)
      .await
  }

  async fn signature_help(
    &self,
    params: SignatureHelpParams,
    token: CancellationToken,
  ) -> LspResult<Option<SignatureHelp>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.signature_help(params, &token).await
  }

  async fn will_rename_files(
    &self,
    params: RenameFilesParams,
    token: CancellationToken,
  ) -> LspResult<Option<WorkspaceEdit>> {
    self.init_flag.wait_raised().await;
    self
      .inner
      .read()
      .await
      .will_rename_files(params, &token)
      .await
  }

  async fn symbol(
    &self,
    params: WorkspaceSymbolParams,
    token: CancellationToken,
  ) -> LspResult<Option<Vec<SymbolInformation>>> {
    self.init_flag.wait_raised().await;
    self.inner.read().await.symbol(params, &token).await
  }
}

struct PrepareCacheResult {
  cli_factory: CliFactory,
  roots: Vec<ModuleSpecifier>,
  open_modules: Vec<Arc<DocumentModule>>,
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
        self.config.root_url().cloned(),
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]

  fn prepare_cache(
    &mut self,
    specifiers: Vec<ModuleSpecifier>,
    referrer: ModuleSpecifier,
    force_global_cache: bool,
  ) -> Result<PrepareCacheResult, AnyError> {
    let config_data = self.config.tree.data_for_specifier(&referrer);
    let scope = config_data.map(|d| d.scope.clone());
    let byonm = config_data.map(|d| d.byonm).unwrap_or(false);
    let mut roots = if !specifiers.is_empty() {
      specifiers
    } else {
      vec![referrer.clone()]
    };

    if byonm {
      roots.retain(|s| s.scheme() != "npm");
    } else if let Some(dep_info) = self
      .document_modules
      .dep_info_by_scope()
      .get(&scope)
      .cloned()
    {
      // always include the npm packages since resolution of one npm package
      // might affect the resolution of other npm packages
      roots.extend(
        dep_info
          .npm_reqs
          .iter()
          .map(|req| ModuleSpecifier::parse(&format!("npm:{}", req)).unwrap()),
      );
    }

    let workspace_settings = self.config.workspace_settings();
    let initial_cwd = config_data
      .and_then(|d| d.scope.to_file_path().ok())
      .unwrap_or_else(|| self.initial_cwd.clone());
    let mut cli_factory = CliFactory::from_flags(Arc::new(Flags {
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
      vendor: if force_global_cache {
        Some(false)
      } else {
        None
      },
      no_lock: force_global_cache,
      ..Default::default()
    }));
    cli_factory.set_initial_cwd(initial_cwd);
    if let Some(d) = &config_data {
      cli_factory.set_workspace_dir(d.member_dir.clone());
    };

    let open_modules = self
      .document_modules
      .documents
      .open_docs()
      .filter(|d| d.is_diagnosable())
      .flat_map(|d| {
        self
          .document_modules
          .module(&Document::Open(d.clone()), scope.as_deref())
      })
      .collect();
    Ok(PrepareCacheResult {
      cli_factory,
      open_modules,
      roots,
    })
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn post_cache(&mut self) {
    self.resolver.did_cache();
    self.refresh_dep_info().await;
    self.diagnostics_server.invalidate_all();
    self.project_changed([], ProjectScopesChange::Config);
    self.ts_server.cleanup_semantic_cache(self.snapshot()).await;
    self.send_diagnostics_update();
    self.send_testing_update();
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]

  fn pre_did_change_workspace_folders(
    &mut self,
    params: DidChangeWorkspaceFoldersParams,
  ) {
    let mut workspace_folders = params
      .event
      .added
      .into_iter()
      .map(|folder| {
        let mut url = uri_to_url(&folder.uri);
        if !url.path().ends_with('/') {
          if let Ok(mut path_segments) = url.path_segments_mut() {
            path_segments.push("");
          }
        }
        (Arc::new(url), folder)
      })
      .collect::<Vec<_>>();
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

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
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
      if let Some(tasks) = config_file.to_tasks_config().ok().flatten() {
        for (name, def) in tasks {
          result.push(TaskDefinition {
            name: name.clone(),
            command: def.command.clone(),
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
            command: Some(command.clone()),
            source_uri: url_to_uri(&package_json.specifier())
              .map_err(|_| LspError::internal_error())?,
          });
        }
      }
    }
    result.sort_by_key(|d| d.name.clone());
    Ok(result)
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  async fn inlay_hint(
    &self,
    params: InlayHintParams,
    token: &CancellationToken,
  ) -> LspResult<Option<Vec<InlayHint>>> {
    let mark = self.performance.mark_with_args("lsp.inlay_hint", &params);
    let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Filter,
      Exists::Enforce,
      Diagnosable::Filter,
    )?
    else {
      return Ok(None);
    };
    let Some(module) = self.get_primary_module(&document)? else {
      return Ok(None);
    };
    if !self
      .config
      .enabled_inlay_hints_for_specifier(&module.specifier)
    {
      return Ok(None);
    }
    let text_span =
      tsc::TextSpan::from_range(&params.range, module.line_index.clone())
        .map_err(|err| {
          error!("Failed to convert range to text_span: {:#}", err);
          LspError::internal_error()
        })?;
    let maybe_inlay_hints = self
      .ts_server
      .provide_inlay_hints(
        self.snapshot(),
        &module.specifier,
        text_span,
        tsc::UserPreferences::from_config_for_specifier(
          &self.config,
          &module.specifier,
        ),
        module.scope.as_ref(),
        module.notebook_uri.as_ref(),
        token,
      )
      .await
      .map_err(|err| {
        if token.is_cancelled() {
          LspError::request_cancelled()
        } else {
          error!("Unable to get inlay hints from TypeScript: {:#}", err);
          LspError::internal_error()
        }
      })?;
    let maybe_inlay_hints = maybe_inlay_hints
      .map(|hints| {
        hints
          .into_iter()
          .map(|hint| {
            if token.is_cancelled() {
              return Err(LspError::request_cancelled());
            }
            Ok(hint.to_lsp(&module, self))
          })
          .collect()
      })
      .transpose()?;
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
    let contents = if params
      .text_document
      .uri
      .scheme()
      .is_some_and(|s| s.eq_lowercase("deno"))
      && params.text_document.uri.path().as_str() == "/status.md"
    {
      let mut contents = String::new();
      let mut open_docs = self
        .document_modules
        .documents
        .open_docs()
        .collect::<Vec<_>>();
      open_docs.sort_by_cached_key(|d| d.uri.to_string());
      let mut server_docs = self.document_modules.documents.server_docs();
      server_docs.sort_by_cached_key(|d| d.uri.to_string());
      let measures = self.performance.to_vec();

      write!(
        contents,
        r#"# Deno Language Server Status

## Workspace Settings

{}

## Documents

<details><summary>Open: {}</summary>

{}

</details>

<details><summary>Server: {}</summary>

{}

</details>

## Performance Measures

<details><summary>Measures: {}</summary>

{}

</details>
"#,
        self.config.settings.by_workspace_folder
          .iter()
          .filter_map(|(folder_url, settings)| Some((format!("\"{folder_url}\""), settings.as_ref()?)))
          .chain(std::iter::once(("Unscoped".to_string(), &self.config.settings.unscoped)))
          .map(|(title, settings)| format!("<details><summary>{title}</summary>\n\n```json\n{}\n```\n\n</details>", serde_json::to_string_pretty(settings).unwrap()))
          .collect::<Vec<_>>()
          .join("\n\n"),
        open_docs.len(),
        open_docs.iter().map(|d| format!("- {}", d.uri.as_str())).collect::<Vec<_>>().join("\n"),
        server_docs.len(),
        server_docs.iter().map(|d| format!("- {}", d.uri.as_str())).collect::<Vec<_>>().join("\n"),
        measures.len(),
        measures
          .iter()
          .map(|m| format!("- {m}"))
          .collect::<Vec<_>>()
          .join("\n"),
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
    } else if let Some(document) = self.get_document(
      &params.text_document.uri,
      Enabled::Ignore,
      Exists::Filter,
      Diagnosable::Ignore,
    )? {
      Some(document.text().to_string())
    } else {
      lsp_warn!(
        "The document was not found: {}",
        params.text_document.uri.as_str()
      );
      None
    };
    self.performance.measure(mark);
    Ok(contents)
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  use super::*;

  #[test]
  fn test_walk_workspace() {
    let temp_dir = TempDir::new();
    temp_dir.create_dir_all("root1/vendor/");
    temp_dir.create_dir_all("root1/coverage/");
    temp_dir.write("root1/vendor/mod.ts", ""); // no, vendor
    temp_dir.write("root1/coverage/mod.ts", ""); // no, coverage

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
    temp_dir.create_dir_all("root2/folder2/inner_folder");
    temp_dir.create_dir_all("root2/sub_folder");
    temp_dir.create_dir_all("root2/root2.1");
    temp_dir.write("root2/file1.ts", ""); // yes, enabled
    temp_dir.write("root2/file2.ts", ""); // no, not enabled
    temp_dir.write("root2/folder/main.ts", ""); // yes, enabled
    temp_dir.write("root2/folder/other.ts", ""); // no, disabled
    temp_dir.write("root2/folder2/inner_folder/main.ts", ""); // yes, enabled (regression test for https://github.com/denoland/vscode_deno/issues/1239)
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
          Arc::new(temp_dir.url().join("root1/").unwrap()),
          WorkspaceSettings {
            enable: Some(true),
            ..Default::default()
          },
        ),
        (
          Arc::new(temp_dir.url().join("root2/").unwrap()),
          WorkspaceSettings {
            enable: Some(true),
            enable_paths: Some(vec![
              "file1.ts".to_string(),
              "folder".to_string(),
              "folder2/inner_folder".to_string(),
            ]),
            disable_paths: vec!["folder/other.ts".to_string()],
            ..Default::default()
          },
        ),
        (
          Arc::new(temp_dir.url().join("root2/root2.1/").unwrap()),
          WorkspaceSettings {
            enable: Some(true),
            ..Default::default()
          },
        ),
        (
          Arc::new(temp_dir.url().join("root3/").unwrap()),
          WorkspaceSettings {
            enable: Some(false),
            ..Default::default()
          },
        ),
        (
          Arc::new(temp_dir.url().join("root4_parent/root4/").unwrap()),
          WorkspaceSettings {
            enable: Some(true),
            ..Default::default()
          },
        ),
      ],
    );

    let (workspace_files, hit_limit) = Inner::walk_workspace(&config);
    let workspace_files = workspace_files
      .into_iter()
      .map(|p| Url::from_file_path(p).unwrap())
      .collect::<IndexSet<_>>();
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
        temp_dir
          .url()
          .join("root2/folder2/inner_folder/main.ts")
          .unwrap(),
      ])
    );
  }
}
