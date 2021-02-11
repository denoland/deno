// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis::get_lint_references;
use super::analysis::references_to_diagnostics;
use super::analysis::ResolvedDependency;
use super::language_server::StateSnapshot;
use super::tsc;

use crate::diagnostics;
use crate::media_type::MediaType;

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use lspower::lsp;
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

pub async fn generate_lint_diagnostics(
  state_snapshot: StateSnapshot,
  diagnostic_collection: DiagnosticCollection,
) -> DiagnosticVec {
  tokio::task::spawn_blocking(move || {
    let mut diagnostic_list = Vec::new();

    for specifier in state_snapshot.documents.open_specifiers() {
      let version = state_snapshot.documents.version(specifier);
      let current_version = diagnostic_collection.get_version(specifier);
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

pub async fn generate_ts_diagnostics(
  state_snapshot: StateSnapshot,
  diagnostic_collection: DiagnosticCollection,
  ts_server: &tsc::TsServer,
) -> Result<DiagnosticVec, AnyError> {
  let mut diagnostics = Vec::new();
  let mut specifiers = Vec::new();
  for specifier in state_snapshot.documents.open_specifiers() {
    let version = state_snapshot.documents.version(specifier);
    let current_version = diagnostic_collection.get_version(specifier);
    if version != current_version {
      specifiers.push(specifier.clone());
    }
  }
  if !specifiers.is_empty() {
    let req = tsc::RequestMethod::GetDiagnostics(specifiers);
    let res = ts_server.request(state_snapshot.clone(), req).await?;
    let ts_diagnostic_map: TsDiagnostics = serde_json::from_value(res)?;
    for (specifier_str, ts_diagnostics) in ts_diagnostic_map.iter() {
      let specifier = ModuleSpecifier::resolve_url(specifier_str)?;
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

pub async fn generate_dependency_diagnostics(
  mut state_snapshot: StateSnapshot,
  diagnostic_collection: DiagnosticCollection,
) -> Result<DiagnosticVec, AnyError> {
  tokio::task::spawn_blocking(move || {
    let mut diagnostics = Vec::new();

    let sources = &mut state_snapshot.sources;
    for specifier in state_snapshot.documents.open_specifiers() {
      let version = state_snapshot.documents.version(specifier);
      let current_version = diagnostic_collection.get_version(specifier);
      if version != current_version {
        let mut diagnostic_list = Vec::new();
        if let Some(dependencies) = state_snapshot.documents.dependencies(specifier) {
          for (_, dependency) in dependencies.iter() {
            if let (Some(code), Some(range)) = (
              &dependency.maybe_code,
              &dependency.maybe_code_specifier_range,
            ) {
              match code.clone() {
                ResolvedDependency::Err(err) => {
                  diagnostic_list.push(lsp::Diagnostic {
                    range: *range,
                    severity: Some(lsp::DiagnosticSeverity::Error),
                    code: None,
                    code_description: None,
                    source: Some("deno".to_string()),
                    message: format!("{}", err),
                    related_information: None,
                    tags: None,
                    data: None,
                  })
                }
                ResolvedDependency::Resolved(specifier) => {
                  if !(state_snapshot.documents.contains(&specifier) || sources.contains(&specifier)) {
                    let is_local = specifier.as_url().scheme() == "file";
                    diagnostic_list.push(lsp::Diagnostic {
                      range: *range,
                      severity: Some(lsp::DiagnosticSeverity::Error),
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
        diagnostics.push((specifier.clone(), version, diagnostic_list))
      }
    }

    Ok(diagnostics)
  })
  .await
  .unwrap()
}
