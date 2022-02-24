// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::diagnostics::DenoDiagnostic;
use super::documents::Documents;
use super::language_server;
use super::tsc;

use crate::config_file::LintConfig;
use crate::tools::lint::create_linter;
use crate::tools::lint::get_configured_rules;

use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use lspower::lsp::Position;
use lspower::lsp::Range;
use once_cell::sync::Lazy;
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;

/// Diagnostic error codes which actually are the same, and so when grouping
/// fixes we treat them the same.
static FIX_ALL_ERROR_CODES: Lazy<HashMap<&'static str, &'static str>> =
  Lazy::new(|| {
    (&[("2339", "2339"), ("2345", "2339")])
      .iter()
      .cloned()
      .collect()
  });

/// Fixes which help determine if there is a preferred fix when there are
/// multiple fixes available.
static PREFERRED_FIXES: Lazy<HashMap<&'static str, (u32, bool)>> =
  Lazy::new(|| {
    (&[
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
      .collect()
  });

static IMPORT_SPECIFIER_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"\sfrom\s+["']([^"']*)["']"#).unwrap());

const SUPPORTED_EXTENSIONS: &[&str] = &[".ts", ".tsx", ".js", ".jsx", ".mjs"];

/// Category of self-generated diagnostic messages (those not coming from)
/// TypeScript.
#[derive(Debug, PartialEq, Eq)]
pub enum Category {
  /// A lint diagnostic, where the first element is the message.
  Lint {
    message: String,
    code: String,
    hint: Option<String>,
  },
}

/// A structure to hold a reference to a diagnostic message.
#[derive(Debug, PartialEq, Eq)]
pub struct Reference {
  category: Category,
  range: Range,
}

impl Reference {
  pub fn to_diagnostic(&self) -> lsp::Diagnostic {
    match &self.category {
      Category::Lint {
        message,
        code,
        hint,
      } => lsp::Diagnostic {
        range: self.range,
        severity: Some(lsp::DiagnosticSeverity::WARNING),
        code: Some(lsp::NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some("deno-lint".to_string()),
        message: {
          let mut msg = message.to_string();
          if let Some(hint) = hint {
            msg.push('\n');
            msg.push_str(hint);
          }
          msg
        },
        related_information: None,
        tags: None, // we should tag unused code
        data: None,
      },
    }
  }
}

fn as_lsp_range(range: &deno_lint::diagnostic::Range) -> Range {
  Range {
    start: Position {
      line: range.start.line_index as u32,
      character: range.start.column_index as u32,
    },
    end: Position {
      line: range.end.line_index as u32,
      character: range.end.column_index as u32,
    },
  }
}

pub fn get_lint_references(
  parsed_source: &deno_ast::ParsedSource,
  maybe_lint_config: Option<&LintConfig>,
) -> Result<Vec<Reference>, AnyError> {
  let lint_rules = get_configured_rules(maybe_lint_config, None, None, None)?;
  let linter = create_linter(parsed_source.media_type(), lint_rules);
  let lint_diagnostics = linter.lint_with_ast(parsed_source);

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

fn code_as_string(code: &Option<lsp::NumberOrString>) -> String {
  match code {
    Some(lsp::NumberOrString::String(str)) => str.clone(),
    Some(lsp::NumberOrString::Number(num)) => num.to_string(),
    _ => "".to_string(),
  }
}

/// Iterate over the supported extensions, concatenating the extension on the
/// specifier, returning the first specifier that is resolve-able, otherwise
/// None if none match.
fn check_specifier(
  specifier: &str,
  referrer: &ModuleSpecifier,
  documents: &Documents,
) -> Option<String> {
  for ext in SUPPORTED_EXTENSIONS {
    let specifier_with_ext = format!("{}{}", specifier, ext);
    if documents.contains_import(&specifier_with_ext, referrer) {
      return Some(specifier_with_ext);
    }
  }
  None
}

/// For a set of tsc changes, can them for any that contain something that looks
/// like an import and rewrite the import specifier to include the extension
pub(crate) fn fix_ts_import_changes(
  referrer: &ModuleSpecifier,
  changes: &[tsc::FileTextChanges],
  documents: &Documents,
) -> Result<Vec<tsc::FileTextChanges>, AnyError> {
  let mut r = Vec::new();
  for change in changes {
    let mut text_changes = Vec::new();
    for text_change in &change.text_changes {
      if let Some(captures) =
        IMPORT_SPECIFIER_RE.captures(&text_change.new_text)
      {
        let specifier = captures
          .get(1)
          .ok_or_else(|| anyhow!("Missing capture."))?
          .as_str();
        if let Some(new_specifier) =
          check_specifier(specifier, referrer, documents)
        {
          let new_text =
            text_change.new_text.replace(specifier, &new_specifier);
          text_changes.push(tsc::TextChange {
            span: text_change.span.clone(),
            new_text,
          });
        } else {
          text_changes.push(text_change.clone());
        }
      } else {
        text_changes.push(text_change.clone());
      }
    }
    r.push(tsc::FileTextChanges {
      file_name: change.file_name.clone(),
      text_changes,
      is_new_file: change.is_new_file,
    });
  }
  Ok(r)
}

/// Fix tsc import code actions so that the module specifier is correct for
/// resolution by Deno (includes the extension).
fn fix_ts_import_action(
  referrer: &ModuleSpecifier,
  action: &tsc::CodeFixAction,
  documents: &Documents,
) -> Result<tsc::CodeFixAction, AnyError> {
  if action.fix_name == "import" {
    let change = action
      .changes
      .get(0)
      .ok_or_else(|| anyhow!("Unexpected action changes."))?;
    let text_change = change
      .text_changes
      .get(0)
      .ok_or_else(|| anyhow!("Missing text change."))?;
    if let Some(captures) = IMPORT_SPECIFIER_RE.captures(&text_change.new_text)
    {
      let specifier = captures
        .get(1)
        .ok_or_else(|| anyhow!("Missing capture."))?
        .as_str();
      if let Some(new_specifier) =
        check_specifier(specifier, referrer, documents)
      {
        let description = action.description.replace(specifier, &new_specifier);
        let changes = action
          .changes
          .iter()
          .map(|c| {
            let text_changes = c
              .text_changes
              .iter()
              .map(|tc| tsc::TextChange {
                span: tc.span.clone(),
                new_text: tc.new_text.replace(specifier, &new_specifier),
              })
              .collect();
            tsc::FileTextChanges {
              file_name: c.file_name.clone(),
              text_changes,
              is_new_file: c.is_new_file,
            }
          })
          .collect();

        return Ok(tsc::CodeFixAction {
          description,
          changes,
          commands: None,
          fix_name: action.fix_name.clone(),
          fix_id: None,
          fix_all_description: None,
        });
      }
    }
  }

  Ok(action.clone())
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
  language_server: &language_server::Inner,
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

#[derive(Debug, Clone)]
enum CodeActionKind {
  Deno(lsp::CodeAction),
  DenoLint(lsp::CodeAction),
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
    specifier: &ModuleSpecifier,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<(), AnyError> {
    let code_action = DenoDiagnostic::get_code_action(specifier, diagnostic)?;
    self.actions.push(CodeActionKind::Deno(code_action));
    Ok(())
  }

  pub(crate) fn add_deno_lint_ignore_action(
    &mut self,
    specifier: &ModuleSpecifier,
    diagnostic: &lsp::Diagnostic,
    maybe_text_info: Option<SourceTextInfo>,
    maybe_parsed_source: Option<deno_ast::ParsedSource>,
  ) -> Result<(), AnyError> {
    let code = diagnostic
      .code
      .as_ref()
      .map(|v| match v {
        lsp::NumberOrString::String(v) => v.to_owned(),
        _ => "".to_string(),
      })
      .unwrap();

    let line_content = maybe_text_info.map(|ti| {
      ti.line_text(diagnostic.range.start.line as usize)
        .to_string()
    });

    let mut changes = HashMap::new();
    changes.insert(
      specifier.clone(),
      vec![lsp::TextEdit {
        new_text: prepend_whitespace(
          format!("// deno-lint-ignore {}\n", code),
          line_content,
        ),
        range: lsp::Range {
          start: lsp::Position {
            line: diagnostic.range.start.line,
            character: 0,
          },
          end: lsp::Position {
            line: diagnostic.range.start.line,
            character: 0,
          },
        },
      }],
    );
    let ignore_error_action = lsp::CodeAction {
      title: format!("Disable {} for this line", code),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      command: None,
      is_preferred: None,
      disabled: None,
      data: None,
      edit: Some(lsp::WorkspaceEdit {
        changes: Some(changes),
        change_annotations: None,
        document_changes: None,
      }),
    };
    self
      .actions
      .push(CodeActionKind::DenoLint(ignore_error_action));

    // Disable a lint error for the entire file.
    let maybe_ignore_comment = maybe_parsed_source.clone().and_then(|ps| {
      // Note: we can use ps.get_leading_comments() but it doesn't
      // work when shebang is present at the top of the file.
      ps.comments().get_vec().iter().find_map(|c| {
        let comment_text = c.text.trim();
        comment_text.split_whitespace().next().and_then(|prefix| {
          if prefix == "deno-lint-ignore-file" {
            Some(c.clone())
          } else {
            None
          }
        })
      })
    });

    let mut new_text = format!("// deno-lint-ignore-file {}\n", code);
    let mut range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 0,
      },
      end: lsp::Position {
        line: 0,
        character: 0,
      },
    };
    // If ignore file comment already exists, append the lint code
    // to the existing comment.
    if let Some(ignore_comment) = maybe_ignore_comment {
      new_text = format!(" {}", code);
      // Get the end position of the comment.
      let line = maybe_parsed_source
        .unwrap()
        .source()
        .line_and_column_index(ignore_comment.span.hi());
      let position = lsp::Position {
        line: line.line_index as u32,
        character: line.column_index as u32,
      };
      // Set the edit range to the end of the comment.
      range.start = position;
      range.end = position;
    }

    let mut changes = HashMap::new();
    changes.insert(specifier.clone(), vec![lsp::TextEdit { new_text, range }]);
    let ignore_file_action = lsp::CodeAction {
      title: format!("Disable {} for the entire file", code),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      command: None,
      is_preferred: None,
      disabled: None,
      data: None,
      edit: Some(lsp::WorkspaceEdit {
        changes: Some(changes),
        change_annotations: None,
        document_changes: None,
      }),
    };
    self
      .actions
      .push(CodeActionKind::DenoLint(ignore_file_action));

    let mut changes = HashMap::new();
    changes.insert(
      specifier.clone(),
      vec![lsp::TextEdit {
        new_text: "// deno-lint-ignore-file\n".to_string(),
        range: lsp::Range {
          start: lsp::Position {
            line: 0,
            character: 0,
          },
          end: lsp::Position {
            line: 0,
            character: 0,
          },
        },
      }],
    );
    let ignore_file_action = lsp::CodeAction {
      title: "Ignore lint errors for the entire file".to_string(),
      kind: Some(lsp::CodeActionKind::QUICKFIX),
      diagnostics: Some(vec![diagnostic.clone()]),
      command: None,
      is_preferred: None,
      disabled: None,
      data: None,
      edit: Some(lsp::WorkspaceEdit {
        changes: Some(changes),
        change_annotations: None,
        document_changes: None,
      }),
    };
    self
      .actions
      .push(CodeActionKind::DenoLint(ignore_file_action));

    Ok(())
  }

  /// Add a TypeScript code fix action to the code actions collection.
  pub(crate) async fn add_ts_fix_action(
    &mut self,
    specifier: &ModuleSpecifier,
    action: &tsc::CodeFixAction,
    diagnostic: &lsp::Diagnostic,
    language_server: &language_server::Inner,
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
    let action =
      fix_ts_import_action(specifier, action, &language_server.documents)?;
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
        CodeActionKind::DenoLint(c) => lsp::CodeActionOrCommand::CodeAction(c),
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

/// Prepend the whitespace characters found at the start of line_content to content.
fn prepend_whitespace(content: String, line_content: Option<String>) -> String {
  if let Some(line) = line_content {
    let whitespaces =
      line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
    let whitespace = &line[0..whitespaces];
    format!("{}{}", &whitespace, content)
  } else {
    content
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_reference_to_diagnostic() {
    let range = Range {
      start: Position {
        line: 1,
        character: 1,
      },
      end: Position {
        line: 2,
        character: 2,
      },
    };

    let test_cases = [
      (
        Reference {
          category: Category::Lint {
            message: "message1".to_string(),
            code: "code1".to_string(),
            hint: None,
          },
          range,
        },
        lsp::Diagnostic {
          range,
          severity: Some(lsp::DiagnosticSeverity::WARNING),
          code: Some(lsp::NumberOrString::String("code1".to_string())),
          source: Some("deno-lint".to_string()),
          message: "message1".to_string(),
          ..Default::default()
        },
      ),
      (
        Reference {
          category: Category::Lint {
            message: "message2".to_string(),
            code: "code2".to_string(),
            hint: Some("hint2".to_string()),
          },
          range,
        },
        lsp::Diagnostic {
          range,
          severity: Some(lsp::DiagnosticSeverity::WARNING),
          code: Some(lsp::NumberOrString::String("code2".to_string())),
          source: Some("deno-lint".to_string()),
          message: "message2\nhint2".to_string(),
          ..Default::default()
        },
      ),
    ];

    for (input, expected) in test_cases.iter() {
      let actual = input.to_diagnostic();
      assert_eq!(&actual, expected);
    }
  }

  #[test]
  fn test_as_lsp_range() {
    let fixture = deno_lint::diagnostic::Range {
      start: deno_lint::diagnostic::Position {
        line_index: 0,
        column_index: 2,
        byte_pos: 23,
      },
      end: deno_lint::diagnostic::Position {
        line_index: 1,
        column_index: 0,
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
}
