// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::analysis::get_lint_references;
use super::analysis::references_to_diagnostics;
use super::analysis::ResolvedDependency;
use super::language_server::StateSnapshot;
use super::memory_cache::FileId;
use super::tsc;

use crate::diagnostics;
use crate::media_type::MediaType;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use lspower::lsp_types;
use std::collections::HashMap;
use std::collections::HashSet;
use std::mem;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum DiagnosticSource {
  Deno,
  Lint,
  TypeScript,
}

#[derive(Debug, Default, Clone)]
pub struct DiagnosticCollection {
  map: HashMap<(FileId, DiagnosticSource), Vec<lsp_types::Diagnostic>>,
  versions: HashMap<FileId, i32>,
  changes: HashSet<FileId>,
}

impl DiagnosticCollection {
  pub fn set(
    &mut self,
    file_id: FileId,
    source: DiagnosticSource,
    version: Option<i32>,
    diagnostics: Vec<lsp_types::Diagnostic>,
  ) {
    self.map.insert((file_id, source), diagnostics);
    if let Some(version) = version {
      self.versions.insert(file_id, version);
    }
    self.changes.insert(file_id);
  }

  pub fn diagnostics_for(
    &self,
    file_id: FileId,
    source: DiagnosticSource,
  ) -> impl Iterator<Item = &lsp_types::Diagnostic> {
    self.map.get(&(file_id, source)).into_iter().flatten()
  }

  pub fn get_version(&self, file_id: &FileId) -> Option<i32> {
    self.versions.get(file_id).cloned()
  }

  pub fn invalidate(&mut self, file_id: &FileId) {
    self.versions.remove(file_id);
  }

  pub fn take_changes(&mut self) -> Option<HashSet<FileId>> {
    if self.changes.is_empty() {
      return None;
    }
    Some(mem::take(&mut self.changes))
  }
}

pub type DiagnosticVec = Vec<(FileId, Option<i32>, Vec<lsp_types::Diagnostic>)>;

pub async fn generate_lint_diagnostics(
  state_snapshot: StateSnapshot,
  diagnostic_collection: DiagnosticCollection,
) -> DiagnosticVec {
  tokio::task::spawn_blocking(move || {
    let mut diagnostic_list = Vec::new();

    let file_cache = state_snapshot.file_cache.lock().unwrap();
    for (specifier, doc_data) in state_snapshot.doc_data.iter() {
      let file_id = file_cache.lookup(specifier).unwrap();
      let version = doc_data.version;
      let current_version = diagnostic_collection.get_version(&file_id);
      if version != current_version {
        let media_type = MediaType::from(specifier);
        if let Ok(source_code) = file_cache.get_contents(file_id) {
          if let Ok(references) =
            get_lint_references(specifier, &media_type, &source_code)
          {
            if !references.is_empty() {
              diagnostic_list.push((
                file_id,
                version,
                references_to_diagnostics(references),
              ));
            } else {
              diagnostic_list.push((file_id, version, Vec::new()));
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

impl<'a> From<&'a diagnostics::DiagnosticCategory>
  for lsp_types::DiagnosticSeverity
{
  fn from(category: &'a diagnostics::DiagnosticCategory) -> Self {
    match category {
      diagnostics::DiagnosticCategory::Error => {
        lsp_types::DiagnosticSeverity::Error
      }
      diagnostics::DiagnosticCategory::Warning => {
        lsp_types::DiagnosticSeverity::Warning
      }
      diagnostics::DiagnosticCategory::Suggestion => {
        lsp_types::DiagnosticSeverity::Hint
      }
      diagnostics::DiagnosticCategory::Message => {
        lsp_types::DiagnosticSeverity::Information
      }
    }
  }
}

impl<'a> From<&'a diagnostics::Position> for lsp_types::Position {
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
) -> lsp_types::Range {
  lsp_types::Range {
    start: start.into(),
    end: end.into(),
  }
}

type TsDiagnostics = Vec<diagnostics::Diagnostic>;

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
) -> Option<Vec<lsp_types::DiagnosticRelatedInformation>> {
  if let Some(related) = related_information {
    Some(
      related
        .iter()
        .filter_map(|ri| {
          if let (Some(source), Some(start), Some(end)) =
            (&ri.source, &ri.start, &ri.end)
          {
            let uri = lsp_types::Url::parse(&source).unwrap();
            Some(lsp_types::DiagnosticRelatedInformation {
              location: lsp_types::Location {
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
  value: Value,
) -> Result<Vec<lsp_types::Diagnostic>, AnyError> {
  let ts_diagnostics: TsDiagnostics = serde_json::from_value(value)?;
  Ok(
    ts_diagnostics
      .iter()
      .filter_map(|d| {
        if let (Some(start), Some(end)) = (&d.start, &d.end) {
          Some(lsp_types::Diagnostic {
            range: to_lsp_range(start, end),
            severity: Some((&d.category).into()),
            code: Some(lsp_types::NumberOrString::Number(d.code as i32)),
            code_description: None,
            source: Some("deno-ts".to_string()),
            message: get_diagnostic_message(d),
            related_information: to_lsp_related_information(
              &d.related_information,
            ),
            tags: match d.code {
              // These are codes that indicate the variable is unused.
              6133 | 6192 | 6196 => {
                Some(vec![lsp_types::DiagnosticTag::Unnecessary])
              }
              _ => None,
            },
            data: None,
          })
        } else {
          None
        }
      })
      .collect(),
  )
}

pub async fn generate_ts_diagnostics(
  ts_server: &tsc::TsServer,
  diagnostic_collection: &DiagnosticCollection,
  state_snapshot: StateSnapshot,
) -> Result<DiagnosticVec, AnyError> {
  let mut diagnostics = Vec::new();
  let state_snapshot_ = state_snapshot.clone();
  for (specifier, doc_data) in state_snapshot_.doc_data.iter() {
    let file_id = {
      // TODO(lucacasonato): this is highly inefficient
      let file_cache = state_snapshot_.file_cache.lock().unwrap();
      file_cache.lookup(specifier).unwrap()
    };
    let version = doc_data.version;
    let current_version = diagnostic_collection.get_version(&file_id);
    if version != current_version {
      let req = tsc::RequestMethod::GetDiagnostics(specifier.clone());
      let ts_diagnostics = ts_json_to_diagnostics(
        ts_server.request(state_snapshot.clone(), req).await?,
      )?;
      diagnostics.push((file_id, version, ts_diagnostics));
    }
  }

  Ok(diagnostics)
}

pub async fn generate_dependency_diagnostics(
  state_snapshot: StateSnapshot,
  diagnostic_collection: DiagnosticCollection,
) -> Result<DiagnosticVec, AnyError> {
  tokio::task::spawn_blocking(move || {
    let mut diagnostics = Vec::new();

    let file_cache = state_snapshot.file_cache.lock().unwrap();
    let mut sources = if let Ok(sources) = state_snapshot.sources.lock() {
      sources
    } else {
      return Err(custom_error("Deadlock", "deadlock locking sources"));
    };
    for (specifier, doc_data) in state_snapshot.doc_data.iter() {
      let file_id = file_cache.lookup(specifier).unwrap();
      let version = doc_data.version;
      let current_version = diagnostic_collection.get_version(&file_id);
      if version != current_version {
        let mut diagnostic_list = Vec::new();
        if let Some(dependencies) = &doc_data.dependencies {
          for (_, dependency) in dependencies.iter() {
            if let (Some(code), Some(range)) = (
              &dependency.maybe_code,
              &dependency.maybe_code_specifier_range,
            ) {
              match code.clone() {
                ResolvedDependency::Err(message) => {
                  diagnostic_list.push(lsp_types::Diagnostic {
                    range: *range,
                    severity: Some(lsp_types::DiagnosticSeverity::Error),
                    code: None,
                    code_description: None,
                    source: Some("deno".to_string()),
                    message,
                    related_information: None,
                    tags: None,
                    data: None,
                  })
                }
                ResolvedDependency::Resolved(specifier) => {
                  if !(state_snapshot.doc_data.contains_key(&specifier) || sources.contains(&specifier)) {
                    let is_local = specifier.as_url().scheme() == "file";
                    diagnostic_list.push(lsp_types::Diagnostic {
                      range: *range,
                      severity: Some(lsp_types::DiagnosticSeverity::Error),
                      code: None,
                      code_description: None,
                      source: Some("deno".to_string()),
                      message: if is_local {
                        format!("Unable to load a local module: \"{}\".\n  Please check the file path.", specifier)
                      } else {
                        format!("Unable to load the module: \"{}\".\n  If the module exists, running `deno cache {}` should resolve this error.", specifier, specifier)
                      },
                      related_information: None,
                      tags: None,
                      data: None,
                    })
                  }
                },
              }
            }
          }
        }
        diagnostics.push((file_id, version, diagnostic_list))
      }
    }

    Ok(diagnostics)
  })
  .await
  .unwrap()
}
