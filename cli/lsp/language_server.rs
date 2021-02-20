// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use dprint_plugin_typescript as dprint;
use lspower::jsonrpc::Error as LspError;
use lspower::jsonrpc::Result as LspResult;
use lspower::lsp::request::*;
use lspower::lsp::*;
use lspower::Client;
use regex::Regex;
use serde_json::from_value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crate::deno_dir;
use crate::import_map::ImportMap;
use crate::media_type::MediaType;
use crate::tsc_config::parse_config;
use crate::tsc_config::TsConfig;

use super::analysis;
use super::analysis::ts_changes_to_edit;
use super::analysis::CodeActionCollection;
use super::analysis::CodeActionData;
use super::analysis::CodeLensData;
use super::analysis::CodeLensSource;
use super::analysis::ResolvedDependency;
use super::capabilities;
use super::config::Config;
use super::diagnostics;
use super::diagnostics::DiagnosticCollection;
use super::diagnostics::DiagnosticSource;
use super::documents::DocumentCache;
use super::performance::Performance;
use super::sources;
use super::sources::Sources;
use super::text;
use super::text::LineIndex;
use super::tsc;
use super::tsc::AssetDocument;
use super::tsc::Assets;
use super::tsc::TsServer;
use super::urls;

lazy_static! {
  static ref ABSTRACT_MODIFIER: Regex = Regex::new(r"\babstract\b").unwrap();
  static ref EXPORT_MODIFIER: Regex = Regex::new(r"\bexport\b").unwrap();
}

#[derive(Debug)]
pub struct LanguageServer {
  /// The LSP client that this LSP server is connected to.
  client: Client,
  /// The LSP client state
  pub(crate) state: Arc<std::sync::Mutex<LanguageServerState>>,
  /// A collection of measurements which instrument that performance of the LSP.
  performance: Performance,
  /// An abstraction that handles interactions with TypeScript.
  ts_server: TsServer,
}

#[derive(Debug, Clone, Default)]
pub struct StateSnapshot {
  pub assets: Assets,
  pub documents: DocumentCache,
  pub performance: Performance,
  pub sources: Sources,
}

#[derive(Debug)]
pub(crate) struct LanguageServerState {
  /// Cached versions of "fixed" assets that can either be inlined in Rust or
  /// are part of the TypeScript snapshot and have to be fetched out.
  assets: Assets,
  /// Configuration information.
  config: Config,
  /// A collection of diagnostics from different sources.
  diagnostics: DiagnosticCollection,
  /// The "in-memory" documents in the editor which can be updated and changed.
  documents: DocumentCache,
  /// An optional URL which provides the location of a TypeScript configuration
  /// file which will be used by the Deno LSP.
  maybe_config_uri: Option<Url>,
  /// An optional import map which is used to resolve modules.
  pub(crate) maybe_import_map: Option<ImportMap>,
  /// The URL for the import map which is used to determine relative imports.
  maybe_import_map_uri: Option<Url>,
  /// A map of all the cached navigation trees.
  navigation_trees: HashMap<ModuleSpecifier, tsc::NavigationTree>,
  /// A collection of measurements which instrument that performance of the LSP.
  performance: Performance,
  /// Cached sources that are read-only.
  sources: Sources,
  /// A memoized version of fixable diagnostic codes retrieved from TypeScript.
  ts_fixable_diagnostics: Vec<String>,
  /// A map of specifiers and URLs used to translate over the LSP.
  pub url_map: urls::LspUrlMap,
}

impl LanguageServer {
  pub fn new(client: Client) -> Self {
    let performance = Performance::default();
    let state = LanguageServerState::new(performance.clone());
    Self {
      client,
      state: Arc::new(std::sync::Mutex::new(state)),
      performance,
      ts_server: TsServer::new(),
    }
  }

  pub async fn state_snapshot(&self) -> StateSnapshot {
    self.state.lock().unwrap().snapshot()
  }

  async fn get_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<tsc::NavigationTree, AnyError> {
    let mark = self.performance.mark("get_navigation_tree");
    let maybe_navigation_trees = {
      let state = self.state.lock().unwrap();
      state.navigation_trees.get(specifier).cloned()
    };
    if let Some(navigation_tree) = maybe_navigation_trees {
      self.performance.measure(mark);
      Ok(navigation_tree)
    } else {
      let state_snapshot = self.state_snapshot().await;
      let res = self
        .ts_server
        .request(
          state_snapshot,
          tsc::RequestMethod::GetNavigationTree(specifier.clone()),
        )
        .await?;
      let navigation_tree: tsc::NavigationTree = serde_json::from_value(res)?;
      let mut state = self.state.lock().unwrap();
      state
        .navigation_trees
        .insert(specifier.clone(), navigation_tree.clone());
      self.performance.measure(mark);
      Ok(navigation_tree)
    }
  }

  async fn prepare_diagnostics(&self) -> Result<(), AnyError> {
    let (enabled, lint_enabled, state_snapshot, diagnostic_collection) = {
      let state = self.state.lock().unwrap();
      let config = &state.config;
      let state_snapshot = state.snapshot();
      let diagnostic_collection = state.diagnostics.clone();
      (
        config.settings.enable,
        config.settings.lint,
        state_snapshot,
        diagnostic_collection,
      )
    };

    let lint = if lint_enabled {
      let state_snapshot = state_snapshot.clone();
      let diagnostic_collection = diagnostic_collection.clone();
      async {
        let mark = self.performance.mark("prepare_diagnostics_lint");
        let diagnostics = Some(
          diagnostics::generate_lint_diagnostics(
            state_snapshot,
            diagnostic_collection,
          )
          .await,
        );
        self.performance.measure(mark);
        Ok::<_, AnyError>(diagnostics)
      }
      .boxed()
    } else {
      async { Ok::<_, AnyError>(None) }.boxed()
    };

    let ts = if enabled {
      let state_snapshot = state_snapshot.clone();
      let diagnostic_collection = diagnostic_collection.clone();
      async {
        let mark = self.performance.mark("prepare_diagnostics_ts");
        let diagnostics = Some(
          diagnostics::generate_ts_diagnostics(
            state_snapshot,
            diagnostic_collection,
            &self.ts_server,
          )
          .await?,
        );
        self.performance.measure(mark);
        Ok::<_, AnyError>(diagnostics)
      }
      .boxed()
    } else {
      async { Ok::<_, AnyError>(None) }.boxed()
    };

    let deps = if enabled {
      async {
        let mark = self.performance.mark("prepare_diagnostics_deps");
        let diagnostics = Some(
          diagnostics::generate_dependency_diagnostics(
            state_snapshot,
            diagnostic_collection,
          )
          .await?,
        );
        self.performance.measure(mark);
        Ok::<_, AnyError>(diagnostics)
      }
      .boxed()
    } else {
      async { Ok::<_, AnyError>(None) }.boxed()
    };

    let (lint_res, ts_res, deps_res) = tokio::join!(lint, ts, deps);

    let disturbed = {
      let mut disturbed = false;
      let mut state = self.state.lock().unwrap();
      if let Some(diagnostics) = lint_res? {
        for (specifier, version, diagnostics) in diagnostics {
          state.diagnostics.set(
            specifier,
            DiagnosticSource::Lint,
            version,
            diagnostics,
          );
          disturbed = true;
        }
      }

      if let Some(diagnostics) = ts_res? {
        for (specifier, version, diagnostics) in diagnostics {
          state.diagnostics.set(
            specifier,
            DiagnosticSource::TypeScript,
            version,
            diagnostics,
          );
          disturbed = true;
        }
      }

      if let Some(diagnostics) = deps_res? {
        for (specifier, version, diagnostics) in diagnostics {
          state.diagnostics.set(
            specifier,
            DiagnosticSource::Deno,
            version,
            diagnostics,
          );
          disturbed = true;
        }
      }

      disturbed
    };

    if disturbed {
      self.publish_diagnostics().await.unwrap();
    }

    Ok(())
  }

  async fn publish_diagnostics(&self) -> Result<(), AnyError> {
    let mark = self.performance.mark("publish_diagnostics");
    let mut diagnostics_to_publish = vec![];
    {
      let mut state = self.state.lock().unwrap();
      let maybe_changes = state.diagnostics.take_changes();

      if let Some(diagnostic_changes) = maybe_changes {
        for specifier in diagnostic_changes {
          // TODO(@kitsonk) not totally happy with the way we collect and store
          // different types of diagnostics and offer them up to the client, we
          // do need to send "empty" vectors though when a particular feature is
          // disabled, otherwise the client will not clear down previous
          // diagnostics
          let mut diagnostics: Vec<Diagnostic> = if state.config.settings.lint {
            state
              .diagnostics
              .diagnostics_for(&specifier, &DiagnosticSource::Lint)
              .cloned()
              .collect()
          } else {
            vec![]
          };
          if state.enabled() {
            diagnostics.extend(
              state
                .diagnostics
                .diagnostics_for(&specifier, &DiagnosticSource::TypeScript)
                .cloned(),
            );
            diagnostics.extend(
              state
                .diagnostics
                .diagnostics_for(&specifier, &DiagnosticSource::Deno)
                .cloned(),
            );
          }
          let uri = specifier.clone();
          let version = state.documents.version(&specifier);
          diagnostics_to_publish.push((uri, diagnostics, version))
        }
      }
    };

    for (uri, diagnostics, version) in diagnostics_to_publish {
      self
        .client
        .publish_diagnostics(uri, diagnostics, version)
        .await;
    }

    self.performance.measure(mark);
    Ok(())
  }

  async fn update_tsconfig(&self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_tsconfig");
    let mut tsconfig = TsConfig::new(json!({
      "allowJs": true,
      "esModuleInterop": true,
      "experimentalDecorators": true,
      "isolatedModules": true,
      "jsx": "react",
      "lib": ["deno.ns", "deno.window"],
      "module": "esnext",
      "noEmit": true,
      "strict": true,
      "target": "esnext",
    }));
    {
      let mut state = self.state.lock().unwrap();
      let (maybe_config, maybe_root_uri) = {
        let config = &state.config;
        if config.settings.unstable {
          let unstable_libs = json!({
            "lib": ["deno.ns", "deno.window", "deno.unstable"]
          });
          tsconfig.merge(&unstable_libs);
        }
        (config.settings.config.clone(), config.root_uri.clone())
      };
      if let Some(config_str) = &maybe_config {
        info!("Updating TypeScript configuration from: \"{}\"", config_str);
        let config_url = if let Ok(url) = Url::from_file_path(config_str) {
          Ok(url)
        } else if let Some(root_uri) = &maybe_root_uri {
          let root_path = root_uri
            .to_file_path()
            .map_err(|_| anyhow!("Bad root_uri: {}", root_uri))?;
          let config_path = root_path.join(config_str);
          Url::from_file_path(config_path).map_err(|_| {
            anyhow!("Bad file path for configuration file: \"{}\"", config_str)
          })
        } else {
          Err(anyhow!(
            "The path to the configuration file (\"{}\") is not resolvable.",
            config_str
          ))
        }?;
        let config_path = config_url
          .to_file_path()
          .map_err(|_| anyhow!("Bad file path."))?;
        let config_text = std::fs::read_to_string(config_path.clone())
          .map_err(|err| {
            anyhow!(
              "Failed to load the configuration file at: {}. [{}]",
              config_url,
              err
            )
          })?;
        let (value, maybe_ignored_options) =
          parse_config(&config_text, &config_path)?;
        tsconfig.merge(&value);
        state.maybe_config_uri = Some(config_url);
        if let Some(ignored_options) = maybe_ignored_options {
          // TODO(@kitsonk) turn these into diagnostics that can be sent to the
          // client
          warn!("{}", ignored_options);
        }
      }
    }
    self
      .ts_server
      .request(
        self.state_snapshot().await,
        tsc::RequestMethod::Configure(tsconfig),
      )
      .await?;
    self.performance.measure(mark);
    Ok(())
  }

  fn get_performance(&self) -> Value {
    let averages = self.performance.averages();
    json!({ "averages": averages })
  }

  /// Similar to `deno cache` on the command line, where modules will be cached
  /// in the Deno cache, including any of their dependencies.
  async fn cache(&self, params: CacheParams) -> LspResult<bool> {
    let mark = self.performance.mark("cache");
    {
      let (referrer, maybe_import_map) = {
        let state = self.state.lock().unwrap();
        let referrer = state.url_map.normalize_url(&params.referrer.uri);
        (referrer, state.maybe_import_map.clone())
      };
      if !params.uris.is_empty() {
        let mut specifiers = vec![];
        {
          let state = self.state.lock().unwrap();
          for identifier in params.uris {
            specifiers.push(state.url_map.normalize_url(&identifier.uri));
          }
        };
        for specifier in specifiers {
          sources::cache(&specifier, &maybe_import_map)
            .await
            .map_err(|err| {
              error!("{}", err);
              LspError::internal_error()
            })?;
        }
      } else {
        sources::cache(&referrer, &maybe_import_map)
          .await
          .map_err(|err| {
            error!("{}", err);
            LspError::internal_error()
          })?;
      }

      // now that we have dependencies loaded, we need to re-analyze them and
      // invalidate some diagnostics
      {
        let mut state = self.state.lock().unwrap();
        if state.documents.contains_key(&referrer) {
          if let Some(source) = state.documents.content(&referrer).unwrap() {
            state.analyze_dependencies(&referrer, &source);
          }
          state.diagnostics.invalidate(&referrer);
        }
      }
    }

    self.prepare_diagnostics().await.map_err(|err| {
      error!("{}", err);
      LspError::internal_error()
    })?;
    self.performance.measure(mark);
    Ok(true)
  }
}

impl LanguageServerState {
  fn new(performance: Performance) -> Self {
    let maybe_custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(maybe_custom_root)
      .expect("could not access DENO_DIR");
    let location = dir.root.join("deps");
    let sources = Sources::new(&location);

    Self {
      assets: Default::default(),
      config: Default::default(),
      diagnostics: Default::default(),
      documents: Default::default(),
      maybe_config_uri: Default::default(),
      maybe_import_map: Default::default(),
      maybe_import_map_uri: Default::default(),
      navigation_trees: Default::default(),
      performance,
      sources,
      ts_fixable_diagnostics: Default::default(),
      url_map: Default::default(),
    }
  }

  /// Analyzes dependencies of a document that has been opened in the editor and
  /// sets the dependencies property on the document.
  fn analyze_dependencies(
    &mut self,
    specifier: &ModuleSpecifier,
    source: &str,
  ) {
    if let Some((mut deps, _)) = analysis::analyze_dependencies(
      specifier,
      source,
      &MediaType::from(specifier),
      &self.maybe_import_map,
    ) {
      for (_, dep) in deps.iter_mut() {
        if dep.maybe_type.is_none() {
          if let Some(ResolvedDependency::Resolved(resolved)) = &dep.maybe_code
          {
            dep.maybe_type = self.sources.get_maybe_types(resolved);
          }
        }
      }
      if let Err(err) = self.documents.set_dependencies(specifier, Some(deps)) {
        error!("{}", err);
      }
    }
  }

  fn enabled(&self) -> bool {
    self.config.settings.enable
  }

  /// Searches assets and documents for a line index. If the line index cannot
  /// be found, `None` is returned.
  pub fn get_line_index(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<LineIndex> {
    let mark = self.performance.mark("get_line_index");
    let maybe_line_index = if specifier.scheme() == "asset" {
      if let Some(asset) = self.assets.get(specifier) {
        Some(asset.line_index)
      } else {
        None
      }
    } else {
      let documents = &self.documents;
      if documents.contains_key(specifier) {
        documents.line_index(specifier)
      } else {
        self.sources.get_line_index(specifier)
      }
    };
    self.performance.measure(mark);
    maybe_line_index
  }

  fn snapshot(&self) -> StateSnapshot {
    StateSnapshot {
      assets: self.assets.clone(),
      documents: self.documents.clone(),
      performance: self.performance.clone(),
      sources: self.sources.clone(),
    }
  }

  pub fn update_import_map(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_import_map");
    let (maybe_import_map, maybe_root_uri) = {
      let config = &self.config;
      (config.settings.import_map.clone(), config.root_uri.clone())
    };
    if let Some(import_map_str) = &maybe_import_map {
      info!("Updating import map from: \"{}\"", import_map_str);
      let import_map_url = if let Ok(url) = Url::from_file_path(import_map_str)
      {
        Ok(url)
      } else if let Some(root_uri) = &maybe_root_uri {
        let root_path = root_uri
          .to_file_path()
          .map_err(|_| anyhow!("Bad root_uri: {}", root_uri))?;
        let import_map_path = root_path.join(import_map_str);
        Url::from_file_path(import_map_path).map_err(|_| {
          anyhow!("Bad file path for import map: {:?}", import_map_str)
        })
      } else {
        Err(anyhow!(
          "The path to the import map (\"{}\") is not resolvable.",
          import_map_str
        ))
      }?;
      let import_map_path = import_map_url
        .to_file_path()
        .map_err(|_| anyhow!("Bad file path."))?;
      let import_map_json =
        std::fs::read_to_string(import_map_path).map_err(|err| {
          anyhow!(
            "Failed to load the import map at: {}. [{}]",
            import_map_url,
            err
          )
        })?;
      let import_map =
        ImportMap::from_json(&import_map_url.to_string(), &import_map_json)?;
      self.maybe_import_map_uri = Some(import_map_url);
      self.maybe_import_map = Some(import_map);
    } else {
      self.maybe_import_map = None;
    }
    self.performance.measure(mark);
    Ok(())
  }

  pub(crate) fn document_version(
    &self,
    specifier: ModuleSpecifier,
  ) -> Option<i32> {
    self.documents.version(&specifier)
  }
}

#[lspower::async_trait]
impl lspower::LanguageServer for LanguageServer {
  async fn initialize(
    &self,
    params: InitializeParams,
  ) -> LspResult<InitializeResult> {
    info!("Starting Deno language server...");
    let mark = self.performance.mark("initialize");

    let capabilities = capabilities::server_capabilities(&params.capabilities);

    let version = format!(
      "{} ({}, {})",
      crate::version::deno(),
      env!("PROFILE"),
      env!("TARGET")
    );
    info!("  version: {}", version);

    let server_info = ServerInfo {
      name: "deno-language-server".to_string(),
      version: Some(version),
    };

    if let Some(client_info) = params.client_info {
      info!(
        "Connected to \"{}\" {}",
        client_info.name,
        client_info.version.unwrap_or_default(),
      );
    }

    {
      let mut state = self.state.lock().unwrap();
      let config = &mut state.config;
      config.root_uri = params.root_uri;
      if let Some(value) = params.initialization_options {
        config.update(value)?;
      }
      config.update_capabilities(&params.capabilities);
    }

    if let Err(err) = self.update_tsconfig().await {
      warn!("Updating tsconfig has errored: {}", err);
    }

    let res = self
      .ts_server
      .request(self.state_snapshot().await, tsc::RequestMethod::GetAssets)
      .await
      .map_err(|err| {
        error!("Unable to get assets: {}", err);
        LspError::internal_error()
      })?;
    let assets: HashMap<String, String> = from_value(res).map_err(|err| {
      error!("Unable to get assets: {}", err);
      LspError::internal_error()
    })?;
    {
      let mut state = self.state.lock().unwrap();
      for (url_str, data) in assets {
        let specifier = resolve_url(&url_str).unwrap();
        let asset = AssetDocument::new(data);
        state.assets.insert(specifier, asset);
      }
    }

    if capabilities.code_action_provider.is_some() {
      let res = self
        .ts_server
        .request(
          self.state_snapshot().await,
          tsc::RequestMethod::GetSupportedCodeFixes,
        )
        .await
        .map_err(|err| {
          error!("Unable to get fixable diagnostics: {}", err);
          LspError::internal_error()
        })?;
      let fixable_diagnostics: Vec<String> =
        from_value(res).map_err(|err| {
          error!("Unable to get fixable diagnostics: {}", err);
          LspError::internal_error()
        })?;
      self.state.lock().unwrap().ts_fixable_diagnostics = fixable_diagnostics;
    }

    self.performance.measure(mark);
    Ok(InitializeResult {
      capabilities,
      server_info: Some(server_info),
    })
  }

  async fn initialized(&self, _params: InitializedParams) {
    let (update_import_map_res, workspace_did_change_watched_files) = {
      let mut state = self.state.lock().unwrap();
      let res = state.update_import_map();
      (
        res,
        state
          .config
          .client_capabilities
          .workspace_did_change_watched_files,
      )
    };

    // Check to see if we need to setup the import map
    if let Err(err) = update_import_map_res {
      self
        .client
        .show_message(MessageType::Warning, err.to_string())
        .await;
    }

    if workspace_did_change_watched_files {
      // we are going to watch all the JSON files in the workspace, and the
      // notification handler will pick up any of the changes of those files we
      // are interested in.
      let watch_registration_options =
        DidChangeWatchedFilesRegistrationOptions {
          watchers: vec![FileSystemWatcher {
            glob_pattern: "**/*.json".to_string(),
            kind: Some(WatchKind::Change),
          }],
        };
      let registration = Registration {
        id: "workspace/didChangeWatchedFiles".to_string(),
        method: "workspace/didChangeWatchedFiles".to_string(),
        register_options: Some(
          serde_json::to_value(watch_registration_options).unwrap(),
        ),
      };
      if let Err(err) =
        self.client.register_capability(vec![registration]).await
      {
        warn!("Client errored on capabilities.\n{}", err);
      }
    }

    info!("Server ready.");
  }

  async fn shutdown(&self) -> LspResult<()> {
    Ok(())
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    let mark = self.performance.mark("did_open");
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    {
      let mut state = self.state.lock().unwrap();
      let specifier = state.url_map.normalize_url(&params.text_document.uri);
      state.documents.open(
        specifier.clone(),
        params.text_document.version,
        &params.text_document.text,
      );
      state.analyze_dependencies(&specifier, &params.text_document.text);
    }
    self.performance.measure(mark);

    // TODO(@kitsonk): how to better lazily do this?
    if let Err(err) = self.prepare_diagnostics().await {
      error!("{}", err);
    }
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let mark = self.performance.mark("did_change");
    {
      let mut state = self.state.lock().unwrap();
      let specifier = state.url_map.normalize_url(&params.text_document.uri);
      match state.documents.change(
        &specifier,
        params.text_document.version,
        params.content_changes,
      ) {
        Ok(Some(source)) => state.analyze_dependencies(&specifier, &source),
        Ok(_) => error!("No content returned from change."),
        Err(err) => error!("{}", err),
      }
    }
    self.performance.measure(mark);

    // TODO(@kitsonk): how to better lazily do this?
    if let Err(err) = self.prepare_diagnostics().await {
      error!("{}", err);
    }
  }

  async fn did_save(&self, _params: DidSaveTextDocumentParams) {
    // We don't need to do anything on save at the moment, but if this isn't
    // implemented, lspower complains about it not being implemented.
  }

  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    let mark = self.performance.mark("did_close");
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    {
      let mut state = self.state.lock().unwrap();
      let specifier = state.url_map.normalize_url(&params.text_document.uri);
      state.documents.close(&specifier);
      state.navigation_trees.remove(&specifier);
    }
    self.performance.measure(mark);
    // TODO(@kitsonk): how to better lazily do this?
    if let Err(err) = self.prepare_diagnostics().await {
      error!("{}", err);
    }
  }

  async fn did_change_configuration(
    &self,
    params: DidChangeConfigurationParams,
  ) {
    let mark = self.performance.mark("did_change_configuration");
    let config = if self
      .state
      .lock()
      .unwrap()
      .config
      .client_capabilities
      .workspace_configuration
    {
      self
        .client
        .configuration(vec![ConfigurationItem {
          scope_uri: None,
          section: Some("deno".to_string()),
        }])
        .await
        .map(|vec| vec.get(0).cloned())
        .unwrap_or_else(|err| {
          error!("failed to fetch the extension settings {}", err);
          None
        })
    } else {
      params
        .settings
        .as_object()
        .map(|settings| settings.get("deno"))
        .flatten()
        .cloned()
    };

    if let Some(config) = config {
      let config_res;
      let import_map_res;
      {
        let mut state = self.state.lock().unwrap();
        config_res = state.config.update(config);
        import_map_res = state.update_import_map();
      }

      if let Err(err) = config_res {
        error!("failed to update settings: {}", err);
      }
      if let Err(err) = import_map_res {
        self
          .client
          .show_message(MessageType::Warning, err.to_string())
          .await;
      }
      if let Err(err) = self.update_tsconfig().await {
        self
          .client
          .show_message(MessageType::Warning, err.to_string())
          .await;
      }
      if let Err(err) = self.prepare_diagnostics().await {
        error!("{}", err);
      }
    } else {
      error!("received empty extension settings from the client");
    }
    self.performance.measure(mark);
  }

  async fn did_change_watched_files(
    &self,
    params: DidChangeWatchedFilesParams,
  ) {
    let mark = self.performance.mark("did_change_watched_files");
    let mut import_map_res = Ok(());
    let maybe_config_uri;
    {
      let mut state = self.state.lock().unwrap();
      // if the current import map has changed, we need to reload it
      if let Some(import_map_uri) = &state.maybe_import_map_uri {
        if params.changes.iter().any(|fe| *import_map_uri == fe.uri) {
          import_map_res = state.update_import_map();
        }
      }
      maybe_config_uri = state.maybe_config_uri.clone();
    };
    if let Err(err) = import_map_res {
      self
        .client
        .show_message(MessageType::Warning, err.to_string())
        .await;
    }
    // if the current tsconfig has changed, we need to reload it
    if let Some(config_uri) = &maybe_config_uri {
      if params.changes.iter().any(|fe| *config_uri == fe.uri) {
        if let Err(err) = self.update_tsconfig().await {
          self
            .client
            .show_message(MessageType::Warning, err.to_string())
            .await;
        }
      }
    }
    self.performance.measure(mark);
  }

  async fn formatting(
    &self,
    params: DocumentFormattingParams,
  ) -> LspResult<Option<Vec<TextEdit>>> {
    let mark = self.performance.mark("formatting");
    let (specifier, file_text, line_index, file_path) = {
      let state = self.state.lock().unwrap();
      let specifier = state.url_map.normalize_url(&params.text_document.uri);
      let file_text = state
        .documents
        .content(&specifier)
        .map_err(|_| {
          LspError::invalid_params(
            "The specified file could not be found in memory.",
          )
        })?
        .unwrap();
      let line_index = state.documents.line_index(&specifier);
      let file_path =
        if let Ok(file_path) = params.text_document.uri.to_file_path() {
          file_path
        } else {
          PathBuf::from(params.text_document.uri.path())
        };
      (specifier, file_text, line_index, file_path)
    };

    // TODO(lucacasonato): handle error properly
    let text_edits = tokio::task::spawn_blocking(move || {
      let config = dprint::configuration::ConfigurationBuilder::new()
        .deno()
        .build();
      // TODO(@kitsonk) this could be handled better in `cli/tools/fmt.rs` in the
      // future.
      match dprint::format_text(&file_path, &file_text, &config) {
        Ok(new_text) => {
          Some(text::get_edits(&file_text, &new_text, line_index))
        }
        Err(err) => {
          warn!("Format error: {}", err);
          None
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
      self.client.show_message(MessageType::Warning, format!("Unable to format \"{}\". Likely due to unrecoverable syntax errors in the file.", specifier)).await;
      Ok(None)
    }
  }

  async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
    let mark;
    let specifier;
    let line_index;
    let snapshot;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("hover");
      specifier = state
        .url_map
        .normalize_url(&params.text_document_position_params.text_document.uri);
      line_index = if let Some(line_index) = state.get_line_index(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
      snapshot = state.snapshot()
    }

    let req = tsc::RequestMethod::GetQuickInfo((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(snapshot, req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_quick_info: Option<tsc::QuickInfo> =
      serde_json::from_value(res).unwrap();
    if let Some(quick_info) = maybe_quick_info {
      let hover = quick_info.to_hover(&line_index);
      self.performance.measure(mark);
      Ok(Some(hover))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn code_action(
    &self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    let mark;
    let specifier;
    let fixable_diagnostics: Vec<&Diagnostic>;
    let line_index;
    let snapshot;
    let mut code_actions;
    let file_diagnostics: Vec<Diagnostic>;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("code_action");
      specifier = state.url_map.normalize_url(&params.text_document.uri);
      fixable_diagnostics = params
        .context
        .diagnostics
        .iter()
        .filter(|d| match &d.source {
          Some(source) => match source.as_str() {
            "deno-ts" => match &d.code {
              Some(NumberOrString::String(code)) => {
                state.ts_fixable_diagnostics.contains(code)
              }
              Some(NumberOrString::Number(code)) => {
                state.ts_fixable_diagnostics.contains(&code.to_string())
              }
              _ => false,
            },
            "deno" => match &d.code {
              Some(NumberOrString::String(code)) => {
                code == "no-cache" || code == "no-cache-data"
              }
              _ => false,
            },
            _ => false,
          },
          None => false,
        })
        .collect();
      if fixable_diagnostics.is_empty() {
        self.performance.measure(mark);
        return Ok(None);
      }
      line_index = state.get_line_index(&specifier).unwrap();
      code_actions = CodeActionCollection::default();
      file_diagnostics = state
        .diagnostics
        .diagnostics_for(&specifier, &DiagnosticSource::TypeScript)
        .cloned()
        .collect();
      snapshot = state.snapshot();
    }
    for diagnostic in &fixable_diagnostics {
      match diagnostic.source.as_deref() {
        Some("deno-ts") => {
          let code = match diagnostic.code.as_ref().unwrap() {
            NumberOrString::String(code) => code.to_string(),
            NumberOrString::Number(code) => code.to_string(),
          };
          let codes = vec![code];
          let req = tsc::RequestMethod::GetCodeFixes((
            specifier.clone(),
            line_index.offset_tsc(diagnostic.range.start)?,
            line_index.offset_tsc(diagnostic.range.end)?,
            codes,
          ));
          let res = self
            .ts_server
            .request(snapshot.clone(), req)
            .await
            .map_err(|err| {
              error!("Error getting actions from TypeScript: {}", err);
              LspError::internal_error()
            })?;
          let actions: Vec<tsc::CodeFixAction> =
            from_value(res).map_err(|err| {
              error!("Cannot decode actions from TypeScript: {}", err);
              LspError::internal_error()
            })?;
          let state = self.state.lock().unwrap();
          for action in actions {
            code_actions
              .add_ts_fix_action(&action, diagnostic, &state)
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
          code_actions
            .add_deno_fix_action(diagnostic)
            .map_err(|err| {
              error!("{}", err);
              LspError::internal_error()
            })?
        }
        _ => (),
      }
    }
    code_actions.set_preferred_fixes();
    let code_action_response = code_actions.get_response();
    self.performance.measure(mark);
    Ok(Some(code_action_response))
  }

  async fn code_action_resolve(
    &self,
    params: CodeAction,
  ) -> LspResult<CodeAction> {
    let mark = self.performance.mark("code_action_resolve");
    let result = if let Some(data) = params.data.clone() {
      let code_action_data: CodeActionData =
        from_value(data).map_err(|err| {
          error!("Unable to decode code action data: {}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let req = tsc::RequestMethod::GetCombinedCodeFix((
        code_action_data.specifier,
        json!(code_action_data.fix_id.clone()),
      ));
      let res = self
        .ts_server
        .request(self.state_snapshot().await, req)
        .await
        .map_err(|err| {
          error!("Unable to get combined fix from TypeScript: {}", err);
          LspError::internal_error()
        })?;
      let combined_code_actions: tsc::CombinedCodeActions = from_value(res)
        .map_err(|err| {
          error!("Cannot decode combined actions from TypeScript: {}", err);
          LspError::internal_error()
        })?;
      if combined_code_actions.commands.is_some() {
        error!("Deno does not support code actions with commands.");
        Err(LspError::invalid_request())
      } else {
        let mut code_action = params.clone();
        let state = self.state.lock().unwrap();
        code_action.edit =
          ts_changes_to_edit(&combined_code_actions.changes, &state).map_err(
            |err| {
              error!("Unable to convert changes to edits: {}", err);
              LspError::internal_error()
            },
          )?;
        Ok(code_action)
      }
    } else {
      // The code action doesn't need to be resolved
      Ok(params)
    };
    self.performance.measure(mark);
    result
  }

  async fn code_lens(
    &self,
    params: CodeLensParams,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    let mark;
    let specifier;
    let line_index;
    let settings;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() || !state.config.settings.enabled_code_lens() {
        return Ok(None);
      }
      mark = self.performance.mark("code_lens");
      specifier = state.url_map.normalize_url(&params.text_document.uri);
      line_index = state.get_line_index(&specifier).unwrap();
      settings = state.config.settings.clone();
    }

    let navigation_tree =
      self.get_navigation_tree(&specifier).await.map_err(|err| {
        error!("Failed to retrieve nav tree: {}", err);
        LspError::invalid_request()
      })?;

    // because we have to use this as a mutable in a closure, the compiler
    // can't be sure when the vector will be mutated, and so a RefCell is
    // required to "protect" the vector.
    let cl = Rc::new(RefCell::new(Vec::new()));
    navigation_tree.walk(&|i, mp| {
      let mut code_lenses = cl.borrow_mut();

      // TSC Implementations Code Lens
      if settings.enabled_code_lens_implementations() {
        let source = CodeLensSource::Implementations;
        match i.kind {
          tsc::ScriptElementKind::InterfaceElement => {
            code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
          }
          tsc::ScriptElementKind::ClassElement
          | tsc::ScriptElementKind::MemberFunctionElement
          | tsc::ScriptElementKind::MemberVariableElement
          | tsc::ScriptElementKind::MemberGetAccessorElement
          | tsc::ScriptElementKind::MemberSetAccessorElement => {
            if ABSTRACT_MODIFIER.is_match(&i.kind_modifiers) {
              code_lenses.push(i.to_code_lens(
                &line_index,
                &specifier,
                &source,
              ));
            }
          }
          _ => (),
        }
      }

      // TSC References Code Lens
      if settings.enabled_code_lens_references() {
        let source = CodeLensSource::References;
        if let Some(parent) = &mp {
          if parent.kind == tsc::ScriptElementKind::EnumElement {
            code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
          }
        }
        match i.kind {
          tsc::ScriptElementKind::FunctionElement => {
            if settings.enabled_code_lens_references_all_functions() {
              code_lenses.push(i.to_code_lens(
                &line_index,
                &specifier,
                &source,
              ));
            }
          }
          tsc::ScriptElementKind::ConstElement
          | tsc::ScriptElementKind::LetElement
          | tsc::ScriptElementKind::VariableElement => {
            if EXPORT_MODIFIER.is_match(&i.kind_modifiers) {
              code_lenses.push(i.to_code_lens(
                &line_index,
                &specifier,
                &source,
              ));
            }
          }
          tsc::ScriptElementKind::ClassElement => {
            if i.text != "<class>" {
              code_lenses.push(i.to_code_lens(
                &line_index,
                &specifier,
                &source,
              ));
            }
          }
          tsc::ScriptElementKind::InterfaceElement
          | tsc::ScriptElementKind::TypeElement
          | tsc::ScriptElementKind::EnumElement => {
            code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
          }
          tsc::ScriptElementKind::LocalFunctionElement
          | tsc::ScriptElementKind::MemberGetAccessorElement
          | tsc::ScriptElementKind::MemberSetAccessorElement
          | tsc::ScriptElementKind::ConstructorImplementationElement
          | tsc::ScriptElementKind::MemberVariableElement => {
            if let Some(parent) = &mp {
              if parent.spans[0].start != i.spans[0].start {
                match parent.kind {
                  tsc::ScriptElementKind::ClassElement
                  | tsc::ScriptElementKind::InterfaceElement
                  | tsc::ScriptElementKind::TypeElement => {
                    code_lenses.push(i.to_code_lens(
                      &line_index,
                      &specifier,
                      &source,
                    ));
                  }
                  _ => (),
                }
              }
            }
          }
          _ => (),
        }
      }
    });

    self.performance.measure(mark);
    Ok(Some(Rc::try_unwrap(cl).unwrap().into_inner()))
  }

  async fn code_lens_resolve(&self, params: CodeLens) -> LspResult<CodeLens> {
    let mark = self.performance.mark("code_lens_resolve");
    if let Some(data) = params.data.clone() {
      let code_lens_data: CodeLensData = serde_json::from_value(data)
        .map_err(|err| LspError::invalid_params(err.to_string()))?;
      let code_lens = match code_lens_data.source {
        CodeLensSource::Implementations => {
          let line_index;
          let snapshot;
          {
            let state = self.state.lock().unwrap();
            line_index =
              state.get_line_index(&code_lens_data.specifier).unwrap();
            snapshot = state.snapshot();
          }
          let req = tsc::RequestMethod::GetImplementation((
            code_lens_data.specifier.clone(),
            line_index.offset_tsc(params.range.start)?,
          ));
          let res =
            self.ts_server.request(snapshot, req).await.map_err(|err| {
              error!("Error processing TypeScript request: {}", err);
              LspError::internal_error()
            })?;
          let maybe_implementations: Option<Vec<tsc::ImplementationLocation>> =
            serde_json::from_value(res).map_err(|err| {
              error!("Error deserializing response: {}", err);
              LspError::internal_error()
            })?;
          if let Some(implementations) = maybe_implementations {
            let mut locations = Vec::new();
            let mut state = self.state.lock().unwrap();
            for implementation in implementations {
              let implementation_specifier = resolve_url(
                &implementation.document_span.file_name,
              )
              .map_err(|err| {
                error!("Invalid specifier returned from TypeScript: {}", err);
                LspError::internal_error()
              })?;
              let implementation_location =
                implementation.to_location(&line_index, &mut state);
              if !(implementation_specifier == code_lens_data.specifier
                && implementation_location.range.start == params.range.start)
              {
                locations.push(implementation_location);
              }
            }
            let command = if !locations.is_empty() {
              let title = if locations.len() > 1 {
                format!("{} implementations", locations.len())
              } else {
                "1 implementation".to_string()
              };
              let url = state
                .url_map
                .normalize_specifier(&code_lens_data.specifier)
                .map_err(|err| {
                  error!("{}", err);
                  LspError::internal_error()
                })?;
              Command {
                title,
                command: "deno.showReferences".to_string(),
                arguments: Some(vec![
                  serde_json::to_value(url).unwrap(),
                  serde_json::to_value(params.range.start).unwrap(),
                  serde_json::to_value(locations).unwrap(),
                ]),
              }
            } else {
              Command {
                title: "0 implementations".to_string(),
                command: "".to_string(),
                arguments: None,
              }
            };
            CodeLens {
              range: params.range,
              command: Some(command),
              data: None,
            }
          } else {
            let command = Command {
              title: "0 implementations".to_string(),
              command: "".to_string(),
              arguments: None,
            };
            CodeLens {
              range: params.range,
              command: Some(command),
              data: None,
            }
          }
        }
        CodeLensSource::References => {
          let line_index;
          let snapshot;
          {
            let state = self.state.lock().unwrap();
            line_index =
              state.get_line_index(&code_lens_data.specifier).unwrap();
            snapshot = state.snapshot();
          }
          let req = tsc::RequestMethod::GetReferences((
            code_lens_data.specifier.clone(),
            line_index.offset_tsc(params.range.start)?,
          ));
          let res =
            self.ts_server.request(snapshot, req).await.map_err(|err| {
              error!("Error processing TypeScript request: {}", err);
              LspError::internal_error()
            })?;
          let maybe_references: Option<Vec<tsc::ReferenceEntry>> =
            serde_json::from_value(res).map_err(|err| {
              error!("Error deserializing response: {}", err);
              LspError::internal_error()
            })?;
          if let Some(references) = maybe_references {
            let mut locations = Vec::new();
            let mut state = self.state.lock().unwrap();
            for reference in references {
              if reference.is_definition {
                continue;
              }
              let reference_specifier = resolve_url(
                &reference.document_span.file_name,
              )
              .map_err(|err| {
                error!("Invalid specifier returned from TypeScript: {}", err);
                LspError::internal_error()
              })?;
              let line_index = state
                .get_line_index(&reference_specifier)
                .ok_or_else(|| {
                  error!("Line index not found: {}", reference_specifier);
                  LspError::internal_error()
                })?;
              locations.push(reference.to_location(&line_index, &mut state));
            }
            let command = if !locations.is_empty() {
              let title = if locations.len() > 1 {
                format!("{} references", locations.len())
              } else {
                "1 reference".to_string()
              };
              let url = state
                .url_map
                .normalize_specifier(&code_lens_data.specifier)
                .map_err(|err| {
                  error!("{}", err);
                  LspError::internal_error()
                })?;
              Command {
                title,
                command: "deno.showReferences".to_string(),
                arguments: Some(vec![
                  serde_json::to_value(url).unwrap(),
                  serde_json::to_value(params.range.start).unwrap(),
                  serde_json::to_value(locations).unwrap(),
                ]),
              }
            } else {
              Command {
                title: "0 references".to_string(),
                command: "".to_string(),
                arguments: None,
              }
            };
            CodeLens {
              range: params.range,
              command: Some(command),
              data: None,
            }
          } else {
            let command = Command {
              title: "0 references".to_string(),
              command: "".to_string(),
              arguments: None,
            };
            CodeLens {
              range: params.range,
              command: Some(command),
              data: None,
            }
          }
        }
      };
      self.performance.measure(mark);
      Ok(code_lens)
    } else {
      self.performance.measure(mark);
      Err(LspError::invalid_params(
        "Code lens is missing the \"data\" property.",
      ))
    }
  }

  async fn document_highlight(
    &self,
    params: DocumentHighlightParams,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    let mark;
    let specifier;
    let line_index;
    let snapshot;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("document_highlight");
      specifier = state
        .url_map
        .normalize_url(&params.text_document_position_params.text_document.uri);
      line_index = if let Some(line_index) = state.get_line_index(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
      snapshot = state.snapshot();
    }
    let files_to_search = vec![specifier.clone()];
    let req = tsc::RequestMethod::GetDocumentHighlights((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
      files_to_search,
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(snapshot, req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_document_highlights: Option<Vec<tsc::DocumentHighlights>> =
      serde_json::from_value(res).unwrap();

    if let Some(document_highlights) = maybe_document_highlights {
      let result = document_highlights
        .into_iter()
        .map(|dh| dh.to_highlight(&line_index))
        .flatten()
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
    let mark;
    let specifier;
    let line_index;
    let snapshot;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("references");
      specifier = state
        .url_map
        .normalize_url(&params.text_document_position.text_document.uri);
      line_index = if let Some(line_index) = state.get_line_index(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
      snapshot = state.snapshot();
    }
    let req = tsc::RequestMethod::GetReferences((
      specifier,
      line_index.offset_tsc(params.text_document_position.position)?,
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(snapshot, req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_references: Option<Vec<tsc::ReferenceEntry>> =
      serde_json::from_value(res).unwrap();

    if let Some(references) = maybe_references {
      let mut results = Vec::new();
      let mut state = self.state.lock().unwrap();
      for reference in references {
        if !params.context.include_declaration && reference.is_definition {
          continue;
        }
        let reference_specifier =
          resolve_url(&reference.document_span.file_name).unwrap();
        // TODO(lucacasonato): handle error correctly
        let line_index =
          state.get_line_index(&reference_specifier).ok_or_else(|| {
            error!("Line index not found: {}", reference_specifier);
            LspError::internal_error()
          })?;
        results.push(reference.to_location(&line_index, &mut state));
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
    let mark;
    let specifier;
    let line_index;
    let snapshot;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("goto_definition");
      specifier = state
        .url_map
        .normalize_url(&params.text_document_position_params.text_document.uri);
      line_index = if let Some(line_index) = state.get_line_index(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
      snapshot = state.snapshot();
    }
    let req = tsc::RequestMethod::GetDefinition((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(snapshot, req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_definition: Option<tsc::DefinitionInfoAndBoundSpan> =
      serde_json::from_value(res).unwrap();

    if let Some(definition) = maybe_definition {
      let mut state = self.state.lock().unwrap();
      let results = definition.to_definition(&line_index, &mut state);
      self.performance.measure(mark);
      Ok(results)
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn completion(
    &self,
    params: CompletionParams,
  ) -> LspResult<Option<CompletionResponse>> {
    let mark;
    let specifier;
    let line_index;
    let snapshot;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("completion");
      specifier = state
        .url_map
        .normalize_url(&params.text_document_position.text_document.uri);
      line_index = if let Some(line_index) = state.get_line_index(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
      snapshot = state.snapshot();
    }

    let req = tsc::RequestMethod::GetCompletions((
      specifier,
      line_index.offset_tsc(params.text_document_position.position)?,
      tsc::UserPreferences {
        // TODO(lucacasonato): enable this. see https://github.com/denoland/deno/pull/8651
        include_completions_with_insert_text: Some(false),
        ..Default::default()
      },
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(snapshot, req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_completion_info: Option<tsc::CompletionInfo> =
      serde_json::from_value(res).unwrap();

    if let Some(completions) = maybe_completion_info {
      let results = completions.into_completion_response(&line_index);
      self.performance.measure(mark);
      Ok(Some(results))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn goto_implementation(
    &self,
    params: GotoImplementationParams,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    let mark;
    let specifier;
    let line_index;
    let snapshot;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("goto_implementation");
      specifier = state
        .url_map
        .normalize_url(&params.text_document_position_params.text_document.uri);
      line_index = if let Some(line_index) = state.get_line_index(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
      snapshot = state.snapshot();
    }

    let req = tsc::RequestMethod::GetImplementation((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
    ));
    let res = self.ts_server.request(snapshot, req).await.map_err(|err| {
      error!("Failed to request to tsserver {}", err);
      LspError::invalid_request()
    })?;

    let maybe_implementations: Option<Vec<tsc::ImplementationLocation>> = serde_json::from_value(res)
      .map_err(|err| {
        error!("Failed to deserialized tsserver response to Vec<ImplementationLocation> {}", err);
        LspError::internal_error()
      })?;

    let result = if let Some(implementations) = maybe_implementations {
      let mut links = Vec::new();
      let mut state = self.state.lock().unwrap();
      for implementation in implementations {
        if let Some(link) = implementation.to_link(&line_index, &mut state) {
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

  async fn rename(
    &self,
    params: RenameParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    let mark;
    let specifier;
    let line_index;
    let snapshot;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("rename");
      specifier = state
        .url_map
        .normalize_url(&params.text_document_position.text_document.uri);
      line_index = if let Some(line_index) = state.get_line_index(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
      snapshot = state.snapshot();
    }

    let req = tsc::RequestMethod::FindRenameLocations((
      specifier,
      line_index.offset_tsc(params.text_document_position.position)?,
      true,
      true,
      false,
    ));

    let res = self.ts_server.request(snapshot, req).await.map_err(|err| {
      error!("Failed to request to tsserver {}", err);
      LspError::invalid_request()
    })?;

    let maybe_locations = serde_json::from_value::<
      Option<Vec<tsc::RenameLocation>>,
    >(res)
    .map_err(|err| {
      error!(
        "Failed to deserialize tsserver response to Vec<RenameLocation> {}",
        err
      );
      LspError::internal_error()
    })?;

    if let Some(locations) = maybe_locations {
      let rename_locations = tsc::RenameLocations { locations };
      let mut state = self.state.lock().unwrap();
      let workspace_edits = rename_locations
        .into_workspace_edit(&params.new_name, &mut state)
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

  async fn request_else(
    &self,
    method: &str,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    match method {
      "deno/cache" => match params.map(serde_json::from_value) {
        Some(Ok(params)) => Ok(Some(
          serde_json::to_value(self.cache(params).await?).map_err(|err| {
            error!("Failed to serialize cache response: {}", err);
            LspError::internal_error()
          })?,
        )),
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      },
      "deno/performance" => Ok(Some(self.get_performance())),
      "deno/virtualTextDocument" => match params.map(serde_json::from_value) {
        Some(Ok(params)) => {
          let state = self.state.lock().unwrap();
          Ok(Some(
            serde_json::to_value(state.virtual_text_document(params)).map_err(
              |err| {
                error!(
                  "Failed to serialize virtual_text_document response: {}",
                  err
                );
                LspError::internal_error()
              },
            )?,
          ))
        }
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      },
      _ => {
        error!("Got a {} request, but no handler is defined", method);
        Err(LspError::method_not_found())
      }
    }
  }

  async fn signature_help(
    &self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    let mark;
    let specifier;
    let line_index;
    let snapshot;
    {
      let state = self.state.lock().unwrap();
      if !state.enabled() {
        return Ok(None);
      }
      mark = self.performance.mark("signature_help");
      specifier = state
        .url_map
        .normalize_url(&params.text_document_position_params.text_document.uri);
      line_index = if let Some(line_index) = state.get_line_index(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
      snapshot = state.snapshot();
    }
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
    let req = tsc::RequestMethod::GetSignatureHelpItems((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
      options,
    ));
    let res = self.ts_server.request(snapshot, req).await.map_err(|err| {
      error!("Failed to request to tsserver: {}", err);
      LspError::invalid_request()
    })?;
    let maybe_signature_help_items: Option<tsc::SignatureHelpItems> =
      serde_json::from_value(res).map_err(|err| {
        error!("Failed to deserialize tsserver response: {}", err);
        LspError::internal_error()
      })?;

    if let Some(signature_help_items) = maybe_signature_help_items {
      let signature_help = signature_help_items.into_signature_help();
      self.performance.measure(mark);
      Ok(Some(signature_help))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheParams {
  /// The document currently open in the editor.  If there are no `uris`
  /// supplied, the referrer will be cached.
  referrer: TextDocumentIdentifier,
  /// Any documents that have been specifically asked to be cached via the
  /// command.
  uris: Vec<TextDocumentIdentifier>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct VirtualTextDocumentParams {
  text_document: TextDocumentIdentifier,
}

// These are implementations of custom commands supported by the LSP
impl LanguageServerState {
  fn virtual_text_document(
    &self,
    params: VirtualTextDocumentParams,
  ) -> Option<String> {
    let mark = self.performance.mark("virtual_text_document");
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    let contents = if specifier.as_str() == "deno:/status.md" {
      let mut contents = String::new();

      contents.push_str(&format!(
        r#"# Deno Language Server Status

  - Documents in memory: {}
"#,
        self.documents.len()
      ));
      contents.push_str("\n## Performance\n\n");
      for average in self.performance.averages() {
        contents.push_str(&format!(
          "  - {}: {}ms ({})\n",
          average.name, average.average_duration, average.count
        ));
      }
      Some(contents)
    } else {
      match specifier.scheme() {
        "asset" => {
          if let Some(asset) = self.assets.get(&specifier) {
            Some(asset.text)
          } else {
            error!("Missing asset: {}", specifier);
            None
          }
        }
        _ => {
          if let Some(source) = self.sources.get_source(&specifier) {
            Some(source)
          } else {
            error!("The cached sources was not found: {}", specifier);
            None
          }
        }
      }
    };
    self.performance.measure(mark);
    contents
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::lsp::performance::PerformanceAverage;
  use lspower::jsonrpc;
  use lspower::ExitedError;
  use lspower::LspService;
  use std::fs;
  use std::task::Poll;
  use std::time::Instant;
  use tower_test::mock::Spawn;

  enum LspResponse<V>
  where
    V: FnOnce(Value),
  {
    None,
    RequestAny,
    Request(u64, Value),
    RequestAssert(V),
    RequestFixture(u64, String),
  }

  type LspTestHarnessRequest = (&'static str, LspResponse<fn(Value)>);

  struct LspTestHarness {
    requests: Vec<LspTestHarnessRequest>,
    service: Spawn<LspService>,
  }

  impl LspTestHarness {
    pub fn new(requests: Vec<LspTestHarnessRequest>) -> Self {
      let (service, _) = LspService::new(LanguageServer::new);
      let service = Spawn::new(service);
      Self { requests, service }
    }

    async fn run(&mut self) {
      for (req_path_str, expected) in self.requests.iter() {
        assert_eq!(self.service.poll_ready(), Poll::Ready(Ok(())));
        let fixtures_path = test_util::root_path().join("cli/tests/lsp");
        assert!(fixtures_path.is_dir());
        let req_path = fixtures_path.join(req_path_str);
        let req_str = fs::read_to_string(req_path).unwrap();
        let req: jsonrpc::Incoming = serde_json::from_str(&req_str).unwrap();
        let response: Result<Option<jsonrpc::Outgoing>, ExitedError> =
          self.service.call(req).await;
        match response {
          Ok(result) => match expected {
            LspResponse::None => assert_eq!(result, None),
            LspResponse::RequestAny => match result {
              Some(jsonrpc::Outgoing::Response(_)) => (),
              _ => panic!("unexpected result: {:?}", result),
            },
            LspResponse::Request(id, value) => match result {
              Some(jsonrpc::Outgoing::Response(resp)) => assert_eq!(
                resp,
                jsonrpc::Response::ok(jsonrpc::Id::Number(*id), value.clone())
              ),
              _ => panic!("unexpected result: {:?}", result),
            },
            LspResponse::RequestAssert(assert) => match result {
              Some(jsonrpc::Outgoing::Response(resp)) => assert(json!(resp)),
              _ => panic!("unexpected result: {:?}", result),
            },
            LspResponse::RequestFixture(id, res_path_str) => {
              let res_path = fixtures_path.join(res_path_str);
              let res_str = fs::read_to_string(res_path).unwrap();
              match result {
                Some(jsonrpc::Outgoing::Response(resp)) => assert_eq!(
                  resp,
                  jsonrpc::Response::ok(
                    jsonrpc::Id::Number(*id),
                    serde_json::from_str(&res_str).unwrap()
                  )
                ),
                _ => panic!("unexpected result: {:?}", result),
              }
            }
          },
          Err(err) => panic!("Error result: {}", err),
        }
      }
    }
  }

  #[tokio::test]
  async fn test_startup_shutdown() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_hover() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification.json", LspResponse::None),
      (
        "hover_request.json",
        LspResponse::Request(
          2,
          json!({
            "contents": [
              {
                "language": "typescript",
                "value": "const Deno.args: string[]"
              },
              "Returns the script arguments to the program. If for example we run a\nprogram:\n\ndeno run --allow-read https://deno.land/std/examples/cat.ts /etc/passwd\n\nThen `Deno.args` will contain:\n\n[ \"/etc/passwd\" ]"
            ],
            "range": {
              "start": {
                "line": 0,
                "character": 17
              },
              "end": {
                "line": 0,
                "character": 21
              }
            }
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_hover_asset() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_asset.json", LspResponse::None),
      ("definition_request_asset.json", LspResponse::RequestAny),
      (
        "virtual_text_document_request.json",
        LspResponse::RequestAny,
      ),
      (
        "hover_request_asset.json",
        LspResponse::Request(
          5,
          json!({
            "contents": [
              {
                "language": "typescript",
                "value": "interface Date",
              },
              "Enables basic storage and retrieval of dates and times."
            ],
            "range": {
              "start": {
                "line": 109,
                "character": 10,
              },
              "end": {
                "line": 109,
                "character": 14,
              }
            }
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_hover_disabled() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request_disabled.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification.json", LspResponse::None),
      ("hover_request.json", LspResponse::Request(2, json!(null))),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_hover_unstable_disabled() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_unstable.json", LspResponse::None),
      (
        "hover_request.json",
        LspResponse::Request(
          2,
          json!({
            "contents": [
              {
                "language": "typescript",
                "value": "any"
              }
            ],
            "range": {
              "start": {
                "line": 0,
                "character": 17
              },
              "end": {
                "line": 0,
                "character": 28
              }
            }
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_hover_unstable_enabled() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request_unstable.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_unstable.json", LspResponse::None),
      (
        "hover_request.json",
        LspResponse::Request(
          2,
          json!({
            "contents": [
              {
                "language": "typescript",
                "value": "const Deno.permissions: Deno.Permissions"
              },
              "**UNSTABLE**: Under consideration to move to `navigator.permissions` to\nmatch web API. It could look like `navigator.permissions.query({ name: Deno.symbols.read })`."
            ],
            "range": {
              "start": {
                "line": 0,
                "character": 17
              },
              "end": {
                "line": 0,
                "character": 28
              }
            }
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_hover_change_mbc() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_mbc.json", LspResponse::None),
      ("did_change_notification_mbc.json", LspResponse::None),
      (
        "hover_request_mbc.json",
        LspResponse::Request(
          2,
          json!({
            "contents": [
              {
                "language": "typescript",
                "value": "const b: \"😃\"",
              },
              "",
            ],
            "range": {
              "start": {
                "line": 2,
                "character": 13,
              },
              "end": {
                "line": 2,
                "character": 14,
              },
            }
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_format_mbc() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_mbc_fmt.json", LspResponse::None),
      (
        "formatting_request_mbc_fmt.json",
        LspResponse::Request(
          2,
          json!([
            {
              "range": {
                "start": {
                  "line": 0,
                  "character": 12
                },
                "end": {
                  "line": 0,
                  "character": 13,
                }
              },
              "newText": "\""
            },
            {
              "range": {
                "start": {
                  "line": 0,
                  "character": 21
                },
                "end": {
                  "line": 0,
                  "character": 22
                }
              },
              "newText": "\";"
            },
            {
              "range": {
                "start": {
                  "line": 1,
                  "character": 12,
                },
                "end": {
                  "line": 1,
                  "character": 13,
                }
              },
              "newText": "\""
            },
            {
              "range": {
                "start": {
                  "line": 1,
                  "character": 23,
                },
                "end": {
                  "line": 1,
                  "character": 25,
                }
              },
              "newText": "\");"
            }
          ]),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_large_doc_change() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_large.json", LspResponse::None),
      ("did_change_notification_large.json", LspResponse::None),
      ("did_change_notification_large_02.json", LspResponse::None),
      ("did_change_notification_large_03.json", LspResponse::None),
      ("hover_request_large_01.json", LspResponse::RequestAny),
      ("hover_request_large_02.json", LspResponse::RequestAny),
      ("hover_request_large_03.json", LspResponse::RequestAny),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    let time = Instant::now();
    harness.run().await;
    assert!(
      time.elapsed().as_millis() <= 15000,
      "the execution time exceeded 10000ms"
    );
  }

  #[tokio::test]
  async fn test_rename() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("rename_did_open_notification.json", LspResponse::None),
      (
        "rename_request.json",
        LspResponse::Request(
          2,
          json!({
            "documentChanges": [{
              "textDocument": {
                "uri": "file:///a/file.ts",
                "version": 1,
              },
              "edits": [{
                "range": {
                  "start": {
                    "line": 0,
                    "character": 4
                  },
                  "end": {
                    "line": 0,
                    "character": 12
                  }
                },
                "newText": "variable_modified"
              }, {
                "range": {
                  "start": {
                    "line": 1,
                    "character": 12
                  },
                  "end": {
                    "line": 1,
                    "character": 20
                  }
                },
                "newText": "variable_modified"
              }]
            }]
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_code_lens_request() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      (
        "did_open_notification_cl_references.json",
        LspResponse::None,
      ),
      (
        "code_lens_request.json",
        LspResponse::Request(
          2,
          json!([
            {
              "range": {
                "start": {
                  "line": 0,
                  "character": 6,
                },
                "end": {
                  "line": 0,
                  "character": 7,
                }
              },
              "data": {
                "specifier": "file:///a/file.ts",
                "source": "references",
              },
            },
            {
              "range": {
                "start": {
                  "line": 1,
                  "character": 2,
                },
                "end": {
                  "line": 1,
                  "character": 3,
                }
              },
              "data": {
                "specifier": "file:///a/file.ts",
                "source": "references",
              }
            }
          ]),
        ),
      ),
      (
        "code_lens_resolve_request.json",
        LspResponse::Request(
          4,
          json!({
            "range": {
              "start": {
                "line": 0,
                "character": 6,
              },
              "end": {
                "line": 0,
                "character": 7,
              }
            },
            "command": {
              "title": "1 reference",
              "command": "deno.showReferences",
              "arguments": [
                "file:///a/file.ts",
                {
                  "line": 0,
                  "character": 6,
                },
                [
                  {
                    "uri": "file:///a/file.ts",
                    "range": {
                      "start": {
                        "line": 12,
                        "character": 14,
                      },
                      "end": {
                        "line": 12,
                        "character": 15,
                      }
                    }
                  }
                ],
              ]
            }
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_signature_help() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      (
        "signature_help_did_open_notification.json",
        LspResponse::None,
      ),
      (
        "signature_help_request_01.json",
        LspResponse::Request(
          1,
          json!({
            "signatures": [
              {
                "label": "add(a: number, b: number): number",
                "documentation": "Adds two numbers.",
                "parameters": [
                  {
                    "label": "a: number",
                    "documentation": "This is a first number."
                  },
                  {
                    "label": "b: number",
                    "documentation": "This is a second number."
                  }
                ]
              }
            ],
            "activeSignature": 0,
            "activeParameter": 0
          }),
        ),
      ),
      (
        "signature_help_did_change_notification.json",
        LspResponse::None,
      ),
      (
        "signature_help_request_02.json",
        LspResponse::Request(
          2,
          json!({
            "signatures": [
              {
                "label": "add(a: number, b: number): number",
                "documentation": "Adds two numbers.",
                "parameters": [
                  {
                    "label": "a: number",
                    "documentation": "This is a first number."
                  },
                  {
                    "label": "b: number",
                    "documentation": "This is a second number."
                  }
                ]
              }
            ],
            "activeSignature": 0,
            "activeParameter": 1
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_code_lens_impl_request() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_cl_impl.json", LspResponse::None),
      (
        "code_lens_request.json",
        LspResponse::Request(
          2,
          json!([
            {
              "range": {
                "start": {
                  "line": 0,
                  "character": 10,
                },
                "end": {
                  "line": 0,
                  "character": 11,
                }
              },
              "data": {
                "specifier": "file:///a/file.ts",
                "source": "implementations",
              },
            },
            {
              "range": {
                "start": {
                  "line": 0,
                  "character": 10,
                },
                "end": {
                  "line": 0,
                  "character": 11,
                }
              },
              "data": {
                "specifier": "file:///a/file.ts",
                "source": "references",
              },
            },
            {
              "range": {
                "start": {
                  "line": 4,
                  "character": 6,
                },
                "end": {
                  "line": 4,
                  "character": 7,
                }
              },
              "data": {
                "specifier": "file:///a/file.ts",
                "source": "references",
              },
            },
          ]),
        ),
      ),
      (
        "code_lens_resolve_request_impl.json",
        LspResponse::Request(
          4,
          json!({
            "range": {
              "start": {
                "line": 0,
                "character": 10,
              },
              "end": {
                "line": 0,
                "character": 11,
              }
            },
            "command": {
              "title": "1 implementation",
              "command": "deno.showReferences",
              "arguments": [
                "file:///a/file.ts",
                {
                  "line": 0,
                  "character": 10,
                },
                [
                  {
                    "uri": "file:///a/file.ts",
                    "range": {
                      "start": {
                        "line": 4,
                        "character": 6,
                      },
                      "end": {
                        "line": 4,
                        "character": 7,
                      }
                    }
                  }
                ],
              ]
            }
          }),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[derive(Deserialize)]
  struct CodeLensResponse {
    pub result: Option<Vec<CodeLens>>,
  }

  #[derive(Deserialize)]
  struct CodeLensResolveResponse {
    pub result: CodeLens,
  }

  #[tokio::test]
  async fn test_code_lens_non_doc_nav_tree() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_asset.json", LspResponse::None),
      ("references_request_asset.json", LspResponse::RequestAny),
      (
        "virtual_text_document_request.json",
        LspResponse::RequestAny,
      ),
      (
        "code_lens_request_asset.json",
        LspResponse::RequestAssert(|value| {
          let resp: CodeLensResponse = serde_json::from_value(value).unwrap();
          let lenses = resp.result.unwrap();
          assert!(lenses.len() > 50);
        }),
      ),
      (
        "code_lens_resolve_request_asset.json",
        LspResponse::RequestAssert(|value| {
          let resp: CodeLensResolveResponse =
            serde_json::from_value(value).unwrap();
          assert!(resp.result.command.is_some());
        }),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_code_actions() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_code_action.json", LspResponse::None),
      (
        "code_action_request.json",
        LspResponse::RequestFixture(2, "code_action_response.json".to_string()),
      ),
      (
        "code_action_resolve_request.json",
        LspResponse::RequestFixture(
          4,
          "code_action_resolve_request_response.json".to_string(),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[tokio::test]
  async fn test_code_actions_deno_cache() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_cache.json", LspResponse::None),
      (
        "code_action_request_cache.json",
        LspResponse::RequestFixture(
          2,
          "code_action_response_cache.json".to_string(),
        ),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }

  #[derive(Deserialize)]
  struct PerformanceAverages {
    averages: Vec<PerformanceAverage>,
  }
  #[derive(Deserialize)]
  struct PerformanceResponse {
    result: PerformanceAverages,
  }

  #[tokio::test]
  async fn test_deno_performance_request() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification.json", LspResponse::None),
      (
        "hover_request.json",
        LspResponse::Request(
          2,
          json!({
            "contents": [
              {
                "language": "typescript",
                "value": "const Deno.args: string[]"
              },
              "Returns the script arguments to the program. If for example we run a\nprogram:\n\ndeno run --allow-read https://deno.land/std/examples/cat.ts /etc/passwd\n\nThen `Deno.args` will contain:\n\n[ \"/etc/passwd\" ]"
            ],
            "range": {
              "start": {
                "line": 0,
                "character": 17
              },
              "end": {
                "line": 0,
                "character": 21
              }
            }
          }),
        ),
      ),
      (
        "performance_request.json",
        LspResponse::RequestAssert(|value| {
          let resp: PerformanceResponse =
            serde_json::from_value(value).unwrap();
          assert_eq!(resp.result.averages.len(), 12);
        }),
      ),
      (
        "shutdown_request.json",
        LspResponse::Request(3, json!(null)),
      ),
      ("exit_notification.json", LspResponse::None),
    ]);
    harness.run().await;
  }
}
