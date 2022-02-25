// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::cache;
use super::client::Client;
use super::config::ConfigSnapshot;
use super::documents;
use super::documents::Document;
use super::documents::Documents;
use super::language_server;
use super::language_server::StateSnapshot;
use super::performance::Performance;
use super::tsc;
use super::tsc::TsServer;

use crate::config_file::LintConfig;
use crate::diagnostics;

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_graph::Resolved;
use deno_runtime::tokio_util::create_basic_runtime;
use log::error;
use lspower::lsp;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

pub(crate) type SnapshotForDiagnostics =
  (Arc<StateSnapshot>, Arc<ConfigSnapshot>, Option<LintConfig>);
pub type DiagnosticRecord =
  (ModuleSpecifier, Option<i32>, Vec<lsp::Diagnostic>);
pub type DiagnosticVec = Vec<DiagnosticRecord>;
type DiagnosticMap =
  HashMap<ModuleSpecifier, (Option<i32>, Vec<lsp::Diagnostic>)>;
type TsDiagnosticsMap = HashMap<String, Vec<diagnostics::Diagnostic>>;
type DiagnosticsByVersionMap = HashMap<Option<i32>, Vec<lsp::Diagnostic>>;

#[derive(Clone)]
struct DiagnosticsPublisher {
  client: Client,
  all_diagnostics:
    Arc<Mutex<HashMap<ModuleSpecifier, DiagnosticsByVersionMap>>>,
}

impl DiagnosticsPublisher {
  pub fn new(client: Client) -> Self {
    Self {
      client,
      all_diagnostics: Default::default(),
    }
  }

  pub async fn publish(
    &self,
    diagnostics: DiagnosticVec,
    token: &CancellationToken,
  ) {
    let mut all_diagnostics = self.all_diagnostics.lock().await;
    for (specifier, version, diagnostics) in diagnostics {
      if token.is_cancelled() {
        return;
      }

      // the versions of all the published diagnostics should be the same, but just
      // in case they're not keep track of that
      let diagnostics_by_version =
        all_diagnostics.entry(specifier.clone()).or_default();
      let version_diagnostics =
        diagnostics_by_version.entry(version).or_default();
      version_diagnostics.extend(diagnostics);

      self
        .client
        .publish_diagnostics(specifier, version_diagnostics.clone(), version)
        .await;
    }
  }

  pub async fn clear(&self) {
    let mut all_diagnostics = self.all_diagnostics.lock().await;
    all_diagnostics.clear();
  }
}

#[derive(Clone, Default, Debug)]
struct TsDiagnosticsStore(Arc<deno_core::parking_lot::Mutex<DiagnosticMap>>);

impl TsDiagnosticsStore {
  pub fn get(
    &self,
    specifier: &ModuleSpecifier,
    document_version: Option<i32>,
  ) -> Vec<lsp::Diagnostic> {
    let ts_diagnostics = self.0.lock();
    if let Some((diagnostics_doc_version, diagnostics)) =
      ts_diagnostics.get(specifier)
    {
      // only get the diagnostics if they're up to date
      if document_version == *diagnostics_doc_version {
        return diagnostics.clone();
      }
    }
    Vec::new()
  }

  pub fn invalidate(&self, specifiers: &[ModuleSpecifier]) {
    let mut ts_diagnostics = self.0.lock();
    for specifier in specifiers {
      ts_diagnostics.remove(specifier);
    }
  }

  pub fn invalidate_all(&self) {
    self.0.lock().clear();
  }

  fn update(&self, diagnostics: &DiagnosticVec) {
    let mut stored_ts_diagnostics = self.0.lock();
    *stored_ts_diagnostics = diagnostics
      .iter()
      .map(|(specifier, version, diagnostics)| {
        (specifier.clone(), (*version, diagnostics.clone()))
      })
      .collect();
  }
}

#[derive(Debug)]
pub(crate) struct DiagnosticsServer {
  channel: Option<mpsc::UnboundedSender<SnapshotForDiagnostics>>,
  ts_diagnostics: TsDiagnosticsStore,
  client: Client,
  performance: Arc<Performance>,
  ts_server: Arc<TsServer>,
}

impl DiagnosticsServer {
  pub fn new(
    client: Client,
    performance: Arc<Performance>,
    ts_server: Arc<TsServer>,
  ) -> Self {
    DiagnosticsServer {
      channel: Default::default(),
      ts_diagnostics: Default::default(),
      client,
      performance,
      ts_server,
    }
  }

  pub(crate) fn get_ts_diagnostics(
    &self,
    specifier: &ModuleSpecifier,
    document_version: Option<i32>,
  ) -> Vec<lsp::Diagnostic> {
    self.ts_diagnostics.get(specifier, document_version)
  }

  pub(crate) fn invalidate(&self, specifiers: &[ModuleSpecifier]) {
    self.ts_diagnostics.invalidate(specifiers);
  }

  pub(crate) fn invalidate_all(&self) {
    self.ts_diagnostics.invalidate_all();
  }

  #[allow(unused_must_use)]
  pub(crate) fn start(&mut self) {
    let (tx, mut rx) = mpsc::unbounded_channel::<SnapshotForDiagnostics>();
    self.channel = Some(tx);
    let client = self.client.clone();
    let performance = self.performance.clone();
    let ts_diagnostics_store = self.ts_diagnostics.clone();
    let ts_server = self.ts_server.clone();

    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        let mut token = CancellationToken::new();
        let mut ts_handle: Option<tokio::task::JoinHandle<()>> = None;
        let mut lint_handle: Option<tokio::task::JoinHandle<()>> = None;
        let mut deps_handle: Option<tokio::task::JoinHandle<()>> = None;
        let diagnostics_publisher = DiagnosticsPublisher::new(client.clone());

        loop {
          match rx.recv().await {
            // channel has closed
            None => break,
            Some((snapshot, config, maybe_lint_config)) => {
              // cancel the previous run
              token.cancel();
              token = CancellationToken::new();
              diagnostics_publisher.clear().await;

              let previous_ts_handle = ts_handle.take();
              ts_handle = Some(tokio::spawn({
                let performance = performance.clone();
                let diagnostics_publisher = diagnostics_publisher.clone();
                let ts_server = ts_server.clone();
                let token = token.clone();
                let ts_diagnostics_store = ts_diagnostics_store.clone();
                let snapshot = snapshot.clone();
                let config = config.clone();
                async move {
                  if let Some(previous_handle) = previous_ts_handle {
                    // Wait on the previous run to complete in order to prevent
                    // multiple threads queueing up a lot of tsc requests.
                    // Do not race this with cancellation because we want a
                    // chain of events to wait for all the previous diagnostics to complete
                    previous_handle.await;
                  }

                  // Debounce timer delay. 150ms between keystrokes is about 45 WPM, so we
                  // want something that is longer than that, but not too long to
                  // introduce detectable UI delay; 200ms is a decent compromise.
                  const DELAY: Duration = Duration::from_millis(200);
                  tokio::select! {
                    _ = token.cancelled() => { return; }
                    _ = tokio::time::sleep(DELAY) => {}
                  };

                  let mark =
                    performance.mark("update_diagnostics_ts", None::<()>);
                  let diagnostics = generate_ts_diagnostics(
                    snapshot.clone(),
                    &config,
                    &ts_server,
                    token.clone(),
                  )
                  .await
                  .map_err(|err| {
                    error!("Error generating TypeScript diagnostics: {}", err);
                  })
                  .unwrap_or_default();

                  if !token.is_cancelled() {
                    ts_diagnostics_store.update(&diagnostics);
                    diagnostics_publisher.publish(diagnostics, &token).await;

                    if !token.is_cancelled() {
                      performance.measure(mark);
                    }
                  }
                }
              }));

              let previous_deps_handle = deps_handle.take();
              deps_handle = Some(tokio::spawn({
                let performance = performance.clone();
                let diagnostics_publisher = diagnostics_publisher.clone();
                let token = token.clone();
                let snapshot = snapshot.clone();
                let config = config.clone();
                async move {
                  if let Some(previous_handle) = previous_deps_handle {
                    previous_handle.await;
                  }
                  let mark =
                    performance.mark("update_diagnostics_deps", None::<()>);
                  let diagnostics = generate_deps_diagnostics(
                    &snapshot,
                    &config,
                    token.clone(),
                  )
                  .await;

                  diagnostics_publisher.publish(diagnostics, &token).await;

                  if !token.is_cancelled() {
                    performance.measure(mark);
                  }
                }
              }));

              let previous_lint_handle = lint_handle.take();
              lint_handle = Some(tokio::spawn({
                let performance = performance.clone();
                let diagnostics_publisher = diagnostics_publisher.clone();
                let token = token.clone();
                let snapshot = snapshot.clone();
                let config = config.clone();
                async move {
                  if let Some(previous_handle) = previous_lint_handle {
                    previous_handle.await;
                  }
                  let mark =
                    performance.mark("update_diagnostics_lint", None::<()>);
                  let diagnostics = generate_lint_diagnostics(
                    &snapshot,
                    &config,
                    maybe_lint_config,
                    token.clone(),
                  )
                  .await;

                  diagnostics_publisher.publish(diagnostics, &token).await;

                  if !token.is_cancelled() {
                    performance.measure(mark);
                  }
                }
              }));
            }
          }
        }
      })
    });
  }

  pub(crate) fn update(
    &self,
    message: SnapshotForDiagnostics,
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

impl<'a> From<&'a diagnostics::DiagnosticCategory> for lsp::DiagnosticSeverity {
  fn from(category: &'a diagnostics::DiagnosticCategory) -> Self {
    match category {
      diagnostics::DiagnosticCategory::Error => lsp::DiagnosticSeverity::ERROR,
      diagnostics::DiagnosticCategory::Warning => {
        lsp::DiagnosticSeverity::WARNING
      }
      diagnostics::DiagnosticCategory::Suggestion => {
        lsp::DiagnosticSeverity::HINT
      }
      diagnostics::DiagnosticCategory::Message => {
        lsp::DiagnosticSeverity::INFORMATION
      }
    }
  }
}

impl<'a> From<&'a diagnostics::Position> for lsp::Position {
  fn from(pos: &'a diagnostics::Position) -> Self {
    Self {
      line: pos.line as u32,
      character: pos.character as u32,
    }
  }
}

fn get_diagnostic_message(diagnostic: &diagnostics::Diagnostic) -> String {
  if let Some(message) = diagnostic.message_text.clone() {
    message
  } else if let Some(message_chain) = diagnostic.message_chain.clone() {
    message_chain.format_message(0)
  } else {
    "[missing message]".to_string()
  }
}

fn to_lsp_range(
  start: &diagnostics::Position,
  end: &diagnostics::Position,
) -> lsp::Range {
  lsp::Range {
    start: start.into(),
    end: end.into(),
  }
}

fn to_lsp_related_information(
  related_information: &Option<Vec<diagnostics::Diagnostic>>,
) -> Option<Vec<lsp::DiagnosticRelatedInformation>> {
  related_information.as_ref().map(|related| {
    related
      .iter()
      .filter_map(|ri| {
        if let (Some(source), Some(start), Some(end)) =
          (&ri.source, &ri.start, &ri.end)
        {
          let uri = lsp::Url::parse(source).unwrap();
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

fn ts_json_to_diagnostics(
  diagnostics: Vec<diagnostics::Diagnostic>,
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
          source: Some("deno-ts".to_string()),
          message: get_diagnostic_message(d),
          related_information: to_lsp_related_information(
            &d.related_information,
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

async fn generate_lint_diagnostics(
  snapshot: &language_server::StateSnapshot,
  config: &ConfigSnapshot,
  maybe_lint_config: Option<LintConfig>,
  token: CancellationToken,
) -> DiagnosticVec {
  let documents = snapshot.documents.documents(true, true);
  let workspace_settings = config.settings.workspace.clone();

  let mut diagnostics_vec = Vec::new();
  if workspace_settings.lint {
    for document in documents {
      // exit early if cancelled
      if token.is_cancelled() {
        break;
      }

      let version = document.maybe_lsp_version();
      diagnostics_vec.push((
        document.specifier().clone(),
        version,
        generate_document_lint_diagnostics(
          config,
          &maybe_lint_config,
          &document,
        ),
      ));
    }
  }
  diagnostics_vec
}

fn generate_document_lint_diagnostics(
  config: &ConfigSnapshot,
  maybe_lint_config: &Option<LintConfig>,
  document: &Document,
) -> Vec<lsp::Diagnostic> {
  if !config.specifier_enabled(document.specifier()) {
    return Vec::new();
  }
  if let Some(lint_config) = &maybe_lint_config {
    if !lint_config.files.matches_specifier(document.specifier()) {
      return Vec::new();
    }
  }
  match document.maybe_parsed_source() {
    Some(Ok(parsed_source)) => {
      if let Ok(references) = analysis::get_lint_references(
        &parsed_source,
        maybe_lint_config.as_ref(),
      ) {
        references
          .into_iter()
          .map(|r| r.to_diagnostic())
          .collect::<Vec<_>>()
      } else {
        Vec::new()
      }
    }
    Some(Err(_)) => Vec::new(),
    None => {
      error!("Missing file contents for: {}", document.specifier());
      Vec::new()
    }
  }
}

async fn generate_ts_diagnostics(
  snapshot: Arc<language_server::StateSnapshot>,
  config: &ConfigSnapshot,
  ts_server: &tsc::TsServer,
  token: CancellationToken,
) -> Result<DiagnosticVec, AnyError> {
  let mut diagnostics_vec = Vec::new();
  let specifiers = snapshot
    .documents
    .documents(true, true)
    .iter()
    .map(|d| d.specifier().clone())
    .collect::<Vec<_>>();
  let (enabled_specifiers, disabled_specifiers) = specifiers
    .iter()
    .cloned()
    .partition::<Vec<_>, _>(|s| config.specifier_enabled(s));
  let ts_diagnostics_map: TsDiagnosticsMap = if !enabled_specifiers.is_empty() {
    let req = tsc::RequestMethod::GetDiagnostics(enabled_specifiers);
    ts_server
      .request_with_cancellation(snapshot.clone(), req, token)
      .await?
  } else {
    Default::default()
  };
  for (specifier_str, ts_json_diagnostics) in ts_diagnostics_map {
    let specifier = resolve_url(&specifier_str)?;
    let version = snapshot
      .documents
      .get(&specifier)
      .and_then(|d| d.maybe_lsp_version());
    // check if the specifier is enabled again just in case TS returns us
    // diagnostics for a disabled specifier
    let ts_diagnostics = if config.specifier_enabled(&specifier) {
      ts_json_to_diagnostics(ts_json_diagnostics)
    } else {
      Vec::new()
    };
    diagnostics_vec.push((specifier, version, ts_diagnostics));
  }
  // add an empty diagnostic publish for disabled specifiers in order
  // to clear those diagnostics if they exist
  for specifier in disabled_specifiers {
    let version = snapshot
      .documents
      .get(&specifier)
      .and_then(|d| d.maybe_lsp_version());
    diagnostics_vec.push((specifier, version, Vec::new()));
  }
  Ok(diagnostics_vec)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataSpecifier {
  pub specifier: ModuleSpecifier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataRedirect {
  pub redirect: ModuleSpecifier,
}

/// An enum which represents diagnostic errors which originate from Deno itself.
pub(crate) enum DenoDiagnostic {
  /// A `x-deno-warn` is associated with the specifier and should be displayed
  /// as a warning to the user.
  DenoWarn(String),
  /// The import assertion type is incorrect.
  InvalidAssertType(String),
  /// A module requires an assertion type to be a valid import.
  NoAssertType,
  /// A remote module was not found in the cache.
  NoCache(ModuleSpecifier),
  /// A blob module was not found in the cache.
  NoCacheBlob,
  /// A data module was not found in the cache.
  NoCacheData(ModuleSpecifier),
  /// A local module was not found on the local file system.
  NoLocal(ModuleSpecifier),
  /// The specifier resolved to a remote specifier that was redirected to
  /// another specifier.
  Redirect {
    from: ModuleSpecifier,
    to: ModuleSpecifier,
  },
  /// An error occurred when resolving the specifier string.
  ResolutionError(deno_graph::ResolutionError),
}

impl DenoDiagnostic {
  fn code(&self) -> &str {
    use deno_graph::ResolutionError;
    use deno_graph::SpecifierError;

    match self {
      Self::DenoWarn(_) => "deno-warn",
      Self::InvalidAssertType(_) => "invalid-assert-type",
      Self::NoAssertType => "no-assert-type",
      Self::NoCache(_) => "no-cache",
      Self::NoCacheBlob => "no-cache-blob",
      Self::NoCacheData(_) => "no-cache-data",
      Self::NoLocal(_) => "no-local",
      Self::Redirect { .. } => "redirect",
      Self::ResolutionError(err) => match err {
        ResolutionError::InvalidDowngrade { .. } => "invalid-downgrade",
        ResolutionError::InvalidLocalImport { .. } => "invalid-local-import",
        ResolutionError::InvalidSpecifier { error, .. } => match error {
          SpecifierError::ImportPrefixMissing(_, _) => "import-prefix-missing",
          SpecifierError::InvalidUrl(_) => "invalid-url",
        },
        ResolutionError::ResolverError { .. } => "resolver-error",
      },
    }
  }

  /// A "static" method which for a diagnostic that originated from the
  /// structure returns a code action which can resolve the diagnostic.
  pub(crate) fn get_code_action(
    specifier: &ModuleSpecifier,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<lsp::CodeAction, AnyError> {
    if let Some(lsp::NumberOrString::String(code)) = &diagnostic.code {
      let code_action = match code.as_str() {
        "no-assert-type" => lsp::CodeAction {
          title: "Insert import assertion.".to_string(),
          kind: Some(lsp::CodeActionKind::QUICKFIX),
          diagnostics: Some(vec![diagnostic.clone()]),
          edit: Some(lsp::WorkspaceEdit {
            changes: Some(HashMap::from([(
              specifier.clone(),
              vec![lsp::TextEdit {
                new_text: " assert { type: \"json\" }".to_string(),
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
        "no-cache" | "no-cache-data" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataSpecifier = serde_json::from_value(data)?;
          let title = if code == "no-cache" {
            format!("Cache \"{}\" and its dependencies.", data.specifier)
          } else {
            "Cache the data URL and its dependencies.".to_string()
          };
          lsp::CodeAction {
            title,
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            command: Some(lsp::Command {
              title: "".to_string(),
              command: "deno.cache".to_string(),
              arguments: Some(vec![json!([data.specifier])]),
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
                specifier.clone(),
                vec![lsp::TextEdit {
                  new_text: format!("\"{}\"", data.redirect),
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
          ))
        }
      };
      Ok(code_action)
    } else {
      Err(anyhow!("Unsupported diagnostic code provided."))
    }
  }

  /// Given a reference to the code from an LSP diagnostic, determine if the
  /// diagnostic is fixable or not
  pub(crate) fn is_fixable(code: &Option<lsp::NumberOrString>) -> bool {
    if let Some(lsp::NumberOrString::String(code)) = code {
      matches!(
        code.as_str(),
        "no-cache" | "no-cache-data" | "no-assert-type" | "redirect"
      )
    } else {
      false
    }
  }

  /// Convert to an lsp Diagnostic when the range the diagnostic applies to is
  /// provided.
  pub(crate) fn to_lsp_diagnostic(
    &self,
    range: &lsp::Range,
  ) -> lsp::Diagnostic {
    let (severity, message, data) = match self {
      Self::DenoWarn(message) => (lsp::DiagnosticSeverity::WARNING, message.to_string(), None),
      Self::InvalidAssertType(assert_type) => (lsp::DiagnosticSeverity::ERROR, format!("The module is a JSON module and expected an assertion type of \"json\". Instead got \"{}\".", assert_type), None),
      Self::NoAssertType => (lsp::DiagnosticSeverity::ERROR, "The module is a JSON module and not being imported with an import assertion. Consider adding `assert { type: \"json\" }` to the import statement.".to_string(), None),
      Self::NoCache(specifier) => (lsp::DiagnosticSeverity::ERROR, format!("Uncached or missing remote URL: \"{}\".", specifier), Some(json!({ "specifier": specifier }))),
      Self::NoCacheBlob => (lsp::DiagnosticSeverity::ERROR, "Uncached blob URL.".to_string(), None),
      Self::NoCacheData(specifier) => (lsp::DiagnosticSeverity::ERROR, "Uncached data URL.".to_string(), Some(json!({ "specifier": specifier }))),
      Self::NoLocal(specifier) => (lsp::DiagnosticSeverity::ERROR, format!("Unable to load a local module: \"{}\".\n  Please check the file path.", specifier), None),
      Self::Redirect { from, to} => (lsp::DiagnosticSeverity::INFORMATION, format!("The import of \"{}\" was redirected to \"{}\".", from, to), Some(json!({ "specifier": from, "redirect": to }))),
      Self::ResolutionError(err) => (lsp::DiagnosticSeverity::ERROR, err.to_string(), None),
    };
    lsp::Diagnostic {
      range: *range,
      severity: Some(severity),
      code: Some(lsp::NumberOrString::String(self.code().to_string())),
      source: Some("deno".to_string()),
      message,
      data,
      ..Default::default()
    }
  }
}

fn diagnose_dependency(
  diagnostics: &mut Vec<lsp::Diagnostic>,
  documents: &Documents,
  cache_metadata: &cache::CacheMetadata,
  resolved: &deno_graph::Resolved,
  is_dynamic: bool,
  maybe_assert_type: Option<&str>,
) {
  match resolved {
    Resolved::Ok {
      specifier, range, ..
    } => {
      let range = documents::to_lsp_range(range);
      // If the module is a remote module and has a `X-Deno-Warn` header, we
      // want a warning diagnostic with that message.
      if let Some(metadata) = cache_metadata.get(specifier) {
        if let Some(message) =
          metadata.get(&cache::MetadataKey::Warning).cloned()
        {
          diagnostics
            .push(DenoDiagnostic::DenoWarn(message).to_lsp_diagnostic(&range));
        }
      }
      if let Some(doc) = documents.get(specifier) {
        let doc_specifier = doc.specifier();
        // If the module was redirected, we want to issue an informational
        // diagnostic that indicates this. This then allows us to issue a code
        // action to replace the specifier with the final redirected one.
        if doc_specifier != specifier {
          diagnostics.push(
            DenoDiagnostic::Redirect {
              from: specifier.clone(),
              to: doc_specifier.clone(),
            }
            .to_lsp_diagnostic(&range),
          );
        }
        if doc.media_type() == MediaType::Json {
          match maybe_assert_type {
            // The module has the correct assertion type, no diagnostic
            Some("json") => (),
            // The dynamic import statement is missing an assertion type, which
            // we might not be able to statically detect, therefore we will
            // not provide a potentially incorrect diagnostic.
            None if is_dynamic => (),
            // The module has an incorrect assertion type, diagnostic
            Some(assert_type) => diagnostics.push(
              DenoDiagnostic::InvalidAssertType(assert_type.to_string())
                .to_lsp_diagnostic(&range),
            ),
            // The module is missing an assertion type, diagnostic
            None => diagnostics
              .push(DenoDiagnostic::NoAssertType.to_lsp_diagnostic(&range)),
          }
        }
      } else {
        // When the document is not available, it means that it cannot be found
        // in the cache or locally on the disk, so we want to issue a diagnostic
        // about that.
        let deno_diagnostic = match specifier.scheme() {
          "file" => DenoDiagnostic::NoLocal(specifier.clone()),
          "data" => DenoDiagnostic::NoCacheData(specifier.clone()),
          "blob" => DenoDiagnostic::NoCacheBlob,
          _ => DenoDiagnostic::NoCache(specifier.clone()),
        };
        diagnostics.push(deno_diagnostic.to_lsp_diagnostic(&range));
      }
    }
    // The specifier resolution resulted in an error, so we want to issue a
    // diagnostic for that.
    Resolved::Err(err) => diagnostics.push(
      DenoDiagnostic::ResolutionError(err.clone())
        .to_lsp_diagnostic(&documents::to_lsp_range(err.range())),
    ),
    _ => (),
  }
}

/// Generate diagnostics for dependencies of a module, attempting to resolve
/// dependencies on the local file system or in the DENO_DIR cache.
async fn generate_deps_diagnostics(
  snapshot: &language_server::StateSnapshot,
  config: &ConfigSnapshot,
  token: CancellationToken,
) -> DiagnosticVec {
  let mut diagnostics_vec = Vec::new();

  for document in snapshot.documents.documents(true, true) {
    if token.is_cancelled() {
      break;
    }
    let mut diagnostics = Vec::new();
    if config.specifier_enabled(document.specifier()) {
      for (_, dependency) in document.dependencies() {
        diagnose_dependency(
          &mut diagnostics,
          &snapshot.documents,
          &snapshot.cache_metadata,
          &dependency.maybe_code,
          dependency.is_dynamic,
          dependency.maybe_assert_type.as_deref(),
        );
        diagnose_dependency(
          &mut diagnostics,
          &snapshot.documents,
          &snapshot.cache_metadata,
          &dependency.maybe_type,
          dependency.is_dynamic,
          dependency.maybe_assert_type.as_deref(),
        );
      }
    }
    diagnostics_vec.push((
      document.specifier().clone(),
      document.maybe_lsp_version(),
      diagnostics,
    ));
  }

  diagnostics_vec
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::lsp::config::ConfigSnapshot;
  use crate::lsp::config::Settings;
  use crate::lsp::config::SpecifierSettings;
  use crate::lsp::config::WorkspaceSettings;
  use crate::lsp::documents::LanguageId;
  use crate::lsp::language_server::StateSnapshot;
  use std::path::Path;
  use std::path::PathBuf;
  use tempfile::TempDir;

  fn mock_state_snapshot(
    fixtures: &[(&str, &str, i32, LanguageId)],
    location: &Path,
  ) -> StateSnapshot {
    let mut documents = Documents::new(location);
    for (specifier, source, version, language_id) in fixtures {
      let specifier =
        resolve_url(specifier).expect("failed to create specifier");
      documents.open(
        specifier.clone(),
        *version,
        language_id.clone(),
        Arc::new(source.to_string()),
      );
    }
    StateSnapshot {
      documents,
      ..Default::default()
    }
  }

  fn mock_config() -> ConfigSnapshot {
    ConfigSnapshot {
      settings: Settings {
        workspace: WorkspaceSettings {
          enable: true,
          lint: true,
          ..Default::default()
        },
        ..Default::default()
      },
      ..Default::default()
    }
  }

  fn setup(
    sources: &[(&str, &str, i32, LanguageId)],
  ) -> (StateSnapshot, PathBuf) {
    let temp_dir = TempDir::new().expect("could not create temp dir");
    let location = temp_dir.path().join("deps");
    let state_snapshot = mock_state_snapshot(sources, &location);
    (state_snapshot, location)
  }

  #[tokio::test]
  async fn test_enabled_then_disabled_specifier() {
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
    let (snapshot, _) = setup(&[(
      "file:///a.ts",
      r#"import * as b from "./b.ts";
let a: any = "a";
let c: number = "a";
"#,
      1,
      LanguageId::TypeScript,
    )]);
    let snapshot = Arc::new(snapshot);
    let ts_server = TsServer::new(Default::default());

    // test enabled
    {
      let enabled_config = mock_config();
      let diagnostics = generate_lint_diagnostics(
        &snapshot,
        &enabled_config,
        None,
        Default::default(),
      )
      .await;
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 6);
      let diagnostics = generate_ts_diagnostics(
        snapshot.clone(),
        &enabled_config,
        &ts_server,
        Default::default(),
      )
      .await
      .unwrap();
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 4);
      let diagnostics = generate_deps_diagnostics(
        &snapshot,
        &enabled_config,
        Default::default(),
      )
      .await;
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 1);
    }

    // now test disabled specifier
    {
      let mut disabled_config = mock_config();
      disabled_config.settings.specifiers.insert(
        specifier.clone(),
        (
          specifier.clone(),
          SpecifierSettings {
            enable: false,
            code_lens: Default::default(),
          },
        ),
      );

      let diagnostics = generate_lint_diagnostics(
        &snapshot,
        &disabled_config,
        None,
        Default::default(),
      )
      .await;
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 0);
      let diagnostics = generate_ts_diagnostics(
        snapshot.clone(),
        &disabled_config,
        &ts_server,
        Default::default(),
      )
      .await
      .unwrap();
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 0);
      let diagnostics = generate_deps_diagnostics(
        &snapshot,
        &disabled_config,
        Default::default(),
      )
      .await;
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 0);
    }
  }

  fn get_diagnostics_for_single(
    diagnostic_vec: DiagnosticVec,
  ) -> Vec<lsp::Diagnostic> {
    assert_eq!(diagnostic_vec.len(), 1);
    let (_, _, diagnostics) = diagnostic_vec.into_iter().next().unwrap();
    diagnostics
  }

  #[tokio::test]
  async fn test_cancelled_ts_diagnostics_request() {
    let (snapshot, _) = setup(&[(
      "file:///a.ts",
      r#"export let a: string = 5;"#,
      1,
      LanguageId::TypeScript,
    )]);
    let snapshot = Arc::new(snapshot);
    let ts_server = TsServer::new(Default::default());

    let config = mock_config();
    let token = CancellationToken::new();
    token.cancel();
    let diagnostics =
      generate_ts_diagnostics(snapshot.clone(), &config, &ts_server, token)
        .await
        .unwrap();
    // should be none because it's cancelled
    assert_eq!(diagnostics.len(), 0);
  }
}
