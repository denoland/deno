// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_graph::ModuleKind;
use deno_runtime::tokio_util::run_local;
use import_map::ImportMap;
use log::error;
use log::warn;
use serde_json::from_value;
use std::env;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tower_lsp::jsonrpc::Error as LspError;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::request::*;
use tower_lsp::lsp_types::*;

use super::analysis::fix_ts_import_changes;
use super::analysis::ts_changes_to_edit;
use super::analysis::CodeActionCollection;
use super::analysis::CodeActionData;
use super::cache;
use super::capabilities;
use super::client::Client;
use super::code_lens;
use super::completions;
use super::config::Config;
use super::config::SETTINGS_SECTION;
use super::diagnostics;
use super::diagnostics::DiagnosticsServer;
use super::documents::to_hover_text;
use super::documents::to_lsp_range;
use super::documents::AssetOrDocument;
use super::documents::Document;
use super::documents::Documents;
use super::documents::LanguageId;
use super::logging::lsp_log;
use super::lsp_custom;
use super::parent_process_checker;
use super::performance::Performance;
use super::refactor;
use super::registries::ModuleRegistry;
use super::registries::ModuleRegistryOptions;
use super::testing;
use super::text;
use super::tsc;
use super::tsc::Assets;
use super::tsc::AssetsSnapshot;
use super::tsc::TsServer;
use super::urls;
use crate::args::CliOptions;
use crate::args::ConfigFile;
use crate::args::Flags;
use crate::args::FmtConfig;
use crate::args::LintConfig;
use crate::args::TsConfig;
use crate::deno_dir;
use crate::file_fetcher::get_source_from_data_url;
use crate::fs_util;
use crate::graph_util::graph_valid;
use crate::proc_state::import_map_from_text;
use crate::proc_state::ProcState;
use crate::tools::fmt::format_file;
use crate::tools::fmt::format_parsed_source;

pub const REGISTRIES_PATH: &str = "registries";
const CACHE_PATH: &str = "deps";

#[derive(Debug, Clone)]
pub struct LanguageServer(Arc<tokio::sync::Mutex<Inner>>);

/// Snapshot of the state used by TSC.
#[derive(Debug, Default)]
pub struct StateSnapshot {
  pub assets: AssetsSnapshot,
  pub cache_metadata: cache::CacheMetadata,
  pub documents: Documents,
  pub maybe_import_map: Option<Arc<ImportMap>>,
  pub root_uri: Option<Url>,
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
  diagnostics_server: diagnostics::DiagnosticsServer,
  /// The collection of documents that the server is currently handling, either
  /// on disk or "open" within the client.
  pub documents: Documents,
  /// Handles module registries, which allow discovery of modules
  module_registries: ModuleRegistry,
  /// The path to the module registries cache
  module_registries_location: PathBuf,
  /// An optional path to the DENO_DIR which has been specified in the client
  /// options.
  maybe_cache_path: Option<PathBuf>,
  /// An optional configuration file which has been specified in the client
  /// options.
  maybe_config_file: Option<ConfigFile>,
  /// An optional configuration for formatter which has been taken from specified config file.
  maybe_fmt_config: Option<FmtConfig>,
  /// An optional import map which is used to resolve modules.
  pub maybe_import_map: Option<Arc<ImportMap>>,
  /// The URL for the import map which is used to determine relative imports.
  maybe_import_map_uri: Option<Url>,
  /// An optional configuration for linter which has been taken from specified config file.
  pub maybe_lint_config: Option<LintConfig>,
  /// A lazily create "server" for handling test run requests.
  maybe_testing_server: Option<testing::TestServer>,
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
  pub fn new(client: Client) -> Self {
    Self(Arc::new(tokio::sync::Mutex::new(Inner::new(client))))
  }

  pub async fn cache_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    match params.map(serde_json::from_value) {
      Some(Ok(params)) => self.0.lock().await.cache(params).await,
      Some(Err(err)) => Err(LspError::invalid_params(err.to_string())),
      None => Err(LspError::invalid_params("Missing parameters")),
    }
  }

  pub async fn performance_request(&self) -> LspResult<Option<Value>> {
    Ok(Some(self.0.lock().await.get_performance()))
  }

  pub async fn reload_import_registries_request(
    &self,
  ) -> LspResult<Option<Value>> {
    self.0.lock().await.reload_import_registries().await
  }

  pub async fn task_request(&self) -> LspResult<Option<Value>> {
    self.0.lock().await.get_tasks()
  }

  pub async fn test_run_request(
    &self,
    params: Option<Value>,
  ) -> LspResult<Option<Value>> {
    let inner = self.0.lock().await;
    if let Some(testing_server) = &inner.maybe_testing_server {
      match params.map(serde_json::from_value) {
        Some(Ok(params)) => testing_server
          .run_request(params, inner.config.get_workspace_settings()),
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
    if let Some(testing_server) = &self.0.lock().await.maybe_testing_server {
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
          self.0.lock().await.virtual_text_document(params)?,
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
}

impl Inner {
  fn new(client: Client) -> Self {
    let maybe_custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(maybe_custom_root)
      .expect("could not access DENO_DIR");
    let module_registries_location = dir.root.join(REGISTRIES_PATH);
    let module_registries = ModuleRegistry::new(
      &module_registries_location,
      ModuleRegistryOptions::default(),
    )
    .expect("could not create module registries");
    let location = dir.root.join(CACHE_PATH);
    let documents = Documents::new(&location);
    let cache_metadata = cache::CacheMetadata::new(&location);
    let performance = Arc::new(Performance::default());
    let ts_server = Arc::new(TsServer::new(performance.clone()));
    let config = Config::new();
    let diagnostics_server = DiagnosticsServer::new(
      client.clone(),
      performance.clone(),
      ts_server.clone(),
    );
    let assets = Assets::new(ts_server.clone());

    Self {
      assets,
      cache_metadata,
      client,
      config,
      diagnostics_server,
      documents,
      maybe_cache_path: None,
      maybe_config_file: None,
      maybe_import_map: None,
      maybe_import_map_uri: None,
      maybe_lint_config: None,
      maybe_fmt_config: None,
      maybe_testing_server: None,
      module_registries,
      module_registries_location,
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
    self.get_maybe_asset_or_document(specifier).map_or_else(
      || {
        Err(LspError::invalid_params(format!(
          "Unable to find asset or document for: {}",
          specifier
        )))
      },
      Ok,
    )
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
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<Arc<tsc::NavigationTree>, AnyError> {
    let mark = self.performance.mark(
      "get_navigation_tree",
      Some(json!({ "specifier": specifier })),
    );
    let asset_or_doc = self.get_asset_or_document(specifier)?;
    let navigation_tree =
      if let Some(navigation_tree) = asset_or_doc.maybe_navigation_tree() {
        navigation_tree
      } else {
        let navigation_tree: tsc::NavigationTree = self
          .ts_server
          .request(
            self.snapshot(),
            tsc::RequestMethod::GetNavigationTree(specifier.clone()),
          )
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

  /// Returns a tuple with parsed `ConfigFile` and `Url` pointing to that file.
  /// If there's no config file specified in settings returns `None`.
  fn get_config_file(&self) -> Result<Option<ConfigFile>, AnyError> {
    let workspace_settings = self.config.get_workspace_settings();
    let maybe_config = workspace_settings.config;
    if let Some(config_str) = &maybe_config {
      if !config_str.is_empty() {
        lsp_log!("Setting Deno configuration from: \"{}\"", config_str);
        let config_url = if let Ok(url) = Url::from_file_path(config_str) {
          Ok(url)
        } else if let Some(root_uri) = &self.config.root_uri {
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

        let config_file = ConfigFile::from_specifier(&config_url)?;
        return Ok(Some(config_file));
      }
    }

    // Auto-discover config

    // It is possible that root_uri is not set, for example when having a single
    // file open and not a workspace.  In those situations we can't
    // automatically discover the configuration
    if let Some(root_uri) = &self.config.root_uri {
      let root_path = fs_util::specifier_to_file_path(root_uri)?;
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

  fn is_diagnosable(&self, specifier: &ModuleSpecifier) -> bool {
    if specifier.scheme() == "asset" {
      matches!(
        MediaType::from(specifier),
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
    if let Some(config_file) = self.maybe_config_file.as_ref() {
      let (value, maybe_ignored_options) = config_file.to_compiler_options()?;
      tsconfig.merge(&value);
      if let Some(ignored_options) = maybe_ignored_options {
        // TODO(@kitsonk) turn these into diagnostics that can be sent to the
        // client
        warn!("{}", ignored_options);
      }
    }

    Ok(())
  }

  pub fn snapshot(&self) -> Arc<StateSnapshot> {
    Arc::new(StateSnapshot {
      assets: self.assets.snapshot(),
      cache_metadata: self.cache_metadata.clone(),
      documents: self.documents.clone(),
      maybe_import_map: self.maybe_import_map.clone(),
      root_uri: self.config.root_uri.clone(),
    })
  }

  pub fn update_cache(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_cache", None::<()>);
    self.performance.measure(mark);
    let maybe_cache = self.config.get_workspace_settings().cache;
    let maybe_cache_path = if let Some(cache_str) = &maybe_cache {
      lsp_log!("Setting cache path from: \"{}\"", cache_str);
      let cache_url = if let Ok(url) = Url::from_file_path(cache_str) {
        Ok(url)
      } else if let Some(root_uri) = &self.config.root_uri {
        let root_path = fs_util::specifier_to_file_path(root_uri)?;
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
      let cache_path = fs_util::specifier_to_file_path(&cache_url)?;
      lsp_log!(
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
      let dir = deno_dir::DenoDir::new(maybe_custom_root)?;
      let module_registries_location = dir.root.join(REGISTRIES_PATH);
      let workspace_settings = self.config.get_workspace_settings();
      let maybe_root_path = self
        .config
        .root_uri
        .as_ref()
        .and_then(|uri| fs_util::specifier_to_file_path(uri).ok());
      self.module_registries = ModuleRegistry::new(
        &module_registries_location,
        ModuleRegistryOptions {
          maybe_root_path,
          maybe_ca_stores: workspace_settings.certificate_stores.clone(),
          maybe_ca_file: workspace_settings.tls_certificate.clone(),
          unsafely_ignore_certificate_errors: workspace_settings
            .unsafely_ignore_certificate_errors,
        },
      )?;
      self.module_registries_location = module_registries_location;
      let location = dir.root.join(CACHE_PATH);
      self.documents.set_location(&location);
      self.cache_metadata.set_location(&location);
      self.maybe_cache_path = maybe_cache_path;
    }
    Ok(())
  }

  pub async fn update_import_map(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_import_map", None::<()>);
    let maybe_import_map_url = if let Some(import_map_str) =
      self.config.get_workspace_settings().import_map
    {
      lsp_log!(
        "Setting import map from workspace settings: \"{}\"",
        import_map_str
      );
      if let Some(config_file) = &self.maybe_config_file {
        if let Some(import_map_path) = config_file.to_import_map_path() {
          lsp_log!("Warning: Import map \"{}\" configured in \"{}\" being ignored due to an import map being explicitly configured in workspace settings.", import_map_path, config_file.specifier);
        }
      }
      if let Ok(url) = Url::from_file_path(&import_map_str) {
        Some(url)
      } else if import_map_str.starts_with("data:") {
        Some(Url::parse(&import_map_str).map_err(|_| {
          anyhow!("Bad data url for import map: {}", import_map_str)
        })?)
      } else if let Some(root_uri) = &self.config.root_uri {
        let root_path = fs_util::specifier_to_file_path(root_uri)?;
        let import_map_path = root_path.join(&import_map_str);
        Some(Url::from_file_path(import_map_path).map_err(|_| {
          anyhow!("Bad file path for import map: {}", import_map_str)
        })?)
      } else {
        return Err(anyhow!(
          "The path to the import map (\"{}\") is not resolvable.",
          import_map_str
        ));
      }
    } else if let Some(config_file) = &self.maybe_config_file {
      if let Some(import_map_path) = config_file.to_import_map_path() {
        lsp_log!(
          "Setting import map from configuration file: \"{}\"",
          import_map_path
        );
        let specifier =
          if let Ok(config_file_path) = config_file.specifier.to_file_path() {
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
    };
    if let Some(import_map_url) = maybe_import_map_url {
      let import_map_json = if import_map_url.scheme() == "data" {
        get_source_from_data_url(&import_map_url)?.0
      } else {
        let import_map_path = fs_util::specifier_to_file_path(&import_map_url)?;
        lsp_log!(
          "  Resolved import map: \"{}\"",
          import_map_path.to_string_lossy()
        );
        fs::read_to_string(import_map_path).await.map_err(|err| {
          anyhow!(
            "Failed to load the import map at: {}. [{}]",
            import_map_url,
            err
          )
        })?
      };
      let import_map = import_map_from_text(&import_map_url, &import_map_json)?;
      self.maybe_import_map_uri = Some(import_map_url);
      self.maybe_import_map = Some(Arc::new(import_map));
    } else {
      self.maybe_import_map_uri = None;
      self.maybe_import_map = None;
    }
    self.performance.measure(mark);
    Ok(())
  }

  pub fn update_debug_flag(&self) {
    let internal_debug = self.config.get_workspace_settings().internal_debug;
    super::logging::set_lsp_debug_flag(internal_debug)
  }

  async fn update_registries(&mut self) -> Result<(), AnyError> {
    let mark = self.performance.mark("update_registries", None::<()>);
    let workspace_settings = self.config.get_workspace_settings();
    let maybe_root_path = self
      .config
      .root_uri
      .as_ref()
      .and_then(|uri| fs_util::specifier_to_file_path(uri).ok());
    self.module_registries = ModuleRegistry::new(
      &self.module_registries_location,
      ModuleRegistryOptions {
        maybe_root_path,
        maybe_ca_stores: workspace_settings.certificate_stores.clone(),
        maybe_ca_file: workspace_settings.tls_certificate.clone(),
        unsafely_ignore_certificate_errors: workspace_settings
          .unsafely_ignore_certificate_errors
          .clone(),
      },
    )?;
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

  fn update_config_file(&mut self) -> Result<(), AnyError> {
    self.maybe_config_file = None;
    self.maybe_fmt_config = None;
    self.maybe_lint_config = None;

    if let Some(config_file) = self.get_config_file()? {
      let lint_config = config_file
        .to_lint_config()
        .map_err(|err| {
          anyhow!("Unable to update lint configuration: {:?}", err)
        })?
        .unwrap_or_default();
      let fmt_config = config_file
        .to_fmt_config()
        .map_err(|err| {
          anyhow!("Unable to update formatter configuration: {:?}", err)
        })?
        .unwrap_or_default();

      self.maybe_config_file = Some(config_file);
      self.maybe_lint_config = Some(lint_config);
      self.maybe_fmt_config = Some(fmt_config);
    }

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
    let workspace_settings = config.get_workspace_settings();
    if workspace_settings.unstable {
      let unstable_libs = json!({
        "lib": ["deno.ns", "deno.window", "deno.unstable"]
      });
      tsconfig.merge(&unstable_libs);
    }
    if let Err(err) = self.merge_user_tsconfig(&mut tsconfig) {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    let _ok: bool = self
      .ts_server
      .request(self.snapshot(), tsc::RequestMethod::Configure(tsconfig))
      .await?;
    self.performance.measure(mark);
    Ok(())
  }
}

// lspower::LanguageServer methods. This file's LanguageServer delegates to us.
impl Inner {
  async fn initialize(
    &mut self,
    params: InitializeParams,
  ) -> LspResult<InitializeResult> {
    lsp_log!("Starting Deno language server...");
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
      // sometimes this root uri may not have a trailing slash, so force it to
      self.config.root_uri = params
        .root_uri
        .map(|s| self.url_map.normalize_url(&s))
        .map(fs_util::ensure_directory_specifier);

      if let Some(value) = params.initialization_options {
        self.config.set_workspace_settings(value).map_err(|err| {
          error!("Cannot set workspace settings: {}", err);
          LspError::internal_error()
        })?;
      }
      self.config.workspace_folders = params.workspace_folders.map(|folders| {
        folders
          .into_iter()
          .map(|folder| (self.url_map.normalize_url(&folder.uri), folder))
          .collect()
      });
      self.config.update_capabilities(&params.capabilities);
    }

    self.update_debug_flag();
    // Check to see if we need to change the cache path
    if let Err(err) = self.update_cache() {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    if let Err(err) = self.update_config_file() {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    if let Err(err) = self.update_tsconfig().await {
      self.client.show_message(MessageType::WARNING, err).await;
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

    // Check to see if we need to setup the import map
    if let Err(err) = self.update_import_map().await {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    // Check to see if we need to setup any module registries
    if let Err(err) = self.update_registries().await {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    self.documents.update_config(
      self.maybe_import_map.clone(),
      self.maybe_config_file.as_ref(),
    );

    self.assets.intitialize(self.snapshot()).await;

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
            glob_pattern: "**/*.{json,jsonc}".to_string(),
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
    self.config.update_enabled_paths(self.client.clone()).await;

    if self.config.client_capabilities.testing_api {
      let test_server = testing::TestServer::new(
        self.client.clone(),
        self.performance.clone(),
        self.config.root_uri.clone(),
      );
      self.maybe_testing_server = Some(test_server);
    }

    lsp_log!("Server ready.");
  }

  async fn shutdown(&self) -> LspResult<()> {
    Ok(())
  }

  async fn did_open(
    &mut self,
    specifier: &ModuleSpecifier,
    params: DidOpenTextDocumentParams,
  ) -> Document {
    let mark = self.performance.mark("did_open", Some(&params));
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
    let mark = self.performance.mark("did_change", Some(&params));
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    match self.documents.change(
      &specifier,
      params.text_document.version,
      params.content_changes,
    ) {
      Ok(document) => {
        if document.is_diagnosable() {
          self
            .diagnostics_server
            .invalidate(&self.documents.dependents(&specifier));
          self.send_diagnostics_update();
          self.send_testing_update();
        }
      }
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

    if let Err(err) = self.documents.close(&specifier) {
      error!("{}", err);
    }
    if self.is_diagnosable(&specifier) {
      let mut specifiers = self.documents.dependents(&specifier);
      specifiers.push(specifier.clone());
      self.diagnostics_server.invalidate(&specifiers);
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    self.performance.measure(mark);
  }

  async fn did_change_configuration(
    &mut self,
    client_workspace_config: Option<Value>,
    params: DidChangeConfigurationParams,
  ) {
    let maybe_config =
      if self.config.client_capabilities.workspace_configuration {
        client_workspace_config
      } else {
        params
          .settings
          .as_object()
          .and_then(|settings| settings.get(SETTINGS_SECTION))
          .cloned()
      };

    if let Some(value) = maybe_config {
      if let Err(err) = self.config.set_workspace_settings(value) {
        error!("failed to update settings: {}", err);
      }
    }

    self.update_debug_flag();
    if let Err(err) = self.update_cache() {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    if let Err(err) = self.update_registries().await {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    if let Err(err) = self.update_config_file() {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    if let Err(err) = self.update_import_map().await {
      self.client.show_message(MessageType::WARNING, err).await;
    }
    if let Err(err) = self.update_tsconfig().await {
      self.client.show_message(MessageType::WARNING, err).await;
    }

    self.documents.update_config(
      self.maybe_import_map.clone(),
      self.maybe_config_file.as_ref(),
    );

    self.send_diagnostics_update();
    self.send_testing_update();
  }

  async fn did_change_watched_files(
    &mut self,
    params: DidChangeWatchedFilesParams,
  ) {
    let mark = self
      .performance
      .mark("did_change_watched_files", Some(&params));
    let mut touched = false;
    let changes: Vec<Url> = params
      .changes
      .iter()
      .map(|f| self.url_map.normalize_url(&f.uri))
      .collect();

    // if the current tsconfig has changed, we need to reload it
    if let Some(config_file) = &self.maybe_config_file {
      if changes.iter().any(|uri| config_file.specifier == *uri) {
        if let Err(err) = self.update_config_file() {
          self.client.show_message(MessageType::WARNING, err).await;
        }
        if let Err(err) = self.update_tsconfig().await {
          self.client.show_message(MessageType::WARNING, err).await;
        }
        touched = true;
      }
    }
    // if the current import map, or config file has changed, we need to reload
    // reload the import map
    if let Some(import_map_uri) = &self.maybe_import_map_uri {
      if changes.iter().any(|uri| import_map_uri == uri) || touched {
        if let Err(err) = self.update_import_map().await {
          self.client.show_message(MessageType::WARNING, err).await;
        }
        touched = true;
      }
    }
    if touched {
      self.documents.update_config(
        self.maybe_import_map.clone(),
        self.maybe_config_file.as_ref(),
      );
      self.diagnostics_server.invalidate_all();
      self.send_diagnostics_update();
      self.send_testing_update();
    }
    self.performance.measure(mark);
  }

  async fn did_change_workspace_folders(
    &mut self,
    params: DidChangeWorkspaceFoldersParams,
  ) {
    let mark = self
      .performance
      .mark("did_change_workspace_folders", Some(&params));
    let mut workspace_folders = params
      .event
      .added
      .into_iter()
      .map(|folder| (self.url_map.normalize_url(&folder.uri), folder))
      .collect::<Vec<(ModuleSpecifier, WorkspaceFolder)>>();
    if let Some(current_folders) = &self.config.workspace_folders {
      for (specifier, folder) in current_folders {
        if !params.event.removed.is_empty()
          && params.event.removed.iter().any(|f| f.uri == folder.uri)
        {
          continue;
        }
        workspace_folders.push((specifier.clone(), folder.clone()));
      }
    }

    self.config.workspace_folders = Some(workspace_folders);
    self.performance.measure(mark);
  }

  async fn document_symbol(
    &mut self,
    params: DocumentSymbolParams,
  ) -> LspResult<Option<DocumentSymbolResponse>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("document_symbol", Some(&params));
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
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    let document = match self.documents.get(&specifier) {
      Some(doc) if doc.is_open() => doc,
      _ => return Ok(None),
    };
    let mark = self.performance.mark("formatting", Some(&params));
    let file_path =
      fs_util::specifier_to_file_path(&specifier).map_err(|err| {
        error!("{}", err);
        LspError::invalid_request()
      })?;

    let fmt_options = if let Some(fmt_config) = self.maybe_fmt_config.as_ref() {
      // skip formatting any files ignored by the config file
      if !fmt_config.files.matches_specifier(&specifier) {
        return Ok(None);
      }
      fmt_config.options.clone()
    } else {
      Default::default()
    };

    let text_edits = tokio::task::spawn_blocking(move || {
      let format_result = match document.maybe_parsed_source() {
        Some(Ok(parsed_source)) => {
          format_parsed_source(&parsed_source, fmt_options)
        }
        Some(Err(err)) => Err(anyhow!("{}", err)),
        None => {
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
          // TODO(lucacasonato): handle error properly
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
      self.client.show_message(MessageType::WARNING, format!("Unable to format \"{}\". Likely due to unrecoverable syntax errors in the file.", specifier)).await;
      Ok(None)
    }
  }

  async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("hover", Some(&params));
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
          format!("{}\n\n---\n\n{}", value, docs)
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
      maybe_quick_info.map(|qi| qi.to_hover(line_index, self))
    };
    self.performance.measure(mark);
    Ok(hover)
  }

  async fn code_action(
    &self,
    params: CodeActionParams,
  ) -> LspResult<Option<CodeActionResponse>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("code_action", Some(&params));
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
          "deno-lint" => matches!(&d.code, Some(_)),
          "deno" => diagnostics::DenoDiagnostic::is_fixable(&d.code),
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
          Some("deno") => code_actions
            .add_deno_fix_action(&specifier, diagnostic)
            .map_err(|err| {
              error!("{}", err);
              LspError::internal_error()
            })?,
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
      .request(self.snapshot(), req)
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
    &self,
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
      let snapshot = self.snapshot();
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
        .request(snapshot.clone(), req)
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
          &self.documents,
        )
        .map_err(|err| {
          error!("Unable to remap changes: {}", err);
          LspError::internal_error()
        })?
      } else {
        combined_code_actions.changes
      };
      let mut code_action = params.clone();
      code_action.edit = ts_changes_to_edit(&changes, self).map_err(|err| {
        error!("Unable to convert changes to edits: {}", err);
        LspError::internal_error()
      })?;
      code_action
    } else if kind.as_str().starts_with(CodeActionKind::REFACTOR.as_str()) {
      let snapshot = self.snapshot();
      let mut code_action = params.clone();
      let action_data: refactor::RefactorCodeActionData = from_value(data)
        .map_err(|err| {
          error!("Unable to decode code action data: {}", err);
          LspError::invalid_params("The CodeAction's data is invalid.")
        })?;
      let asset_or_doc = self.get_asset_or_document(&action_data.specifier)?;
      let line_index = asset_or_doc.line_index();
      let start = line_index.offset_tsc(action_data.range.start)?;
      let length = line_index.offset_tsc(action_data.range.end)? - start;
      let req = tsc::RequestMethod::GetEditsForRefactor((
        action_data.specifier.clone(),
        tsc::TextSpan { start, length },
        action_data.refactor_name.clone(),
        action_data.action_name.clone(),
      ));
      let refactor_edit_info: tsc::RefactorEditInfo =
        self.ts_server.request(snapshot, req).await.map_err(|err| {
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
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
      || !(self.config.get_workspace_settings().enabled_code_lens()
        || self.config.specifier_code_lens_test(&specifier))
    {
      return Ok(None);
    }

    let mark = self.performance.mark("code_lens", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let navigation_tree =
      self.get_navigation_tree(&specifier).await.map_err(|err| {
        error!("Error getting code lenses for \"{}\": {}", specifier, err);
        LspError::internal_error()
      })?;
    let parsed_source = asset_or_doc.maybe_parsed_source().and_then(|r| r.ok());
    let line_index = asset_or_doc.line_index();
    let code_lenses = code_lens::collect(
      &specifier,
      parsed_source,
      &self.config,
      line_index,
      &navigation_tree,
    )
    .await
    .map_err(|err| {
      error!("Error getting code lenses for \"{}\": {}", specifier, err);
      LspError::internal_error()
    })?;
    self.performance.measure(mark);

    Ok(Some(code_lenses))
  }

  async fn code_lens_resolve(
    &self,
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
    &self,
    params: DocumentHighlightParams,
  ) -> LspResult<Option<Vec<DocumentHighlight>>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("document_highlight", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("references", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let req = tsc::RequestMethod::GetReferences((
      specifier.clone(),
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
        let reference_line_index = if reference_specifier == specifier {
          line_index.clone()
        } else {
          let asset_or_doc =
            self.get_asset_or_document(&reference_specifier)?;
          asset_or_doc.line_index()
        };
        results
          .push(reference.to_location(reference_line_index, &self.url_map));
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("goto_definition", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("goto_definition", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let req = tsc::RequestMethod::GetTypeDefinition {
      specifier,
      position: line_index
        .offset_tsc(params.text_document_position_params.position)?,
    };
    let maybe_definition_info: Option<Vec<tsc::DefinitionInfo>> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Unable to get type definition from TypeScript: {}", err);
        LspError::internal_error()
      })?;

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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("completion", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    // Import specifiers are something wholly internal to Deno, so for
    // completions, we will use internal logic and if there are completions
    // for imports, we will return those and not send a message into tsc, where
    // other completions come from.
    let response = if let Some(response) = completions::get_import_completions(
      &specifier,
      &params.text_document_position.position,
      &self.config.snapshot(),
      self.client.clone(),
      &self.module_registries,
      &self.documents,
      self.maybe_import_map.clone(),
    )
    .await
    {
      Some(response)
    } else {
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
      let req = tsc::RequestMethod::GetCompletions((
        specifier.clone(),
        position,
        tsc::GetCompletionsAtPositionOptions {
          user_preferences: tsc::UserPreferences {
            allow_incomplete_completions: Some(true),
            allow_text_changes_in_new_files: Some(specifier.scheme() == "file"),
            import_module_specifier_ending: Some(
              tsc::ImportModuleSpecifierEnding::Index,
            ),
            include_automatic_optional_chain_completions: Some(true),
            include_completions_for_import_statements: Some(
              self.config.get_workspace_settings().suggest.auto_imports,
            ),
            include_completions_for_module_exports: Some(true),
            include_completions_with_object_literal_method_snippets: Some(true),
            include_completions_with_class_member_snippets: Some(true),
            include_completions_with_insert_text: Some(true),
            include_completions_with_snippet_text: Some(true),
            jsx_attribute_completion_style: Some(
              tsc::JsxAttributeCompletionStyle::Auto,
            ),
            provide_prefix_and_suffix_text_for_rename: Some(true),
            provide_refactor_not_applicable_reason: Some(true),
            use_label_details_in_completion_entries: Some(true),
            ..Default::default()
          },
          trigger_character,
          trigger_kind,
        },
      ));
      let snapshot = self.snapshot();
      let maybe_completion_info: Option<tsc::CompletionInfo> =
        self.ts_server.request(snapshot, req).await.map_err(|err| {
          error!("Unable to get completion info from TypeScript: {}", err);
          LspError::internal_error()
        })?;

      if let Some(completions) = maybe_completion_info {
        let results = completions.as_completion_response(
          line_index,
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
    &self,
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
      if let Some(data) = &data.tsc {
        let specifier = data.specifier.clone();
        let req = tsc::RequestMethod::GetCompletionDetails(data.into());
        let maybe_completion_info: Option<tsc::CompletionEntryDetails> =
          self.ts_server.request(self.snapshot(), req).await.map_err(
            |err| {
              error!("Unable to get completion info from TypeScript: {}", err);
              LspError::internal_error()
            },
          )?;
        if let Some(completion_info) = maybe_completion_info {
          completion_info
            .as_completion_item(&params, data, &specifier, self)
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("goto_implementation", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

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
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("folding_range", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;

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
    let specifier = self.url_map.normalize_url(&params.item.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("incoming_calls", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let req = tsc::RequestMethod::ProvideCallHierarchyIncomingCalls((
      specifier.clone(),
      line_index.offset_tsc(params.item.selection_range.start)?,
    ));
    let incoming_calls: Vec<tsc::CallHierarchyIncomingCall> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let maybe_root_path_owned = self
      .config
      .root_uri
      .as_ref()
      .and_then(|uri| fs_util::specifier_to_file_path(uri).ok());
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
    let specifier = self.url_map.normalize_url(&params.item.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("outgoing_calls", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let req = tsc::RequestMethod::ProvideCallHierarchyOutgoingCalls((
      specifier.clone(),
      line_index.offset_tsc(params.item.selection_range.start)?,
    ));
    let outgoing_calls: Vec<tsc::CallHierarchyOutgoingCall> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

    let maybe_root_path_owned = self
      .config
      .root_uri
      .as_ref()
      .and_then(|uri| fs_util::specifier_to_file_path(uri).ok());
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark("prepare_call_hierarchy", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let req = tsc::RequestMethod::PrepareCallHierarchy((
      specifier.clone(),
      line_index.offset_tsc(params.text_document_position_params.position)?,
    ));
    let maybe_one_or_many: Option<tsc::OneOrMany<tsc::CallHierarchyItem>> =
      self
        .ts_server
        .request(self.snapshot(), req)
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
        .and_then(|uri| fs_util::specifier_to_file_path(uri).ok());
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
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("rename", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

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

  async fn selection_range(
    &mut self,
    params: SelectionRangeParams,
  ) -> LspResult<Option<Vec<SelectionRange>>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("selection_range", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

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

      selection_ranges
        .push(selection_range.to_selection_range(line_index.clone()));
    }
    self.performance.measure(mark);
    Ok(Some(selection_ranges))
  }

  async fn semantic_tokens_full(
    &mut self,
    params: SemanticTokensParams,
  ) -> LspResult<Option<SemanticTokensResult>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("semantic_tokens_full", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let req = tsc::RequestMethod::GetEncodedSemanticClassifications((
      specifier.clone(),
      tsc::TextSpan {
        start: 0,
        length: line_index.text_content_length_utf16().into(),
      },
    ));
    let semantic_classification: tsc::Classifications = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

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
    &mut self,
    params: SemanticTokensRangeParams,
  ) -> LspResult<Option<SemanticTokensRangeResult>> {
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self
      .performance
      .mark("semantic_tokens_range", Some(&params));
    let asset_or_doc = self.get_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();

    let start = line_index.offset_tsc(params.range.start)?;
    let length = line_index.offset_tsc(params.range.end)? - start;
    let req = tsc::RequestMethod::GetEncodedSemanticClassifications((
      specifier.clone(),
      tsc::TextSpan { start, length },
    ));
    let semantic_classification: tsc::Classifications = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })?;

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
    &mut self,
    params: SignatureHelpParams,
  ) -> LspResult<Option<SignatureHelp>> {
    let specifier = self
      .url_map
      .normalize_url(&params.text_document_position_params.text_document.uri);
    if !self.is_diagnosable(&specifier)
      || !self.config.specifier_enabled(&specifier)
    {
      return Ok(None);
    }

    let mark = self.performance.mark("signature_help", Some(&params));
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
      let signature_help = signature_help_items.into_signature_help(self);
      self.performance.measure(mark);
      Ok(Some(signature_help))
    } else {
      self.performance.measure(mark);
      Ok(None)
    }
  }

  async fn symbol(
    &mut self,
    params: WorkspaceSymbolParams,
  ) -> LspResult<Option<Vec<SymbolInformation>>> {
    let mark = self.performance.mark("symbol", Some(&params));

    let req = tsc::RequestMethod::GetNavigateToItems {
      search: params.query,
      // this matches vscode's hard coded result count
      max_result_count: Some(256),
      file: None,
    };

    let navigate_to_items: Vec<tsc::NavigateToItem> = self
      .ts_server
      .request(self.snapshot(), req)
      .await
      .map_err(|err| {
        error!("Failed request to tsserver: {}", err);
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

  fn send_diagnostics_update(&self) {
    let snapshot = (
      self.snapshot(),
      self.config.snapshot(),
      self.maybe_lint_config.clone(),
    );
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
  async fn initialize(
    &self,
    params: InitializeParams,
  ) -> LspResult<InitializeResult> {
    let mut language_server = self.0.lock().await;
    language_server.diagnostics_server.start();
    language_server.initialize(params).await
  }

  async fn initialized(&self, params: InitializedParams) {
    self.0.lock().await.initialized(params).await
  }

  async fn shutdown(&self) -> LspResult<()> {
    self.0.lock().await.shutdown().await
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    if params.text_document.uri.scheme() == "deno" {
      // we can ignore virtual text documents opening, as they don't need to
      // be tracked in memory, as they are static assets that won't change
      // already managed by the language service
      return;
    }

    let (client, uri, specifier, had_specifier_settings) = {
      let mut inner = self.0.lock().await;
      let client = inner.client.clone();
      let uri = params.text_document.uri.clone();
      let specifier = inner.url_map.normalize_url(&uri);
      let document = inner.did_open(&specifier, params).await;
      let has_specifier_settings =
        inner.config.has_specifier_settings(&specifier);
      if document.is_diagnosable() {
        let specifiers = inner.documents.dependents(&specifier);
        inner.diagnostics_server.invalidate(&specifiers);
        // don't send diagnostics yet if we don't have the specifier settings
        if has_specifier_settings {
          inner.send_diagnostics_update();
          inner.send_testing_update();
        }
      }
      (client, uri, specifier, has_specifier_settings)
    };

    // retrieve the specifier settings outside the lock if
    // they haven't been asked for yet on its own time
    if !had_specifier_settings {
      let language_server = self.clone();
      tokio::spawn(async move {
        let response = client.specifier_configuration(&uri).await;
        let mut inner = language_server.0.lock().await;
        match response {
          Ok(specifier_settings) => {
            // now update the config and send a diagnostics update
            inner.config.set_specifier_settings(
              specifier.clone(),
              uri,
              specifier_settings,
            );
          }
          Err(err) => {
            error!("{}", err);
          }
        }
        if inner
          .documents
          .get(&specifier)
          .map(|d| d.is_diagnosable())
          .unwrap_or(false)
        {
          inner.send_diagnostics_update();
          inner.send_testing_update();
        }
      });
    }
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
    let (has_workspace_capability, client, specifiers, mark) = {
      let inner = self.0.lock().await;
      let mark = inner
        .performance
        .mark("did_change_configuration", Some(&params));

      let specifiers =
        if inner.config.client_capabilities.workspace_configuration {
          Some(inner.config.get_specifiers_with_client_uris())
        } else {
          None
        };
      (
        inner.config.client_capabilities.workspace_configuration,
        inner.client.clone(),
        specifiers,
        mark,
      )
    };

    // start retrieving all the specifiers' settings outside the lock on its own
    // time
    if let Some(specifiers) = specifiers {
      let language_server = self.clone();
      let client = client.clone();
      tokio::spawn(async move {
        if let Ok(configs) = client
          .specifier_configurations(
            specifiers.iter().map(|s| s.client_uri.clone()).collect(),
          )
          .await
        {
          let mut inner = language_server.0.lock().await;
          for (i, value) in configs.into_iter().enumerate() {
            match value {
              Ok(specifier_settings) => {
                let entry = specifiers[i].clone();
                inner.config.set_specifier_settings(
                  entry.specifier,
                  entry.client_uri,
                  specifier_settings,
                );
              }
              Err(err) => {
                error!("{}", err);
              }
            }
          }
        }
        let mut ls = language_server.0.lock().await;
        if ls.config.update_enabled_paths(client).await {
          ls.diagnostics_server.invalidate_all();
          // this will be called in the inner did_change_configuration, but the
          // problem then becomes, if there was a change, the snapshot used
          // will be an out of date one, so we will call it again here if the
          // workspace folders have been touched
          ls.send_diagnostics_update();
        }
      });
    }

    // Get the configuration from the client outside of the lock
    // in order to prevent potential deadlocking scenarios where
    // the server holds a lock and calls into the client, which
    // calls into the server which deadlocks acquiring the lock.
    // There is a gap here between when the configuration is
    // received and acquiring the lock, but most likely there
    // won't be any racing here.
    let client_workspace_config = if has_workspace_capability {
      let config_response = client.workspace_configuration().await;
      match config_response {
        Ok(value) => Some(value),
        Err(err) => {
          error!("{}", err);
          None
        }
      }
    } else {
      None
    };

    // now update the inner state
    let mut inner = self.0.lock().await;
    inner
      .did_change_configuration(client_workspace_config, params)
      .await;
    inner.performance.measure(mark);
  }

  async fn did_change_watched_files(
    &self,
    params: DidChangeWatchedFilesParams,
  ) {
    self.0.lock().await.did_change_watched_files(params).await
  }

  async fn did_change_workspace_folders(
    &self,
    params: DidChangeWorkspaceFoldersParams,
  ) {
    let client = {
      let mut inner = self.0.lock().await;
      inner.did_change_workspace_folders(params).await;
      inner.client.clone()
    };
    let language_server = self.clone();
    tokio::spawn(async move {
      let mut ls = language_server.0.lock().await;
      if ls.config.update_enabled_paths(client).await {
        ls.diagnostics_server.invalidate_all();
        ls.send_diagnostics_update();
      }
    });
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

  async fn goto_type_definition(
    &self,
    params: GotoTypeDefinitionParams,
  ) -> LspResult<Option<GotoTypeDefinitionResponse>> {
    self.0.lock().await.goto_type_definition(params).await
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

  async fn symbol(
    &self,
    params: WorkspaceSymbolParams,
  ) -> LspResult<Option<Vec<SymbolInformation>>> {
    self.0.lock().await.symbol(params).await
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
    async fn create_graph_for_caching(
      cli_options: CliOptions,
      roots: Vec<(ModuleSpecifier, ModuleKind)>,
    ) -> Result<(), AnyError> {
      let ps = ProcState::from_options(Arc::new(cli_options)).await?;
      let graph = ps.create_graph(roots).await?;
      graph_valid(&graph, true, false)?;
      Ok(())
    }

    let referrer = self.url_map.normalize_url(&params.referrer.uri);
    if !self.is_diagnosable(&referrer) {
      return Ok(None);
    }

    let mark = self.performance.mark("cache", Some(&params));
    let roots = if !params.uris.is_empty() {
      params
        .uris
        .iter()
        .map(|t| {
          (
            self.url_map.normalize_url(&t.uri),
            deno_graph::ModuleKind::Esm,
          )
        })
        .collect()
    } else {
      vec![(referrer.clone(), deno_graph::ModuleKind::Esm)]
    };

    let mut cli_options = CliOptions::new(
      Flags {
        cache_path: self.maybe_cache_path.clone(),
        ca_stores: None,
        ca_file: None,
        unsafely_ignore_certificate_errors: None,
        ..Default::default()
      },
      self.maybe_config_file.clone(),
    );
    cli_options.set_import_map_specifier(self.maybe_import_map_uri.clone());

    // todo(dsherret): why is running this on a new thread necessary? It does
    // a compile error otherwise.
    let handle = tokio::task::spawn_blocking(|| {
      run_local(
        async move { create_graph_for_caching(cli_options, roots).await },
      )
    });
    if let Err(err) = handle.await.unwrap() {
      self.client.show_message(MessageType::WARNING, err).await;
    }

    // Now that we have dependencies loaded, we need to re-analyze all the files.
    // For that we're invalidating all the existing diagnostics and restarting
    // the language server for TypeScript (as it might hold to some stale
    // documents).
    self.diagnostics_server.invalidate_all();
    let _: bool = self
      .ts_server
      .request(self.snapshot(), tsc::RequestMethod::Restart)
      .await
      .unwrap();
    self.send_diagnostics_update();
    self.send_testing_update();

    self.performance.measure(mark);
    Ok(Some(json!(true)))
  }

  fn get_performance(&self) -> Value {
    let averages = self.performance.averages();
    json!({ "averages": averages })
  }

  fn get_tasks(&self) -> LspResult<Option<Value>> {
    Ok(
      self
        .maybe_config_file
        .as_ref()
        .and_then(|cf| cf.to_lsp_tasks()),
    )
  }

  async fn reload_import_registries(&mut self) -> LspResult<Option<Value>> {
    fs_util::remove_dir_all_if_exists(&self.module_registries_location)
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
    &mut self,
    params: lsp_custom::VirtualTextDocumentParams,
  ) -> LspResult<Option<String>> {
    let mark = self
      .performance
      .mark("virtual_text_document", Some(&params));
    let specifier = self.url_map.normalize_url(&params.text_document.uri);
    let contents = if specifier.as_str() == "deno:/status.md" {
      let mut contents = String::new();
      let mut documents_specifiers = self
        .documents
        .documents(false, false)
        .into_iter()
        .map(|d| d.specifier().clone())
        .collect::<Vec<_>>();
      documents_specifiers.sort();
      let measures = self.performance.to_vec();
      let workspace_settings = self.config.get_workspace_settings();

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
        .push_str("\n## Performance\n\n|Name|Duration|Count|\n|---|---|---|\n");
      let mut averages = self.performance.averages();
      averages.sort();
      for average in averages {
        writeln!(
          contents,
          "|{}|{}ms|{}|",
          average.name, average.average_duration, average.count
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
