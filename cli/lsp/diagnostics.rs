// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::documents;
use super::documents::Documents;
use super::language_server;
use super::tsc;

use crate::diagnostics;

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

pub type DiagnosticRecord =
  (ModuleSpecifier, Option<i32>, Vec<lsp::Diagnostic>);
pub type DiagnosticVec = Vec<DiagnosticRecord>;
type TsDiagnosticsMap = HashMap<String, Vec<diagnostics::Diagnostic>>;

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub(crate) enum DiagnosticSource {
  Deno,
  DenoLint,
  TypeScript,
}

#[derive(Debug, Default)]
struct DiagnosticCollection {
  map: HashMap<(ModuleSpecifier, DiagnosticSource), Vec<lsp::Diagnostic>>,
  versions: HashMap<ModuleSpecifier, HashMap<DiagnosticSource, i32>>,
  changes: HashSet<ModuleSpecifier>,
}

impl DiagnosticCollection {
  pub fn get(
    &self,
    specifier: &ModuleSpecifier,
    source: DiagnosticSource,
  ) -> impl Iterator<Item = &lsp::Diagnostic> {
    self
      .map
      .get(&(specifier.clone(), source))
      .into_iter()
      .flatten()
  }

  pub fn get_version(
    &self,
    specifier: &ModuleSpecifier,
    source: &DiagnosticSource,
  ) -> Option<i32> {
    let source_version = self.versions.get(specifier)?;
    source_version.get(source).cloned()
  }

  pub fn set(&mut self, source: DiagnosticSource, record: DiagnosticRecord) {
    let (specifier, maybe_version, diagnostics) = record;
    self
      .map
      .insert((specifier.clone(), source.clone()), diagnostics);
    if let Some(version) = maybe_version {
      let source_version = self.versions.entry(specifier.clone()).or_default();
      source_version.insert(source, version);
    }
    self.changes.insert(specifier);
  }

  pub fn take_changes(&mut self) -> Option<HashSet<ModuleSpecifier>> {
    if self.changes.is_empty() {
      None
    } else {
      Some(mem::take(&mut self.changes))
    }
  }
}

#[derive(Debug, Default)]
pub(crate) struct DiagnosticsServer {
  channel: Option<mpsc::UnboundedSender<()>>,
  collection: Arc<Mutex<DiagnosticCollection>>,
}

impl DiagnosticsServer {
  pub(crate) async fn get(
    &self,
    specifier: &ModuleSpecifier,
    source: DiagnosticSource,
  ) -> Vec<lsp::Diagnostic> {
    self
      .collection
      .lock()
      .await
      .get(specifier, source)
      .cloned()
      .collect()
  }

  pub(crate) async fn invalidate(&self, specifiers: Vec<ModuleSpecifier>) {
    let mut collection = self.collection.lock().await;
    for specifier in &specifiers {
      collection.versions.remove(specifier);
    }
  }

  pub(crate) async fn invalidate_all(&self) {
    let mut collection = self.collection.lock().await;
    collection.versions.clear();
  }

  pub(crate) fn start(
    &mut self,
    language_server: Arc<Mutex<language_server::Inner>>,
    client: lspower::Client,
    ts_server: Arc<tsc::TsServer>,
  ) {
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();
    self.channel = Some(tx);
    let collection = self.collection.clone();

    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        // Debounce timer delay. 150ms between keystrokes is about 45 WPM, so we
        // want something that is longer than that, but not too long to
        // introduce detectable UI delay; 200ms is a decent compromise.
        const DELAY: Duration = Duration::from_millis(200);
        // If the debounce timer isn't active, it will be set to expire "never",
        // which is actually just 1 year in the future.
        const NEVER: Duration = Duration::from_secs(365 * 24 * 60 * 60);

        // A flag that is set whenever something has changed that requires the
        // diagnostics collection to be updated.
        let mut dirty = false;

        let debounce_timer = sleep(NEVER);
        tokio::pin!(debounce_timer);

        loop {
          // "race" the next message off the rx queue or the debounce timer.
          // The debounce timer gets reset every time a message comes off the
          // queue. When the debounce timer expires, a snapshot of the most
          // up-to-date state is used to produce diagnostics.
          tokio::select! {
            maybe_request = rx.recv() => {
              match maybe_request {
                // channel has closed
                None => break,
                Some(_) => {
                  dirty = true;
                  debounce_timer.as_mut().reset(Instant::now() + DELAY);
                }
              }
            }
            _ = debounce_timer.as_mut(), if dirty => {
              dirty = false;
              debounce_timer.as_mut().reset(Instant::now() + NEVER);

              let snapshot = language_server.lock().await.snapshot().unwrap();
              update_diagnostics(
                &client,
                collection.clone(),
                snapshot,
                &ts_server
              ).await;
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
      diagnostics::DiagnosticCategory::Error => lsp::DiagnosticSeverity::Error,
      diagnostics::DiagnosticCategory::Warning => {
        lsp::DiagnosticSeverity::Warning
      }
      diagnostics::DiagnosticCategory::Suggestion => {
        lsp::DiagnosticSeverity::Hint
      }
      diagnostics::DiagnosticCategory::Message => {
        lsp::DiagnosticSeverity::Information
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
            | 7028 => Some(vec![lsp::DiagnosticTag::Unnecessary]),
            // These are codes that indicated the variable is deprecated.
            2789 | 6385 | 6387 => Some(vec![lsp::DiagnosticTag::Deprecated]),
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
  collection: Arc<Mutex<DiagnosticCollection>>,
) -> Result<DiagnosticVec, AnyError> {
  let documents = snapshot.documents.documents(true, true);
  let workspace_settings = snapshot.config.settings.workspace.clone();
  let maybe_lint_config = snapshot.maybe_lint_config.clone();
  tokio::task::spawn(async move {
    let mut diagnostics_vec = Vec::new();
    if workspace_settings.lint {
      for document in documents {
        let version = document.maybe_lsp_version();
        let current_version = collection
          .lock()
          .await
          .get_version(document.specifier(), &DiagnosticSource::DenoLint);
        if version != current_version {
          let diagnostics = match document.maybe_parsed_source() {
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
          };
          diagnostics_vec.push((
            document.specifier().clone(),
            version,
            diagnostics,
          ));
        }
      }
    }
    Ok(diagnostics_vec)
  })
  .await
  .unwrap()
}

async fn generate_ts_diagnostics(
  snapshot: Arc<language_server::StateSnapshot>,
  collection: Arc<Mutex<DiagnosticCollection>>,
  ts_server: &tsc::TsServer,
) -> Result<DiagnosticVec, AnyError> {
  let mut diagnostics_vec = Vec::new();
  let specifiers: Vec<ModuleSpecifier> = {
    let collection = collection.lock().await;
    snapshot
      .documents
      .documents(true, true)
      .iter()
      .filter_map(|d| {
        let version = d.maybe_lsp_version();
        let current_version =
          collection.get_version(d.specifier(), &DiagnosticSource::TypeScript);
        if version != current_version {
          Some(d.specifier().clone())
        } else {
          None
        }
      })
      .collect()
  };
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
) {
  match resolved {
    Some(Ok((specifier, range))) => {
      if let Some(doc) = documents.get(specifier) {
        if let Some(message) = doc.maybe_warning() {
          diagnostics.push(lsp::Diagnostic {
            range: documents::to_lsp_range(range),
            severity: Some(lsp::DiagnosticSeverity::Warning),
            code: Some(lsp::NumberOrString::String("deno-warn".to_string())),
            source: Some("deno".to_string()),
            message,
            ..Default::default()
          })
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
          severity: Some(lsp::DiagnosticSeverity::Error),
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
      severity: Some(lsp::DiagnosticSeverity::Error),
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
  collection: Arc<Mutex<DiagnosticCollection>>,
) -> Result<DiagnosticVec, AnyError> {
  tokio::task::spawn(async move {
    let mut diagnostics_vec = Vec::new();

    for document in snapshot.documents.documents(true, true) {
      if !snapshot.config.specifier_enabled(document.specifier()) {
        continue;
      }
      let version = document.maybe_lsp_version();
      let current_version = collection
        .lock()
        .await
        .get_version(document.specifier(), &DiagnosticSource::Deno);
      if version != current_version {
        let mut diagnostics = Vec::new();
        for (_, dependency) in document.dependencies() {
          diagnose_dependency(
            &mut diagnostics,
            &snapshot.documents,
            &dependency.maybe_code,
          );
          diagnose_dependency(
            &mut diagnostics,
            &snapshot.documents,
            &dependency.maybe_type,
          );
        }
        diagnostics_vec.push((
          document.specifier().clone(),
          version,
          diagnostics,
        ));
      }
    }

    Ok(diagnostics_vec)
  })
  .await
  .unwrap()
}

/// Publishes diagnostics to the client.
async fn publish_diagnostics(
  client: &lspower::Client,
  collection: Arc<Mutex<DiagnosticCollection>>,
  snapshot: &language_server::StateSnapshot,
) {
  let mut collection = collection.lock().await;
  if let Some(changes) = collection.take_changes() {
    for specifier in changes {
      let mut diagnostics: Vec<lsp::Diagnostic> =
        if snapshot.config.settings.workspace.lint {
          collection
            .get(&specifier, DiagnosticSource::DenoLint)
            .cloned()
            .collect()
        } else {
          Vec::new()
        };
      if snapshot.config.specifier_enabled(&specifier) {
        diagnostics.extend(
          collection
            .get(&specifier, DiagnosticSource::TypeScript)
            .cloned(),
        );
        diagnostics
          .extend(collection.get(&specifier, DiagnosticSource::Deno).cloned());
      }
      let version = snapshot
        .documents
        .get(&specifier)
        .map(|d| d.maybe_lsp_version())
        .flatten();
      client
        .publish_diagnostics(specifier.clone(), diagnostics, version)
        .await;
    }
  }
}

/// Updates diagnostics for any specifiers that don't have the correct version
/// generated and publishes the diagnostics to the client.
async fn update_diagnostics(
  client: &lspower::Client,
  collection: Arc<Mutex<DiagnosticCollection>>,
  snapshot: Arc<language_server::StateSnapshot>,
  ts_server: &tsc::TsServer,
) {
  let mark = snapshot.performance.mark("update_diagnostics", None::<()>);

  let lint = async {
    let mark = snapshot
      .performance
      .mark("update_diagnostics_lint", None::<()>);
    let collection = collection.clone();
    let diagnostics = generate_lint_diagnostics(&snapshot, collection.clone())
      .await
      .map_err(|err| {
        error!("Error generating lint diagnostics: {}", err);
      })
      .unwrap_or_default();
    {
      let mut collection = collection.lock().await;
      for diagnostic_record in diagnostics {
        collection.set(DiagnosticSource::DenoLint, diagnostic_record);
      }
    }
    publish_diagnostics(client, collection, &snapshot).await;
    snapshot.performance.measure(mark);
  };

  let ts = async {
    let mark = snapshot
      .performance
      .mark("update_diagnostics_ts", None::<()>);
    let collection = collection.clone();
    let diagnostics =
      generate_ts_diagnostics(snapshot.clone(), collection.clone(), ts_server)
        .await
        .map_err(|err| {
          error!("Error generating TypeScript diagnostics: {}", err);
        })
        .unwrap_or_default();
    {
      let mut collection = collection.lock().await;
      for diagnostic_record in diagnostics {
        collection.set(DiagnosticSource::TypeScript, diagnostic_record);
      }
    }
    publish_diagnostics(client, collection, &snapshot).await;
    snapshot.performance.measure(mark);
  };

  let deps = async {
    let mark = snapshot
      .performance
      .mark("update_diagnostics_deps", None::<()>);
    let collection = collection.clone();
    let diagnostics =
      generate_deps_diagnostics(snapshot.clone(), collection.clone())
        .await
        .map_err(|err| {
          error!("Error generating Deno diagnostics: {}", err);
        })
        .unwrap_or_default();
    {
      let mut collection = collection.lock().await;
      for diagnostic_record in diagnostics {
        collection.set(DiagnosticSource::Deno, diagnostic_record);
      }
    }
    publish_diagnostics(client, collection, &snapshot).await;
    snapshot.performance.measure(mark);
  };

  tokio::join!(lint, ts, deps);
  snapshot.performance.measure(mark);
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
    let config = ConfigSnapshot {
      settings: Settings {
        workspace: WorkspaceSettings {
          enable: true,
          lint: true,
          ..Default::default()
        },
        ..Default::default()
      },
      ..Default::default()
    };
    StateSnapshot {
      config,
      documents,
      ..Default::default()
    }
  }

  fn setup(
    sources: &[(&str, &str, i32, LanguageId)],
  ) -> (StateSnapshot, Arc<Mutex<DiagnosticCollection>>, PathBuf) {
    let temp_dir = TempDir::new().expect("could not create temp dir");
    let location = temp_dir.path().join("deps");
    let state_snapshot = mock_state_snapshot(sources, &location);
    let collection = Arc::new(Mutex::new(DiagnosticCollection::default()));
    (state_snapshot, collection, location)
  }

  #[tokio::test]
  async fn test_generate_lint_diagnostics() {
    let (snapshot, collection, _) = setup(&[(
      "file:///a.ts",
      r#"import * as b from "./b.ts";

let a = "a";
console.log(a);
"#,
      1,
      LanguageId::TypeScript,
    )]);
    let result = generate_lint_diagnostics(&snapshot, collection).await;
    assert!(result.is_ok());
    let diagnostics = result.unwrap();
    assert_eq!(diagnostics.len(), 1);
    let (_, _, diagnostics) = &diagnostics[0];
    assert_eq!(diagnostics.len(), 2);
  }
}
