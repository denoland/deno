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
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_graph::Resolved;
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
      .map(|d| d.maybe_lsp_version())
      .flatten();
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
      .map(|d| d.maybe_lsp_version())
      .flatten();
    diagnostics_vec.push((specifier, version, Vec::new()));
  }
  Ok(diagnostics_vec)
}

fn resolution_error_as_code(
  err: &deno_graph::ResolutionError,
) -> lsp::NumberOrString {
  use deno_graph::ResolutionError;
  use deno_graph::SpecifierError;

  match err {
    ResolutionError::InvalidDowngrade { .. } => {
      lsp::NumberOrString::String("invalid-downgrade".to_string())
    }
    ResolutionError::InvalidLocalImport { .. } => {
      lsp::NumberOrString::String("invalid-local-import".to_string())
    }
    ResolutionError::InvalidSpecifier { error, .. } => match error {
      SpecifierError::ImportPrefixMissing(_, _) => {
        lsp::NumberOrString::String("import-prefix-missing".to_string())
      }
      SpecifierError::InvalidUrl(_) => {
        lsp::NumberOrString::String("invalid-url".to_string())
      }
    },
    ResolutionError::ResolverError { .. } => {
      lsp::NumberOrString::String("resolver-error".to_string())
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
      if let Some(metadata) = cache_metadata.get(specifier) {
        if let Some(message) =
          metadata.get(&cache::MetadataKey::Warning).cloned()
        {
          diagnostics.push(lsp::Diagnostic {
            range: documents::to_lsp_range(range),
            severity: Some(lsp::DiagnosticSeverity::WARNING),
            code: Some(lsp::NumberOrString::String("deno-warn".to_string())),
            source: Some("deno".to_string()),
            message,
            ..Default::default()
          });
        }
      }
      if let Some(doc) = documents.get(specifier) {
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
    Resolved::Err(err) => diagnostics.push(lsp::Diagnostic {
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
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
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
