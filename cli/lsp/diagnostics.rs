// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis::get_lint_references;
use super::analysis::references_to_diagnostics;
use super::analysis::ResolvedDependency;
use super::language_server;
use super::performance::Performance;
use super::tsc;

use crate::diagnostics;
use crate::media_type::MediaType;
use crate::tokio_util::create_basic_runtime;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use lspower::Client;
use std::collections::HashMap;
use std::collections::HashSet;
use std::mem;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time;

// 150ms between keystrokes is about 45 WPM.
const DIAGNOSTIC_DEBOUNCE_MS: u64 = 200;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum DiagnosticSource {
  Deno,
  Lint,
  TypeScript,
}

#[derive(Debug)]
enum DiagnosticRequest {
  Get(
    ModuleSpecifier,
    DiagnosticSource,
    oneshot::Sender<Vec<lsp::Diagnostic>>,
  ),
  Invalidate(ModuleSpecifier),
  Update,
}

/// Given a client and a diagnostics collection, publish the appropriate changes
/// to the client.
async fn publish_diagnostics(
  client: &Client,
  collection: &mut DiagnosticCollection,
  snapshot: &language_server::StateSnapshot,
  performance: &Performance,
) {
  let mark = performance.mark("publish_diagnostics");
  let maybe_changes = collection.take_changes();
  if let Some(diagnostic_changes) = maybe_changes {
    for specifier in diagnostic_changes {
      // TODO(@kitsonk) not totally happy with the way we collect and store
      // different types of diagnostics and offer them up to the client, we
      // do need to send "empty" vectors though when a particular feature is
      // disabled, otherwise the client will not clear down previous
      // diagnostics
      let mut diagnostics: Vec<lsp::Diagnostic> =
        if snapshot.config.settings.lint {
          collection
            .diagnostics_for(&specifier, &DiagnosticSource::Lint)
            .cloned()
            .collect()
        } else {
          vec![]
        };
      if snapshot.config.settings.enable {
        diagnostics.extend(
          collection
            .diagnostics_for(&specifier, &DiagnosticSource::TypeScript)
            .cloned(),
        );
        diagnostics.extend(
          collection
            .diagnostics_for(&specifier, &DiagnosticSource::Deno)
            .cloned(),
        );
      }
      let uri = specifier.clone();
      let version = snapshot.documents.version(&specifier);
      client.publish_diagnostics(uri, diagnostics, version).await;
    }
  }

  performance.measure(mark);
}

async fn update_diagnostics(
  client: &Client,
  collection: &mut DiagnosticCollection,
  snapshot: &language_server::StateSnapshot,
  ts_server: &tsc::TsServer,
  performance: &Performance,
) {
  let (enabled, lint_enabled) = {
    let config = &snapshot.config;
    (config.settings.enable, config.settings.lint)
  };

  let mark = snapshot.performance.mark("update_diagnostics");
  let lint = async {
    let mut diagnostics = None;
    if lint_enabled {
      let mark = snapshot.performance.mark("prepare_diagnostics_lint");
      diagnostics = Some(
        generate_lint_diagnostics(snapshot.clone(), collection.clone()).await,
      );
      snapshot.performance.measure(mark);
    };
    Ok::<_, AnyError>(diagnostics)
  };

  let ts = async {
    let mut diagnostics = None;
    if enabled {
      let mark = snapshot.performance.mark("prepare_diagnostics_ts");
      diagnostics = Some(
        generate_ts_diagnostics(
          snapshot.clone(),
          collection.clone(),
          ts_server,
        )
        .await?,
      );
      snapshot.performance.measure(mark);
    };
    Ok::<_, AnyError>(diagnostics)
  };

  let deps = async {
    let mut diagnostics = None;
    if enabled {
      let mark = snapshot.performance.mark("prepare_diagnostics_deps");
      diagnostics = Some(
        generate_dependency_diagnostics(snapshot.clone(), collection.clone())
          .await?,
      );
      snapshot.performance.measure(mark);
    };
    Ok::<_, AnyError>(diagnostics)
  };

  let (lint_res, ts_res, deps_res) = tokio::join!(lint, ts, deps);
  let mut disturbed = false;

  match lint_res {
    Ok(Some(diagnostics)) => {
      for (specifier, version, diagnostics) in diagnostics {
        collection.set(specifier, DiagnosticSource::Lint, version, diagnostics);
        disturbed = true;
      }
    }
    Err(err) => {
      error!("Internal error: {}", err);
    }
    _ => (),
  }

  match ts_res {
    Ok(Some(diagnostics)) => {
      for (specifier, version, diagnostics) in diagnostics {
        collection.set(
          specifier,
          DiagnosticSource::TypeScript,
          version,
          diagnostics,
        );
        disturbed = true;
      }
    }
    Err(err) => {
      error!("Internal error: {}", err);
    }
    _ => (),
  }

  match deps_res {
    Ok(Some(diagnostics)) => {
      for (specifier, version, diagnostics) in diagnostics {
        collection.set(specifier, DiagnosticSource::Deno, version, diagnostics);
        disturbed = true;
      }
    }
    Err(err) => {
      error!("Internal error: {}", err);
    }
    _ => (),
  }
  performance.measure(mark);

  if disturbed {
    publish_diagnostics(client, collection, snapshot, performance).await
  }
}

fn handle_request(
  maybe_request: Option<DiagnosticRequest>,
  collection: &mut DiagnosticCollection,
  dirty: &mut bool,
) -> bool {
  match maybe_request {
    Some(request) => {
      match request {
        DiagnosticRequest::Get(specifier, source, tx) => {
          let diagnostics = collection
            .diagnostics_for(&specifier, &source)
            .cloned()
            .collect();
          if tx.send(diagnostics).is_err() {
            error!("DiagnosticServer unable to send response on channel.");
          }
        }
        DiagnosticRequest::Invalidate(specifier) => {
          collection.invalidate(&specifier)
        }
        DiagnosticRequest::Update => *dirty = true,
      }
      true
    }
    _ => false,
  }
}

/// A server which calculates diagnostics in its own thread and publishes them
/// to an LSP client.
#[derive(Debug)]
pub(crate) struct DiagnosticsServer(
  Option<mpsc::UnboundedSender<DiagnosticRequest>>,
);

impl DiagnosticsServer {
  pub(crate) fn new() -> Self {
    Self(None)
  }

  pub(crate) fn start(
    &mut self,
    language_server: Arc<tokio::sync::Mutex<language_server::Inner>>,
    client: Client,
    ts_server: Arc<tsc::TsServer>,
    performance: Performance,
  ) {
    let (tx, mut rx) = mpsc::unbounded_channel::<DiagnosticRequest>();
    self.0 = Some(tx);

    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();
      let mut collection = DiagnosticCollection::default();

      runtime.block_on(async {
        // Some(snapshot) is the flag we use to determine if something has
        // changed where we will wait for the timeout of changes or a request
        // that forces us to update diagnostics
        let mut dirty = false;

        loop {
          let next = rx.recv();
          tokio::pin!(next);

          let duration = if dirty {
            time::Duration::from_millis(DIAGNOSTIC_DEBOUNCE_MS)
          } else {
            // we need to await an arbitrary silly amount of time, so this is
            // 1 year in seconds
            time::Duration::new(31_622_400, 0)
          };
          let debounce = time::sleep(duration);
          tokio::pin!(debounce);

          // "race" the next message off the rx queue or the debounce timer.
          // if the next message comes off the queue, the next iteration of the
          // loop will reset the debounce future.  When the debounce future
          // occurs, the diagnostics will be updated based on only the most
          // recent snapshot, thereby "abandoning" any interim states.
          tokio::select! {
            _ = &mut debounce => {
              if dirty {
                dirty = false;
                let snapshot = language_server.lock().await.snapshot();
                update_diagnostics(
                  &client,
                  &mut collection,
                  &snapshot,
                  &ts_server,
                  &performance
                ).await;
              }
              let maybe_request = next.await;
              if !handle_request(maybe_request, &mut collection, &mut dirty) {
                break;
              }
            }
            maybe_request = &mut next => {
              if !handle_request(maybe_request, &mut collection, &mut dirty) {
                break;
              }
            }
          }
        }
      })
    });
  }

  pub async fn get(
    &self,
    specifier: ModuleSpecifier,
    source: DiagnosticSource,
  ) -> Result<Vec<lsp::Diagnostic>, AnyError> {
    let (tx, rx) = oneshot::channel::<Vec<lsp::Diagnostic>>();
    if let Some(self_tx) = &self.0 {
      self_tx.send(DiagnosticRequest::Get(specifier, source, tx))?;
      rx.await.map_err(|err| err.into())
    } else {
      Err(anyhow!("diagnostic server not started"))
    }
  }

  pub fn invalidate(&self, specifier: ModuleSpecifier) -> Result<(), AnyError> {
    if let Some(tx) = &self.0 {
      tx.send(DiagnosticRequest::Invalidate(specifier))
        .map_err(|err| err.into())
    } else {
      Err(anyhow!("diagnostic server not started"))
    }
  }

  pub fn update(&self) -> Result<(), AnyError> {
    if let Some(tx) = &self.0 {
      tx.send(DiagnosticRequest::Update).map_err(|err| err.into())
    } else {
      Err(anyhow!("diagnostic server not started"))
    }
  }
}

#[derive(Debug, Default, Clone)]
struct DiagnosticCollection {
  map: HashMap<(ModuleSpecifier, DiagnosticSource), Vec<lsp::Diagnostic>>,
  versions: HashMap<ModuleSpecifier, i32>,
  changes: HashSet<ModuleSpecifier>,
}

impl DiagnosticCollection {
  pub fn set(
    &mut self,
    specifier: ModuleSpecifier,
    source: DiagnosticSource,
    version: Option<i32>,
    diagnostics: Vec<lsp::Diagnostic>,
  ) {
    self.map.insert((specifier.clone(), source), diagnostics);
    if let Some(version) = version {
      self.versions.insert(specifier.clone(), version);
    }
    self.changes.insert(specifier);
  }

  pub fn diagnostics_for(
    &self,
    specifier: &ModuleSpecifier,
    source: &DiagnosticSource,
  ) -> impl Iterator<Item = &lsp::Diagnostic> {
    self
      .map
      .get(&(specifier.clone(), source.clone()))
      .into_iter()
      .flatten()
  }

  pub fn get_version(&self, specifier: &ModuleSpecifier) -> Option<i32> {
    self.versions.get(specifier).cloned()
  }

  pub fn invalidate(&mut self, specifier: &ModuleSpecifier) {
    self.versions.remove(specifier);
  }

  pub fn take_changes(&mut self) -> Option<HashSet<ModuleSpecifier>> {
    if self.changes.is_empty() {
      return None;
    }
    Some(mem::take(&mut self.changes))
  }
}

pub type DiagnosticVec =
  Vec<(ModuleSpecifier, Option<i32>, Vec<lsp::Diagnostic>)>;

async fn generate_lint_diagnostics(
  state_snapshot: language_server::StateSnapshot,
  collection: DiagnosticCollection,
) -> DiagnosticVec {
  tokio::task::spawn_blocking(move || {
    let mut diagnostic_list = Vec::new();

    for specifier in state_snapshot.documents.open_specifiers() {
      let version = state_snapshot.documents.version(specifier);
      let current_version = collection.get_version(specifier);
      if version != current_version {
        let media_type = MediaType::from(specifier);
        if let Ok(Some(source_code)) =
          state_snapshot.documents.content(specifier)
        {
          if let Ok(references) =
            get_lint_references(specifier, &media_type, &source_code)
          {
            if !references.is_empty() {
              diagnostic_list.push((
                specifier.clone(),
                version,
                references_to_diagnostics(references),
              ));
            } else {
              diagnostic_list.push((specifier.clone(), version, Vec::new()));
            }
          }
        } else {
          error!("Missing file contents for: {}", specifier);
        }
      }
    }

    diagnostic_list
  })
  .await
  .unwrap()
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

fn to_lsp_range(
  start: &diagnostics::Position,
  end: &diagnostics::Position,
) -> lsp::Range {
  lsp::Range {
    start: start.into(),
    end: end.into(),
  }
}

type TsDiagnostics = HashMap<String, Vec<diagnostics::Diagnostic>>;

fn get_diagnostic_message(diagnostic: &diagnostics::Diagnostic) -> String {
  if let Some(message) = diagnostic.message_text.clone() {
    message
  } else if let Some(message_chain) = diagnostic.message_chain.clone() {
    message_chain.format_message(0)
  } else {
    "[missing message]".to_string()
  }
}

fn to_lsp_related_information(
  related_information: &Option<Vec<diagnostics::Diagnostic>>,
) -> Option<Vec<lsp::DiagnosticRelatedInformation>> {
  if let Some(related) = related_information {
    Some(
      related
        .iter()
        .filter_map(|ri| {
          if let (Some(source), Some(start), Some(end)) =
            (&ri.source, &ri.start, &ri.end)
          {
            let uri = lsp::Url::parse(&source).unwrap();
            Some(lsp::DiagnosticRelatedInformation {
              location: lsp::Location {
                uri,
                range: to_lsp_range(start, end),
              },
              message: get_diagnostic_message(&ri),
            })
          } else {
            None
          }
        })
        .collect(),
    )
  } else {
    None
  }
}

fn ts_json_to_diagnostics(
  diagnostics: &[diagnostics::Diagnostic],
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
            2695 | 6133 | 6138 | 6192 | 6196 | 6198 | 6199 | 7027 | 7028 => {
              Some(vec![lsp::DiagnosticTag::Unnecessary])
            }
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

async fn generate_ts_diagnostics(
  state_snapshot: language_server::StateSnapshot,
  collection: DiagnosticCollection,
  ts_server: &tsc::TsServer,
) -> Result<DiagnosticVec, AnyError> {
  let mut diagnostics = Vec::new();
  let mut specifiers = Vec::new();
  for specifier in state_snapshot.documents.open_specifiers() {
    let version = state_snapshot.documents.version(specifier);
    let current_version = collection.get_version(specifier);
    if version != current_version {
      specifiers.push(specifier.clone());
    }
  }
  if !specifiers.is_empty() {
    let req = tsc::RequestMethod::GetDiagnostics(specifiers);
    let res = ts_server.request(state_snapshot.clone(), req).await?;
    let ts_diagnostic_map: TsDiagnostics = serde_json::from_value(res)?;
    for (specifier_str, ts_diagnostics) in ts_diagnostic_map.iter() {
      let specifier = deno_core::resolve_url(specifier_str)?;
      let version = state_snapshot.documents.version(&specifier);
      diagnostics.push((
        specifier,
        version,
        ts_json_to_diagnostics(ts_diagnostics),
      ));
    }
  }
  Ok(diagnostics)
}

async fn generate_dependency_diagnostics(
  mut state_snapshot: language_server::StateSnapshot,
  collection: DiagnosticCollection,
) -> Result<DiagnosticVec, AnyError> {
  tokio::task::spawn_blocking(move || {
    let mut diagnostics = Vec::new();

    let sources = &mut state_snapshot.sources;
    for specifier in state_snapshot.documents.open_specifiers() {
      let version = state_snapshot.documents.version(specifier);
      let current_version = collection.get_version(specifier);
      if version != current_version {
        let mut diagnostic_list = Vec::new();
        if let Some(dependencies) = state_snapshot.documents.dependencies(specifier) {
          for (_, dependency) in dependencies.iter() {
            if let (Some(code), Some(range)) = (
              &dependency.maybe_code,
              &dependency.maybe_code_specifier_range,
            ) {
              match code.clone() {
                ResolvedDependency::Err(dependency_err) => {
                  diagnostic_list.push(lsp::Diagnostic {
                    range: *range,
                    severity: Some(lsp::DiagnosticSeverity::Error),
                    code: Some(dependency_err.as_code()),
                    code_description: None,
                    source: Some("deno".to_string()),
                    message: format!("{}", dependency_err),
                    related_information: None,
                    tags: None,
                    data: None,
                  })
                }
                ResolvedDependency::Resolved(specifier) => {
                  if !(state_snapshot.documents.contains_key(&specifier) || sources.contains_key(&specifier)) {
                    let scheme = specifier.scheme();
                    let (code, message) = if scheme == "file" {
                      (Some(lsp::NumberOrString::String("no-local".to_string())), format!("Unable to load a local module: \"{}\".\n  Please check the file path.", specifier))
                    } else if scheme == "data" {
                      (Some(lsp::NumberOrString::String("no-cache-data".to_string())), "Uncached data URL.".to_string())
                    } else {
                      (Some(lsp::NumberOrString::String("no-cache".to_string())), format!("Unable to load the remote module: \"{}\".", specifier))
                    };
                    diagnostic_list.push(lsp::Diagnostic {
                      range: *range,
                      severity: Some(lsp::DiagnosticSeverity::Error),
                      code,
                      code_description: None,
                      source: Some("deno".to_string()),
                      message,
                      related_information: None,
                      tags: None,
                      data: Some(json!({
                        "specifier": specifier
                      })),
                    })
                  }
                },
              }
            }
          }
        }
        diagnostics.push((specifier.clone(), version, diagnostic_list))
      }
    }

    Ok(diagnostics)
  })
  .await
  .unwrap()
}
