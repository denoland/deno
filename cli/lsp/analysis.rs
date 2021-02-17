// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::language_server;
use super::tsc;

use crate::ast;
use crate::import_map::ImportMap;
use crate::media_type::MediaType;
use crate::module_graph::parse_deno_types;
use crate::module_graph::parse_ts_reference;
use crate::module_graph::TypeScriptReference;
use crate::tools::lint::create_linter;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleResolutionError;
use deno_core::ModuleSpecifier;
use deno_lint::rules;
use lspower::lsp;
use lspower::lsp::Position;
use lspower::lsp::Range;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

lazy_static! {
  /// Diagnostic error codes which actually are the same, and so when grouping
  /// fixes we treat them the same.
  static ref FIX_ALL_ERROR_CODES: HashMap<&'static str, &'static str> =
    (&[("2339", "2339"), ("2345", "2339"),])
      .iter()
      .cloned()
      .collect();

  /// Fixes which help determine if there is a preferred fix when there are
  /// multiple fixes available.
  static ref PREFERRED_FIXES: HashMap<&'static str, (u32, bool)> = (&[
    ("annotateWithTypeFromJSDoc", (1, false)),
    ("constructorForDerivedNeedSuperCall", (1, false)),
    ("extendsInterfaceBecomesImplements", (1, false)),
    ("awaitInSyncFunction", (1, false)),
    ("classIncorrectlyImplementsInterface", (3, false)),
    ("classDoesntImplementInheritedAbstractMember", (3, false)),
    ("unreachableCode", (1, false)),
    ("unusedIdentifier", (1, false)),
    ("forgottenThisPropertyAccess", (1, false)),
    ("spelling", (2, false)),
    ("addMissingAwait", (1, false)),
    ("fixImport", (0, true)),
  ])
  .iter()
  .cloned()
  .collect();
}

/// Category of self-generated diagnostic messages (those not coming from)
/// TypeScript.
pub enum Category {
  /// A lint diagnostic, where the first element is the message.
  Lint {
    message: String,
    code: String,
    hint: Option<String>,
  },
}

/// A structure to hold a reference to a diagnostic message.
pub struct Reference {
  category: Category,
  range: Range,
}

fn as_lsp_range(range: &deno_lint::diagnostic::Range) -> Range {
  Range {
    start: Position {
      line: (range.start.line - 1) as u32,
      character: range.start.col as u32,
    },
    end: Position {
      line: (range.end.line - 1) as u32,
      character: range.end.col as u32,
    },
  }
}

pub fn get_lint_references(
  specifier: &ModuleSpecifier,
  media_type: &MediaType,
  source_code: &str,
) -> Result<Vec<Reference>, AnyError> {
  let syntax = ast::get_syntax(media_type);
  let lint_rules = rules::get_recommended_rules();
  let mut linter = create_linter(syntax, lint_rules);
  // TODO(@kitsonk) we should consider caching the swc source file versions for
  // reuse by other processes
  let (_, lint_diagnostics) =
    linter.lint(specifier.to_string(), source_code.to_string())?;

  Ok(
    lint_diagnostics
      .into_iter()
      .map(|d| Reference {
        category: Category::Lint {
          message: d.message,
          code: d.code,
          hint: d.hint,
        },
        range: as_lsp_range(&d.range),
      })
      .collect(),
  )
}

pub fn references_to_diagnostics(
  references: Vec<Reference>,
) -> Vec<lsp::Diagnostic> {
  references
    .into_iter()
    .map(|r| match r.category {
      Category::Lint { message, code, .. } => lsp::Diagnostic {
        range: r.range,
        severity: Some(lsp::DiagnosticSeverity::Warning),
        code: Some(lsp::NumberOrString::String(code)),
        code_description: None,
        source: Some("deno-lint".to_string()),
        message,
        related_information: None,
        tags: None, // we should tag unused code
        data: None,
      },
    })
    .collect()
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Dependency {
  pub is_dynamic: bool,
  pub maybe_code: Option<ResolvedDependency>,
  pub maybe_code_specifier_range: Option<Range>,
  pub maybe_type: Option<ResolvedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedDependencyErr {
  InvalidDowngrade,
  InvalidLocalImport,
  InvalidSpecifier(ModuleResolutionError),
  Missing,
}

impl ResolvedDependencyErr {
  pub fn as_code(&self) -> lsp::NumberOrString {
    match self {
      Self::InvalidDowngrade => {
        lsp::NumberOrString::String("invalid-downgrade".to_string())
      }
      Self::InvalidLocalImport => {
        lsp::NumberOrString::String("invalid-local-import".to_string())
      }
      Self::InvalidSpecifier(_) => {
        lsp::NumberOrString::String("invalid-specifier".to_string())
      }
      Self::Missing => lsp::NumberOrString::String("missing".to_string()),
    }
  }
}

impl fmt::Display for ResolvedDependencyErr {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Self::InvalidDowngrade => {
        write!(f, "HTTPS modules cannot import HTTP modules.")
      }
      Self::InvalidLocalImport => {
        write!(f, "Remote modules cannot import local modules.")
      }
      Self::InvalidSpecifier(err) => write!(f, "{}", err),
      Self::Missing => write!(f, "The module is unexpectedly missing."),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedDependency {
  Resolved(ModuleSpecifier),
  Err(ResolvedDependencyErr),
}

pub fn resolve_import(
  specifier: &str,
  referrer: &ModuleSpecifier,
  maybe_import_map: &Option<ImportMap>,
) -> ResolvedDependency {
  let maybe_mapped = if let Some(import_map) = maybe_import_map {
    if let Ok(maybe_specifier) =
      import_map.resolve(specifier, referrer.as_str())
    {
      maybe_specifier
    } else {
      None
    }
  } else {
    None
  };
  let remapped = maybe_mapped.is_some();
  let specifier = if let Some(remapped) = maybe_mapped {
    remapped
  } else {
    match deno_core::resolve_import(specifier, referrer.as_str()) {
      Ok(resolved) => resolved,
      Err(err) => {
        return ResolvedDependency::Err(
          ResolvedDependencyErr::InvalidSpecifier(err),
        )
      }
    }
  };
  let referrer_scheme = referrer.scheme();
  let specifier_scheme = specifier.scheme();
  if referrer_scheme == "https" && specifier_scheme == "http" {
    return ResolvedDependency::Err(ResolvedDependencyErr::InvalidDowngrade);
  }
  if (referrer_scheme == "https" || referrer_scheme == "http")
    && !(specifier_scheme == "https" || specifier_scheme == "http")
    && !remapped
  {
    return ResolvedDependency::Err(ResolvedDependencyErr::InvalidLocalImport);
  }

  ResolvedDependency::Resolved(specifier)
}

// TODO(@kitsonk) a lot of this logic is duplicated in module_graph.rs in
// Module::parse() and should be refactored out to a common function.
pub fn analyze_dependencies(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: &MediaType,
  maybe_import_map: &Option<ImportMap>,
) -> Option<(HashMap<String, Dependency>, Option<ResolvedDependency>)> {
  let specifier_str = specifier.to_string();
  let source_map = Rc::new(swc_common::SourceMap::default());
  let mut maybe_type = None;
  if let Ok(parsed_module) =
    ast::parse_with_source_map(&specifier_str, source, &media_type, source_map)
  {
    let mut dependencies = HashMap::<String, Dependency>::new();

    // Parse leading comments for supported triple slash references.
    for comment in parsed_module.get_leading_comments().iter() {
      if let Some(ts_reference) = parse_ts_reference(&comment.text) {
        match ts_reference {
          TypeScriptReference::Path(import) => {
            let dep = dependencies.entry(import.clone()).or_default();
            let resolved_import =
              resolve_import(&import, specifier, maybe_import_map);
            dep.maybe_code = Some(resolved_import);
          }
          TypeScriptReference::Types(import) => {
            let resolved_import =
              resolve_import(&import, specifier, maybe_import_map);
            if media_type == &MediaType::JavaScript
              || media_type == &MediaType::JSX
            {
              maybe_type = Some(resolved_import)
            } else {
              let dep = dependencies.entry(import).or_default();
              dep.maybe_type = Some(resolved_import);
            }
          }
        }
      }
    }

    // Parse ES and type only imports
    let descriptors = parsed_module.analyze_dependencies();
    for desc in descriptors.into_iter().filter(|desc| {
      desc.kind != swc_ecmascript::dep_graph::DependencyKind::Require
    }) {
      let resolved_import =
        resolve_import(&desc.specifier, specifier, maybe_import_map);

      let maybe_resolved_type_dependency =
        // Check for `@deno-types` pragmas that affect the import
        if let Some(comment) = desc.leading_comments.last() {
          if let Some(deno_types) = parse_deno_types(&comment.text).as_ref() {
            Some(resolve_import(deno_types, specifier, maybe_import_map))
          } else {
            None
          }
        } else {
          None
        };

      let dep = dependencies.entry(desc.specifier.to_string()).or_default();
      dep.is_dynamic = desc.is_dynamic;
      match desc.kind {
        swc_ecmascript::dep_graph::DependencyKind::ExportType
        | swc_ecmascript::dep_graph::DependencyKind::ImportType => {
          dep.maybe_type = Some(resolved_import)
        }
        _ => {
          dep.maybe_code_specifier_range = Some(Range {
            start: Position {
              line: (desc.specifier_line - 1) as u32,
              character: desc.specifier_col as u32,
            },
            end: Position {
              line: (desc.specifier_line - 1) as u32,
              character: (desc.specifier_col
                + desc.specifier.chars().count()
                + 2) as u32,
            },
          });
          dep.maybe_code = Some(resolved_import);
        }
      }
      if maybe_resolved_type_dependency.is_some() && dep.maybe_type.is_none() {
        dep.maybe_type = maybe_resolved_type_dependency;
      }
    }

    Some((dependencies, maybe_type))
  } else {
    None
  }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum CodeLensSource {
  #[serde(rename = "implementations")]
  Implementations,
  #[serde(rename = "references")]
  References,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensData {
  pub source: CodeLensSource,
  pub specifier: ModuleSpecifier,
}

fn code_as_string(code: &Option<lsp::NumberOrString>) -> String {
  match code {
    Some(lsp::NumberOrString::String(str)) => str.clone(),
    Some(lsp::NumberOrString::Number(num)) => num.to_string(),
    _ => "".to_string(),
  }
}

/// Determines if two TypeScript diagnostic codes are effectively equivalent.
fn is_equivalent_code(
  a: &Option<lsp::NumberOrString>,
  b: &Option<lsp::NumberOrString>,
) -> bool {
  let a_code = code_as_string(a);
  let b_code = code_as_string(b);
  FIX_ALL_ERROR_CODES.get(a_code.as_str())
    == FIX_ALL_ERROR_CODES.get(b_code.as_str())
}

/// Return a boolean flag to indicate if the specified action is the preferred
/// action for a given set of actions.
fn is_preferred(
  action: &tsc::CodeFixAction,
  actions: &[CodeActionKind],
  fix_priority: u32,
  only_one: bool,
) -> bool {
  actions.iter().all(|i| {
    if let CodeActionKind::Tsc(_, a) = i {
      if action == a {
        return true;
      }
      if a.fix_id.is_some() {
        return true;
      }
      if let Some((other_fix_priority, _)) =
        PREFERRED_FIXES.get(a.fix_name.as_str())
      {
        match other_fix_priority.cmp(&fix_priority) {
          Ordering::Less => return true,
          Ordering::Greater => return false,
          Ordering::Equal => (),
        }
        if only_one && action.fix_name == a.fix_name {
          return false;
        }
      }
      true
    } else {
      true
    }
  })
}

/// Convert changes returned from a TypeScript quick fix action into edits
/// for an LSP CodeAction.
pub(crate) async fn ts_changes_to_edit(
  changes: &[tsc::FileTextChanges],
  language_server: &mut language_server::Inner,
) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
  let mut text_document_edits = Vec::new();
  for change in changes {
    let text_document_edit =
      change.to_text_document_edit(language_server).await?;
    text_document_edits.push(text_document_edit);
  }
  Ok(Some(lsp::WorkspaceEdit {
    changes: None,
    document_changes: Some(lsp::DocumentChanges::Edits(text_document_edits)),
    change_annotations: None,
  }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionData {
  pub specifier: ModuleSpecifier,
  pub fix_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DenoFixData {
  pub specifier: ModuleSpecifier,
}

#[derive(Debug, Clone)]
enum CodeActionKind {
  Deno(lsp::CodeAction),
  Tsc(lsp::CodeAction, tsc::CodeFixAction),
}

#[derive(Debug, Hash, PartialEq, Eq)]
enum FixAllKind {
  Tsc(String),
}

#[derive(Debug, Default)]
pub struct CodeActionCollection {
  actions: Vec<CodeActionKind>,
  fix_all_actions: HashMap<FixAllKind, CodeActionKind>,
}

impl CodeActionCollection {
  pub(crate) fn add_deno_fix_action(
    &mut self,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<(), AnyError> {
    if let Some(data) = diagnostic.data.clone() {
      let fix_data: DenoFixData = serde_json::from_value(data)?;
      let code_action = lsp::CodeAction {
        title: format!(
          "Cache \"{}\" and its dependencies.",
          fix_data.specifier
        ),
        kind: Some(lsp::CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: None,
        command: Some(lsp::Command {
          title: "".to_string(),
          command: "deno.cache".to_string(),
          arguments: Some(vec![json!([fix_data.specifier])]),
        }),
        is_preferred: None,
        disabled: None,
        data: None,
      };
      self.actions.push(CodeActionKind::Deno(code_action));
    }
    Ok(())
  }

  /// Add a TypeScript code fix action to the code actions collection.
  pub(crate) async fn add_ts_fix_action(
    &mut self,
    action: &tsc::CodeFixAction,
    diagnostic: &lsp::Diagnostic,
    language_server: &mut language_server::Inner,
  ) -> Result<(), AnyError> {
    if action.commands.is_some() {
      // In theory, tsc can return actions that require "commands" to be applied
      // back into TypeScript.  Currently there is only one command, `install
      // package` but Deno doesn't support that.  The problem is that the
      // `.applyCodeActionCommand()` returns a promise, and with the current way
      // we wrap tsc, we can't handle the asynchronous response, so it is
      // actually easier to return errors if we ever encounter one of these,
      // which we really wouldn't expect from the Deno lsp.
      return Err(custom_error(
        "UnsupportedFix",
        "The action returned from TypeScript is unsupported.",
      ));
    }
    let edit = ts_changes_to_edit(&action.changes, language_server).await?;
    let code_action = lsp::CodeAction {
      title: action.description.clone(),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      edit,
      command: None,
      is_preferred: None,
      disabled: None,
      data: None,
    };
    self.actions.retain(|i| match i {
      CodeActionKind::Tsc(c, a) => {
        !(action.fix_name == a.fix_name && code_action.edit == c.edit)
      }
      _ => true,
    });
    self
      .actions
      .push(CodeActionKind::Tsc(code_action, action.clone()));

    if let Some(fix_id) = &action.fix_id {
      if let Some(CodeActionKind::Tsc(existing_fix_all, existing_action)) =
        self.fix_all_actions.get(&FixAllKind::Tsc(fix_id.clone()))
      {
        self.actions.retain(|i| match i {
          CodeActionKind::Tsc(c, _) => c != existing_fix_all,
          _ => true,
        });
        self.actions.push(CodeActionKind::Tsc(
          existing_fix_all.clone(),
          existing_action.clone(),
        ));
      }
    }
    Ok(())
  }

  /// Add a TypeScript action to the actions as a "fix all" action, where it
  /// will fix all occurrences of the diagnostic in the file.
  pub fn add_ts_fix_all_action(
    &mut self,
    action: &tsc::CodeFixAction,
    specifier: &ModuleSpecifier,
    diagnostic: &lsp::Diagnostic,
  ) {
    let data = Some(json!({
      "specifier": specifier,
      "fixId": action.fix_id,
    }));
    let title = if let Some(description) = &action.fix_all_description {
      description.clone()
    } else {
      format!("{} (Fix all in file)", action.description)
    };

    let code_action = lsp::CodeAction {
      title,
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      edit: None,
      command: None,
      is_preferred: None,
      disabled: None,
      data,
    };
    if let Some(CodeActionKind::Tsc(existing, _)) = self
      .fix_all_actions
      .get(&FixAllKind::Tsc(action.fix_id.clone().unwrap()))
    {
      self.actions.retain(|i| match i {
        CodeActionKind::Tsc(c, _) => c != existing,
        _ => true,
      });
    }
    self
      .actions
      .push(CodeActionKind::Tsc(code_action.clone(), action.clone()));
    self.fix_all_actions.insert(
      FixAllKind::Tsc(action.fix_id.clone().unwrap()),
      CodeActionKind::Tsc(code_action, action.clone()),
    );
  }

  /// Move out the code actions and return them as a `CodeActionResponse`.
  pub fn get_response(self) -> lsp::CodeActionResponse {
    self
      .actions
      .into_iter()
      .map(|i| match i {
        CodeActionKind::Tsc(c, _) => lsp::CodeActionOrCommand::CodeAction(c),
        CodeActionKind::Deno(c) => lsp::CodeActionOrCommand::CodeAction(c),
      })
      .collect()
  }

  /// Determine if a action can be converted into a "fix all" action.
  pub fn is_fix_all_action(
    &self,
    action: &tsc::CodeFixAction,
    diagnostic: &lsp::Diagnostic,
    file_diagnostics: &[lsp::Diagnostic],
  ) -> bool {
    // If the action does not have a fix id (indicating it can be "bundled up")
    // or if the collection already contains a "bundled" action return false
    if action.fix_id.is_none()
      || self
        .fix_all_actions
        .contains_key(&FixAllKind::Tsc(action.fix_id.clone().unwrap()))
    {
      false
    } else {
      // else iterate over the diagnostic in the file and see if there are any
      // other diagnostics that could be bundled together in a "fix all" code
      // action
      file_diagnostics.iter().any(|d| {
        if d == diagnostic || d.code.is_none() || diagnostic.code.is_none() {
          false
        } else {
          d.code == diagnostic.code
            || is_equivalent_code(&d.code, &diagnostic.code)
        }
      })
    }
  }

  /// Set the `.is_preferred` flag on code actions, this should be only executed
  /// when all actions are added to the collection.
  pub fn set_preferred_fixes(&mut self) {
    let actions = self.actions.clone();
    for entry in self.actions.iter_mut() {
      if let CodeActionKind::Tsc(code_action, action) = entry {
        if action.fix_id.is_some() {
          continue;
        }
        if let Some((fix_priority, only_one)) =
          PREFERRED_FIXES.get(action.fix_name.as_str())
        {
          code_action.is_preferred =
            Some(is_preferred(action, &actions, *fix_priority, *only_one));
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url;

  #[test]
  fn test_as_lsp_range() {
    let fixture = deno_lint::diagnostic::Range {
      start: deno_lint::diagnostic::Position {
        line: 1,
        col: 2,
        byte_pos: 23,
      },
      end: deno_lint::diagnostic::Position {
        line: 2,
        col: 0,
        byte_pos: 33,
      },
    };
    let actual = as_lsp_range(&fixture);
    assert_eq!(
      actual,
      lsp::Range {
        start: lsp::Position {
          line: 0,
          character: 2,
        },
        end: lsp::Position {
          line: 1,
          character: 0,
        },
      }
    );
  }

  #[test]
  fn test_analyze_dependencies() {
    let specifier = resolve_url("file:///a.ts").expect("bad specifier");
    let source = r#"import {
      Application,
      Context,
      Router,
      Status,
    } from "https://deno.land/x/oak@v6.3.2/mod.ts";

    // @deno-types="https://deno.land/x/types/react/index.d.ts";
    import * as React from "https://cdn.skypack.dev/react";
    "#;
    let actual =
      analyze_dependencies(&specifier, source, &MediaType::TypeScript, &None);
    assert!(actual.is_some());
    let (actual, maybe_type) = actual.unwrap();
    assert!(maybe_type.is_none());
    assert_eq!(actual.len(), 2);
    assert_eq!(
      actual.get("https://cdn.skypack.dev/react").cloned(),
      Some(Dependency {
        is_dynamic: false,
        maybe_code: Some(ResolvedDependency::Resolved(
          resolve_url("https://cdn.skypack.dev/react").unwrap()
        )),
        maybe_type: Some(ResolvedDependency::Resolved(
          resolve_url("https://deno.land/x/types/react/index.d.ts").unwrap()
        )),
        maybe_code_specifier_range: Some(Range {
          start: Position {
            line: 8,
            character: 27,
          },
          end: Position {
            line: 8,
            character: 58,
          }
        }),
      })
    );
    assert_eq!(
      actual.get("https://deno.land/x/oak@v6.3.2/mod.ts").cloned(),
      Some(Dependency {
        is_dynamic: false,
        maybe_code: Some(ResolvedDependency::Resolved(
          resolve_url("https://deno.land/x/oak@v6.3.2/mod.ts").unwrap()
        )),
        maybe_type: None,
        maybe_code_specifier_range: Some(Range {
          start: Position {
            line: 5,
            character: 11,
          },
          end: Position {
            line: 5,
            character: 50,
          }
        }),
      })
    );
  }
}
