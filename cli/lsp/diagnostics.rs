// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::client::Client;
use super::config::ConfigSnapshot;
use super::documents;
use super::documents::Documents;
use super::language_server;
use super::performance::Performance;
use super::tsc;
use super::tsc::TsServer;

use crate::config_file::LintConfig;
use crate::diagnostics;

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_runtime::tokio_util::create_basic_runtime;
use log::error;
use lspower::lsp;
use std::collections::HashMap;
use std::collections::HashSet;
use std::mem;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

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
      let mut version_diagnostics =
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

#[derive(Debug)]
pub(crate) struct DiagnosticsServer {
  channel: Option<mpsc::UnboundedSender<()>>,
  ts_diagnostics: Arc<Mutex<DiagnosticMap>>,
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

  pub(crate) async fn get_ts_diagnostics(
    &self,
    specifier: &ModuleSpecifier,
    document_version: Option<i32>,
  ) -> Vec<lsp::Diagnostic> {
    let ts_diagnostics = self.ts_diagnostics.lock().await;
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

  pub(crate) async fn invalidate(&self, specifiers: Vec<ModuleSpecifier>) {
    let mut ts_diagnostics = self.ts_diagnostics.lock().await;
    for specifier in &specifiers {
      ts_diagnostics.remove(specifier);
    }
  }

  pub(crate) async fn invalidate_all(&self) {
    let mut ts_diagnostics = self.ts_diagnostics.lock().await;
    ts_diagnostics.clear();
  }

  pub(crate) fn start(
    &mut self,
    language_server: Arc<Mutex<language_server::Inner>>,
  ) {
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();
    self.channel = Some(tx);
    let client = self.client.clone();
    let performance = self.performance.clone();
    let stored_ts_diagnostics = self.ts_diagnostics.clone();
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
            Some(()) => {
              // cancel the previous run
              token.cancel();
              token = CancellationToken::new();
              diagnostics_publisher.clear().await;

              let (snapshot, config, maybe_lint_config) = {
                let language_server = language_server.lock().await;
                (
                  language_server.snapshot(),
                  language_server.config.snapshot(),
                  language_server.maybe_lint_config.clone(),
                )
              };

              let previous_ts_handle = ts_handle.take();
              ts_handle = Some(tokio::spawn({
                let performance = performance.clone();
                let diagnostics_publisher = diagnostics_publisher.clone();
                let ts_server = ts_server.clone();
                let token = token.clone();
                let stored_ts_diagnostics = stored_ts_diagnostics.clone();
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
                  let diagnostics =
                    generate_ts_diagnostics(snapshot.clone(), &ts_server)
                      .await
                      .map_err(|err| {
                        error!(
                          "Error generating TypeScript diagnostics: {}",
                          err
                        );
                      })
                      .unwrap_or_default();

                  if !token.is_cancelled() {
                    {
                      let mut stored_ts_diagnostics =
                        stored_ts_diagnostics.lock().await;
                      *stored_ts_diagnostics = diagnostics
                        .iter()
                        .map(|(specifier, version, diagnostics)| {
                          (specifier.clone(), (*version, diagnostics.clone()))
                        })
                        .collect();
                    }

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
                    snapshot.clone(),
                    config.clone(),
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

  pub(crate) fn update(&self) -> Result<(), AnyError> {
    if let Some(tx) = &self.channel {
      tx.send(()).map_err(|err| err.into())
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
      let is_allowed = match &maybe_lint_config {
        Some(lint_config) => {
          lint_config.files.matches_specifier(document.specifier())
        }
        None => true,
      };
      let diagnostics = if is_allowed {
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
      } else {
        Vec::new()
      };
      diagnostics_vec.push((
        document.specifier().clone(),
        version,
        diagnostics,
      ));
    }
  }
  diagnostics_vec
}

async fn generate_ts_diagnostics(
  snapshot: Arc<language_server::StateSnapshot>,
  ts_server: &tsc::TsServer,
) -> Result<DiagnosticVec, AnyError> {
  let mut diagnostics_vec = Vec::new();
  let specifiers = snapshot
    .documents
    .documents(true, true)
    .iter()
    .map(|d| d.specifier().clone())
    .collect::<Vec<_>>();
  if !specifiers.is_empty() {
    let req = tsc::RequestMethod::GetDiagnostics(specifiers);
    let ts_diagnostics_map: TsDiagnosticsMap =
      ts_server.request(snapshot.clone(), req).await?;
    for (specifier_str, ts_diagnostics) in ts_diagnostics_map {
      let specifier = resolve_url(&specifier_str)?;
      let version = snapshot
        .documents
        .get(&specifier)
        .map(|d| d.maybe_lsp_version())
        .flatten();
      diagnostics_vec.push((
        specifier,
        version,
        ts_json_to_diagnostics(ts_diagnostics),
      ));
    }
  }
  Ok(diagnostics_vec)
}

fn resolution_error_as_code(
  err: &deno_graph::ResolutionError,
) -> lsp::NumberOrString {
  use deno_graph::ResolutionError;
  use deno_graph::SpecifierError;

  match err {
    ResolutionError::InvalidDowngrade(_, _) => {
      lsp::NumberOrString::String("invalid-downgrade".to_string())
    }
    ResolutionError::InvalidLocalImport(_, _) => {
      lsp::NumberOrString::String("invalid-local-import".to_string())
    }
    ResolutionError::InvalidSpecifier(err, _) => match err {
      SpecifierError::ImportPrefixMissing(_, _) => {
        lsp::NumberOrString::String("import-prefix-missing".to_string())
      }
      SpecifierError::InvalidUrl(_) => {
        lsp::NumberOrString::String("invalid-url".to_string())
      }
    },
    ResolutionError::ResolverError(_, _, _) => {
      lsp::NumberOrString::String("resolver-error".to_string())
    }
  }
}

fn diagnose_dependency(
  diagnostics: &mut Vec<lsp::Diagnostic>,
  documents: &Documents,
  resolved: &deno_graph::Resolved,
  is_dynamic: bool,
  maybe_assert_type: Option<&str>,
) {
  match resolved {
    Some(Ok((specifier, range))) => {
      if let Some(doc) = documents.get(specifier) {
        if let Some(message) = doc.maybe_warning() {
          diagnostics.push(lsp::Diagnostic {
            range: documents::to_lsp_range(range),
            severity: Some(lsp::DiagnosticSeverity::WARNING),
            code: Some(lsp::NumberOrString::String("deno-warn".to_string())),
            source: Some("deno".to_string()),
            message,
            ..Default::default()
          })
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
            Some(assert_type) => diagnostics.push(lsp::Diagnostic {
              range: documents::to_lsp_range(range),
              severity: Some(lsp::DiagnosticSeverity::ERROR),
              code: Some(lsp::NumberOrString::String("invalid-assert-type".to_string())),
              source: Some("deno".to_string()),
              message: format!("The module is a JSON module and expected an assertion type of \"json\". Instead got \"{}\".", assert_type),
              ..Default::default()
            }),
            // The module is missing an assertion type, diagnostic
            None => diagnostics.push(lsp::Diagnostic {
              range: documents::to_lsp_range(range),
              severity: Some(lsp::DiagnosticSeverity::ERROR),
              code: Some(lsp::NumberOrString::String("no-assert-type".to_string())),
              source: Some("deno".to_string()),
              message: "The module is a JSON module and not being imported with an import assertion. Consider adding `assert { type: \"json\" }` to the import statement.".to_string(),
              ..Default::default()
            }),
          }
        }
      } else {
        let (code, message) = match specifier.scheme() {
          "file" => (Some(lsp::NumberOrString::String("no-local".to_string())), format!("Unable to load a local module: \"{}\".\n  Please check the file path.", specifier)),
          "data" => (Some(lsp::NumberOrString::String("no-cache-data".to_string())), "Uncached data URL.".to_string()),
            "blob" => (Some(lsp::NumberOrString::String("no-cache-blob".to_string())), "Uncached blob URL.".to_string()),
            _ => (Some(lsp::NumberOrString::String("no-cache".to_string())), format!("Uncached or missing remote URL: \"{}\".", specifier)),
        };
        diagnostics.push(lsp::Diagnostic {
          range: documents::to_lsp_range(range),
          severity: Some(lsp::DiagnosticSeverity::ERROR),
          code,
          source: Some("deno".to_string()),
          message,
          data: Some(json!({ "specifier": specifier })),
          ..Default::default()
        });
      }
    }
    Some(Err(err)) => diagnostics.push(lsp::Diagnostic {
      range: documents::to_lsp_range(err.range()),
      severity: Some(lsp::DiagnosticSeverity::ERROR),
      code: Some(resolution_error_as_code(err)),
      source: Some("deno".to_string()),
      message: err.to_string(),
      ..Default::default()
    }),
    _ => (),
  }
}

/// Generate diagnostics for dependencies of a module, attempting to resolve
/// dependencies on the local file system or in the DENO_DIR cache.
async fn generate_deps_diagnostics(
  snapshot: Arc<language_server::StateSnapshot>,
  config: Arc<ConfigSnapshot>,
  token: CancellationToken,
) -> DiagnosticVec {
  let mut diagnostics_vec = Vec::new();

  for document in snapshot.documents.documents(true, true) {
    if token.is_cancelled() {
      break;
    }
    if !config.specifier_enabled(document.specifier()) {
      continue;
    }
    let mut diagnostics = Vec::new();
    for (_, dependency) in document.dependencies() {
      diagnose_dependency(
        &mut diagnostics,
        &snapshot.documents,
        &dependency.maybe_code,
        dependency.is_dynamic,
        dependency.maybe_assert_type.as_deref(),
      );
      diagnose_dependency(
        &mut diagnostics,
        &snapshot.documents,
        &dependency.maybe_type,
        dependency.is_dynamic,
        dependency.maybe_assert_type.as_deref(),
      );
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
  ) -> (StateSnapshot, PathBuf, ConfigSnapshot) {
    let temp_dir = TempDir::new().expect("could not create temp dir");
    let location = temp_dir.path().join("deps");
    let state_snapshot = mock_state_snapshot(sources, &location);
    let config = mock_config();
    (state_snapshot, location, config)
  }

  #[tokio::test]
  async fn test_generate_lint_diagnostics() {
    let (snapshot, _, config) = setup(&[(
      "file:///a.ts",
      r#"import * as b from "./b.ts";

let a = "a";
console.log(a);
"#,
      1,
      LanguageId::TypeScript,
    )]);
    let diagnostics =
      generate_lint_diagnostics(&snapshot, &config, None, Default::default())
        .await;
    assert_eq!(diagnostics.len(), 1);
    let (_, _, diagnostics) = &diagnostics[0];
    assert_eq!(diagnostics.len(), 2);
  }
}
