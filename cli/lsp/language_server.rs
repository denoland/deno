// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use dprint_plugin_typescript as dprint;
use lspower::jsonrpc::Error as LspError;
use lspower::jsonrpc::Result as LspResult;
use lspower::lsp_types::request::*;
use lspower::lsp_types::*;
use lspower::Client;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::fs;

use crate::deno_dir;
use crate::import_map::ImportMap;
use crate::media_type::MediaType;
use crate::tsc_config::parse_config;
use crate::tsc_config::TsConfig;

use super::analysis;
use super::capabilities;
use super::config::Config;
use super::diagnostics;
use super::diagnostics::DiagnosticCollection;
use super::diagnostics::DiagnosticSource;
use super::memory_cache::MemoryCache;
use super::sources;
use super::sources::Sources;
use super::text;
use super::text::apply_content_changes;
use super::tsc;
use super::tsc::TsServer;
use super::utils;

#[derive(Debug, Clone)]
pub struct LanguageServer {
  assets: Arc<Mutex<HashMap<ModuleSpecifier, Option<String>>>>,
  client: Client,
  ts_server: TsServer,
  config: Arc<Mutex<Config>>,
  doc_data: Arc<Mutex<HashMap<ModuleSpecifier, DocumentData>>>,
  file_cache: Arc<Mutex<MemoryCache>>,
  sources: Arc<Mutex<Sources>>,
  diagnostics: Arc<Mutex<DiagnosticCollection>>,
  maybe_config_uri: Arc<Mutex<Option<Url>>>,
  maybe_import_map: Arc<Mutex<Option<ImportMap>>>,
  maybe_import_map_uri: Arc<Mutex<Option<Url>>>,
}

#[derive(Debug, Clone, Default)]
pub struct StateSnapshot {
  pub assets: Arc<Mutex<HashMap<ModuleSpecifier, Option<String>>>>,
  pub doc_data: HashMap<ModuleSpecifier, DocumentData>,
  pub file_cache: Arc<Mutex<MemoryCache>>,
  pub sources: Arc<Mutex<Sources>>,
}

impl LanguageServer {
  pub fn new(client: Client) -> Self {
    let maybe_custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(maybe_custom_root)
      .expect("could not access DENO_DIR");
    let location = dir.root.join("deps");
    let sources = Arc::new(Mutex::new(Sources::new(&location)));

    LanguageServer {
      assets: Default::default(),
      client,
      ts_server: TsServer::new(),
      config: Default::default(),
      doc_data: Default::default(),
      file_cache: Default::default(),
      sources,
      diagnostics: Default::default(),
      maybe_config_uri: Default::default(),
      maybe_import_map: Default::default(),
      maybe_import_map_uri: Default::default(),
    }
  }

  fn enabled(&self) -> bool {
    let config = self.config.lock().unwrap();
    config.settings.enable
  }

  pub async fn get_line_index(
    &self,
    specifier: ModuleSpecifier,
  ) -> Result<Vec<u32>, AnyError> {
    let line_index = if specifier.as_url().scheme() == "asset" {
      let state_snapshot = self.snapshot();
      if let Some(source) =
        tsc::get_asset(&specifier, &self.ts_server, &state_snapshot).await?
      {
        text::index_lines(&source)
      } else {
        return Err(anyhow!("asset source missing: {}", specifier));
      }
    } else {
      let file_cache = self.file_cache.lock().unwrap();
      if let Some(file_id) = file_cache.lookup(&specifier) {
        let file_text = file_cache.get_contents(file_id)?;
        text::index_lines(&file_text)
      } else {
        let mut sources = self.sources.lock().unwrap();
        if let Some(line_index) = sources.get_line_index(&specifier) {
          line_index
        } else {
          return Err(anyhow!("source for specifier not found: {}", specifier));
        }
      }
    };
    Ok(line_index)
  }

  async fn prepare_diagnostics(&self) -> Result<(), AnyError> {
    let (enabled, lint_enabled) = {
      let config = self.config.lock().unwrap();
      (config.settings.enable, config.settings.lint)
    };

    let lint = async {
      if lint_enabled {
        let diagnostic_collection = self.diagnostics.lock().unwrap().clone();
        let diagnostics = diagnostics::generate_lint_diagnostics(
          self.snapshot(),
          diagnostic_collection,
        )
        .await;
        {
          let mut diagnostics_collection = self.diagnostics.lock().unwrap();
          for (file_id, version, diagnostics) in diagnostics {
            diagnostics_collection.set(
              file_id,
              DiagnosticSource::Lint,
              version,
              diagnostics,
            );
          }
        }
        self.publish_diagnostics().await?
      };

      Ok::<(), AnyError>(())
    };

    let ts = async {
      if enabled {
        let diagnostics = {
          let diagnostic_collection = self.diagnostics.lock().unwrap().clone();
          match diagnostics::generate_ts_diagnostics(
            &self.ts_server,
            &diagnostic_collection,
            self.snapshot(),
          )
          .await
          {
            Ok(diagnostics) => diagnostics,
            Err(err) => {
              error!("Error processing TypeScript diagnostics:\n{}", err);
              vec![]
            }
          }
        };
        {
          let mut diagnostics_collection = self.diagnostics.lock().unwrap();
          for (file_id, version, diagnostics) in diagnostics {
            diagnostics_collection.set(
              file_id,
              DiagnosticSource::TypeScript,
              version,
              diagnostics,
            );
          }
        };
        self.publish_diagnostics().await?
      }

      Ok::<(), AnyError>(())
    };

    let deps = async {
      if enabled {
        let diagnostics_collection = self.diagnostics.lock().unwrap().clone();
        let diagnostics = diagnostics::generate_dependency_diagnostics(
          self.snapshot(),
          diagnostics_collection,
        )
        .await?;
        {
          let mut diagnostics_collection = self.diagnostics.lock().unwrap();
          for (file_id, version, diagnostics) in diagnostics {
            diagnostics_collection.set(
              file_id,
              DiagnosticSource::Deno,
              version,
              diagnostics,
            );
          }
        }
        self.publish_diagnostics().await?
      };

      Ok::<(), AnyError>(())
    };

    let (lint_res, ts_res, deps_res) = tokio::join!(lint, ts, deps);
    lint_res?;
    ts_res?;
    deps_res?;

    Ok(())
  }

  async fn publish_diagnostics(&self) -> Result<(), AnyError> {
    let (maybe_changes, diagnostics_collection) = {
      let mut diagnostics_collection = self.diagnostics.lock().unwrap();
      let maybe_changes = diagnostics_collection.take_changes();
      (maybe_changes, diagnostics_collection.clone())
    };
    if let Some(diagnostic_changes) = maybe_changes {
      let settings = self.config.lock().unwrap().settings.clone();
      for file_id in diagnostic_changes {
        // TODO(@kitsonk) not totally happy with the way we collect and store
        // different types of diagnostics and offer them up to the client, we
        // do need to send "empty" vectors though when a particular feature is
        // disabled, otherwise the client will not clear down previous
        // diagnostics
        let mut diagnostics: Vec<Diagnostic> = if settings.lint {
          diagnostics_collection
            .diagnostics_for(file_id, DiagnosticSource::Lint)
            .cloned()
            .collect()
        } else {
          vec![]
        };
        if self.enabled() {
          diagnostics.extend(
            diagnostics_collection
              .diagnostics_for(file_id, DiagnosticSource::TypeScript)
              .cloned(),
          );
          diagnostics.extend(
            diagnostics_collection
              .diagnostics_for(file_id, DiagnosticSource::Deno)
              .cloned(),
          );
        }
        let specifier = {
          let file_cache = self.file_cache.lock().unwrap();
          file_cache.get_specifier(file_id).clone()
        };
        let uri = specifier.as_url().clone();
        let version = if let Some(doc_data) =
          self.doc_data.lock().unwrap().get(&specifier)
        {
          doc_data.version
        } else {
          None
        };
        self
          .client
          .publish_diagnostics(uri, diagnostics, version)
          .await;
      }
    }

    Ok(())
  }

  pub fn snapshot(&self) -> StateSnapshot {
    StateSnapshot {
      assets: self.assets.clone(),
      doc_data: self.doc_data.lock().unwrap().clone(),
      file_cache: self.file_cache.clone(),
      sources: self.sources.clone(),
    }
  }

  pub async fn update_import_map(&self) -> Result<(), AnyError> {
    let (maybe_import_map, maybe_root_uri) = {
      let config = self.config.lock().unwrap();
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
      *self.maybe_import_map_uri.lock().unwrap() = Some(import_map_url);
      *self.maybe_import_map.lock().unwrap() = Some(import_map);
    } else {
      *self.maybe_import_map.lock().unwrap() = None;
    }
    Ok(())
  }

  async fn update_tsconfig(&self) -> Result<(), AnyError> {
    let mut tsconfig = TsConfig::new(json!({
      "allowJs": true,
      "experimentalDecorators": true,
      "isolatedModules": true,
      "lib": ["deno.ns", "deno.window"],
      "module": "esnext",
      "noEmit": true,
      "strict": true,
      "target": "esnext",
    }));
    let (maybe_config, maybe_root_uri) = {
      let config = self.config.lock().unwrap();
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
      *self.maybe_config_uri.lock().unwrap() = Some(config_url);
      if let Some(ignored_options) = maybe_ignored_options {
        // TODO(@kitsonk) turn these into diagnostics that can be sent to the
        // client
        warn!("{}", ignored_options);
      }
    }
    self
      .ts_server
      .request(self.snapshot(), tsc::RequestMethod::Configure(tsconfig))
      .await?;
    Ok(())
  }
}

#[lspower::async_trait]
impl lspower::LanguageServer for LanguageServer {
  async fn initialize(
    &self,
    params: InitializeParams,
  ) -> LspResult<InitializeResult> {
    info!("Starting Deno language server...");

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
      let mut config = self.config.lock().unwrap();
      config.root_uri = params.root_uri;
      if let Some(value) = params.initialization_options {
        config.update(value)?;
      }
      config.update_capabilities(&params.capabilities);
    }

    if let Err(err) = self.update_tsconfig().await {
      warn!("Updating tsconfig has errored: {}", err);
    }

    Ok(InitializeResult {
      capabilities,
      server_info: Some(server_info),
    })
  }

  async fn initialized(&self, _: InitializedParams) {
    // Check to see if we need to setup the import map
    if let Err(err) = self.update_import_map().await {
      self
        .client
        .show_message(MessageType::Warning, err.to_string())
        .await;
    }

    if self
      .config
      .lock()
      .unwrap()
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

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    let specifier = utils::normalize_url(params.text_document.uri);
    let maybe_import_map = self.maybe_import_map.lock().unwrap().clone();
    if self
      .doc_data
      .lock()
      .unwrap()
      .insert(
        specifier.clone(),
        DocumentData::new(
          specifier.clone(),
          params.text_document.version,
          &params.text_document.text,
          maybe_import_map,
        ),
      )
      .is_some()
    {
      error!("duplicate DidOpenTextDocument: {}", specifier);
    }

    self
      .file_cache
      .lock()
      .unwrap()
      .set_contents(specifier, Some(params.text_document.text.into_bytes()));
    // TODO(@lucacasonato): error handling
    self.prepare_diagnostics().await.unwrap();
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let specifier = utils::normalize_url(params.text_document.uri);
    let mut content = {
      let file_cache = self.file_cache.lock().unwrap();
      let file_id = file_cache.lookup(&specifier).unwrap();
      file_cache.get_contents(file_id).unwrap()
    };
    apply_content_changes(&mut content, params.content_changes);
    {
      let mut doc_data = self.doc_data.lock().unwrap();
      let doc_data = doc_data.get_mut(&specifier).unwrap();
      let maybe_import_map = self.maybe_import_map.lock().unwrap();
      doc_data.update(
        params.text_document.version,
        &content,
        &maybe_import_map,
      );
    }

    self
      .file_cache
      .lock()
      .unwrap()
      .set_contents(specifier, Some(content.into_bytes()));

    // TODO(@lucacasonato): error handling
    self.prepare_diagnostics().await.unwrap();
  }

  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    let specifier = utils::normalize_url(params.text_document.uri);
    if self.doc_data.lock().unwrap().remove(&specifier).is_none() {
      error!("orphaned document: {}", specifier);
    }
    // TODO(@kitsonk) should we do garbage collection on the diagnostics?
    // TODO(@lucacasonato): error handling
    self.prepare_diagnostics().await.unwrap();
  }

  async fn did_save(&self, _params: DidSaveTextDocumentParams) {
    // nothing to do yet... cleanup things?
  }

  async fn did_change_configuration(
    &self,
    params: DidChangeConfigurationParams,
  ) {
    let config = if self
      .config
      .lock()
      .unwrap()
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
          error!("failed to fetch the extension settings {:?}", err);
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
      if let Err(err) = self.config.lock().unwrap().update(config) {
        error!("failed to update settings: {}", err);
      }
      if let Err(err) = self.update_import_map().await {
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
    } else {
      error!("received empty extension settings from the client");
    }
  }

  async fn did_change_watched_files(
    &self,
    params: DidChangeWatchedFilesParams,
  ) {
    // if the current import map has changed, we need to reload it
    let maybe_import_map_uri =
      self.maybe_import_map_uri.lock().unwrap().clone();
    if let Some(import_map_uri) = maybe_import_map_uri {
      if params.changes.iter().any(|fe| import_map_uri == fe.uri) {
        if let Err(err) = self.update_import_map().await {
          self
            .client
            .show_message(MessageType::Warning, err.to_string())
            .await;
        }
      }
    }
    // if the current tsconfig has changed, we need to reload it
    let maybe_config_uri = self.maybe_config_uri.lock().unwrap().clone();
    if let Some(config_uri) = maybe_config_uri {
      if params.changes.iter().any(|fe| config_uri == fe.uri) {
        if let Err(err) = self.update_tsconfig().await {
          self
            .client
            .show_message(MessageType::Warning, err.to_string())
            .await;
        }
      }
    }
  }

  async fn formatting(
    &self,
    params: DocumentFormattingParams,
  ) -> LspResult<Option<Vec<TextEdit>>> {
    let specifier = utils::normalize_url(params.text_document.uri.clone());
    let file_text = {
      let file_cache = self.file_cache.lock().unwrap();
      let file_id = file_cache.lookup(&specifier).unwrap();
      // TODO(lucacasonato): handle error properly
      file_cache.get_contents(file_id).unwrap()
    };

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
        Ok(new_text) => Some(text::get_edits(&file_text, &new_text)),
        Err(err) => {
          warn!("Format error: {}", err);
          None
        }
      }
    })
    .await
    .unwrap();

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
    if !self.enabled() {
      return Ok(None);
    }
    let specifier = utils::normalize_url(
      params.text_document_position_params.text_document.uri,
    );
    // TODO(lucacasonato): handle error correctly
    let line_index = self.get_line_index(specifier.clone()).await.unwrap();
    let req = tsc::RequestMethod::GetQuickInfo((
      specifier,
      text::to_char_pos(
        &line_index,
        params.text_document_position_params.position,
      ),
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(self.snapshot(), req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_quick_info: Option<tsc::QuickInfo> =
      serde_json::from_value(res).unwrap();
    if let Some(quick_info) = maybe_quick_info {
      Ok(Some(quick_info.to_hover(&line_index)))
    } else {
      Ok(None)
    }
  }

  async fn document_highlight(
    &self,
    params: DocumentHighlightParams,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    if !self.enabled() {
      return Ok(None);
    }
    let specifier = utils::normalize_url(
      params.text_document_position_params.text_document.uri,
    );
    // TODO(lucacasonato): handle error correctly
    let line_index = self.get_line_index(specifier.clone()).await.unwrap();
    let files_to_search = vec![specifier.clone()];
    let req = tsc::RequestMethod::GetDocumentHighlights((
      specifier,
      text::to_char_pos(
        &line_index,
        params.text_document_position_params.position,
      ),
      files_to_search,
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(self.snapshot(), req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_document_highlights: Option<Vec<tsc::DocumentHighlights>> =
      serde_json::from_value(res).unwrap();

    if let Some(document_highlights) = maybe_document_highlights {
      Ok(Some(
        document_highlights
          .into_iter()
          .map(|dh| dh.to_highlight(&line_index))
          .flatten()
          .collect(),
      ))
    } else {
      Ok(None)
    }
  }

  async fn references(
    &self,
    params: ReferenceParams,
  ) -> LspResult<Option<Vec<Location>>> {
    if !self.enabled() {
      return Ok(None);
    }
    let specifier =
      utils::normalize_url(params.text_document_position.text_document.uri);
    // TODO(lucacasonato): handle error correctly
    let line_index = self.get_line_index(specifier.clone()).await.unwrap();
    let req = tsc::RequestMethod::GetReferences((
      specifier,
      text::to_char_pos(&line_index, params.text_document_position.position),
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(self.snapshot(), req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_references: Option<Vec<tsc::ReferenceEntry>> =
      serde_json::from_value(res).unwrap();

    if let Some(references) = maybe_references {
      let mut results = Vec::new();
      for reference in references {
        if !params.context.include_declaration && reference.is_definition {
          continue;
        }
        let reference_specifier =
          ModuleSpecifier::resolve_url(&reference.document_span.file_name)
            .unwrap();
        // TODO(lucacasonato): handle error correctly
        let line_index =
          self.get_line_index(reference_specifier).await.unwrap();
        results.push(reference.to_location(&line_index));
      }

      Ok(Some(results))
    } else {
      Ok(None)
    }
  }

  async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
  ) -> LspResult<Option<GotoDefinitionResponse>> {
    if !self.enabled() {
      return Ok(None);
    }
    let specifier = utils::normalize_url(
      params.text_document_position_params.text_document.uri,
    );
    // TODO(lucacasonato): handle error correctly
    let line_index = self.get_line_index(specifier.clone()).await.unwrap();
    let req = tsc::RequestMethod::GetDefinition((
      specifier,
      text::to_char_pos(
        &line_index,
        params.text_document_position_params.position,
      ),
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(self.snapshot(), req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_definition: Option<tsc::DefinitionInfoAndBoundSpan> =
      serde_json::from_value(res).unwrap();

    if let Some(definition) = maybe_definition {
      Ok(
        definition
          .to_definition(&line_index, |s| self.get_line_index(s))
          .await,
      )
    } else {
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
    let specifier =
      utils::normalize_url(params.text_document_position.text_document.uri);
    // TODO(lucacasonato): handle error correctly
    let line_index = self.get_line_index(specifier.clone()).await.unwrap();
    let req = tsc::RequestMethod::GetCompletions((
      specifier,
      text::to_char_pos(&line_index, params.text_document_position.position),
      tsc::UserPreferences {
        // TODO(lucacasonato): enable this. see https://github.com/denoland/deno/pull/8651
        include_completions_with_insert_text: Some(false),
        ..Default::default()
      },
    ));
    // TODO(lucacasonato): handle error correctly
    let res = self.ts_server.request(self.snapshot(), req).await.unwrap();
    // TODO(lucacasonato): handle error correctly
    let maybe_completion_info: Option<tsc::CompletionInfo> =
      serde_json::from_value(res).unwrap();

    if let Some(completions) = maybe_completion_info {
      Ok(Some(completions.into_completion_response(&line_index)))
    } else {
      Ok(None)
    }
  }

  async fn goto_implementation(
    &self,
    params: GotoImplementationParams,
  ) -> LspResult<Option<GotoImplementationResponse>> {
    if !self.enabled() {
      return Ok(None);
    }
    let specifier = utils::normalize_url(
      params.text_document_position_params.text_document.uri,
    );
    let line_index =
      self
        .get_line_index(specifier.clone())
        .await
        .map_err(|err| {
          error!("Failed to get line_index {:#?}", err);
          LspError::internal_error()
        })?;

    let req = tsc::RequestMethod::GetImplementation((
      specifier,
      text::to_char_pos(
        &line_index,
        params.text_document_position_params.position,
      ),
    ));
    let res =
      self
        .ts_server
        .request(self.snapshot(), req)
        .await
        .map_err(|err| {
          error!("Failed to request to tsserver {:#?}", err);
          LspError::invalid_request()
        })?;

    let maybe_implementations = serde_json::from_value::<Option<Vec<tsc::ImplementationLocation>>>(res)
      .map_err(|err| {
        error!("Failed to deserialized tsserver response to Vec<ImplementationLocation> {:#?}", err);
        LspError::internal_error()
      })?;

    if let Some(implementations) = maybe_implementations {
      let mut results = Vec::new();
      for impl_ in implementations {
        let document_span = impl_.document_span;
        let impl_specifier =
          ModuleSpecifier::resolve_url(&document_span.file_name).unwrap();
        let impl_line_index =
          &self.get_line_index(impl_specifier).await.unwrap();
        if let Some(link) = document_span
          .to_link(impl_line_index, |s| self.get_line_index(s))
          .await
        {
          results.push(link);
        }
      }
      Ok(Some(GotoDefinitionResponse::Link(results)))
    } else {
      Ok(None)
    }
  }

  async fn rename(
    &self,
    params: RenameParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    if !self.enabled() {
      return Ok(None);
    }

    let snapshot = self.snapshot();
    let specifier =
      utils::normalize_url(params.text_document_position.text_document.uri);

    let line_index =
      self
        .get_line_index(specifier.clone())
        .await
        .map_err(|err| {
          error!("Failed to get line_index {:#?}", err);
          LspError::internal_error()
        })?;

    let req = tsc::RequestMethod::FindRenameLocations((
      specifier,
      text::to_char_pos(&line_index, params.text_document_position.position),
      true,
      true,
      false,
    ));

    let res = self
      .ts_server
      .request(snapshot.clone(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {:#?}", err);
        LspError::invalid_request()
      })?;

    let maybe_locations = serde_json::from_value::<
      Option<Vec<tsc::RenameLocation>>,
    >(res)
    .map_err(|err| {
      error!(
        "Failed to deserialize tsserver response to Vec<RenameLocation> {:#?}",
        err
      );
      LspError::internal_error()
    })?;

    match maybe_locations {
      Some(locations) => {
        let rename_locations = tsc::RenameLocations { locations };
        let workpace_edits = rename_locations
          .into_workspace_edit(
            snapshot,
            |s| self.get_line_index(s),
            &params.new_name,
          )
          .await
          .map_err(|err| {
            error!(
              "Failed to convert tsc::RenameLocations to WorkspaceEdit {:#?}",
              err
            );
            LspError::internal_error()
          })?;
        Ok(Some(workpace_edits))
      }
      None => Ok(None),
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
            error!("Failed to serialize cache response: {:#?}", err);
            LspError::internal_error()
          })?,
        )),
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      },
      "deno/virtualTextDocument" => match params.map(serde_json::from_value) {
        Some(Ok(params)) => Ok(Some(
          serde_json::to_value(self.virtual_text_document(params).await?)
            .map_err(|err| {
              error!(
                "Failed to serialize virtual_text_document response: {:#?}",
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
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheParams {
  pub text_document: TextDocumentIdentifier,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualTextDocumentParams {
  pub text_document: TextDocumentIdentifier,
}

impl LanguageServer {
  async fn cache(&self, params: CacheParams) -> LspResult<bool> {
    let specifier = utils::normalize_url(params.text_document.uri);
    let maybe_import_map = self.maybe_import_map.lock().unwrap().clone();
    sources::cache(specifier.clone(), maybe_import_map)
      .await
      .map_err(|err| {
        error!("{}", err);
        LspError::internal_error()
      })?;
    {
      let file_cache = self.file_cache.lock().unwrap();
      if let Some(file_id) = file_cache.lookup(&specifier) {
        let mut diagnostics_collection = self.diagnostics.lock().unwrap();
        diagnostics_collection.invalidate(&file_id);
      }
    }
    self.prepare_diagnostics().await.map_err(|err| {
      error!("{}", err);
      LspError::internal_error()
    })?;
    Ok(true)
  }

  async fn virtual_text_document(
    &self,
    params: VirtualTextDocumentParams,
  ) -> LspResult<Option<String>> {
    let specifier = utils::normalize_url(params.text_document.uri);
    let url = specifier.as_url();
    let contents = if url.as_str() == "deno:/status.md" {
      let file_cache = self.file_cache.lock().unwrap();
      Some(format!(
        r#"# Deno Language Server Status

  - Documents in memory: {}

  "#,
        file_cache.len()
      ))
    } else {
      match url.scheme() {
        "asset" => {
          let state_snapshot = self.snapshot();
          if let Some(text) =
            tsc::get_asset(&specifier, &self.ts_server, &state_snapshot)
              .await
              .map_err(|_| LspError::internal_error())?
          {
            Some(text)
          } else {
            error!("Missing asset: {}", specifier);
            None
          }
        }
        _ => {
          let mut sources = self.sources.lock().unwrap();
          if let Some(text) = sources.get_text(&specifier) {
            Some(text)
          } else {
            error!("The cached sources was not found: {}", specifier);
            None
          }
        }
      }
    };
    Ok(contents)
  }
}

#[derive(Debug, Clone)]
pub struct DocumentData {
  pub dependencies: Option<HashMap<String, analysis::Dependency>>,
  pub version: Option<i32>,
  specifier: ModuleSpecifier,
}

impl DocumentData {
  pub fn new(
    specifier: ModuleSpecifier,
    version: i32,
    source: &str,
    maybe_import_map: Option<ImportMap>,
  ) -> Self {
    let dependencies = if let Some((dependencies, _)) =
      analysis::analyze_dependencies(
        &specifier,
        source,
        &MediaType::from(&specifier),
        &maybe_import_map,
      ) {
      Some(dependencies)
    } else {
      None
    };
    Self {
      dependencies,
      version: Some(version),
      specifier,
    }
  }

  pub fn update(
    &mut self,
    version: i32,
    source: &str,
    maybe_import_map: &Option<ImportMap>,
  ) {
    self.dependencies = if let Some((dependencies, _)) =
      analysis::analyze_dependencies(
        &self.specifier,
        source,
        &MediaType::from(&self.specifier),
        maybe_import_map,
      ) {
      Some(dependencies)
    } else {
      None
    };
    self.version = Some(version)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use lspower::jsonrpc;
  use lspower::ExitedError;
  use lspower::LspService;
  use std::fs;
  use std::task::Poll;
  use tower_test::mock::Spawn;

  enum LspResponse {
    None,
    RequestAny,
    Request(u64, Value),
  }

  struct LspTestHarness {
    requests: Vec<(&'static str, LspResponse)>,
    service: Spawn<LspService>,
  }

  impl LspTestHarness {
    pub fn new(requests: Vec<(&'static str, LspResponse)>) -> Self {
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
}
