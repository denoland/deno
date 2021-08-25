// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use log::error;
use log::info;
use log::warn;
use lspower::jsonrpc::Error as LspError;
use lspower::jsonrpc::Result as LspResult;
use lspower::lsp::request::*;
use lspower::lsp::*;
use lspower::Client;
use serde_json::from_value;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::fs;

use super::analysis;
use super::analysis::fix_ts_import_changes;
use super::analysis::ts_changes_to_edit;
use super::analysis::CodeActionCollection;
use super::analysis::CodeActionData;
use super::analysis::ResolvedDependency;
use super::capabilities;
use super::code_lens;
use super::completions;
use super::config::Config;
use super::config::ConfigSnapshot;
use super::config::SETTINGS_SECTION;
use super::diagnostics;
use super::diagnostics::DiagnosticSource;
use super::documents::DocumentCache;
use super::documents::LanguageId;
use super::lsp_custom;
use super::parent_process_checker;
use super::performance::Performance;
use super::refactor;
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
use crate::config_file::ConfigFile;
use crate::config_file::TsConfig;
use crate::deno_dir;
use crate::fs_util;
use crate::import_map::ImportMap;
use crate::logger;
use crate::media_type::MediaType;
use crate::tools::fmt::format_file;
use crate::tools::fmt::get_typescript_config;

pub const REGISTRIES_PATH: &str = "registries";
const SOURCES_PATH: &str = "deps";

#[derive(Debug, Clone)]
pub struct LanguageServer(Arc<tokio::sync::Mutex<Inner>>);

#[derive(Debug, Clone, Default)]
pub struct StateSnapshot {
  pub assets: Assets,
  pub config: ConfigSnapshot,
  pub documents: DocumentCache,
  pub maybe_config_uri: Option<ModuleSpecifier>,
  pub module_registries: registries::ModuleRegistry,
  pub performance: Performance,
  pub sources: Sources,
  pub url_map: urls::LspUrlMap,
}

#[derive(Debug)]
pub(crate) struct Inner {
  /// Cached versions of "fixed" assets that can either be inlined in Rust or
  /// are part of the TypeScript snapshot and have to be fetched out.
  assets: Assets,
  /// The LSP client that this LSP server is connected to.
  pub(crate) client: Client,
  /// Configuration information.
  pub(crate) config: Config,
  diagnostics_server: diagnostics::DiagnosticsServer,
  /// The "in-memory" documents in the editor which can be updated and changed.
  documents: DocumentCache,
  /// Handles module registries, which allow discovery of modules
  module_registries: registries::ModuleRegistry,
  /// The path to the module registries cache
  module_registries_location: PathBuf,
  /// An optional path to the DENO_DIR which has been specified in the client
  /// options.
  maybe_cache_path: Option<PathBuf>,
  /// An optional configuration file which has been specified in the client
  /// options.
  maybe_config_file: Option<ConfigFile>,
  /// An optional URL which provides the location of a TypeScript configuration
  /// file which will be used by the Deno LSP.
  maybe_config_uri: Option<Url>,
  /// An optional import map which is used to resolve modules.
  pub(crate) maybe_import_map: Option<ImportMap>,
  /// The URL for the import map which is used to determine relative imports.
  maybe_import_map_uri: Option<Url>,
  /// A collection of measurements which instrument that performance of the LSP.
  performance: Performance,
  /// Cached sources that are read-only.
  sources: Sources,
  /// A memoized version of fixable diagnostic codes retrieved from TypeScript.
  ts_fixable_diagnostics: Vec<String>,
  /// An abstraction that handles interactions with TypeScript.
  pub(crate) ts_server: Arc<TsServer>,
  /// A map of specifiers and URLs used to translate over the LSP.
  pub(crate) url_map: urls::LspUrlMap,
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
    let config = Config::new(client.clone());

    Self {
      assets: Default::default(),
      client,
      config,
      diagnostics_server,
      documents: Default::default(),
      maybe_cache_path: None,
      maybe_config_file: None,
      maybe_config_uri: None,
      maybe_import_map: None,
      maybe_import_map_uri: None,
      module_registries,
      module_registries_location,
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
    media_type: &MediaType,
    source: &str,
  ) {
    if let Ok(parsed_module) =
      analysis::parse_module(specifier, source, media_type)
    {
      let (mut deps, _) = analysis::analyze_dependencies(
        specifier,
        media_type,
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
      let dep_ranges = analysis::analyze_dependency_ranges(&parsed_module).ok();
      if let Err(err) =
        self
          .documents
          .set_dependencies(specifier, Some(deps), dep_ranges)
      {
        error!("{}", err);
      }
    }
  }

  /// Analyzes all dependencies for all documents that have been opened in the
  /// editor and sets the dependencies property on the documents.
  fn analyze_dependencies_all(&mut self) {
    let docs: Vec<(ModuleSpecifier, String, MediaType)> = self
      .documents
      .docs
      .iter()
      .filter_map(|(s, doc)| {
        let source = doc.content().ok().flatten()?;
        let media_type = MediaType::from(&doc.language_id);
        Some((s.clone(), source, media_type))
      })
      .collect();
    for (specifier, source, media_type) in docs {
      self.analyze_dependencies(&specifier, &media_type, &source);
    }
  }

  /// Searches assets, open documents and external sources for a line_index,
  /// which might be performed asynchronously, hydrating in memory caches for
  /// subsequent requests.
  pub(crate) async fn get_line_index(
    &mut self,
    specifier: ModuleSpecifier,
  ) -> Result<LineIndex, AnyError> {
    let mark = self
      .performance
      .mark("get_line_index", Some(json!({ "specifier": specifier })));
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
  pub fn get_line_index_sync(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<LineIndex> {
    let mark = self.performance.mark(
      "get_line_index_sync",
      Some(json!({ "specifier": specifier })),
    );
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
  pub(crate) fn get_text_content(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
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

  pub(crate) fn get_media_type(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<MediaType> {
    if specifier.scheme() == "asset" || self.documents.contains_key(specifier) {
      Some(MediaType::from(specifier))
    } else {
      self.sources.get_media_type(specifier)
    }
  }

  pub(crate) async fn get_navigation_tree(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<tsc::NavigationTree, AnyError> {
    let mark = self.performance.mark(
      "get_navigation_tree",
      Some(json!({ "specifier": specifier })),
    );
    let maybe_navigation_tree = if specifier.scheme() == "asset" {
      self
        .assets
        .get(specifier)
        .map(|o| o.clone().map(|a| a.maybe_navigation_tree).flatten())
        .flatten()
    } else if self.documents.contains_key(specifier) {
      self.documents.get_navigation_tree(specifier)
    } else {
      self.sources.get_navigation_tree(specifier)
    };
    let navigation_tree = if let Some(navigation_tree) = maybe_navigation_tree {
      navigation_tree
    } else {
      let navigation_tree: tsc::NavigationTree = self
        .ts_server
        .request(
          self.snapshot()?,
          tsc::RequestMethod::GetNavigationTree(specifier.clone()),
        )
        .await?;
      if specifier.scheme() == "asset" {
        self
          .assets
          .set_navigation_tree(specifier, navigation_tree.clone())?;
      } else if self.documents.contains_key(specifier) {
        self
          .documents
          .set_navigation_tree(specifier, navigation_tree.clone())?;
      } else {
        self
          .sources
          .set_navigation_tree(specifier, navigation_tree.clone())?;
      }
      navigation_tree
    };
    self.performance.measure(mark);
    Ok(navigation_tree)
  }

  fn merge_user_tsconfig(
    &mut self,
    maybe_config: &Option<String>,
    maybe_root_uri: &Option<Url>,
    tsconfig: &mut TsConfig,
  ) -> Result<(), AnyError> {
    self.maybe_config_file = None;
    self.maybe_config_uri = None;
    if let Some(config_str) = maybe_config {
      if !config_str.is_empty() {
        info!("Setting TypeScript configuration from: \"{}\"", config_str);
        let config_url = if let Ok(url) = Url::from_file_path(config_str) {
          Ok(url)
        } else if let Some(root_uri) = maybe_root_uri {
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
        info!("  Resolved configuration file: \"{}\"", config_url);

        let config_file = {
          let buffer = config_url
            .to_file_path()
            .map_err(|_| anyhow!("Bad uri: \"{}\"", config_url))?;
          let path = buffer
            .to_str()
            .ok_or_else(|| anyhow!("Bad uri: \"{}\"", config_url))?;
          ConfigFile::read(path)?
        };
        let (value, maybe_ignored_options) =
          config_file.as_compiler_options()?;
        tsconfig.merge(&value);
        self.maybe_config_file = Some(config_file);
        self.maybe_config_uri = Some(config_url);
        if let Some(ignored_options) = maybe_ignored_options {
          // TODO(@kitsonk) turn these into diagnostics that can be sent to the
          // client
          warn!("{}", ignored_options);
        }
      }
    }
    Ok(())
  }

  pub(crate) fn snapshot(&self) -> LspResult<StateSnapshot> {
    Ok(StateSnapshot {
      assets: self.assets.clone(),
      config: self.config.snapshot().map_err(|err| {
        error!("{}", err);
        LspError::internal_error()
      })?,
      documents: self.documents.clone(),
      maybe_config_uri: self.maybe_config_uri.clone(),
      module_registries: self.module_registries.clone(),
      performance: self.performance.clone(),
      sources: self.sources.clone(),
      url_map: self.url_map.clone(),
    })
  }

  pub fn update_cache(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_cache", None::<()>);
    self.performance.measure(mark);
    let (maybe_cache, maybe_root_uri) = {
      let config = &self.config;
      (
        config.get_workspace_settings().cache,
        config.root_uri.clone(),
      )
    };
    let maybe_cache_path = if let Some(cache_str) = &maybe_cache {
      info!("Setting cache path from: \"{}\"", cache_str);
      let cache_url = if let Ok(url) = Url::from_file_path(cache_str) {
        Ok(url)
      } else if let Some(root_uri) = &maybe_root_uri {
        let root_path = root_uri
          .to_file_path()
          .map_err(|_| anyhow!("Bad root_uri: {}", root_uri))?;
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
      let cache_path = cache_url.to_file_path().map_err(|_| {
        anyhow!("Cannot convert \"{}\" into a file path.", cache_url)
      })?;
      info!(
        "  Resolved cache path: \"{}\"",
        cache_path.to_string_lossy()
      );
      Some(cache_path)
    } else {
      None
    };
    if self.maybe_cache_path != maybe_cache_path {
      let maybe_custom_root = maybe_cache_path
        .clone()
        .or_else(|| env::var("DENO_DIR").map(String::into).ok());
      let dir = deno_dir::DenoDir::new(maybe_custom_root)
        .expect("could not access DENO_DIR");
      let module_registries_location = dir.root.join(REGISTRIES_PATH);
      self.module_registries =
        registries::ModuleRegistry::new(&module_registries_location);
      self.module_registries_location = module_registries_location;
      let sources_location = dir.root.join(SOURCES_PATH);
      self.sources = Sources::new(&sources_location);
      self.maybe_cache_path = maybe_cache_path;
    }
    Ok(())
  }

  pub async fn update_import_map(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_import_map", None::<()>);
    let (maybe_import_map, maybe_root_uri) = {
      let config = &self.config;
      (
        config.get_workspace_settings().import_map,
        config.root_uri.clone(),
      )
    };
    if let Some(import_map_str) = &maybe_import_map {
      info!("Setting import map from: \"{}\"", import_map_str);
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
      let import_map_path = import_map_url.to_file_path().map_err(|_| {
        anyhow!("Cannot convert \"{}\" into a file path.", import_map_url)
      })?;
      info!(
        "  Resolved import map: \"{}\"",
        import_map_path.to_string_lossy()
      );
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
      self.maybe_import_map = Some(import_map.clone());
      self.sources.set_import_map(Some(import_map));
    } else {
      self.sources.set_import_map(None);
      self.maybe_import_map = None;
    }
    self.performance.measure(mark);
    Ok(())
  }

  pub fn update_debug_flag(&self) -> bool {
    let internal_debug = self.config.get_workspace_settings().internal_debug;
    logger::LSP_DEBUG_FLAG
      .compare_exchange(
        !internal_debug,
        internal_debug,
        Ordering::Acquire,
        Ordering::Relaxed,
      )
      .is_ok()
  }

  async fn update_registries(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_registries", None::<()>);
    for (registry, enabled) in self
      .config
      .get_workspace_settings()
      .suggest
      .imports
      .hosts
      .iter()
    {
      if *enabled {
        info!("Enabling import suggestions for: {}", registry);
        self.module_registries.enable(registry).await?;
      } else {
        self.module_registries.disable(registry).await?;
      }
    }
    self.performance.measure(mark);
    Ok(())
  }

  async fn update_tsconfig(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_tsconfig", None::<()>);
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
      let workspace_settings = config.get_workspace_settings();
      if workspace_settings.unstable {
        let unstable_libs = json!({
          "lib": ["deno.ns", "deno.window", "deno.unstable"]
        });
        tsconfig.merge(&unstable_libs);
      }
      (workspace_settings.config, config.root_uri.clone())
    };
    if let Err(err) =
      self.merge_user_tsconfig(&maybe_config, &maybe_root_uri, &mut tsconfig)
    {
      self.client.show_message(MessageType::Warning, err).await;
    }
    let _ok: bool = self
      .ts_server
      .request(self.snapshot()?, tsc::RequestMethod::Configure(tsconfig))
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
      Ok(maybe_asset.clone())
    } else {
      let maybe_asset =
        tsc::get_asset(specifier, &self.ts_server, self.snapshot()?).await?;
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
    let mark = self.performance.mark("initialize", Some(&params));

    // exit this process when the parent is lost
    if let Some(parent_pid) = params.process_id {
      parent_process_checker::start(parent_pid)
    }

    let capabilities = capabilities::server_capabilities(&params.capabilities);

    let version = format!(
      "{} ({}, {})",
      crate::version::deno(),
      env!("PROFILE"),
      env!("TARGET")
    );
    info!("  version: {}", version);
    if let Ok(path) = std::env::current_exe() {
      info!("  executable: {}", path.to_string_lossy());
    }

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
        config.set_workspace_settings(value).map_err(|err| {
          error!("Cannot set workspace settings: {}", err);
          LspError::internal_error()
        })?;
      }
      config.update_capabilities(&params.capabilities);
    }

    self.update_debug_flag();
    // Check to see if we need to change the cache path
    if let Err(err) = self.update_cache() {
      self.client.show_message(MessageType::Warning, err).await;
    }
    if let Err(err) = self.update_tsconfig().await {
      self.client.show_message(MessageType::Warning, err).await;
    }

    if capabilities.code_action_provider.is_some() {
      let fixable_diagnostics: Vec<String> = self
        .ts_server
        .request(self.snapshot()?, tsc::RequestMethod::GetSupportedCodeFixes)
        .await
        .map_err(|err| {
          error!("Unable to get fixable diagnostics: {}", err);
          LspError::internal_error()
        })?;
      self.ts_fixable_diagnostics = fixable_diagnostics;
    }

    // Check to see if we need to setup the import map
    if let Err(err) = self.update_import_map().await {
      self.client.show_message(MessageType::Warning, err).await;
    }
    // Check to see if we need to setup any module registries
    if let Err(err) = self.update_registries().await {
      self.client.show_message(MessageType::Warning, err).await;
    }

    self.performance.measure(mark);
    Ok(InitializeResult {
      capabilities,
      server_info: Some(server_info),
    })
  }

  async fn initialized(&mut self, _: InitializedParams) {
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
    let mark = self.performance.mark("did_open", Some(&params));
    let specifier = self.url_map.normalize_url(&params.text_document.uri);

    if let Err(err) = self
      .config
      .update_specifier_settings(&specifier, &params.text_document.uri)
      .await
    {
      error!("Error updating specifier settings: {}", err);
    }

    if params.text_document.uri.scheme() == "deno" {
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
          error!("{}", err);
          LanguageId::Unknown
        });
    if language_id == LanguageId::Unknown {
      warn!(
        "Unsupported language id \"{}\" received for document \"{}\".",
        params.text_document.language_id, params.text_document.uri
      );
    }
    let media_type = MediaType::from(&language_id);
    self.documents.open(
      specifier.clone(),
      params.text_document.version,
      language_id,
      &params.text_document.text,
    );

    if self.documents.is_diagnosable(&specifier) {
      self.analyze_dependencies(
        &specifier,
        &media_type,
        &params.text_document.text,
      );
      self
        .diagnostics_server
        .invalidate(self.documents.dependents(&specifier))
        .await;
      if let Err(err) = self.diagnostics_server.update() {
        error!("{}", err);
      }
    }
    self.performance.measure(mark);
  }

  async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
    let mark = self.performance.mark("did_change", Some(&params));
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    match self.documents.change(
      &specifier,
      params.text_document.version,
      params.content_changes,
    ) {
      Ok(Some(source)) => {
        if self.documents.is_diagnosable(&specifier) {
          let media_type = MediaType::from(
            &self.documents.get_language_id(&specifier).unwrap(),
          );
          self.analyze_dependencies(&specifier, &media_type, &source);
          self
            .diagnostics_server
            .invalidate(self.documents.dependents(&specifier))
            .await;
          if let Err(err) = self.diagnostics_server.update() {
            error!("{}", err);
          }
        }
      }
      Ok(_) => error!("No content returned from change."),
      Err(err) => error!("{}", err),
    }
    self.performance.measure(mark);
  }

  async fn did_close(&mut self, params: DidCloseTextDocumentParams) {
    let mark = self.performance.mark("did_close", Some(&params));
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents closing, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    let is_diagnosable = self.documents.is_diagnosable(&specifier);

    if is_diagnosable {
      let mut specifiers = self.documents.dependents(&specifier);
      specifiers.push(specifier.clone());
      self.diagnostics_server.invalidate(specifiers).await;
    }
    self.documents.close(&specifier);
    if is_diagnosable {
      if let Err(err) = self.diagnostics_server.update() {
        error!("{}", err);
      }
    }
    self.performance.measure(mark);
  }

  async fn did_change_configuration(
    &mut self,
    params: DidChangeConfigurationParams,
  ) {
    let mark = self
      .performance
      .mark("did_change_configuration", Some(&params));

    let maybe_config =
      if self.config.client_capabilities.workspace_configuration {
        let config_response = self
          .client
          .configuration(vec![ConfigurationItem {
            scope_uri: None,
            section: Some(SETTINGS_SECTION.to_string()),
          }])
          .await;
        if let Err(err) = self.config.update_all_settings().await {
          error!("Cannot request updating all settings: {}", err);
        }
        match config_response {
          Ok(value_vec) => value_vec.get(0).cloned(),
          Err(err) => {
            error!("Error getting workspace configuration: {}", err);
            None
          }
        }
      } else {
        params
          .settings
          .as_object()
          .map(|settings| settings.get(SETTINGS_SECTION))
          .flatten()
          .cloned()
      };

    if let Some(value) = maybe_config {
      if let Err(err) = self.config.set_workspace_settings(value) {
        error!("failed to update settings: {}", err);
      }
    }

    self.update_debug_flag();
    if let Err(err) = self.update_cache() {
      self.client.show_message(MessageType::Warning, err).await;
    }
    if let Err(err) = self.update_import_map().await {
      self.client.show_message(MessageType::Warning, err).await;
    }
    if let Err(err) = self.update_registries().await {
      self.client.show_message(MessageType::Warning, err).await;
    }
    if let Err(err) = self.update_tsconfig().await {
      self.client.show_message(MessageType::Warning, err).await;
    }
    if let Err(err) = self.diagnostics_server.update() {
      error!("{}", err);
    }

    self.performance.measure(mark);
  }

  async fn did_change_watched_files(
    &mut self,
    params: DidChangeWatchedFilesParams,
  ) {
    let mark = self
      .performance
      .mark("did_change_watched_files", Some(&params));
    let mut touched = false;
    // if the current import map has changed, we need to reload it
    if let Some(import_map_uri) = &self.maybe_import_map_uri {
      if params.changes.iter().any(|fe| *import_map_uri == fe.uri) {
        if let Err(err) = self.update_import_map().await {
          self.client.show_message(MessageType::Warning, err).await;
        }
        touched = true;
      }
    }
    // if the current tsconfig has changed, we need to reload it
    if let Some(config_uri) = &self.maybe_config_uri {
      if params.changes.iter().any(|fe| *config_uri == fe.uri) {
        if let Err(err) = self.update_tsconfig().await {
          self.client.show_message(MessageType::Warning, err).await;
        }
        touched = true;
      }
    }
    if touched {
      self.analyze_dependencies_all();
      self.diagnostics_server.invalidate_all().await;
      if let Err(err) = self.diagnostics_server.update() {
        error!("Cannot update diagnostics: {}", err);
      }
    }
    self.performance.measure(mark);
  }

  async fn document_symbol(
    &mut self,
    params: DocumentSymbolParams,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("document_symbol", Some(&params));

    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::GetNavigationTree(specifier);
    let navigation_tree: tsc::NavigationTree = self
      .ts_server
      .request(self.snapshot()?, req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let response = if let Some(child_items) = navigation_tree.child_items {
      let mut document_symbols = Vec::<DocumentSymbol>::new();
      for item in child_items {
        item.collect_document_symbols(&line_index, &mut document_symbols);
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
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.documents.is_formattable(&specifier) {
      return Ok(None);
    }
    let mark = self.performance.mark("formatting", Some(&params));
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
      let config = get_typescript_config();
      match format_file(&file_path, &file_text, config) {
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

  async fn hover(&mut self, params: HoverParams) -> LspResult<Option<Hover>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("hover", Some(&params));
    let hover = if let Some(dependency_range) =
      self.documents.is_specifier_position(
        &specifier,
        &params.text_document_position_params.position,
      ) {
      if let Some(dependencies) = &self.documents.dependencies(&specifier) {
        if let Some(dep) = dependencies.get(&dependency_range.specifier) {
          let value = match (&dep.maybe_code, &dep.maybe_type) {
            (Some(code_dep), Some(type_dep)) => {
              format!(
                "**Resolved Dependency**\n\n**Code**: {}\n\n**Types**: {}\n",
                code_dep.as_hover_text(),
                type_dep.as_hover_text()
              )
            }
            (Some(code_dep), None) => {
              format!(
                "**Resolved Dependency**\n\n**Code**: {}\n",
                code_dep.as_hover_text()
              )
            }
            (None, Some(type_dep)) => {
              format!(
                "**Resolved Dependency**\n\n**Types**: {}\n",
                type_dep.as_hover_text()
              )
            }
            (None, None) => {
              error!(
                "Unexpected state hovering on dependency. Dependency \"{}\" in \"{}\" not found.",
                dependency_range.specifier,
                specifier
              );
              "".to_string()
            }
          };
          Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
              kind: MarkupKind::Markdown,
              value,
            }),
            range: Some(dependency_range.range),
          })
        } else {
          None
        }
      } else {
        None
      }
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
      let req = tsc::RequestMethod::GetQuickInfo((
        specifier,
        line_index.offset_tsc(params.text_document_position_params.position)?,
      ));
      let maybe_quick_info: Option<tsc::QuickInfo> = self
        .ts_server
        .request(self.snapshot()?, req)
        .await
        .map_err(|err| {
          error!("Unable to get quick info: {}", err);
          LspError::internal_error()
        })?;
      maybe_quick_info.map(|qi| qi.to_hover(&line_index))
    };
    self.performance.measure(mark);
    Ok(hover)
  }

  async fn code_action(
    &mut self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("code_action", Some(&params));
    let mut all_actions = CodeActionResponse::new();
    let line_index = self.get_line_index_sync(&specifier).unwrap();

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
          "deno-lint" => matches!(&d.code, Some(_)),
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
    if !fixable_diagnostics.is_empty() {
      let mut code_actions = CodeActionCollection::default();
      let file_diagnostics = self
        .diagnostics_server
        .get(&specifier, DiagnosticSource::TypeScript)
        .await;
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
              match self.ts_server.request(self.snapshot()?, req).await {
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
                .add_ts_fix_action(&specifier, &action, diagnostic, self)
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
          Some("deno") => code_actions
            .add_deno_fix_action(diagnostic)
            .map_err(|err| {
              error!("{}", err);
              LspError::internal_error()
            })?,
          Some("deno-lint") => code_actions
            .add_deno_lint_ignore_action(
              &specifier,
              self.documents.docs.get(&specifier),
              diagnostic,
            )
            .map_err(|err| {
              error!("Unable to fix lint error: {}", err);
              LspError::internal_error()
            })?,
          _ => (),
        }
      }
      code_actions.set_preferred_fixes();
      all_actions.extend(code_actions.get_response());
    }

    // Refactor
    let start = line_index.offset_tsc(params.range.start)?;
    let length = line_index.offset_tsc(params.range.end)? - start;
    let only =
      params
        .context
        .only
        .as_ref()
        .map_or(String::default(), |values| {
          values
            .first()
            .map_or(String::default(), |v| v.as_str().to_owned())
        });
    let req = tsc::RequestMethod::GetApplicableRefactors((
      specifier.clone(),
      tsc::TextSpan { start, length },
      only,
    ));
    let refactor_infos: Vec<tsc::ApplicableRefactorInfo> = self
      .ts_server
      .request(self.snapshot()?, req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;
    let mut refactor_actions = Vec::<CodeAction>::new();
    for refactor_info in refactor_infos.iter() {
      refactor_actions
        .extend(refactor_info.to_code_actions(&specifier, &params.range));
    }
    all_actions.extend(
      refactor::prune_invalid_actions(&refactor_actions, 5)
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
    &mut self,
    params: CodeAction,
  ) -> LspResult<CodeAction> {
    if params.kind.is_none() || params.data.is_none() {
      return Ok(params);
    }

    let mark = self.performance.mark("code_action_resolve", Some(&params));
    let kind = params.kind.clone().unwrap();
    let data = params.data.clone().unwrap();

    let result = if kind.as_str().starts_with(CodeActionKind::QUICKFIX.as_str())
    {
      let code_action_data: CodeActionData =
        from_value(data).map_err(|err| {
          error!("Unable to decode code action data: {}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let req = tsc::RequestMethod::GetCombinedCodeFix((
        code_action_data.specifier.clone(),
        json!(code_action_data.fix_id.clone()),
      ));
      let combined_code_actions: tsc::CombinedCodeActions = self
        .ts_server
        .request(self.snapshot()?, req)
        .await
        .map_err(|err| {
          error!("Unable to get combined fix from TypeScript: {}", err);
          LspError::internal_error()
        })?;
      if combined_code_actions.commands.is_some() {
        error!("Deno does not support code actions with commands.");
        return Err(LspError::invalid_request());
      }

      let changes = if code_action_data.fix_id == "fixMissingImport" {
        fix_ts_import_changes(
          &code_action_data.specifier,
          &combined_code_actions.changes,
          self,
        )
        .map_err(|err| {
          error!("Unable to remap changes: {}", err);
          LspError::internal_error()
        })?
      } else {
        combined_code_actions.changes.clone()
      };
      let mut code_action = params.clone();
      code_action.edit =
        ts_changes_to_edit(&changes, self).await.map_err(|err| {
          error!("Unable to convert changes to edits: {}", err);
          LspError::internal_error()
        })?;
      code_action
    } else if kind.as_str().starts_with(CodeActionKind::REFACTOR.as_str()) {
      let mut code_action = params.clone();
      let action_data: refactor::RefactorCodeActionData = from_value(data)
        .map_err(|err| {
          error!("Unable to decode code action data: {}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let line_index =
        self.get_line_index_sync(&action_data.specifier).unwrap();
      let start = line_index.offset_tsc(action_data.range.start)?;
      let length = line_index.offset_tsc(action_data.range.end)? - start;
      let req = tsc::RequestMethod::GetEditsForRefactor((
        action_data.specifier.clone(),
        tsc::TextSpan { start, length },
        action_data.refactor_name.clone(),
        action_data.action_name.clone(),
      ));
      let refactor_edit_info: tsc::RefactorEditInfo = self
        .ts_server
        .request(self.snapshot()?, req)
        .await
        .map_err(|err| {
          error!("Failed to request to tsserver {}", err);
          LspError::invalid_request()
        })?;
      code_action.edit = refactor_edit_info
        .to_workspace_edit(self)
        .await
        .map_err(|err| {
          error!("Unable to convert changes to edits: {}", err);
          LspError::internal_error()
        })?;
      code_action
    } else {
      // The code action doesn't need to be resolved
      params
    };

    self.performance.measure(mark);
    Ok(result)
  }

  async fn code_lens(
    &mut self,
    params: CodeLensParams,
  ) -> LspResult<Option<Vec<CodeLens>>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
      || !(self.config.get_workspace_settings().enabled_code_lens()
        || self.config.specifier_code_lens_test(&specifier))
    {
      return Ok(None);
    }

    let mark = self.performance.mark("code_lens", Some(&params));
    let code_lenses =
      code_lens::collect(&specifier, self).await.map_err(|err| {
        error!("Error getting code lenses for \"{}\": {}", specifier, err);
        LspError::internal_error()
      })?;
    self.performance.measure(mark);

    Ok(Some(code_lenses))
  }

  async fn code_lens_resolve(
    &mut self,
    code_lens: CodeLens,
  ) -> LspResult<CodeLens> {
    let mark = self.performance.mark("code_lens_resolve", Some(&code_lens));
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
    &mut self,
    params: DocumentHighlightParams,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("document_highlight", Some(&params));
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
      .request(self.snapshot()?, req)
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("references", Some(&params));
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
      .request(self.snapshot()?, req)
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("goto_definition", Some(&params));
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
      .request(self.snapshot()?, req)
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
    &mut self,
    params: CompletionParams,
  ) -> LspResult<Option<CompletionResponse>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("completion", Some(&params));
    // Import specifiers are something wholly internal to Deno, so for
    // completions, we will use internal logic and if there are completions
    // for imports, we will return those and not send a message into tsc, where
    // other completions come from.
    let response = if let Some(response) = completions::get_import_completions(
      &specifier,
      &params.text_document_position.position,
      &self.snapshot()?,
      self.client.clone(),
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
        .request(self.snapshot()?, req)
        .await
        .map_err(|err| {
          error!("Unable to get completion info from TypeScript: {}", err);
          LspError::internal_error()
        })?;

      if let Some(completions) = maybe_completion_info {
        let results = completions.as_completion_response(
          &line_index,
          &self.config.get_workspace_settings().suggest,
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
    let mark = self.performance.mark("completion_resolve", Some(&params));
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
        let maybe_completion_info: Option<tsc::CompletionEntryDetails> = self
          .ts_server
          .request(self.snapshot()?, req)
          .await
          .map_err(|err| {
            error!("Unable to get completion info from TypeScript: {}", err);
            LspError::internal_error()
          })?;
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("goto_implementation", Some(&params));
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
      .request(self.snapshot()?, req)
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
    &mut self,
    params: FoldingRangeParams,
  ) -> LspResult<Option<Vec<FoldingRange>>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("folding_range", Some(&params));
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
      .request(self.snapshot()?, req)
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

  async fn incoming_calls(
    &mut self,
    params: CallHierarchyIncomingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
    let specifier = self.url_map.normalize_url(&params.item.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("incoming_calls", Some(&params));
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::ProvideCallHierarchyIncomingCalls((
      specifier.clone(),
      line_index.offset_tsc(params.item.selection_range.start)?,
    ));
    let incoming_calls: Vec<tsc::CallHierarchyIncomingCall> = self
      .ts_server
      .request(self.snapshot()?, req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let maybe_root_path_owned = self
      .config
      .root_uri
      .as_ref()
      .and_then(|uri| uri.to_file_path().ok());
    let mut resolved_items = Vec::<CallHierarchyIncomingCall>::new();
    for item in incoming_calls.iter() {
      if let Some(resolved) = item
        .try_resolve_call_hierarchy_incoming_call(
          self,
          maybe_root_path_owned.as_deref(),
        )
        .await
      {
        resolved_items.push(resolved);
      }
    }
    self.performance.measure(mark);
    Ok(Some(resolved_items))
  }

  async fn outgoing_calls(
    &mut self,
    params: CallHierarchyOutgoingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
    let specifier = self.url_map.normalize_url(&params.item.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("outgoing_calls", Some(&params));
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::ProvideCallHierarchyOutgoingCalls((
      specifier.clone(),
      line_index.offset_tsc(params.item.selection_range.start)?,
    ));
    let outgoing_calls: Vec<tsc::CallHierarchyOutgoingCall> = self
      .ts_server
      .request(self.snapshot()?, req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let maybe_root_path_owned = self
      .config
      .root_uri
      .as_ref()
      .and_then(|uri| uri.to_file_path().ok());
    let mut resolved_items = Vec::<CallHierarchyOutgoingCall>::new();
    for item in outgoing_calls.iter() {
      if let Some(resolved) = item
        .try_resolve_call_hierarchy_outgoing_call(
          &line_index,
          self,
          maybe_root_path_owned.as_deref(),
        )
        .await
      {
        resolved_items.push(resolved);
      }
    }
    self.performance.measure(mark);
    Ok(Some(resolved_items))
  }

  async fn prepare_call_hierarchy(
    &mut self,
    params: CallHierarchyPrepareParams,
  ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark("prepare_call_hierarchy", Some(&params));
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::PrepareCallHierarchy((
      specifier.clone(),
      line_index.offset_tsc(params.text_document_position_params.position)?,
    ));
    let maybe_one_or_many: Option<tsc::OneOrMany<tsc::CallHierarchyItem>> =
      self
        .ts_server
        .request(self.snapshot()?, req)
        .await
        .map_err(|err| {
          error!("Failed to request to tsserver {}", err);
          LspError::invalid_request()
        })?;

    let response = if let Some(one_or_many) = maybe_one_or_many {
      let maybe_root_path_owned = self
        .config
        .root_uri
        .as_ref()
        .and_then(|uri| uri.to_file_path().ok());
      let mut resolved_items = Vec::<CallHierarchyItem>::new();
      match one_or_many {
        tsc::OneOrMany::One(item) => {
          if let Some(resolved) = item
            .try_resolve_call_hierarchy_item(
              self,
              maybe_root_path_owned.as_deref(),
            )
            .await
          {
            resolved_items.push(resolved)
          }
        }
        tsc::OneOrMany::Many(items) => {
          for item in items.iter() {
            if let Some(resolved) = item
              .try_resolve_call_hierarchy_item(
                self,
                maybe_root_path_owned.as_deref(),
              )
              .await
            {
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
    &mut self,
    params: RenameParams,
  ) -> LspResult<Option<WorkspaceEdit>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("rename", Some(&params));
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::FindRenameLocations {
      specifier,
      position: line_index
        .offset_tsc(params.text_document_position.position)?,
      find_in_strings: false,
      find_in_comments: false,
      provide_prefix_and_suffix_text_for_rename: false,
    };

    let maybe_locations: Option<Vec<tsc::RenameLocation>> = self
      .ts_server
      .request(self.snapshot()?, req)
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
      lsp_custom::CACHE_REQUEST => match params.map(serde_json::from_value) {
        Some(Ok(params)) => self.cache(params).await,
        Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
        None => Err(LspError::invalid_params("Missing parameters")),
      },
      lsp_custom::PERFORMANCE_REQUEST => Ok(Some(self.get_performance())),
      lsp_custom::RELOAD_IMPORT_REGISTRIES_REQUEST => {
        self.reload_import_registries().await
      }
      lsp_custom::VIRTUAL_TEXT_DOCUMENT => {
        match params.map(serde_json::from_value) {
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
        }
      }
      _ => {
        error!("Got a {} request, but no handler is defined", method);
        Err(LspError::method_not_found())
      }
    }
  }

  async fn selection_range(
    &mut self,
    params: SelectionRangeParams,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("selection_range", Some(&params));
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
        .request(self.snapshot()?, req)
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

  async fn semantic_tokens_full(
    &mut self,
    params: SemanticTokensParams,
  ) -> LspResult<Option<SemanticTokensResult>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("semantic_tokens_full", Some(&params));
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let req = tsc::RequestMethod::GetEncodedSemanticClassifications((
      specifier.clone(),
      tsc::TextSpan {
        start: 0,
        length: line_index.text_content_length_utf16().into(),
      },
    ));
    let semantic_classification: tsc::Classifications = self
      .ts_server
      .request(self.snapshot()?, req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let semantic_tokens: SemanticTokens =
      semantic_classification.to_semantic_tokens(&line_index);
    let response = if !semantic_tokens.data.is_empty() {
      Some(SemanticTokensResult::Tokens(semantic_tokens))
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn semantic_tokens_range(
    &mut self,
    params: SemanticTokensRangeParams,
  ) -> LspResult<Option<SemanticTokensRangeResult>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark("semantic_tokens_range", Some(&params));
    let line_index =
      if let Some(line_index) = self.get_line_index_sync(&specifier) {
        line_index
      } else {
        return Err(LspError::invalid_params(format!(
          "An unexpected specifier ({}) was provided.",
          specifier
        )));
      };

    let start = line_index.offset_tsc(params.range.start)?;
    let length = line_index.offset_tsc(params.range.end)? - start;
    let req = tsc::RequestMethod::GetEncodedSemanticClassifications((
      specifier.clone(),
      tsc::TextSpan { start, length },
    ));
    let semantic_classification: tsc::Classifications = self
      .ts_server
      .request(self.snapshot()?, req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let semantic_tokens: SemanticTokens =
      semantic_classification.to_semantic_tokens(&line_index);
    let response = if !semantic_tokens.data.is_empty() {
      Some(SemanticTokensRangeResult::Tokens(semantic_tokens))
    } else {
      None
    };
    self.performance.measure(mark);
    Ok(response)
  }

  async fn signature_help(
    &mut self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.documents.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("signature_help", Some(&params));
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
      .request(self.snapshot()?, req)
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

  async fn document_symbol(
    &self,
    params: DocumentSymbolParams,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    self.0.lock().await.document_symbol(params).await
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

  async fn incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
    self.0.lock().await.incoming_calls(params).await
  }

  async fn outgoing_calls(
    &self,
    params: CallHierarchyOutgoingCallsParams,
  ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
    self.0.lock().await.outgoing_calls(params).await
  }

  async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams,
  ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
    self.0.lock().await.prepare_call_hierarchy(params).await
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

  async fn semantic_tokens_full(
    &self,
    params: SemanticTokensParams,
  ) -> LspResult<Option<SemanticTokensResult>> {
    self.0.lock().await.semantic_tokens_full(params).await
  }

  async fn semantic_tokens_range(
    &self,
    params: SemanticTokensRangeParams,
  ) -> LspResult<Option<SemanticTokensRangeResult>> {
    self.0.lock().await.semantic_tokens_range(params).await
  }

  async fn signature_help(
    &self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    self.0.lock().await.signature_help(params).await
  }
}

// These are implementations of custom commands supported by the LSP
impl Inner {
  /// Similar to `deno cache` on the command line, where modules will be cached
  /// in the Deno cache, including any of their dependencies.
  async fn cache(
    &mut self,
    params: lsp_custom::CacheParams,
  ) -> LspResult<Option<Value>> {
    let referrer = self.url_map.normalize_url(&params.referrer.uri);
    if !self.documents.is_diagnosable(&referrer) {
      return Ok(None);
    }

    let mark = self.performance.mark("cache", Some(&params));
    if !params.uris.is_empty() {
      for identifier in &params.uris {
        let specifier = self.url_map.normalize_url(&identifier.uri);
        sources::cache(
          &specifier,
          &self.maybe_import_map,
          &self.maybe_config_file,
          &self.maybe_cache_path,
        )
        .await
        .map_err(|err| {
          error!("{}", err);
          LspError::internal_error()
        })?;
      }
    } else {
      sources::cache(
        &referrer,
        &self.maybe_import_map,
        &self.maybe_config_file,
        &self.maybe_cache_path,
      )
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
        let media_type =
          MediaType::from(&self.documents.get_language_id(&referrer).unwrap());
        self.analyze_dependencies(&referrer, &media_type, &source);
      }
      self.diagnostics_server.invalidate(vec![referrer]).await;
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
    fs_util::remove_dir_all_if_exists(&self.module_registries_location)
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
    params: lsp_custom::VirtualTextDocumentParams,
  ) -> LspResult<Option<String>> {
    let mark = self
      .performance
      .mark("virtual_text_document", Some(&params));
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    let contents = if specifier.as_str() == "deno:/status.md" {
      let mut contents = String::new();
      let mut documents_specifiers = self.documents.specifiers();
      documents_specifiers.sort();
      let mut sources_specifiers = self.sources.specifiers();
      sources_specifiers.sort();
      let measures = self.performance.to_vec();
      let workspace_settings = self.config.get_workspace_settings();

      contents.push_str(&format!(
        r#"# Deno Language Server Status

## Workspace Settings

```json
{}
```

## Workspace Details

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
        serde_json::to_string_pretty(&workspace_settings).unwrap(),
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
