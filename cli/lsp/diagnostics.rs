// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

use console_static_text::ansi::strip_ansi_codes;
use deno_ast::MediaType;
use deno_core::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_graph::Resolution;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_graph::source::ResolveError;
use deno_resolver::deno_json::CompilerOptionsKey;
use deno_resolver::graph::enhanced_resolution_error_message;
use deno_resolver::workspace::sloppy_imports_resolve;
use deno_runtime::deno_node;
use deno_runtime::tokio_util::create_basic_runtime;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use import_map::ImportMap;
use import_map::ImportMapErrorKind;
use log::error;
use lsp_types::Uri;
use node_resolver::NodeResolutionKind;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;
use tower_lsp::lsp_types as lsp;

use super::analysis;
use super::analysis::import_map_lookup;
use super::client::Client;
use super::documents::Document;
use super::documents::DocumentModule;
use super::documents::DocumentModules;
use super::language_server;
use super::language_server::StateSnapshot;
use super::performance::Performance;
use super::tsc::TsServer;
use crate::lsp::documents::OpenDocument;
use crate::lsp::language_server::OnceCellMap;
use crate::lsp::lint::LspLinter;
use crate::lsp::logging::lsp_warn;
use crate::lsp::urls::uri_to_url;
use crate::sys::CliSys;
use crate::tsc::DiagnosticCategory;
use crate::type_checker::ambient_modules_to_regex_string;
use crate::util::path::to_percent_decoded_str;

#[derive(Debug)]
pub struct DiagnosticsUpdateMessage {
  pub snapshot: Arc<StateSnapshot>,
}

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub enum DiagnosticSource {
  Deno,
  Lint,
  Ts,
}

impl DiagnosticSource {
  pub fn as_lsp_source(&self) -> &'static str {
    match self {
      Self::Deno => "deno",
      Self::Lint => "deno-lint",
      Self::Ts => "deno-ts",
    }
  }
}

fn should_send_diagnostic_batch_notifications() -> bool {
  deno_lib::args::has_flag_env_var(
    "DENO_INTERNAL_DIAGNOSTIC_BATCH_NOTIFICATIONS",
  )
}

#[derive(Debug)]
struct DocumentDiagnosticsState {
  version: i32,
  ts_diagnostics: Arc<Vec<lsp::Diagnostic>>,
  no_cache_diagnostics: Arc<Vec<lsp::Diagnostic>>,
}

#[derive(Debug, Default)]
pub struct DiagnosticsState {
  documents: RwLock<HashMap<Uri, DocumentDiagnosticsState>>,
}

impl DiagnosticsState {
  fn update(&self, uri: &Uri, version: i32, diagnostics: &[lsp::Diagnostic]) {
    let mut specifiers = self.documents.write();
    let current_version = specifiers.get(uri).map(|s| s.version);
    if let Some(current_version) = current_version
      && version < current_version
    {
      return;
    }
    let mut ts_diagnostics = vec![];
    let mut no_cache_diagnostics = vec![];
    for diagnostic in diagnostics {
      if diagnostic.source.as_deref()
        == Some(DiagnosticSource::Ts.as_lsp_source())
      {
        ts_diagnostics.push(diagnostic.clone());
      }
      if diagnostic.code
        == Some(lsp::NumberOrString::String("no-cache".to_string()))
        || diagnostic.code
          == Some(lsp::NumberOrString::String("not-installed-jsr".to_string()))
        || diagnostic.code
          == Some(lsp::NumberOrString::String("not-installed-npm".to_string()))
      {
        no_cache_diagnostics.push(diagnostic.clone());
      }
    }
    specifiers.insert(
      uri.clone(),
      DocumentDiagnosticsState {
        version,
        ts_diagnostics: Arc::new(ts_diagnostics),
        no_cache_diagnostics: Arc::new(no_cache_diagnostics),
      },
    );
  }

  pub fn clear(&self, uri: &Uri) {
    self.documents.write().remove(uri);
  }

  pub fn ts_diagnostics(&self, uri: &Uri) -> Arc<Vec<lsp::Diagnostic>> {
    self
      .documents
      .read()
      .get(uri)
      .map(|s| s.ts_diagnostics.clone())
      .unwrap_or_default()
  }

  pub fn has_no_cache_diagnostics(&self, uri: &Uri) -> bool {
    self
      .documents
      .read()
      .get(uri)
      .map(|s| !s.no_cache_diagnostics.is_empty())
      .unwrap_or(false)
  }

  pub fn no_cache_diagnostics(&self, uri: &Uri) -> Arc<Vec<lsp::Diagnostic>> {
    self
      .documents
      .read()
      .get(uri)
      .map(|s| s.no_cache_diagnostics.clone())
      .unwrap_or_default()
  }
}

pub struct DiagnosticsServer {
  channel: Option<mpsc::UnboundedSender<DiagnosticsUpdateMessage>>,
  client: Client,
  performance: Arc<Performance>,
  ts_server: Arc<TsServer>,
  pub state: Arc<DiagnosticsState>,
}

impl std::fmt::Debug for DiagnosticsServer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DiagnosticsServer")
      .field("channel", &self.channel)
      .field("client", &self.client)
      .field("performance", &self.performance)
      .field("ts_server", &self.ts_server)
      .field("state", &self.state)
      .finish()
  }
}

impl DiagnosticsServer {
  pub fn new(
    client: Client,
    performance: Arc<Performance>,
    ts_server: Arc<TsServer>,
  ) -> Self {
    DiagnosticsServer {
      channel: Default::default(),
      client,
      performance,
      ts_server,
      state: Default::default(),
    }
  }

  #[allow(unused_must_use)]
  pub fn start(&mut self) {
    let (tx, mut rx) = mpsc::unbounded_channel::<DiagnosticsUpdateMessage>();
    self.channel = Some(tx);
    let client = self.client.clone();
    let state = self.state.clone();
    let performance = self.performance.clone();
    let ts_server = self.ts_server.clone();
    let should_send_batch_notifications =
      should_send_diagnostic_batch_notifications();

    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        let ambient_modules_regex_cache = Arc::new(OnceCellMap::<
          (CompilerOptionsKey, Option<Arc<Uri>>),
          Option<regex::Regex>,
        >::new());
        let mut _previous_handle;
        while let Some(message) = rx.recv().await {
          let _mark = performance.measure_scope("lsp.update_diagnostics");
          let client = client.clone();
          let state = state.clone();
          let ts_server = ts_server.clone();
          let ambient_modules_regex_cache = ambient_modules_regex_cache.clone();
          let join_handle = tokio::task::spawn(async move {
            let token = CancellationToken::new();
            let _drop_guard = token.drop_guard_ref();
            let DiagnosticsUpdateMessage { snapshot } = message;
            if should_send_batch_notifications {
              client.send_diagnostic_batch_start_notification();
            }
            let open_docs =
              snapshot.document_modules.documents.open_docs().cloned();
            deno_core::futures::stream::iter(open_docs.map(|document| {
              let snapshot = snapshot.clone();
              let ts_server = ts_server.clone();
              let ambient_modules_regex_cache =
                ambient_modules_regex_cache.clone();
              let token = token.clone();
              AbortOnDropHandle::new(tokio::task::spawn(async move {
                let diagnostics = generate_document_diagnostics(
                  &document,
                  &snapshot,
                  &ts_server,
                  &ambient_modules_regex_cache,
                  &token,
                )
                .await
                .unwrap_or_else(|err| {
                  lsp_warn!(
                    "Couldn't generate diagnostics for \"{}\": {err:#}",
                    document.uri.as_str()
                  );
                  vec![]
                });
                (document, diagnostics)
              }))
            }))
            .buffered(
              std::thread::available_parallelism()
                .map(From::from)
                .unwrap_or(8),
            )
            .for_each(|result| async {
              let (document, diagnostics) = match result {
                Ok(r) => r,
                Err(err) => {
                  lsp_warn!("Diagnostics task join error: {err:#}");
                  return;
                }
              };
              publish_document_diagnostics(
                &document,
                diagnostics,
                &client,
                &state,
                &token,
              )
              .await
            })
            .await;
            if should_send_batch_notifications {
              client.send_diagnostic_batch_end_notification();
            }
          });
          _previous_handle = AbortOnDropHandle::new(join_handle);
        }
      })
    });
  }

  pub fn update(
    &self,
    message: DiagnosticsUpdateMessage,
  ) -> Result<(), AnyError> {
    // todo(dsherret): instead of queuing up messages, it would be better to
    // instead only store the latest message (ex. maybe using a
    // tokio::sync::watch::channel)
    if let Some(tx) = &self.channel {
      tx.send(message).map_err(|err| err.into())
    } else {
      Err(anyhow!("diagnostics server not started"))
    }
  }
}

impl<'a> From<&'a crate::tsc::DiagnosticCategory> for lsp::DiagnosticSeverity {
  fn from(category: &'a crate::tsc::DiagnosticCategory) -> Self {
    match category {
      crate::tsc::DiagnosticCategory::Error => lsp::DiagnosticSeverity::ERROR,
      crate::tsc::DiagnosticCategory::Warning => {
        lsp::DiagnosticSeverity::WARNING
      }
      crate::tsc::DiagnosticCategory::Suggestion => {
        lsp::DiagnosticSeverity::HINT
      }
      crate::tsc::DiagnosticCategory::Message => {
        lsp::DiagnosticSeverity::INFORMATION
      }
    }
  }
}

impl<'a> From<&'a crate::tsc::Position> for lsp::Position {
  fn from(pos: &'a crate::tsc::Position) -> Self {
    Self {
      line: pos.line as u32,
      character: pos.character as u32,
    }
  }
}

fn get_diagnostic_message(diagnostic: &crate::tsc::Diagnostic) -> String {
  if let Some(message) = diagnostic.message_text.clone() {
    message
  } else if let Some(message_chain) = diagnostic.message_chain.clone() {
    message_chain.format_message(0)
  } else {
    "[missing message]".to_string()
  }
}

fn to_lsp_range(
  start: &crate::tsc::Position,
  end: &crate::tsc::Position,
) -> lsp::Range {
  lsp::Range {
    start: start.into(),
    end: end.into(),
  }
}

fn to_lsp_related_information(
  related_information: &Option<Vec<crate::tsc::Diagnostic>>,
  module: &DocumentModule,
  document_modules: &DocumentModules,
) -> Option<Vec<lsp::DiagnosticRelatedInformation>> {
  related_information.as_ref().map(|related| {
    related
      .iter()
      .filter_map(|ri| {
        if let (Some(file_name), Some(start), Some(end)) =
          (&ri.file_name, &ri.start, &ri.end)
        {
          let uri = resolve_url(file_name)
            .ok()
            .and_then(|s| {
              document_modules.module_for_specifier(
                &s,
                module.scope.as_deref(),
                Some(&module.compiler_options_key),
              )
            })
            .map(|m| m.uri.as_ref().clone())
            .unwrap_or_else(|| Uri::from_str("unknown:").unwrap());
          Some(lsp::DiagnosticRelatedInformation {
            location: lsp::Location {
              uri,
              range: to_lsp_range(start, end),
            },
            message: get_diagnostic_message(ri),
          })
        } else {
          None
        }
      })
      .collect()
  })
}

pub fn ts_json_to_diagnostics(
  diagnostics: Vec<crate::tsc::Diagnostic>,
  module: &DocumentModule,
  document_modules: &DocumentModules,
) -> Vec<lsp::Diagnostic> {
  diagnostics
    .iter()
    .filter_map(|d| {
      if let (Some(start), Some(end)) = (&d.start, &d.end) {
        Some(lsp::Diagnostic {
          range: to_lsp_range(start, end),
          severity: Some((&d.category).into()),
          code: Some(lsp::NumberOrString::Number(d.code as i32)),
          code_description: None,
          source: Some(DiagnosticSource::Ts.as_lsp_source().to_string()),
          message: get_diagnostic_message(d),
          related_information: to_lsp_related_information(
            &d.related_information,
            module,
            document_modules,
          ),
          tags: match d.code {
            // These are codes that indicate the variable is unused.
            2695 | 6133 | 6138 | 6192 | 6196 | 6198 | 6199 | 6205 | 7027
            | 7028 => Some(vec![lsp::DiagnosticTag::UNNECESSARY]),
            // These are codes that indicated the variable is deprecated.
            2789 | 6385 | 6387 => Some(vec![lsp::DiagnosticTag::DEPRECATED]),
            _ => None,
          },
          data: None,
        })
      } else {
        None
      }
    })
    .collect()
}

fn generate_document_lint_diagnostics(
  module: &DocumentModule,
  linter: &LspLinter,
  token: CancellationToken,
) -> Vec<lsp::Diagnostic> {
  if !module.is_diagnosable()
    || !linter
      .lint_config
      .files
      .matches_specifier(&module.specifier)
  {
    return Vec::new();
  }
  match &module
    .open_data
    .as_ref()
    .and_then(|d| d.parsed_source.as_ref())
  {
    Some(Ok(parsed_source)) => {
      match analysis::get_lint_references(parsed_source, &linter.inner, token) {
        Ok(references) => references
          .into_iter()
          .map(|r| r.to_diagnostic())
          .collect::<Vec<_>>(),
        _ => Vec::new(),
      }
    }
    Some(Err(_)) => Vec::new(),
    None => {
      error!("Missing file contents for: {}", &module.specifier);
      Vec::new()
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDataSpecifier {
  pub specifier: ModuleSpecifier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataStrSpecifier {
  pub specifier: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataRedirect {
  pub redirect: ModuleSpecifier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataNoLocal {
  pub to: ModuleSpecifier,
  pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataImportMapRemap {
  pub from: String,
  pub to: String,
}

/// An enum which represents diagnostic errors which originate from Deno itself.
pub enum DenoDiagnostic {
  /// A `x-deno-warning` is associated with the specifier and should be displayed
  /// as a warning to the user.
  DenoWarn(String),
  /// An informational diagnostic that indicates an existing specifier can be
  /// remapped to an import map import specifier.
  ImportMapRemap { from: String, to: String },
  /// The import assertion type is incorrect.
  InvalidAttributeType(String),
  /// A module requires an attribute type to be a valid import.
  NoAttributeType,
  /// A remote module was not found in the cache.
  NoCache(ModuleSpecifier),
  /// A jsr package reference was not found in the cache.
  NotInstalledJsr(PackageReq, ModuleSpecifier),
  /// An npm package reference was not found in the cache.
  NotInstalledNpm(PackageReq, ModuleSpecifier),
  /// An npm package reference was not exported by its package.
  NoExportNpm(NpmPackageReqReference),
  /// A local module was not found on the local file system.
  NoLocal(ModuleSpecifier),
  /// An error occurred when resolving the specifier string.
  ResolutionError(deno_graph::ResolutionError),
  /// Unknown `node:` specifier.
  UnknownNodeSpecifier(ModuleSpecifier),
  /// Bare specifier is used for `node:` specifier
  BareNodeSpecifier(String),
}

impl DenoDiagnostic {
  fn code(&self) -> &str {
    match self {
      Self::DenoWarn(_) => "deno-warn",
      Self::ImportMapRemap { .. } => "import-map-remap",
      Self::InvalidAttributeType(_) => "invalid-attribute-type",
      Self::NoAttributeType => "no-attribute-type",
      Self::NoCache(_) => "no-cache",
      Self::NotInstalledJsr(_, _) => "not-installed-jsr",
      Self::NotInstalledNpm(_, _) => "not-installed-npm",
      Self::NoExportNpm(_) => "no-export-npm",
      Self::NoLocal(_) => "no-local",
      Self::ResolutionError(err) => {
        if deno_resolver::graph::get_resolution_error_bare_node_specifier(err)
          .is_some()
        {
          "import-node-prefix-missing"
        } else {
          match err {
            ResolutionError::InvalidDowngrade { .. } => "invalid-downgrade",
            ResolutionError::InvalidJsrHttpsTypesImport { .. } => {
              "invalid-jsr-https-types-import"
            }
            ResolutionError::InvalidLocalImport { .. } => {
              "invalid-local-import"
            }
            ResolutionError::InvalidSpecifier { error, .. } => match error {
              SpecifierError::ImportPrefixMissing { .. } => {
                "import-prefix-missing"
              }
              SpecifierError::InvalidUrl(_) => "invalid-url",
            },
            ResolutionError::ResolverError { .. } => "resolver-error",
          }
        }
      }
      Self::UnknownNodeSpecifier(_) => "resolver-error",
      Self::BareNodeSpecifier(_) => "import-node-prefix-missing",
    }
  }

  /// A "static" method which for a diagnostic that originated from the
  /// structure returns a code action which can resolve the diagnostic.
  pub fn get_code_action(
    uri: &Uri,
    specifier: &ModuleSpecifier,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<lsp::CodeAction, AnyError> {
    if let Some(lsp::NumberOrString::String(code)) = &diagnostic.code {
      let code_action = match code.as_str() {
        "import-map-remap" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let DiagnosticDataImportMapRemap { from, to } =
            serde_json::from_value(data)?;
          lsp::CodeAction {
            title: format!("Update \"{from}\" to \"{to}\" to use import map."),
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(lsp::WorkspaceEdit {
              changes: Some(HashMap::from([(
                uri.clone(),
                vec![lsp::TextEdit {
                  new_text: format!("\"{to}\""),
                  range: diagnostic.range,
                }],
              )])),
              ..Default::default()
            }),
            ..Default::default()
          }
        }
        "no-attribute-type" => lsp::CodeAction {
          title: "Insert import attribute.".to_string(),
          kind: Some(lsp::CodeActionKind::QUICKFIX),
          diagnostics: Some(vec![diagnostic.clone()]),
          edit: Some(lsp::WorkspaceEdit {
            changes: Some(HashMap::from([(
              uri.clone(),
              vec![lsp::TextEdit {
                new_text: " with { type: \"json\" }".to_string(),
                range: lsp::Range {
                  start: diagnostic.range.end,
                  end: diagnostic.range.end,
                },
              }],
            )])),
            ..Default::default()
          }),
          ..Default::default()
        },
        "no-cache" | "not-installed-jsr" | "not-installed-npm" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataSpecifier = serde_json::from_value(data)?;
          let title = if matches!(
            code.as_str(),
            "not-installed-jsr" | "not-installed-npm"
          ) {
            format!("Install \"{}\" and its dependencies.", data.specifier)
          } else {
            format!("Cache \"{}\" and its dependencies.", data.specifier)
          };
          lsp::CodeAction {
            title,
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            command: Some(lsp::Command {
              title: "".to_string(),
              command: "deno.cache".to_string(),
              arguments: Some(vec![json!([data.specifier]), json!(&specifier)]),
            }),
            ..Default::default()
          }
        }
        "no-local" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataNoLocal = serde_json::from_value(data)?;
          lsp::CodeAction {
            title: data.message,
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(lsp::WorkspaceEdit {
              changes: Some(HashMap::from([(
                uri.clone(),
                vec![lsp::TextEdit {
                  new_text: format!(
                    "\"{}\"",
                    relative_specifier(&data.to, specifier)
                  ),
                  range: diagnostic.range,
                }],
              )])),
              ..Default::default()
            }),
            ..Default::default()
          }
        }
        "redirect" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataRedirect = serde_json::from_value(data)?;
          lsp::CodeAction {
            title: "Update specifier to its redirected specifier.".to_string(),
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(lsp::WorkspaceEdit {
              changes: Some(HashMap::from([(
                uri.clone(),
                vec![lsp::TextEdit {
                  new_text: format!(
                    "\"{}\"",
                    specifier_text_for_redirected(&data.redirect, specifier)
                  ),
                  range: diagnostic.range,
                }],
              )])),
              ..Default::default()
            }),
            ..Default::default()
          }
        }
        "import-node-prefix-missing" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataStrSpecifier = serde_json::from_value(data)?;
          lsp::CodeAction {
            title: format!("Update specifier to node:{}", data.specifier),
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(lsp::WorkspaceEdit {
              changes: Some(HashMap::from([(
                uri.clone(),
                vec![lsp::TextEdit {
                  new_text: format!("\"node:{}\"", data.specifier),
                  range: diagnostic.range,
                }],
              )])),
              ..Default::default()
            }),
            ..Default::default()
          }
        }
        _ => {
          return Err(anyhow!(
            "Unsupported diagnostic code (\"{}\") provided.",
            code
          ));
        }
      };
      Ok(code_action)
    } else {
      Err(anyhow!("Unsupported diagnostic code provided."))
    }
  }

  /// Given a reference to the code from an LSP diagnostic, determine if the
  /// diagnostic is fixable or not
  pub fn is_fixable(diagnostic: &lsp_types::Diagnostic) -> bool {
    if let Some(lsp::NumberOrString::String(code)) = &diagnostic.code {
      match code.as_str() {
        "import-map-remap"
        | "no-cache"
        | "not-installed-jsr"
        | "not-installed-npm"
        | "no-attribute-type"
        | "redirect"
        | "import-node-prefix-missing" => true,
        "no-local" => diagnostic.data.is_some(),
        _ => false,
      }
    } else {
      false
    }
  }

  /// Convert to an lsp Diagnostic when the range the diagnostic applies to is
  /// provided.
  pub fn to_lsp_diagnostic(&self, range: &lsp::Range) -> lsp::Diagnostic {
    fn no_local_message(
      specifier: &ModuleSpecifier,
      suggestion_message: Option<String>,
    ) -> String {
      let mut message = format!(
        "Unable to load a local module: {}\n",
        to_percent_decoded_str(specifier.as_ref())
      );
      if let Some(suggestion_message) = suggestion_message {
        message.push_str(&suggestion_message);
        message.push('.');
      } else {
        message.push_str("Please check the file path.");
      }
      message
    }

    let (severity, message, data) = match self {
      Self::DenoWarn(message) => (lsp::DiagnosticSeverity::WARNING, message.to_string(), None),
      Self::ImportMapRemap { from, to } => (lsp::DiagnosticSeverity::HINT, format!("The import specifier can be remapped to \"{to}\" which will resolve it via the active import map."), Some(json!({ "from": from, "to": to }))),
      Self::InvalidAttributeType(assert_type) => (lsp::DiagnosticSeverity::ERROR, format!("The module is a JSON module and expected an attribute type of \"json\". Instead got \"{assert_type}\"."), None),
      Self::NoAttributeType => (lsp::DiagnosticSeverity::ERROR, "The module is a JSON module and not being imported with an import attribute. Consider adding `with { type: \"json\" }` to the import statement.".to_string(), None),
      Self::NoCache(specifier) => (lsp::DiagnosticSeverity::ERROR, format!("Uncached or missing remote URL: {specifier}"), Some(json!({ "specifier": specifier }))),
      Self::NotInstalledJsr(pkg_req, specifier) => (lsp::DiagnosticSeverity::ERROR, format!("JSR package \"{pkg_req}\" is not installed or doesn't exist."), Some(json!({ "specifier": specifier }))),
      Self::NotInstalledNpm(pkg_req, specifier) => (lsp::DiagnosticSeverity::ERROR, format!("npm package \"{pkg_req}\" is not installed or doesn't exist."), Some(json!({ "specifier": specifier }))),
      Self::NoExportNpm(pkg_ref) => (lsp::DiagnosticSeverity::ERROR, format!("NPM package \"{}\" does not define an export \"{}\".", pkg_ref.req(), pkg_ref.sub_path().unwrap_or(".")), None),
      Self::NoLocal(specifier) => {
        let sloppy_resolution = sloppy_imports_resolve(specifier, deno_resolver::workspace::ResolutionKind::Execution, CliSys::default());
        let data = sloppy_resolution.as_ref().map(|(resolved, sloppy_reason)| {
          json!({
            "specifier": specifier,
            "to": resolved,
            "message": sloppy_reason.quick_fix_message_for_specifier(resolved),
          })
        });
        (lsp::DiagnosticSeverity::ERROR, no_local_message(specifier, sloppy_resolution.as_ref().map(|(resolved, sloppy_reason)| sloppy_reason.suggestion_message_for_specifier(resolved))), data)
      },
      Self::ResolutionError(err) => {
        let message = strip_ansi_codes(&enhanced_resolution_error_message(err)).into_owned();
        (
        lsp::DiagnosticSeverity::ERROR,
        message,
        deno_resolver::graph::get_resolution_error_bare_node_specifier(err)
          .map(|specifier| json!({ "specifier": specifier }))
      )},
      Self::UnknownNodeSpecifier(specifier) => (lsp::DiagnosticSeverity::ERROR, format!("No such built-in module: node:{}", specifier.path()), None),
      Self::BareNodeSpecifier(specifier) => (lsp::DiagnosticSeverity::WARNING, format!("\"{0}\" is resolved to \"node:{0}\". If you want to use a built-in Node module, add a \"node:\" prefix.", specifier), Some(json!({ "specifier": specifier }))),
    };
    lsp::Diagnostic {
      range: *range,
      severity: Some(severity),
      code: Some(lsp::NumberOrString::String(self.code().to_string())),
      source: Some(DiagnosticSource::Deno.as_lsp_source().to_string()),
      message,
      data,
      ..Default::default()
    }
  }
}

fn specifier_text_for_redirected(redirect: &Url, referrer: &Url) -> String {
  if redirect.scheme() == "file" && referrer.scheme() == "file" {
    // use a relative specifier when it's going to a file url
    relative_specifier(redirect, referrer)
  } else {
    redirect.to_string()
  }
}

fn relative_specifier(specifier: &Url, referrer: &Url) -> String {
  match referrer.make_relative(specifier) {
    Some(relative) => {
      if relative.starts_with('.') {
        relative
      } else {
        format!("./{}", relative)
      }
    }
    None => specifier.to_string(),
  }
}

fn maybe_ambient_import_specifier(
  diagnostic: &DenoDiagnostic,
) -> Option<String> {
  match diagnostic {
    DenoDiagnostic::NoCache(url) | DenoDiagnostic::NoLocal(url) => {
      Some(url.to_string())
    }
    DenoDiagnostic::ResolutionError(err) => {
      maybe_ambient_specifier_resolution_err(err)
    }
    _ => None,
  }
}

fn maybe_ambient_specifier_resolution_err(
  err: &ResolutionError,
) -> Option<String> {
  match err {
    ResolutionError::InvalidDowngrade { .. }
    | ResolutionError::InvalidJsrHttpsTypesImport { .. }
    | ResolutionError::InvalidLocalImport { .. } => None,
    ResolutionError::InvalidSpecifier { error, .. } => match error {
      SpecifierError::InvalidUrl(..) => None,
      SpecifierError::ImportPrefixMissing { specifier, .. } => {
        Some(specifier.to_string())
      }
    },
    ResolutionError::ResolverError { error, .. } => match &**error {
      ResolveError::Specifier(specifier_error) => match specifier_error {
        SpecifierError::InvalidUrl(..) => None,
        SpecifierError::ImportPrefixMissing { specifier, .. } => {
          Some(specifier.to_string())
        }
      },
      ResolveError::ImportMap(import_map_error) => {
        match import_map_error.as_kind() {
          ImportMapErrorKind::UnmappedBareSpecifier(spec, _) => {
            Some(spec.clone())
          }
          ImportMapErrorKind::JsonParse(_)
          | ImportMapErrorKind::ImportMapNotObject
          | ImportMapErrorKind::ImportsFieldNotObject
          | ImportMapErrorKind::ScopesFieldNotObject
          | ImportMapErrorKind::ScopePrefixNotObject(_)
          | ImportMapErrorKind::BlockedByNullEntry(_)
          | ImportMapErrorKind::SpecifierResolutionFailure { .. }
          | ImportMapErrorKind::SpecifierBacktracksAbovePrefix { .. } => None,
        }
      }
      ResolveError::Other(..) => None,
    },
  }
}

#[allow(clippy::too_many_arguments)]
fn diagnose_resolution(
  snapshot: &language_server::StateSnapshot,
  dependency_key: &str,
  resolution: &Resolution,
  is_dynamic: bool,
  maybe_assert_type: Option<&str>,
  referrer_module: &DocumentModule,
  import_map: Option<&ImportMap>,
) -> (Vec<DenoDiagnostic>, Vec<DenoDiagnostic>) {
  let mut diagnostics = vec![];
  let mut deferred_diagnostics = vec![];
  match resolution {
    Resolution::Ok(resolved) => {
      let specifier = &resolved.specifier;
      let scoped_resolver = snapshot
        .resolver
        .get_scoped_resolver(referrer_module.scope.as_deref());
      let managed_npm_resolver =
        scoped_resolver.as_maybe_managed_npm_resolver();
      for (_, headers) in scoped_resolver.redirect_chain_headers(specifier) {
        if let Some(message) = headers.get("x-deno-warning") {
          diagnostics.push(DenoDiagnostic::DenoWarn(message.clone()));
        }
      }
      match snapshot.document_modules.module_for_specifier(
        specifier,
        referrer_module.scope.as_deref(),
        Some(&referrer_module.compiler_options_key),
      ) {
        Some(module) => {
          if let Some(headers) = &module.headers
            && let Some(message) = headers.get("x-deno-warning")
          {
            diagnostics.push(DenoDiagnostic::DenoWarn(message.clone()));
          }
          if module.media_type == MediaType::Json {
            match maybe_assert_type {
              // The module has the correct assertion type, no diagnostic
              Some("json" | "text" | "bytes") => (),
              // The dynamic import statement is missing an attribute type, which
              // we might not be able to statically detect, therefore we will
              // not provide a potentially incorrect diagnostic.
              None if is_dynamic => (),
              // The module has an incorrect assertion type, diagnostic
              Some(assert_type) => diagnostics.push(
                DenoDiagnostic::InvalidAttributeType(assert_type.to_string()),
              ),
              // The module is missing an attribute type, diagnostic
              None => diagnostics.push(DenoDiagnostic::NoAttributeType),
            }
          }
        }
        _ => {
          match JsrPackageReqReference::from_specifier(specifier) {
            Ok(pkg_ref) => {
              let req = pkg_ref.into_inner().req;
              diagnostics
                .push(DenoDiagnostic::NotInstalledJsr(req, specifier.clone()));
            }
            _ => {
              match NpmPackageReqReference::from_specifier(specifier) {
                Ok(pkg_ref) => {
                  if let Some(npm_resolver) = managed_npm_resolver {
                    // show diagnostics for npm package references that aren't cached
                    let req = pkg_ref.req();
                    if !npm_resolver.is_pkg_req_folder_cached(req) {
                      diagnostics.push(DenoDiagnostic::NotInstalledNpm(
                        req.clone(),
                        specifier.clone(),
                      ));
                    } else {
                      let resolution_kinds = [
                        NodeResolutionKind::Types,
                        NodeResolutionKind::Execution,
                      ];
                      if resolution_kinds.into_iter().all(|k| {
                        scoped_resolver
                          .npm_to_file_url(
                            &pkg_ref,
                            &referrer_module.specifier,
                            k,
                            referrer_module.resolution_mode,
                          )
                          .is_none()
                      }) {
                        diagnostics
                          .push(DenoDiagnostic::NoExportNpm(pkg_ref.clone()));
                      }
                    }
                  }
                }
                _ => {
                  if let Some(module_name) =
                    specifier.as_str().strip_prefix("node:")
                  {
                    if !deno_node::is_builtin_node_module(module_name) {
                      diagnostics.push(DenoDiagnostic::UnknownNodeSpecifier(
                        specifier.clone(),
                      ));
                    } else if module_name == dependency_key {
                      let mut is_mapped = false;
                      if let Some(import_map) = import_map
                        && let Resolution::Ok(resolved) = &resolution
                        && import_map
                          .resolve(module_name, &resolved.specifier)
                          .is_ok()
                      {
                        is_mapped = true;
                      }
                      // show diagnostics for bare node specifiers that aren't mapped by import map
                      if !is_mapped {
                        diagnostics.push(DenoDiagnostic::BareNodeSpecifier(
                          module_name.to_string(),
                        ));
                      }
                    }
                  } else {
                    // When the document is not available, it means that it cannot be found
                    // in the cache or locally on the disk, so we want to issue a diagnostic
                    // about that.
                    // these may be invalid, however, if this is an ambient module with
                    // no real source (as in the case of a virtual module).
                    let deno_diagnostic = match specifier.scheme() {
                      "file" => DenoDiagnostic::NoLocal(specifier.clone()),
                      _ => DenoDiagnostic::NoCache(specifier.clone()),
                    };
                    deferred_diagnostics.push(deno_diagnostic);
                  }
                }
              }
            }
          }
        }
      }
    }
    // The specifier resolution resulted in an error, so we want to issue a
    // diagnostic for that.
    Resolution::Err(err) => {
      if maybe_ambient_specifier_resolution_err(err).is_none() {
        diagnostics.push(DenoDiagnostic::ResolutionError(*err.clone()))
      } else {
        deferred_diagnostics
          .push(DenoDiagnostic::ResolutionError(*err.clone()));
      }
    }
    _ => (),
  }
  (diagnostics, deferred_diagnostics)
}

/// Generate diagnostics related to a dependency. The dependency is analyzed to
/// determine if it can be remapped to the active import map as well as surface
/// any diagnostics related to the resolved code or type dependency.
fn diagnose_dependency(
  diagnostics: &mut Vec<lsp::Diagnostic>,
  deferred_diagnostics: &mut Vec<(String, lsp::Diagnostic)>,
  snapshot: &language_server::StateSnapshot,
  referrer_module: &DocumentModule,
  dependency_key: &str,
  dependency: &deno_graph::Dependency,
) {
  if snapshot
    .resolver
    .in_node_modules(&referrer_module.specifier)
  {
    return; // ignore, surface typescript errors instead
  }

  if referrer_module.media_type.is_declaration() {
    let compiler_options_data = snapshot
      .compiler_options_resolver
      .for_key(&referrer_module.compiler_options_key);
    if compiler_options_data.is_none() {
      lsp_warn!(
        "Key was not in sync with resolver while checking `skipLibCheck`. This should be impossible."
      );
      #[cfg(debug_assertions)]
      unreachable!();
    }
    if compiler_options_data.is_some_and(|d| d.skip_lib_check) {
      return;
    }
  }

  let import_map = snapshot
    .resolver
    .get_scoped_resolver(referrer_module.scope.as_deref())
    .as_workspace_resolver()
    .maybe_import_map();
  if let Some(import_map) = import_map {
    let resolved = dependency
      .maybe_code
      .ok()
      .or_else(|| dependency.maybe_type.ok());
    if let Some(resolved) = resolved
      && let Some(to) = import_map_lookup(
        import_map,
        &resolved.specifier,
        &referrer_module.specifier,
      )
      && dependency_key != to
    {
      diagnostics.push(
        DenoDiagnostic::ImportMapRemap {
          from: dependency_key.to_string(),
          to,
        }
        .to_lsp_diagnostic(&language_server::to_lsp_range(&resolved.range)),
      );
    }
  }

  let import_ranges: Vec<_> = dependency
    .imports
    .iter()
    .map(|i| language_server::to_lsp_range(&i.specifier_range))
    .collect();
  // TODO(nayeemrmn): This is a crude way of detecting `@ts-types` which has
  // a different specifier and therefore needs a separate call to
  // `diagnose_resolution()`. It would be much cleaner if that were modelled as
  // a separate dependency: https://github.com/denoland/deno_graph/issues/247.
  let is_types_deno_types = !dependency.maybe_type.is_none()
    && !dependency.imports.iter().any(|i| {
      dependency
        .maybe_type
        .includes(i.specifier_range.range.start)
        .is_some()
    });
  let resolution = if dependency.maybe_code.is_none()
  // If not @ts-types, diagnose the types if the code errored because
  // it's likely resolving into the node_modules folder, which might be
  // erroring correctly due to resolution only being for bundlers. Let this
  // fail at runtime if necessary, but don't bother erroring in the editor
  || !is_types_deno_types && matches!(dependency.maybe_type, Resolution::Ok(_))
    && matches!(dependency.maybe_code, Resolution::Err(_))
  {
    &dependency.maybe_type
  } else {
    &dependency.maybe_code
  };
  let (resolution_diagnostics, deferred) = diagnose_resolution(
    snapshot,
    dependency_key,
    resolution,
    dependency.is_dynamic,
    dependency.maybe_attribute_type.as_deref(),
    referrer_module,
    import_map,
  );
  diagnostics.extend(resolution_diagnostics.iter().flat_map(|diag| {
    import_ranges
      .iter()
      .map(|range| diag.to_lsp_diagnostic(range))
  }));
  deferred_diagnostics.extend(
    deferred
      .iter()
      .filter_map(|diag| {
        maybe_ambient_import_specifier(diag).map(|spec| {
          import_ranges
            .iter()
            .map(move |range| (spec.clone(), diag.to_lsp_diagnostic(range)))
        })
      })
      .flatten(),
  );

  if is_types_deno_types {
    let range = match &dependency.maybe_type {
      Resolution::Ok(resolved) => {
        language_server::to_lsp_range(&resolved.range)
      }
      Resolution::Err(error) => language_server::to_lsp_range(error.range()),
      Resolution::None => unreachable!(),
    };
    let (resolution_diagnostics, deferred) = diagnose_resolution(
      snapshot,
      dependency_key,
      &dependency.maybe_type,
      dependency.is_dynamic,
      dependency.maybe_attribute_type.as_deref(),
      referrer_module,
      import_map,
    );
    diagnostics.extend(
      resolution_diagnostics
        .iter()
        .map(|diag| diag.to_lsp_diagnostic(&range)),
    );
    deferred_diagnostics.extend(Box::new(deferred.iter().filter_map(|diag| {
      maybe_ambient_import_specifier(diag)
        .map(|spec| (spec, diag.to_lsp_diagnostic(&range)))
    })));
  }
}

async fn publish_document_diagnostics(
  document: &Arc<OpenDocument>,
  diagnostics: Vec<lsp::Diagnostic>,
  client: &Client,
  state: &DiagnosticsState,
  _token: &CancellationToken,
) {
  state.update(&document.uri, document.version, &diagnostics);
  client
    .publish_diagnostics(
      document.uri.as_ref().clone(),
      diagnostics,
      Some(document.version),
    )
    .await;
}

async fn generate_document_diagnostics(
  document: &Arc<OpenDocument>,
  snapshot: &Arc<StateSnapshot>,
  ts_server: &TsServer,
  ambient_modules_regex_cache: &OnceCellMap<
    (CompilerOptionsKey, Option<Arc<Uri>>),
    Option<regex::Regex>,
  >,
  token: &CancellationToken,
) -> Result<Vec<lsp::Diagnostic>, AnyError> {
  if !document.is_diagnosable() {
    return Ok(Vec::new());
  }
  let document_ = Document::Open(document.clone());
  let module = match snapshot.document_modules.primary_module(&document_) {
    Some(module) => module,
    None => {
      let url = uri_to_url(document_.uri());
      if url.scheme() == "file"
        && !snapshot.resolver.in_node_modules(&url)
        && !snapshot.cache.in_cache_directory(&url)
      {
        return Ok(Vec::new());
      }
      // If this document represents a non-local module, the module may not be
      // retrievable until its referrer is known through some other request.
      // Wait and try one more time.
      tokio::time::sleep(std::time::Duration::from_millis(200)).await;
      let Some(module) = snapshot.document_modules.primary_module(&document_)
      else {
        return Ok(Vec::new());
      };
      module
    }
  };
  if !snapshot.config.specifier_enabled(&module.specifier) {
    return Ok(Vec::new());
  }
  generate_module_diagnostics(
    &module,
    snapshot,
    ts_server,
    ambient_modules_regex_cache,
    token,
  )
  .await
}

pub async fn generate_module_diagnostics(
  module: &Arc<DocumentModule>,
  snapshot: &Arc<StateSnapshot>,
  ts_server: &TsServer,
  ambient_modules_regex_cache: &OnceCellMap<
    (CompilerOptionsKey, Option<Arc<Uri>>),
    Option<regex::Regex>,
  >,
  token: &CancellationToken,
) -> Result<Vec<lsp::Diagnostic>, AnyError> {
  let deps_handle = tokio::task::spawn_blocking({
    let snapshot = snapshot.clone();
    let module = module.clone();
    let token = token.clone();
    move || {
      let mut diagnostics = Vec::new();
      let mut deferred = Vec::new();
      for (dependency_key, dependency) in module.dependencies.iter() {
        if token.is_cancelled() {
          return (Vec::new(), Vec::new());
        }
        diagnose_dependency(
          &mut diagnostics,
          &mut deferred,
          &snapshot,
          &module,
          dependency_key,
          dependency,
        );
      }
      (diagnostics, deferred)
    }
  });

  let lint_handle = tokio::task::spawn_blocking({
    let snapshot = snapshot.clone();
    let module = module.clone();
    let token = token.clone();
    move || {
      // TODO(nayeemrmn): Support linting notebooks cells. Will require
      // stitching cells from the same notebook into one module, linting it
      // and then splitting/relocating the diagnostics to each cell.
      if token.is_cancelled()
        || module.notebook_uri.is_some()
        || module.specifier.scheme() != "file"
        || snapshot.resolver.in_node_modules(&module.specifier)
      {
        return Vec::new();
      }
      let settings = snapshot
        .config
        .workspace_settings_for_specifier(&module.specifier);
      if !settings.lint {
        return Vec::new();
      }
      let linter = snapshot.linter_resolver.for_module(&module);
      generate_document_lint_diagnostics(&module, &linter, token)
    }
  });

  let mut ts_diagnostics = ts_server
    .get_diagnostics(snapshot.clone(), module, token)
    .await?;
  let suggestion_actions_settings = snapshot
    .config
    .language_settings_for_specifier(&module.specifier)
    .map(|s| s.suggestion_actions.clone())
    .unwrap_or_default();
  if !suggestion_actions_settings.enabled {
    ts_diagnostics.retain(|d| {
      d.category != DiagnosticCategory::Suggestion
        // Still show deprecated and unused diagnostics.
        // https://github.com/microsoft/vscode/blob/ce50bd4876af457f64d83cfd956bc916535285f4/extensions/typescript-language-features/src/languageFeatures/diagnostics.ts#L113-L114
        || d.reports_deprecated == Some(true)
        || d.reports_unnecessary == Some(true)
    });
  }
  let mut diagnostics =
    ts_json_to_diagnostics(ts_diagnostics, module, &snapshot.document_modules);

  let (deps_diagnostics, deferred_deps_diagnostics) = deps_handle
    .await
    .inspect_err(|err| {
      lsp_warn!("Deps diagnostics task join error: {err:#}");
    })
    .unwrap_or_default();
  diagnostics.extend(deps_diagnostics);
  let ambient_modules_regex_cell = ambient_modules_regex_cache
    .entry((
      module.compiler_options_key.clone(),
      module.notebook_uri.clone(),
    ))
    .or_default()
    .clone();
  let ambient_modules_regex = ambient_modules_regex_cell
    .get_or_init(async || {
      ts_server
        .get_ambient_modules(
          snapshot.clone(),
          &module.compiler_options_key,
          module.notebook_uri.as_ref(),
          token,
        )
        .await
        .inspect_err(|err| {
          if !token.is_cancelled() {
            lsp_warn!("Unable to get ambient modules: {:#}", err);
          }
        })
        .ok()
        .filter(|a| !a.is_empty())
        .and_then(|ambient_modules| {
          let regex_string = ambient_modules_to_regex_string(&ambient_modules);
          regex::Regex::new(&regex_string).inspect_err(|err| {
            lsp_warn!("Failed to compile ambient modules pattern: {err:#} (pattern is {regex_string:?})");
          }).ok()
        })
    }).await;
  if let Some(ambient_modules_regex) = ambient_modules_regex {
    diagnostics.extend(deferred_deps_diagnostics.into_iter().filter_map(
      |(import_url, diag)| {
        if ambient_modules_regex.is_match(import_url.as_str()) {
          return None;
        }
        Some(diag)
      },
    ));
  } else {
    diagnostics.extend(deferred_deps_diagnostics.into_iter().map(|(_, d)| d));
  }

  let lint_diagnostics = lint_handle
    .await
    .inspect_err(|err| {
      lsp_warn!("Lint task join error: {err:#}");
    })
    .unwrap_or_default();
  diagnostics.extend(lint_diagnostics);

  Ok(diagnostics)
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;
  use std::sync::Arc;

  use deno_config::deno_json::ConfigFile;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  use super::*;
  use crate::lsp::cache::LspCache;
  use crate::lsp::compiler_options::LspCompilerOptionsResolver;
  use crate::lsp::config::Config;
  use crate::lsp::config::WorkspaceSettings;
  use crate::lsp::documents::DocumentModules;
  use crate::lsp::documents::LanguageId;
  use crate::lsp::language_server::StateSnapshot;
  use crate::lsp::lint::LspLinterResolver;
  use crate::lsp::resolver::LspResolver;
  use crate::lsp::urls::url_to_uri;

  async fn setup(
    sources: &[(&str, &str, i32, LanguageId)],
    maybe_import_map: Option<(&str, &str)>,
  ) -> (TempDir, StateSnapshot) {
    let temp_dir = TempDir::new();
    let root_url = temp_dir.url();
    let cache = LspCache::new(Some(root_url.join(".deno_dir").unwrap()));
    let mut config = Config::new_with_roots([root_url.clone()]);
    let enabled_settings =
      serde_json::from_str::<WorkspaceSettings>(r#"{ "enable": true }"#)
        .unwrap();
    config.set_workspace_settings(
      enabled_settings.clone(),
      vec![(Arc::new(root_url.clone()), enabled_settings)],
    );
    if let Some((relative_path, json_string)) = maybe_import_map {
      let base_url = root_url.join(relative_path).unwrap();
      let config_file = ConfigFile::new(json_string, base_url).unwrap();
      config.tree.inject_config_file(config_file).await;
    }
    let resolver =
      Arc::new(LspResolver::from_config(&config, &cache, None).await);
    let compiler_options_resolver =
      Arc::new(LspCompilerOptionsResolver::new(&config, &resolver));
    resolver.set_compiler_options_resolver(&compiler_options_resolver.inner);
    let linter_resolver = Arc::new(LspLinterResolver::new(
      &config,
      &compiler_options_resolver,
      &resolver,
    ));
    let mut document_modules = DocumentModules::default();
    document_modules.update_config(
      &config,
      &compiler_options_resolver,
      &resolver,
      &cache,
      &Default::default(),
    );
    for (relative_path, source, version, language_id) in sources {
      let specifier = root_url.join(relative_path).unwrap();
      document_modules.open_document(
        url_to_uri(&specifier).unwrap(),
        *version,
        *language_id,
        (*source).into(),
        None,
      );
    }
    (
      temp_dir,
      StateSnapshot {
        project_version: 0,
        document_modules,
        config: Arc::new(config),
        compiler_options_resolver,
        linter_resolver,
        resolver,
        cache: Arc::new(cache),
      },
    )
  }

  fn generate_all_deno_diagnostics(
    snapshot: &StateSnapshot,
  ) -> Vec<(Uri, Vec<lsp::Diagnostic>)> {
    snapshot
      .document_modules
      .documents
      .open_docs()
      .filter_map(|doc| {
        if !doc.is_diagnosable() {
          return None;
        }
        let module = snapshot
          .document_modules
          .primary_module(&Document::Open(doc.clone()))?;
        if !snapshot.config.specifier_enabled(&module.specifier) {
          return None;
        }
        let mut diagnostics = Vec::new();
        let mut deferred = Vec::new();
        for (dependency_key, dependency) in module.dependencies.iter() {
          diagnose_dependency(
            &mut diagnostics,
            &mut deferred,
            snapshot,
            &module,
            dependency_key,
            dependency,
          );
        }
        diagnostics.extend(deferred.into_iter().map(|(_, d)| d));
        Some((doc.uri.as_ref().clone(), diagnostics))
      })
      .collect::<Vec<_>>()
  }

  #[tokio::test]
  async fn test_deno_diagnostics_with_import_map() {
    let (temp_dir, snapshot) = setup(
      &[
        (
          "std/assert/mod.ts",
          "export function assert() {}",
          1,
          LanguageId::TypeScript,
        ),
        (
          "a/file.ts",
          "import { assert } from \"../std/assert/mod.ts\";\n\nassert();\nexport function a() {}",
          1,
          LanguageId::TypeScript,
        ),
        (
          "a/file2.ts",
          "import { a } from './file.ts';\nconsole.log(a);\n",
          1,
          LanguageId::TypeScript,
        ),
      ],
      Some((
        "a/deno.json",
        r#"{
        "imports": {
          "/~/std/": "../std/",
          "$@/": "./",
          "$a/": "../a/"
        }
      }"#,
      )),
    )
    .await;
    let actual = generate_all_deno_diagnostics(&snapshot);
    assert_eq!(
      json!(actual),
      json!([
        [
          url_to_uri(&temp_dir.url().join("std/assert/mod.ts").unwrap()).unwrap(),
          [],
        ],
        [
          url_to_uri(&temp_dir.url().join("a/file.ts").unwrap()).unwrap(),
          [
            {
              "range": {
                "start": { "line": 0, "character": 23 },
                "end": { "line": 0, "character": 45 },
              },
              "severity": 4,
              "code": "import-map-remap",
              "source": "deno",
              "message": "The import specifier can be remapped to \"/~/std/assert/mod.ts\" which will resolve it via the active import map.",
              "data": {
                "from": "../std/assert/mod.ts",
                "to": "/~/std/assert/mod.ts",
              },
            },
          ],
        ],
        [
          url_to_uri(&temp_dir.url().join("a/file2.ts").unwrap()).unwrap(),
          [],
        ],
      ]),
    );
  }

  #[test]
  fn test_get_code_action_import_map_remap() {
    let uri = Uri::from_str("file:///a/file.ts").unwrap();
    let specifier = ModuleSpecifier::parse("file:///a/file.ts").unwrap();
    let result = DenoDiagnostic::get_code_action(&uri, &specifier, &lsp::Diagnostic {
      range: lsp::Range {
        start: lsp::Position { line: 0, character: 23 },
        end: lsp::Position { line: 0, character: 50 },
      },
      severity: Some(lsp::DiagnosticSeverity::HINT),
      code: Some(lsp::NumberOrString::String("import-map-remap".to_string())),
      source: Some("deno".to_string()),
      message: "The import specifier can be remapped to \"/~/std/assert/mod.ts\" which will resolve it via the active import map.".to_string(),
      data: Some(json!({
        "from": "../std/assert/mod.ts",
        "to": "/~/std/assert/mod.ts"
      })),
      ..Default::default()
    });
    assert!(result.is_ok());
    let actual = result.unwrap();
    assert_eq!(
      json!(actual),
      json!({
        "title": "Update \"../std/assert/mod.ts\" to \"/~/std/assert/mod.ts\" to use import map.",
        "kind": "quickfix",
        "diagnostics": [
          {
            "range": {
              "start": {
                "line": 0,
                "character": 23
              },
              "end": {
                "line": 0,
                "character": 50
              }
            },
            "severity": 4,
            "code": "import-map-remap",
            "source": "deno",
            "message": "The import specifier can be remapped to \"/~/std/assert/mod.ts\" which will resolve it via the active import map.",
            "data": {
              "from": "../std/assert/mod.ts",
              "to": "/~/std/assert/mod.ts"
            }
          }
        ],
        "edit": {
          "changes": {
            "file:///a/file.ts": [
              {
                "range": {
                  "start": {
                    "line": 0,
                    "character": 23
                  },
                  "end": {
                    "line": 0,
                    "character": 50
                  }
                },
                "newText": "\"/~/std/assert/mod.ts\""
              }
            ]
          }
        }
      })
    );
  }

  #[tokio::test]
  async fn duplicate_diagnostics_for_duplicate_imports() {
    let (temp_dir, snapshot) = setup(
      &[(
        "a.ts",
        r#"
        // @ts-types="bad.d.ts"
        import "bad.js";
        import "bad.js";
        "#,
        1,
        LanguageId::TypeScript,
      )],
      None,
    )
    .await;
    let actual = generate_all_deno_diagnostics(&snapshot);
    assert_eq!(
      json!(actual),
      json!([
        [
          url_to_uri(&temp_dir.url().join("a.ts").unwrap()).unwrap(),
          [
            {
              "range": {
                "start": { "line": 2, "character": 15 },
                "end": { "line": 2, "character": 23 },
              },
              "severity": 1,
              "code": "import-prefix-missing",
              "source": "deno",
              "message": "Import \"bad.js\" not a dependency",
            },
            {
              "range": {
                "start": { "line": 3, "character": 15 },
                "end": { "line": 3, "character": 23 },
              },
              "severity": 1,
              "code": "import-prefix-missing",
              "source": "deno",
              "message": "Import \"bad.js\" not a dependency",
            },
            {
              "range": {
                "start": { "line": 1, "character": 21 },
                "end": { "line": 1, "character": 31 },
              },
              "severity": 1,
              "code": "import-prefix-missing",
              "source": "deno",
              "message": "Import \"bad.d.ts\" not a dependency",
            },
          ],
        ],
      ]),
    );
  }

  #[tokio::test]
  async fn unable_to_load_a_local_module() {
    let (temp_dir, snapshot) = setup(
      &[(
        "a.ts",
        r#"
        import { 東京 } from "./🦕.ts";
        "#,
        1,
        LanguageId::TypeScript,
      )],
      None,
    )
    .await;
    let actual = generate_all_deno_diagnostics(&snapshot);
    assert_eq!(
      json!(actual),
      json!([
        [
          url_to_uri(&temp_dir.url().join("a.ts").unwrap()).unwrap(),
          [
            {
              "range": {
                "start": { "line": 1, "character": 27 },
                "end": { "line": 1, "character": 35 },
              },
              "severity": 1,
              "code": "no-local",
              "source": "deno",
              "message": format!(
                "Unable to load a local module: {}🦕.ts\nPlease check the file path.",
                temp_dir.url(),
              ),
            },
          ],
        ],
      ]),
    );
  }

  #[test]
  fn test_specifier_text_for_redirected() {
    #[track_caller]
    fn run_test(specifier: &str, referrer: &str, expected: &str) {
      let result = specifier_text_for_redirected(
        &ModuleSpecifier::parse(specifier).unwrap(),
        &ModuleSpecifier::parse(referrer).unwrap(),
      );
      assert_eq!(result, expected);
    }

    run_test("file:///a/a.ts", "file:///a/mod.ts", "./a.ts");
    run_test("file:///a/a.ts", "file:///a/sub_dir/mod.ts", "../a.ts");
    run_test(
      "file:///a/sub_dir/a.ts",
      "file:///a/mod.ts",
      "./sub_dir/a.ts",
    );
    run_test(
      "https://deno.land/x/example/mod.ts",
      "file:///a/sub_dir/a.ts",
      "https://deno.land/x/example/mod.ts",
    );
  }
}
