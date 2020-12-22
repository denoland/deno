// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use dprint_plugin_typescript as dprint;
use lspower::jsonrpc::Error as LSPError;
use lspower::jsonrpc::ErrorCode as LSPErrorCode;
use lspower::jsonrpc::Result as LSPResult;
use lspower::lsp_types::*;
use lspower::Client;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::fs;

use crate::deno_dir;
use crate::import_map::ImportMap;
use crate::media_type::MediaType;
use crate::tsc_config::TsConfig;

use super::analysis;
use super::capabilities;
use super::config::Config;
use super::diagnostics;
use super::diagnostics::DiagnosticCollection;
use super::diagnostics::DiagnosticSource;
use super::memory_cache::MemoryCache;
use super::sources::Sources;
use super::text;
use super::text::apply_content_changes;
use super::tsc;
use super::tsc::TsServer;
use super::utils;

#[derive(Debug, Clone)]
pub struct LanguageServer {
  assets: Arc<RwLock<HashMap<ModuleSpecifier, Option<String>>>>,
  client: Client,
  ts_server: TsServer,
  config: Arc<RwLock<Config>>,
  doc_data: Arc<RwLock<HashMap<ModuleSpecifier, DocumentData>>>,
  file_cache: Arc<RwLock<MemoryCache>>,
  sources: Arc<RwLock<Sources>>,
  diagnostics: Arc<RwLock<DiagnosticCollection>>,
  maybe_import_map: Arc<RwLock<Option<ImportMap>>>,
  maybe_import_map_uri: Arc<RwLock<Option<Url>>>,
}

#[derive(Debug, Clone, Default)]
pub struct StateSnapshot {
  pub assets: Arc<RwLock<HashMap<ModuleSpecifier, Option<String>>>>,
  pub doc_data: HashMap<ModuleSpecifier, DocumentData>,
  pub file_cache: Arc<RwLock<MemoryCache>>,
  pub sources: Arc<RwLock<Sources>>,
}

impl LanguageServer {
  pub fn new(client: Client) -> Self {
    let maybe_custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(maybe_custom_root)
      .expect("could not access DENO_DIR");
    let location = dir.root.join("deps");
    let sources = Arc::new(RwLock::new(Sources::new(&location)));

    LanguageServer {
      assets: Default::default(),
      client,
      ts_server: TsServer::new(),
      config: Default::default(),
      doc_data: Default::default(),
      file_cache: Default::default(),
      sources,
      diagnostics: Default::default(),
      maybe_import_map: Default::default(),
      maybe_import_map_uri: Default::default(),
    }
  }

  fn enabled(&self) -> bool {
    let config = self.config.read().unwrap();
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
      let file_cache = self.file_cache.read().unwrap();
      if let Some(file_id) = file_cache.lookup(&specifier) {
        let file_text = file_cache.get_contents(file_id)?;
        text::index_lines(&file_text)
      } else {
        let mut sources = self.sources.write().unwrap();
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
      let config = self.config.read().unwrap();
      (config.settings.enable, config.settings.lint)
    };

    let lint = async {
      if lint_enabled {
        let diagnostic_collection = self.diagnostics.read().unwrap().clone();
        let diagnostics = diagnostics::generate_lint_diagnostics(
          self.snapshot(),
          diagnostic_collection,
        )
        .await;
        {
          let mut diagnostics_collection = self.diagnostics.write().unwrap();
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
          let diagnostic_collection = self.diagnostics.read().unwrap().clone();
          diagnostics::generate_ts_diagnostics(
            &self.ts_server,
            &diagnostic_collection,
            self.snapshot(),
          )
          .await?
        };
        {
          let mut diagnostics_collection = self.diagnostics.write().unwrap();
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

    let (lint_res, ts_res) = tokio::join!(lint, ts);
    lint_res?;
    ts_res?;

    Ok(())
  }

  async fn publish_diagnostics(&self) -> Result<(), AnyError> {
    let (maybe_changes, diagnostics_collection) = {
      let mut diagnostics_collection = self.diagnostics.write().unwrap();
      let maybe_changes = diagnostics_collection.take_changes();
      (maybe_changes, diagnostics_collection.clone())
    };
    if let Some(diagnostic_changes) = maybe_changes {
      let settings = self.config.read().unwrap().settings.clone();
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
        }
        let specifier = {
          let file_cache = self.file_cache.read().unwrap();
          file_cache.get_specifier(file_id).clone()
        };
        let uri = specifier.as_url().clone();
        let version = if let Some(doc_data) =
          self.doc_data.read().unwrap().get(&specifier)
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
      doc_data: self.doc_data.read().unwrap().clone(),
      file_cache: self.file_cache.clone(),
      sources: self.sources.clone(),
    }
  }

  pub async fn update_import_map(&self) -> Result<(), AnyError> {
    let (maybe_import_map, maybe_root_uri) = {
      let config = self.config.read().unwrap();
      (config.settings.import_map.clone(), config.root_uri.clone())
    };
    if let Some(import_map_str) = &maybe_import_map {
      info!("update import map");
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
      *self.maybe_import_map_uri.write().unwrap() = Some(import_map_url);
      *self.maybe_import_map.write().unwrap() = Some(import_map);
    } else {
      *self.maybe_import_map.write().unwrap() = None;
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
    {
      let config = self.config.read().unwrap();
      if config.settings.unstable {
        let unstable_libs = json!({
          "lib": ["deno.ns", "deno.window", "deno.unstable"]
        });
        tsconfig.merge(&unstable_libs);
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
  ) -> LSPResult<InitializeResult> {
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
      let mut config = self.config.write().unwrap();
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
      .read()
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

  async fn shutdown(&self) -> LSPResult<()> {
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
    let maybe_import_map = self.maybe_import_map.read().unwrap().clone();
    if self
      .doc_data
      .write()
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
      .write()
      .unwrap()
      .set_contents(specifier, Some(params.text_document.text.into_bytes()));
    // TODO(@lucacasonato): error handling
    self.prepare_diagnostics().await.unwrap();
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let specifier = utils::normalize_url(params.text_document.uri);
    let mut content = {
      let file_cache = self.file_cache.read().unwrap();
      let file_id = file_cache.lookup(&specifier).unwrap();
      file_cache.get_contents(file_id).unwrap()
    };
    apply_content_changes(&mut content, params.content_changes);
    {
      let mut doc_data = self.doc_data.write().unwrap();
      let doc_data = doc_data.get_mut(&specifier).unwrap();
      let maybe_import_map = self.maybe_import_map.read().unwrap();
      doc_data.update(
        params.text_document.version,
        &content,
        &maybe_import_map,
      );
    }

    self
      .file_cache
      .write()
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
    if self.doc_data.write().unwrap().remove(&specifier).is_none() {
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
    _params: DidChangeConfigurationParams,
  ) {
    if !self
      .config
      .read()
      .unwrap()
      .client_capabilities
      .workspace_configuration
    {
      // Client does not support workspace configuration
      return;
    }

    let res = self
      .client
      .configuration(vec![ConfigurationItem {
        scope_uri: None,
        section: Some("deno".to_string()),
      }])
      .await
      .map(|vec| vec.get(0).cloned());

    match res {
      Err(err) => error!("failed to fetch the extension settings {:?}", err),
      Ok(Some(config)) => {
        if let Err(err) = self.config.write().unwrap().update(config) {
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
      }
      _ => error!("received empty extension settings from the client"),
    }
  }

  async fn did_change_watched_files(
    &self,
    params: DidChangeWatchedFilesParams,
  ) {
    // if the current import map has changed, we need to reload it
    let maybe_import_map_uri =
      self.maybe_import_map_uri.read().unwrap().clone();
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
  }

  async fn formatting(
    &self,
    params: DocumentFormattingParams,
  ) -> LSPResult<Option<Vec<TextEdit>>> {
    let specifier = utils::normalize_url(params.text_document.uri.clone());
    let file_text = {
      let file_cache = self.file_cache.read().unwrap();
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

  async fn hover(&self, params: HoverParams) -> LSPResult<Option<Hover>> {
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
  ) -> LSPResult<Option<Vec<DocumentHighlight>>> {
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
  ) -> LSPResult<Option<Vec<Location>>> {
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
          ModuleSpecifier::resolve_url(&reference.file_name).unwrap();
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
  ) -> LSPResult<Option<GotoDefinitionResponse>> {
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
  ) -> LSPResult<Option<CompletionResponse>> {
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

  async fn request_else(
    &self,
    method: &str,
    params: Option<Value>,
  ) -> LSPResult<Option<Value>> {
    match method {
      "deno/virtualTextDocument" => match params.map(serde_json::from_value) {
        Some(Ok(params)) => Ok(Some(
          serde_json::to_value(self.virtual_text_document(params).await?)
            .map_err(|err| {
              error!(
                "Failed to serialize virtual_text_document response: {:#?}",
                err
              );
              LSPError::internal_error()
            })?,
        )),
        Some(Err(err)) => Err(LSPError::invalid_params(err.to_string())),
        None => Err(LSPError::invalid_params("Missing parameters")),
      },
      _ => {
        error!("Got a {} request, but no handler is defined", method);
        Err(LSPError::method_not_found())
      }
    }
  }
}

impl LanguageServer {
  async fn virtual_text_document(
    &self,
    params: VirtualTextDocumentParams,
  ) -> LSPResult<Option<String>> {
    let specifier = utils::normalize_url(params.text_document.uri);
    let url = specifier.as_url();
    let contents = if url.as_str() == "deno:/status.md" {
      let file_cache = self.file_cache.read().unwrap();
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
              .map_err(|_| LSPError::new(LSPErrorCode::InternalError))?
          {
            Some(text)
          } else {
            error!("Missing asset: {}", specifier);
            None
          }
        }
        _ => {
          let mut sources = self.sources.write().unwrap();
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualTextDocumentParams {
  pub text_document: TextDocumentIdentifier,
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
}
