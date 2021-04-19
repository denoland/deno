// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use dprint_plugin_typescript as dprint;
use log::error;
use log::info;
use log::warn;
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
use tokio::fs;

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
use super::completions;
use super::config::Config;
use super::diagnostics;
use super::diagnostics::DiagnosticSource;
use super::documents::DocumentCache;
use super::performance::Performance;
use super::registries;
use super::sources;
use super::sources::Sources;
use super::text;
use super::text::LineIndex;
use super::tsc;
use super::tsc::AssetDocument;
use super::tsc::Assets;
use super::tsc::TsServer;
use super::urls;

pub const REGISTRIES_PATH: &str = "registries";
const SOURCES_PATH: &str = "deps";

lazy_static::lazy_static! {
  static ref ABSTRACT_MODIFIER: Regex = Regex::new(r"\babstract\b").unwrap();
  static ref EXPORT_MODIFIER: Regex = Regex::new(r"\bexport\b").unwrap();
}

#[derive(Debug, Clone)]
pub struct LanguageServer(Arc<tokio::sync::Mutex<Inner>>);

#[derive(Debug, Clone, Default)]
pub struct StateSnapshot {
  pub assets: Assets,
  pub config: Config,
  pub documents: DocumentCache,
  pub module_registries: registries::ModuleRegistry,
  pub performance: Performance,
  pub sources: Sources,
}

#[derive(Debug)]
pub(crate) struct Inner {
  /// Cached versions of "fixed" assets that can either be inlined in Rust or
  /// are part of the TypeScript snapshot and have to be fetched out.
  assets: Assets,
  /// The LSP client that this LSP server is connected to.
  client: Client,
  /// Configuration information.
  config: Config,
  diagnostics_server: diagnostics::DiagnosticsServer,
  /// The "in-memory" documents in the editor which can be updated and changed.
  documents: DocumentCache,
  /// Handles module registries, which allow discovery of modules
  module_registries: registries::ModuleRegistry,
  /// The path to the module registries cache
  module_registries_location: PathBuf,
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
  /// An abstraction that handles interactions with TypeScript.
  ts_server: Arc<TsServer>,
  /// A map of specifiers and URLs used to translate over the LSP.
  pub url_map: urls::LspUrlMap,
}

impl LanguageServer {
  pub fn new(client: Client) -> Self {
    Self(Arc::new(tokio::sync::Mutex::new(Inner::new(client))))
  }
}

impl Inner {
  fn new(client: Client) -> Self {
    let maybe_custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(maybe_custom_root)
      .expect("could not access DENO_DIR");
    let module_registries_location = dir.root.join(REGISTRIES_PATH);
    let module_registries =
      registries::ModuleRegistry::new(&module_registries_location);
    let sources_location = dir.root.join(SOURCES_PATH);
    let sources = Sources::new(&sources_location);
    let ts_server = Arc::new(TsServer::new());
    let performance = Performance::default();
    let diagnostics_server = diagnostics::DiagnosticsServer::new();

    Self {
      assets: Default::default(),
      client,
      config: Default::default(),
      diagnostics_server,
      documents: Default::default(),
      maybe_config_uri: Default::default(),
      maybe_import_map: Default::default(),
      maybe_import_map_uri: Default::default(),
      module_registries,
      module_registries_location,
      navigation_trees: Default::default(),
      performance,
      sources,
      ts_fixable_diagnostics: Default::default(),
      ts_server,
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
    let media_type = MediaType::from(specifier);
    if let Ok(parsed_module) =
      analysis::parse_module(specifier, source, &media_type)
    {
      let (mut deps, _) = analysis::analyze_dependencies(
        specifier,
        &media_type,
        &parsed_module,
        &self.maybe_import_map,
      );
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

  /// Searches assets, open documents and external sources for a line_index,
  /// which might be performed asynchronously, hydrating in memory caches for
  /// subsequent requests.
  pub(crate) async fn get_line_index(
    &mut self,
    specifier: ModuleSpecifier,
  ) -> Result<LineIndex, AnyError> {
    let mark = self.performance.mark("get_line_index");
    let result = if specifier.scheme() == "asset" {
      if let Some(asset) = self.get_asset(&specifier).await? {
        Ok(asset.line_index)
      } else {
        Err(anyhow!("asset is missing: {}", specifier))
      }
    } else if let Some(line_index) = self.documents.line_index(&specifier) {
      Ok(line_index)
    } else if let Some(line_index) = self.sources.get_line_index(&specifier) {
      Ok(line_index)
    } else {
      Err(anyhow!("Unable to find line index for: {}", specifier))
    };
    self.performance.measure(mark);
    result
  }

  /// Only searches already cached assets and documents for a line index.  If
  /// the line index cannot be found, `None` is returned.
  fn get_line_index_sync(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<LineIndex> {
    let mark = self.performance.mark("get_line_index_sync");
    let maybe_line_index = if specifier.scheme() == "asset" {
      if let Some(Some(asset)) = self.assets.get(specifier) {
        Some(asset.line_index.clone())
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

  // TODO(@kitsonk) we really should find a better way to just return the
  // content as a `&str`, or be able to get the byte at a particular offset
  // which is all that this API that is consuming it is trying to do at the
  // moment
  /// Searches already cached assets and documents and returns its text
  /// content. If not found, `None` is returned.
  fn get_text_content(&self, specifier: &ModuleSpecifier) -> Option<String> {
    if specifier.scheme() == "asset" {
      self
        .assets
        .get(specifier)
        .map(|o| o.clone().map(|a| a.text))?
    } else if self.documents.contains_key(specifier) {
      self.documents.content(specifier).unwrap()
    } else {
      self.sources.get_source(specifier)
    }
  }

  async fn get_navigation_tree(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<tsc::NavigationTree, AnyError> {
    let mark = self.performance.mark("get_navigation_tree");
    if let Some(navigation_tree) = self.navigation_trees.get(specifier) {
      self.performance.measure(mark);
      Ok(navigation_tree.clone())
    } else {
      let navigation_tree: tsc::NavigationTree = self
        .ts_server
        .request(
          self.snapshot(),
          tsc::RequestMethod::GetNavigationTree(specifier.clone()),
        )
        .await?;
      self
        .navigation_trees
        .insert(specifier.clone(), navigation_tree.clone());
      self.performance.measure(mark);
      Ok(navigation_tree)
    }
  }

  pub(crate) fn snapshot(&self) -> StateSnapshot {
    StateSnapshot {
      assets: self.assets.clone(),
      config: self.config.clone(),
      documents: self.documents.clone(),
      module_registries: self.module_registries.clone(),
      performance: self.performance.clone(),
      sources: self.sources.clone(),
    }
  }

  pub async fn update_import_map(&mut self) -> Result<(), AnyError> {
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
        fs::read_to_string(import_map_path).await.map_err(|err| {
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

  async fn update_registries(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_registries");
    for (registry, enabled) in self.config.settings.suggest.imports.hosts.iter()
    {
      if *enabled {
        info!("Enabling auto complete registry for: {}", registry);
        self.module_registries.enable(registry).await?;
      } else {
        info!("Disabling auto complete registry for: {}", registry);
        self.module_registries.disable(registry).await?;
      }
    }
    self.performance.measure(mark);
    Ok(())
  }

  async fn update_tsconfig(&mut self) -> Result<(), AnyError> {
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
      "useDefineForClassFields": true,
    }));
    let (maybe_config, maybe_root_uri) = {
      let config = &self.config;
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
      let config_text =
        fs::read_to_string(config_path.clone())
          .await
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
      self.maybe_config_uri = Some(config_url);
      if let Some(ignored_options) = maybe_ignored_options {
        // TODO(@kitsonk) turn these into diagnostics that can be sent to the
        // client
        warn!("{}", ignored_options);
      }
    }
    let _ok: bool = self
      .ts_server
      .request(self.snapshot(), tsc::RequestMethod::Configure(tsconfig))
      .await?;
    self.performance.measure(mark);
    Ok(())
  }

  pub(crate) fn document_version(
    &self,
    specifier: ModuleSpecifier,
  ) -> Option<i32> {
    self.documents.version(&specifier)
  }

  async fn get_asset(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<AssetDocument>, AnyError> {
    if let Some(maybe_asset) = self.assets.get(specifier) {
      return Ok(maybe_asset.clone());
    } else {
      let maybe_asset =
        tsc::get_asset(&specifier, &self.ts_server, self.snapshot()).await?;
      self.assets.insert(specifier.clone(), maybe_asset.clone());
      Ok(maybe_asset)
    }
  }
}

// lspower::LanguageServer methods. This file's LanguageServer delegates to us.
impl Inner {
  async fn initialize(
    &mut self,
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
      let config = &mut self.config;
      config.root_uri = params.root_uri;
      if let Some(value) = params.initialization_options {
        config.update(value)?;
      }
      config.update_capabilities(&params.capabilities);
    }

    if let Err(err) = self.update_tsconfig().await {
      warn!("Updating tsconfig has errored: {}", err);
    }

    if capabilities.code_action_provider.is_some() {
      let fixable_diagnostics: Vec<String> = self
        .ts_server
        .request(self.snapshot(), tsc::RequestMethod::GetSupportedCodeFixes)
        .await
        .map_err(|err| {
          error!("Unable to get fixable diagnostics: {}", err);
          LspError::internal_error()
        })?;
      self.ts_fixable_diagnostics = fixable_diagnostics;
    }

    self.performance.measure(mark);
    Ok(InitializeResult {
      capabilities,
      server_info: Some(server_info),
    })
  }

  async fn initialized(&mut self, _: InitializedParams) {
    // Check to see if we need to setup the import map
    if let Err(err) = self.update_import_map().await {
      self
        .client
        .show_message(MessageType::Warning, err.to_string())
        .await;
    }
    // Check to see if we need to setup any module registries
    if let Err(err) = self.update_registries().await {
      self
        .client
        .show_message(MessageType::Warning, err.to_string())
        .await;
    }

    if self
      .config
      .client_capabilities
      .workspace_did_change_watched_files
    {
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

  async fn did_open(&mut self, params: DidOpenTextDocumentParams) {
    let mark = self.performance.mark("did_open");
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    self.documents.open(
      specifier.clone(),
      params.text_document.version,
      &params.text_document.text,
    );
    self.analyze_dependencies(&specifier, &params.text_document.text);
    self.performance.measure(mark);

    if let Err(err) = self.diagnostics_server.update() {
      error!("{}", err);
    }
  }

  async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
    let mark = self.performance.mark("did_change");
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    match self.documents.change(
      &specifier,
      params.text_document.version,
      params.content_changes,
    ) {
      Ok(Some(source)) => self.analyze_dependencies(&specifier, &source),
      Ok(_) => error!("No content returned from change."),
      Err(err) => error!("{}", err),
    }
    self.performance.measure(mark);

    if let Err(err) = self.diagnostics_server.update() {
      error!("{}", err);
    }
  }

  async fn did_close(&mut self, params: DidCloseTextDocumentParams) {
    let mark = self.performance.mark("did_close");
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    self.documents.close(&specifier);
    self.navigation_trees.remove(&specifier);

    self.performance.measure(mark);
    if let Err(err) = self.diagnostics_server.update() {
      error!("{}", err);
    }
  }

  async fn did_change_configuration(
    &mut self,
    params: DidChangeConfigurationParams,
  ) {
    let mark = self.performance.mark("did_change_configuration");
    let config = if self.config.client_capabilities.workspace_configuration {
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
      if let Err(err) = self.config.update(config) {
        error!("failed to update settings: {}", err);
      }
      if let Err(err) = self.update_import_map().await {
        self
          .client
          .show_message(MessageType::Warning, err.to_string())
          .await;
      }
      if let Err(err) = self.update_registries().await {
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
      if let Err(err) = self.diagnostics_server.update() {
        error!("{}", err);
      }
    } else {
      error!("received empty extension settings from the client");
    }
    self.performance.measure(mark);
  }

  async fn did_change_watched_files(
    &mut self,
    params: DidChangeWatchedFilesParams,
  ) {
    let mark = self.performance.mark("did_change_watched_files");
    // if the current import map has changed, we need to reload it
    if let Some(import_map_uri) = &self.maybe_import_map_uri {
      if params.changes.iter().any(|fe| *import_map_uri == fe.uri) {
        if let Err(err) = self.update_import_map().await {
          self
            .client
            .show_message(MessageType::Warning, err.to_string())
            .await;
        }
      }
    }
    // if the current tsconfig has changed, we need to reload it
    if let Some(config_uri) = &self.maybe_config_uri {
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
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    let file_text = self
      .documents
      .content(&specifier)
      .map_err(|_| {
        LspError::invalid_params(
          "The specified file could not be found in memory.",
        )
      })?
      .unwrap();
    let line_index = self.documents.line_index(&specifier);
    let file_path =
      if let Ok(file_path) = params.text_document.uri.to_file_path() {
        file_path
      } else {
        PathBuf::from(params.text_document.uri.path())
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
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("hover");
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
    let req = tsc::RequestMethod::GetQuickInfo((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
    ));
    let maybe_quick_info: Option<tsc::QuickInfo> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Unable to get quick info: {}", err);
        LspError::internal_error()
      })?;
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
    &mut self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    if !self.enabled() {
      return Ok(None);
    }

    let mark = self.performance.mark("code_action");
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
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
    let line_index = self.get_line_index_sync(&specifier).unwrap();
    let mut code_actions = CodeActionCollection::default();
    let file_diagnostics = self
      .diagnostics_server
      .get(specifier.clone(), DiagnosticSource::TypeScript)
      .await
      .map_err(|err| {
        error!("Unable to get diagnostics: {}", err);
        LspError::internal_error()
      })?;
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
          let actions: Vec<tsc::CodeFixAction> =
            match self.ts_server.request(self.snapshot(), req).await {
              Ok(items) => items,
              Err(err) => {
                // sometimes tsc reports errors when retrieving code actions
                // because they don't reflect the current state of the document
                // so we will log them to the output, but we won't send an error
                // message back to the client.
                error!("Error getting actions from TypeScript: {}", err);
                Vec::new()
              }
            };
          for action in actions {
            code_actions
              .add_ts_fix_action(&action, diagnostic, self)
              .await
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
    &mut self,
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
      let combined_code_actions: tsc::CombinedCodeActions = self
        .ts_server
        .request(self.snapshot(), req)
        .await
        .map_err(|err| {
          error!("Unable to get combined fix from TypeScript: {}", err);
          LspError::internal_error()
        })?;
      if combined_code_actions.commands.is_some() {
        error!("Deno does not support code actions with commands.");
        Err(LspError::invalid_request())
      } else {
        let mut code_action = params.clone();
        code_action.edit =
          ts_changes_to_edit(&combined_code_actions.changes, self)
            .await
            .map_err(|err| {
              error!("Unable to convert changes to edits: {}", err);
              LspError::internal_error()
            })?;
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
    &mut self,
    params: CodeLensParams,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    if !self.enabled() || !self.config.settings.enabled_code_lens() {
      return Ok(None);
    }

    let mark = self.performance.mark("code_lens");
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    let line_index = self.get_line_index_sync(&specifier).unwrap();
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
      if self.config.settings.code_lens.implementations {
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
      if self.config.settings.code_lens.references {
        let source = CodeLensSource::References;
        if let Some(parent) = &mp {
          if parent.kind == tsc::ScriptElementKind::EnumElement {
            code_lenses.push(i.to_code_lens(&line_index, &specifier, &source));
          }
        }
        match i.kind {
          tsc::ScriptElementKind::FunctionElement => {
            if self.config.settings.code_lens.references_all_functions {
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

  async fn code_lens_resolve(
    &mut self,
    params: CodeLens,
  ) -> LspResult<CodeLens> {
    let mark = self.performance.mark("code_lens_resolve");
    if let Some(data) = params.data.clone() {
      let code_lens_data: CodeLensData = serde_json::from_value(data)
        .map_err(|err| LspError::invalid_params(err.to_string()))?;
      let code_lens = match code_lens_data.source {
        CodeLensSource::Implementations => {
          let line_index =
            self.get_line_index_sync(&code_lens_data.specifier).unwrap();
          let req = tsc::RequestMethod::GetImplementation((
            code_lens_data.specifier.clone(),
            line_index.offset_tsc(params.range.start)?,
          ));
          let maybe_implementations: Option<Vec<tsc::ImplementationLocation>> =
            self.ts_server.request(self.snapshot(), req).await.map_err(
              |err| {
                error!("Error processing TypeScript request: {}", err);
                LspError::internal_error()
              },
            )?;
          if let Some(implementations) = maybe_implementations {
            let mut locations = Vec::new();
            for implementation in implementations {
              let implementation_specifier = resolve_url(
                &implementation.document_span.file_name,
              )
              .map_err(|err| {
                error!("Invalid specifier returned from TypeScript: {}", err);
                LspError::internal_error()
              })?;
              let implementation_location =
                implementation.to_location(&line_index, self);
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
              let url = self
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
          let line_index =
            self.get_line_index_sync(&code_lens_data.specifier).unwrap();
          let req = tsc::RequestMethod::GetReferences((
            code_lens_data.specifier.clone(),
            line_index.offset_tsc(params.range.start)?,
          ));
          let maybe_references: Option<Vec<tsc::ReferenceEntry>> =
            self.ts_server.request(self.snapshot(), req).await.map_err(
              |err| {
                error!("Error processing TypeScript request: {}", err);
                LspError::internal_error()
              },
            )?;
          if let Some(references) = maybe_references {
            let mut locations = Vec::new();
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
              let line_index = self
                .get_line_index(reference_specifier)
                .await
                .map_err(|err| {
                error!("Unable to get line index: {}", err);
                LspError::internal_error()
              })?;
              locations.push(reference.to_location(&line_index, self));
            }
            let command = if !locations.is_empty() {
              let title = if locations.len() > 1 {
                format!("{} references", locations.len())
              } else {
                "1 reference".to_string()
              };
              let url = self
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
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("document_highlight");
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
    let files_to_search = vec![specifier.clone()];
    let req = tsc::RequestMethod::GetDocumentHighlights((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
      files_to_search,
    ));
    let maybe_document_highlights: Option<Vec<tsc::DocumentHighlights>> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Unable to get document highlights from TypeScript: {}", err);
        LspError::internal_error()
      })?;

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
    &mut self,
    params: ReferenceParams,
  ) -> LspResult<Option<Vec<Location>>> {
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("references");
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
    let req = tsc::RequestMethod::GetReferences((
      specifier,
      line_index.offset_tsc(params.text_document_position.position)?,
    ));
    let maybe_references: Option<Vec<tsc::ReferenceEntry>> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Unable to get references from TypeScript: {}", err);
        LspError::internal_error()
      })?;

    if let Some(references) = maybe_references {
      let mut results = Vec::new();
      for reference in references {
        if !params.context.include_declaration && reference.is_definition {
          continue;
        }
        let reference_specifier =
          resolve_url(&reference.document_span.file_name).unwrap();
        // TODO(lucacasonato): handle error correctly
        let line_index =
          self.get_line_index(reference_specifier).await.unwrap();
        results.push(reference.to_location(&line_index, self));
      }

      self.performance.measure(mark);
      Ok(Some(results))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn goto_definition(
    &mut self,
    params: GotoDefinitionParams,
  ) -> LspResult<Option<GotoDefinitionResponse>> {
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("goto_definition");
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };
    let req = tsc::RequestMethod::GetDefinition((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
    ));
    let maybe_definition: Option<tsc::DefinitionInfoAndBoundSpan> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Unable to get definition from TypeScript: {}", err);
        LspError::internal_error()
      })?;

    if let Some(definition) = maybe_definition {
      let results = definition.to_definition(&line_index, self).await;
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
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("completion");
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);
    // Import specifiers are something wholly internal to Deno, so for
    // completions, we will use internal logic and if there are completions
    // for imports, we will return those and not send a message into tsc, where
    // other completions come from.
    let response = if let Some(response) = completions::get_import_completions(
      &specifier,
      &params.text_document_position.position,
      &self.snapshot(),
    )
    .await
    {
      Some(response)
    } else {
      let line_index =
        if let Some(line_index) = self.get_line_index_sync(&specifier) {
          line_index
        } else {
          return Err(LspError::invalid_params(format!(
            "An unexpected specifier ({}) was provided.",
            specifier
          )));
        };
      let trigger_character = if let Some(context) = &params.context {
        context.trigger_character.clone()
      } else {
        None
      };
      let position =
        line_index.offset_tsc(params.text_document_position.position)?;
      let req = tsc::RequestMethod::GetCompletions((
        specifier.clone(),
        position,
        tsc::GetCompletionsAtPositionOptions {
          user_preferences: tsc::UserPreferences {
            include_completions_with_insert_text: Some(true),
            ..Default::default()
          },
          trigger_character,
        },
      ));
      let maybe_completion_info: Option<tsc::CompletionInfo> = self
        .ts_server
        .request(self.snapshot(), req)
        .await
        .map_err(|err| {
          error!("Unable to get completion info from TypeScript: {}", err);
          LspError::internal_error()
        })?;

      if let Some(completions) = maybe_completion_info {
        let results = completions.as_completion_response(
          &line_index,
          &self.config.settings.suggest,
          &specifier,
          position,
        );
        Some(results)
      } else {
        None
      }
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn completion_resolve(
    &mut self,
    params: CompletionItem,
  ) -> LspResult<CompletionItem> {
    let mark = self.performance.mark("completion_resolve");
    let completion_item = if let Some(data) = &params.data {
      let data: completions::CompletionItemData =
        serde_json::from_value(data.clone()).map_err(|err| {
          error!("{}", err);
          LspError::invalid_params(
            "Could not decode data field of completion item.",
          )
        })?;
      if let Some(data) = data.tsc {
        let req = tsc::RequestMethod::GetCompletionDetails(data.into());
        let maybe_completion_info: Option<tsc::CompletionEntryDetails> =
          self.ts_server.request(self.snapshot(), req).await.map_err(
            |err| {
              error!("Unable to get completion info from TypeScript: {}", err);
              LspError::internal_error()
            },
          )?;
        if let Some(completion_info) = maybe_completion_info {
          completion_info.as_completion_item(&params)
        } else {
          error!(
            "Received an undefined response from tsc for completion details."
          );
          params
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
    &mut self,
    params: GotoImplementationParams,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("goto_implementation");
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::GetImplementation((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
    ));
    let maybe_implementations: Option<Vec<tsc::ImplementationLocation>> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let result = if let Some(implementations) = maybe_implementations {
      let mut links = Vec::new();
      for implementation in implementations {
        if let Some(link) = implementation.to_link(&line_index, self).await {
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
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("folding_range");
    let specifier = self.url_map.normalize_url(&params.text_document.uri);

    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::GetOutliningSpans(specifier.clone());
    let outlining_spans: Vec<tsc::OutliningSpan> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let response = if !outlining_spans.is_empty() {
      let text_content =
        self.get_text_content(&specifier).ok_or_else(|| {
          LspError::invalid_params(format!(
            "An unexpected specifier ({}) was provided.",
            specifier
          ))
        })?;
      Some(
        outlining_spans
          .iter()
          .map(|span| {
            span.to_folding_range(
              &line_index,
              text_content.as_str().as_bytes(),
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

  async fn rename(
    &mut self,
    params: RenameParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("rename");
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);

    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::FindRenameLocations((
      specifier,
      line_index.offset_tsc(params.text_document_position.position)?,
      true,
      true,
      false,
    ));

    let maybe_locations: Option<Vec<tsc::RenameLocation>> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

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

  async fn request_else(
    &mut self,
    method: &str,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    match method {
      "deno/cache" => match params.map(serde_json::from_value) {
        Some(Ok(params)) => self.cache(params).await,
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      },
      "deno/performance" => Ok(Some(self.get_performance())),
      "deno/reloadImportRegistries" => self.reload_import_registries().await,
      "deno/virtualTextDocument" => match params.map(serde_json::from_value) {
        Some(Ok(params)) => Ok(Some(
          serde_json::to_value(self.virtual_text_document(params).await?)
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
      },
      _ => {
        error!("Got a {} request, but no handler is defined", method);
        Err(LspError::method_not_found())
      }
    }
  }

  async fn selection_range(
    &self,
    params: SelectionRangeParams,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("selection_range");
    let specifier = self.url_map.normalize_url(&params.text_document.uri);

    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let mut selection_ranges = Vec::<SelectionRange>::new();
    for position in params.positions {
      let req = tsc::RequestMethod::GetSmartSelectionRange((
        specifier.clone(),
        line_index.offset_tsc(position)?,
      ));

      let selection_range: tsc::SelectionRange = self
        .ts_server
        .request(self.snapshot(), req)
        .await
        .map_err(|err| {
          error!("Failed to request to tsserver {}", err);
          LspError::invalid_request()
        })?;

      selection_ranges.push(selection_range.to_selection_range(&line_index));
    }
    self.performance.measure(mark);
    Ok(Some(selection_ranges))
  }

  async fn signature_help(
    &self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    if !self.enabled() {
      return Ok(None);
    }
    let mark = self.performance.mark("signature_help");
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
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
    let req = tsc::RequestMethod::GetSignatureHelpItems((
      specifier,
      line_index.offset_tsc(params.text_document_position_params.position)?,
      options,
    ));
    let maybe_signature_help_items: Option<tsc::SignatureHelpItems> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver: {}", err);
        LspError::invalid_request()
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

#[lspower::async_trait]
impl lspower::LanguageServer for LanguageServer {
  async fn initialize(
    &self,
    params: InitializeParams,
  ) -> LspResult<InitializeResult> {
    let mut language_server = self.0.lock().await;
    let client = language_server.client.clone();
    let ts_server = language_server.ts_server.clone();
    language_server
      .diagnostics_server
      .start(self.0.clone(), client, ts_server);
    language_server.initialize(params).await
  }

  async fn initialized(&self, params: InitializedParams) {
    self.0.lock().await.initialized(params).await
  }

  async fn shutdown(&self) -> LspResult<()> {
    self.0.lock().await.shutdown().await
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    self.0.lock().await.did_open(params).await
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    self.0.lock().await.did_change(params).await
  }

  async fn did_save(&self, _params: DidSaveTextDocumentParams) {
    // We don't need to do anything on save at the moment, but if this isn't
    // implemented, lspower complains about it not being implemented.
  }

  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    self.0.lock().await.did_close(params).await
  }

  async fn did_change_configuration(
    &self,
    params: DidChangeConfigurationParams,
  ) {
    self.0.lock().await.did_change_configuration(params).await
  }

  async fn did_change_watched_files(
    &self,
    params: DidChangeWatchedFilesParams,
  ) {
    self.0.lock().await.did_change_watched_files(params).await
  }

  async fn formatting(
    &self,
    params: DocumentFormattingParams,
  ) -> LspResult<Option<Vec<TextEdit>>> {
    self.0.lock().await.formatting(params).await
  }

  async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
    self.0.lock().await.hover(params).await
  }

  async fn code_action(
    &self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    self.0.lock().await.code_action(params).await
  }

  async fn code_action_resolve(
    &self,
    params: CodeAction,
  ) -> LspResult<CodeAction> {
    self.0.lock().await.code_action_resolve(params).await
  }

  async fn code_lens(
    &self,
    params: CodeLensParams,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    self.0.lock().await.code_lens(params).await
  }

  async fn code_lens_resolve(&self, params: CodeLens) -> LspResult<CodeLens> {
    self.0.lock().await.code_lens_resolve(params).await
  }

  async fn document_highlight(
    &self,
    params: DocumentHighlightParams,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    self.0.lock().await.document_highlight(params).await
  }

  async fn references(
    &self,
    params: ReferenceParams,
  ) -> LspResult<Option<Vec<Location>>> {
    self.0.lock().await.references(params).await
  }

  async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
  ) -> LspResult<Option<GotoDefinitionResponse>> {
    self.0.lock().await.goto_definition(params).await
  }

  async fn completion(
    &self,
    params: CompletionParams,
  ) -> LspResult<Option<CompletionResponse>> {
    self.0.lock().await.completion(params).await
  }

  async fn completion_resolve(
    &self,
    params: CompletionItem,
  ) -> LspResult<CompletionItem> {
    self.0.lock().await.completion_resolve(params).await
  }

  async fn goto_implementation(
    &self,
    params: GotoImplementationParams,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    self.0.lock().await.goto_implementation(params).await
  }

  async fn folding_range(
    &self,
    params: FoldingRangeParams,
  ) -> LspResult<Option<Vec<FoldingRange>>> {
    self.0.lock().await.folding_range(params).await
  }

  async fn rename(
    &self,
    params: RenameParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    self.0.lock().await.rename(params).await
  }

  async fn request_else(
    &self,
    method: &str,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    self.0.lock().await.request_else(method, params).await
  }

  async fn selection_range(
    &self,
    params: SelectionRangeParams,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    self.0.lock().await.selection_range(params).await
  }

  async fn signature_help(
    &self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    self.0.lock().await.signature_help(params).await
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
impl Inner {
  /// Similar to `deno cache` on the command line, where modules will be cached
  /// in the Deno cache, including any of their dependencies.
  async fn cache(&mut self, params: CacheParams) -> LspResult<Option<Value>> {
    let mark = self.performance.mark("cache");
    let referrer = self.url_map.normalize_url(&params.referrer.uri);
    if !params.uris.is_empty() {
      for identifier in &params.uris {
        let specifier = self.url_map.normalize_url(&identifier.uri);
        sources::cache(&specifier, &self.maybe_import_map)
          .await
          .map_err(|err| {
            error!("{}", err);
            LspError::internal_error()
          })?;
      }
    } else {
      sources::cache(&referrer, &self.maybe_import_map)
        .await
        .map_err(|err| {
          error!("{}", err);
          LspError::internal_error()
        })?;
    }
    // now that we have dependencies loaded, we need to re-analyze them and
    // invalidate some diagnostics
    if self.documents.contains_key(&referrer) {
      if let Some(source) = self.documents.content(&referrer).unwrap() {
        self.analyze_dependencies(&referrer, &source);
      }
      self
        .diagnostics_server
        .invalidate(referrer)
        .map_err(|err| {
          error!("{}", err);
          LspError::internal_error()
        })?;
    }

    self.diagnostics_server.update().map_err(|err| {
      error!("{}", err);
      LspError::internal_error()
    })?;
    self.performance.measure(mark);
    Ok(Some(json!(true)))
  }

  fn get_performance(&self) -> Value {
    let averages = self.performance.averages();
    json!({ "averages": averages })
  }

  async fn reload_import_registries(&mut self) -> LspResult<Option<Value>> {
    fs::remove_dir_all(&self.module_registries_location)
      .await
      .map_err(|err| {
        error!("Unable to remove registries cache: {}", err);
        LspError::internal_error()
      })?;
    self.module_registries =
      registries::ModuleRegistry::new(&self.module_registries_location);
    self.update_registries().await.map_err(|err| {
      error!("Unable to update registries: {}", err);
      LspError::internal_error()
    })?;
    Ok(Some(json!(true)))
  }

  async fn virtual_text_document(
    &mut self,
    params: VirtualTextDocumentParams,
  ) -> LspResult<Option<String>> {
    let mark = self.performance.mark("virtual_text_document");
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    let contents = if specifier.as_str() == "deno:/status.md" {
      let mut contents = String::new();
      let mut documents_specifiers = self.documents.specifiers();
      documents_specifiers.sort();
      let mut sources_specifiers = self.sources.specifiers();
      sources_specifiers.sort();
      let measures = self.performance.to_vec();

      contents.push_str(&format!(
        r#"# Deno Language Server Status

  - <details><summary>Documents in memory: {}</summary>

    - {}

  </details>

  - <details><summary>Sources in memory: {}</summary>

    - {}
  
  </details>

  - <details><summary>Performance measures: {}</summary>

    - {}

  </details>
"#,
        self.documents.len(),
        documents_specifiers
          .into_iter()
          .map(|s| s.to_string())
          .collect::<Vec<String>>()
          .join("\n    - "),
        self.sources.len(),
        sources_specifiers
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
      ));
      contents
        .push_str("\n## Performance\n\n|Name|Duration|Count|\n|---|---|---|\n");
      let mut averages = self.performance.averages();
      averages.sort();
      for average in averages {
        contents.push_str(&format!(
          "|{}|{}ms|{}|\n",
          average.name, average.average_duration, average.count
        ));
      }
      Some(contents)
    } else {
      match specifier.scheme() {
        "asset" => {
          if let Some(asset) = self
            .get_asset(&specifier)
            .await
            .map_err(|_| LspError::internal_error())?
          {
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
            error!("The cached source was not found: {}", specifier);
            None
          }
        }
      }
    };
    self.performance.measure(mark);
    Ok(contents)
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
    Delay(u64),
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
        let response: Result<Option<jsonrpc::Outgoing>, ExitedError> =
          if req_path_str.is_empty() {
            Ok(None)
          } else {
            let req_path = fixtures_path.join(req_path_str);
            let req_str = fs::read_to_string(req_path).unwrap();
            let req: jsonrpc::Incoming =
              serde_json::from_str(&req_str).unwrap();
            self.service.call(req).await
          };
        match response {
          Ok(result) => match expected {
            LspResponse::None => assert_eq!(result, None),
            LspResponse::Delay(millis) => {
              tokio::time::sleep(tokio::time::Duration::from_millis(*millis))
                .await
            }
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
                "character": 27
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
                "value": "function Deno.openPlugin(filename: string): number"
              },
              "**UNSTABLE**: new API, yet to be vetted.\n\nOpen and initialize a plugin.\n\n```ts\nconst rid = Deno.openPlugin(\"./path/to/some/plugin.so\");\nconst opId = Deno.core.ops()[\"some_op\"];\nconst response = Deno.core.dispatch(opId, new Uint8Array([1,2,3,4]));\nconsole.log(`Response from plugin ${response}`);\n```\n\nRequires `allow-plugin` permission.\n\nThe plugin system is not stable and will change in the future, hence the\nlack of docs. For now take a look at the example\nhttps://github.com/denoland/deno/tree/master/test_plugin"
            ],
            "range": {
              "start": {
                "line": 0,
                "character": 17
              },
              "end": {
                "line": 0,
                "character": 27
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
                "value": "const b: \"🦕😃\"",
              },
              "",
            ],
            "range": {
              "start": {
                "line": 2,
                "character": 15,
              },
              "end": {
                "line": 2,
                "character": 16,
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
      time.elapsed().as_millis() <= 10000,
      "the execution time exceeded 10000ms"
    );
  }

  #[tokio::test]
  async fn test_folding_range() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      (
        "folding_range_did_open_notification.json",
        LspResponse::None,
      ),
      (
        "folding_range_request.json",
        LspResponse::Request(
          2,
          json!([
            {
              "startLine": 0,
              "endLine": 12,
              "kind": "region"
            },
            {
              "startLine": 1,
              "endLine": 3,
              "kind": "comment"
            },
            {
              "startLine": 4,
              "endLine": 10
            },
            {
              "startLine": 5,
              "endLine": 9
            },
            {
              "startLine": 6,
              "endLine": 7
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
  async fn test_selection_range() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      (
        "selection_range_did_open_notification.json",
        LspResponse::None,
      ),
      (
        "selection_range_request.json",
        LspResponse::Request(
          2,
          json!([{
            "range": {
              "start": {
                "line": 2,
                "character": 8
              },
              "end": {
                "line": 2,
                "character": 9
              }
            },
            "parent": {
              "range": {
                "start": {
                  "line": 2,
                  "character": 8
                },
                "end": {
                  "line": 2,
                  "character": 15
                }
              },
              "parent": {
                "range": {
                  "start": {
                    "line": 2,
                    "character": 4
                  },
                  "end": {
                    "line": 4,
                    "character": 5
                  }
                },
                "parent": {
                  "range": {
                    "start": {
                      "line": 1,
                      "character": 13
                    },
                    "end": {
                      "line": 6,
                      "character": 2
                    }
                  },
                  "parent": {
                    "range": {
                      "start": {
                        "line": 1,
                        "character": 2
                      },
                      "end": {
                        "line": 6,
                        "character": 3
                      }
                    },
                    "parent": {
                      "range": {
                        "start": {
                          "line": 0,
                          "character": 11
                        },
                        "end": {
                          "line": 7,
                          "character": 0
                        }
                      },
                      "parent": {
                        "range": {
                          "start": {
                            "line": 0,
                            "character": 0
                          },
                          "end": {
                            "line": 7,
                            "character": 1
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }]),
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
      ("", LspResponse::Delay(500)),
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
  struct CompletionResult {
    pub result: Option<CompletionResponse>,
  }

  #[tokio::test]
  async fn test_completions() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      ("did_open_notification_completions.json", LspResponse::None),
      (
        "completion_request.json",
        LspResponse::RequestAssert(|value| {
          let response: CompletionResult =
            serde_json::from_value(value).unwrap();
          let result = response.result.unwrap();
          match result {
            CompletionResponse::List(list) => {
              // there should be at least 90 completions for `Deno.`
              assert!(list.items.len() > 90);
            }
            _ => panic!("unexpected result"),
          }
        }),
      ),
      (
        "completion_resolve_request.json",
        LspResponse::Request(
          4,
          json!({
            "label": "build",
            "kind": 6,
            "detail": "const Deno.build: {\n    target: string;\n    arch: \"x86_64\";\n    os: \"darwin\" | \"linux\" | \"windows\";\n    vendor: string;\n    env?: string | undefined;\n}",
            "documentation": {
              "kind": "markdown",
              "value": "Build related information."
            },
            "sortText": "1",
            "insertTextFormat": 1,
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
  async fn test_completions_optional() {
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      (
        "did_open_notification_completion_optional.json",
        LspResponse::None,
      ),
      (
        "completion_request_optional.json",
        LspResponse::Request(
          2,
          json!({
            "isIncomplete": false,
            "items": [
              {
                "label": "b?",
                "kind": 5,
                "sortText": "1",
                "filterText": "b",
                "insertText": "b",
                "data": {
                  "tsc": {
                    "specifier": "file:///a/file.ts",
                    "position": 79,
                    "name": "b",
                    "useCodeSnippet": false
                  }
                }
              }
            ]
          }),
        ),
      ),
      (
        "completion_resolve_request_optional.json",
        LspResponse::Request(
          4,
          json!({
            "label": "b?",
            "kind": 5,
            "detail": "(property) A.b?: string | undefined",
            "documentation": {
              "kind": "markdown",
              "value": ""
            },
            "sortText": "1",
            "filterText": "b",
            "insertText": "b"
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
  async fn test_completions_registry() {
    let _g = test_util::http_server();
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request_registry.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      (
        "did_open_notification_completion_registry.json",
        LspResponse::None,
      ),
      (
        "completion_request_registry.json",
        LspResponse::RequestAssert(|value| {
          let response: CompletionResult =
            serde_json::from_value(value).unwrap();
          let result = response.result.unwrap();
          if let CompletionResponse::List(list) = result {
            assert_eq!(list.items.len(), 3);
          } else {
            panic!("unexpected result");
          }
        }),
      ),
      (
        "completion_resolve_request_registry.json",
        LspResponse::Request(
          4,
          json!({
            "label": "v2.0.0",
            "kind": 19,
            "detail": "(version)",
            "sortText": "0000000003",
            "filterText": "http://localhost:4545/x/a@v2.0.0",
            "textEdit": {
              "range": {
                "start": {
                  "line": 0,
                  "character": 20
                },
                "end": {
                  "line": 0,
                  "character": 46
                }
              },
              "newText": "http://localhost:4545/x/a@v2.0.0"
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
  async fn test_completion_registry_empty_specifier() {
    let _g = test_util::http_server();
    let mut harness = LspTestHarness::new(vec![
      ("initialize_request_registry.json", LspResponse::RequestAny),
      ("initialized_notification.json", LspResponse::None),
      (
        "did_open_notification_completion_registry_02.json",
        LspResponse::None,
      ),
      (
        "completion_request_registry_02.json",
        LspResponse::Request(
          2,
          json!({
            "isIncomplete": false,
            "items": [
              {
                "label": ".",
                "kind": 19,
                "detail": "(local)",
                "sortText": "1",
                "insertText": "."
              },
              {
                "label": "..",
                "kind": 19,
                "detail": "(local)",
                "sortText": "1",
                "insertText": ".."
              },
              {
                "label": "http://localhost:4545",
                "kind": 19,
                "detail": "(registry)",
                "sortText": "2",
                "textEdit": {
                  "range": {
                    "start": {
                      "line": 0,
                      "character": 20
                    },
                    "end": {
                      "line": 0,
                      "character": 20
                    }
                  },
                  "newText": "http://localhost:4545"
                }
              }
            ]
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
          // the len can be variable since some of the parts of the language
          // server run in separate threads and may not add to performance by
          // the time the results are checked.
          assert!(resp.result.averages.len() >= 6);
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
