// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::analysis::get_lint_references;
use super::analysis::references_to_diagnostics;
use super::memory_cache::FileId;
use super::state::ServerStateSnapshot;
use super::tsc;

use crate::diagnostics;
use crate::media_type::MediaType;

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::JsRuntime;
use std::collections::HashMap;
use std::collections::HashSet;
use std::mem;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum DiagnosticSource {
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

  pub fn take_changes(&mut self) -> Option<HashSet<FileId>> {
    if self.changes.is_empty() {
      return None;
    }
    Some(mem::take(&mut self.changes))
  }
}

pub type DiagnosticVec = Vec<(FileId, Option<i32>, Vec<lsp_types::Diagnostic>)>;

pub fn generate_linting_diagnostics(
  state: &ServerStateSnapshot,
) -> DiagnosticVec {
  if !state.config.settings.lint {
    return Vec::new();
  }
  let mut diagnostics = Vec::new();
  let file_cache = state.file_cache.read().unwrap();
  for (specifier, doc_data) in state.doc_data.iter() {
    let file_id = file_cache.lookup(specifier).unwrap();
    let version = doc_data.version;
    let current_version = state.diagnostics.get_version(&file_id);
    if version != current_version {
      let media_type = MediaType::from(specifier);
      if let Ok(source_code) = file_cache.get_contents(file_id) {
        if let Ok(references) =
          get_lint_references(specifier, &media_type, &source_code)
        {
          if !references.is_empty() {
            diagnostics.push((
              file_id,
              version,
              references_to_diagnostics(references),
            ));
          } else {
            diagnostics.push((file_id, version, Vec::new()));
          }
        }
      } else {
        error!("Missing file contents for: {}", specifier);
      }
    }
  }

  diagnostics
}

type TsDiagnostics = Vec<diagnostics::Diagnostic>;

fn to_lsp_range(
  start: &diagnostics::Position,
  end: &diagnostics::Position,
) -> lsp_types::Range {
  lsp_types::Range {
    start: lsp_types::Position {
      line: start.line as u32,
      character: start.character as u32,
    },
    end: lsp_types::Position {
      line: end.line as u32,
      character: end.character as u32,
    },
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
            let uri = Url::parse(&source).unwrap();
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
            severity: Some(match d.category {
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
            }),
            code: Some(lsp_types::NumberOrString::Number(d.code as i32)),
            code_description: None,
            source: Some("deno-ts".to_string()),
            message: get_diagnostic_message(d),
            related_information: to_lsp_related_information(
              &d.related_information,
            ),
            tags: match d.code {
              // 6133 is unused variable, which with this tag gets displayed
              // faded out
              6133 => Some(vec![lsp_types::DiagnosticTag::Unnecessary]),
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

pub fn generate_ts_diagnostics(
  state: &ServerStateSnapshot,
  runtime: &mut JsRuntime,
) -> Result<DiagnosticVec, AnyError> {
  if !state.config.settings.enable {
    return Ok(Vec::new());
  }
  let mut diagnostics = Vec::new();
  let file_cache = state.file_cache.read().unwrap();
  for (specifier, doc_data) in state.doc_data.iter() {
    let file_id = file_cache.lookup(specifier).unwrap();
    let version = doc_data.version;
    let current_version = state.diagnostics.get_version(&file_id);
    if version != current_version {
      // TODO(@kitsonk): consider refactoring to get all diagnostics in one shot
      // for a file.
      let request_semantic_diagnostics =
        tsc::RequestMethod::GetSemanticDiagnostics(specifier.clone());
      let mut ts_diagnostics = ts_json_to_diagnostics(tsc::request(
        runtime,
        state,
        request_semantic_diagnostics,
      )?)?;
      let request_suggestion_diagnostics =
        tsc::RequestMethod::GetSuggestionDiagnostics(specifier.clone());
      ts_diagnostics.append(&mut ts_json_to_diagnostics(tsc::request(
        runtime,
        state,
        request_suggestion_diagnostics,
      )?)?);
      let request_syntactic_diagnostics =
        tsc::RequestMethod::GetSyntacticDiagnostics(specifier.clone());
      ts_diagnostics.append(&mut ts_json_to_diagnostics(tsc::request(
        runtime,
        state,
        request_syntactic_diagnostics,
      )?)?);
      if !ts_diagnostics.is_empty() {
        diagnostics.push((file_id, version, ts_diagnostics));
      } else {
        diagnostics.push((file_id, version, Vec::new()));
      }
    }
  }

  Ok(diagnostics)
}
