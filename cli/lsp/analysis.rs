// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::language_server;
use super::tsc;

use crate::ast;
use crate::import_map::ImportMap;
use crate::lsp::documents::DocumentData;
use crate::media_type::MediaType;
use crate::module_graph::parse_deno_types;
use crate::module_graph::parse_ts_reference;
use crate::module_graph::TypeScriptReference;
use crate::tools::lint::create_linter;

use deno_core::error::anyhow;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url;
use deno_core::ModuleResolutionError;
use deno_core::ModuleSpecifier;
use deno_lint::rules;
use lspower::lsp;
use lspower::lsp::Position;
use lspower::lsp::Range;
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use swc_common::DUMMY_SP;
use swc_ecmascript::ast as swc_ast;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;
use swc_ecmascript::visit::VisitWith;

lazy_static::lazy_static! {
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

  static ref IMPORT_SPECIFIER_RE: Regex = Regex::new(r#"\sfrom\s+["']([^"']*)["']"#).unwrap();
}

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
        severity: Some(lsp::DiagnosticSeverity::Warning),
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
  let linter = create_linter(syntax, lint_rules);
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Dependency {
  pub is_dynamic: bool,
  pub maybe_code: Option<ResolvedDependency>,
  pub maybe_code_specifier_range: Option<Range>,
  pub maybe_type: Option<ResolvedDependency>,
  pub maybe_type_specifier_range: Option<Range>,
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
      Self::InvalidSpecifier(error) => match error {
        ModuleResolutionError::ImportPrefixMissing(_, _) => {
          lsp::NumberOrString::String("import-prefix-missing".to_string())
        }
        ModuleResolutionError::InvalidBaseUrl(_) => {
          lsp::NumberOrString::String("invalid-base-url".to_string())
        }
        ModuleResolutionError::InvalidPath(_) => {
          lsp::NumberOrString::String("invalid-path".to_string())
        }
        ModuleResolutionError::InvalidUrl(_) => {
          lsp::NumberOrString::String("invalid-url".to_string())
        }
      },
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

impl ResolvedDependency {
  pub fn as_hover_text(&self) -> String {
    match self {
      Self::Resolved(specifier) => match specifier.scheme() {
        "data" => "_(a data url)_".to_string(),
        "blob" => "_(a blob url)_".to_string(),
        _ => format!(
          "{}&#8203;{}",
          specifier[..url::Position::AfterScheme].to_string(),
          specifier[url::Position::AfterScheme..].to_string()
        ),
      },
      Self::Err(_) => "_[errored]_".to_string(),
    }
  }
}

pub fn resolve_import(
  specifier: &str,
  referrer: &ModuleSpecifier,
  maybe_import_map: &Option<ImportMap>,
) -> ResolvedDependency {
  let maybe_mapped = if let Some(import_map) = maybe_import_map {
    import_map.resolve(specifier, referrer.as_str()).ok()
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

pub fn parse_module(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: &MediaType,
) -> Result<ast::ParsedModule, AnyError> {
  ast::parse(&specifier.to_string(), source, media_type)
}

// TODO(@kitsonk) a lot of this logic is duplicated in module_graph.rs in
// Module::parse() and should be refactored out to a common function.
pub fn analyze_dependencies(
  specifier: &ModuleSpecifier,
  media_type: &MediaType,
  parsed_module: &ast::ParsedModule,
  maybe_import_map: &Option<ImportMap>,
) -> (HashMap<String, Dependency>, Option<ResolvedDependency>) {
  let mut maybe_type = None;
  let mut dependencies = HashMap::<String, Dependency>::new();

  // Parse leading comments for supported triple slash references.
  for comment in parsed_module.get_leading_comments().iter() {
    if let Some((ts_reference, span)) = parse_ts_reference(comment) {
      let loc = parsed_module.get_location(span.lo);
      match ts_reference {
        TypeScriptReference::Path(import) => {
          let dep = dependencies.entry(import.clone()).or_default();
          let resolved_import =
            resolve_import(&import, specifier, maybe_import_map);
          dep.maybe_code = Some(resolved_import);
          dep.maybe_code_specifier_range = Some(Range {
            start: Position {
              line: (loc.line - 1) as u32,
              character: loc.col as u32,
            },
            end: Position {
              line: (loc.line - 1) as u32,
              character: (loc.col + import.chars().count() + 2) as u32,
            },
          });
        }
        TypeScriptReference::Types(import) => {
          let resolved_import =
            resolve_import(&import, specifier, maybe_import_map);
          if media_type == &MediaType::JavaScript
            || media_type == &MediaType::Jsx
          {
            maybe_type = Some(resolved_import.clone());
          }
          let dep = dependencies.entry(import.clone()).or_default();
          dep.maybe_type = Some(resolved_import);
          dep.maybe_type_specifier_range = Some(Range {
            start: Position {
              line: (loc.line - 1) as u32,
              character: loc.col as u32,
            },
            end: Position {
              line: (loc.line - 1) as u32,
              character: (loc.col + import.chars().count() + 2) as u32,
            },
          });
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
        parse_deno_types(comment).as_ref().map(|(deno_types, span)| {
          (
            resolve_import(deno_types, specifier, maybe_import_map),
            deno_types.clone(),
            parsed_module.get_location(span.lo)
          )
        })
      } else {
        None
      };

    let dep = dependencies.entry(desc.specifier.to_string()).or_default();
    dep.is_dynamic = desc.is_dynamic;
    let start = parsed_module.get_location(desc.specifier_span.lo);
    let end = parsed_module.get_location(desc.specifier_span.hi);
    let range = Range {
      start: Position {
        line: (start.line - 1) as u32,
        character: start.col as u32,
      },
      end: Position {
        line: (end.line - 1) as u32,
        character: end.col as u32,
      },
    };
    dep.maybe_code_specifier_range = Some(range);
    dep.maybe_code = Some(resolved_import);
    if dep.maybe_type.is_none() {
      if let Some((resolved_dependency, specifier, loc)) =
        maybe_resolved_type_dependency
      {
        dep.maybe_type_specifier_range = Some(Range {
          start: Position {
            line: (loc.line - 1) as u32,
            character: (loc.col + 1) as u32,
          },
          end: Position {
            line: (loc.line - 1) as u32,
            character: (loc.col + 1 + specifier.chars().count()) as u32,
          },
        });
        dep.maybe_type = Some(resolved_dependency);
      }
    }
  }

  (dependencies, maybe_type)
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
  snapshot: &language_server::StateSnapshot,
  maybe_import_map: &Option<ImportMap>,
) -> Option<String> {
  for ext in SUPPORTED_EXTENSIONS {
    let specifier_with_ext = format!("{}{}", specifier, ext);
    if let ResolvedDependency::Resolved(resolved_specifier) =
      resolve_import(&specifier_with_ext, referrer, maybe_import_map)
    {
      if snapshot.documents.contains_key(&resolved_specifier)
        || snapshot.sources.contains_key(&resolved_specifier)
      {
        return Some(specifier_with_ext);
      }
    }
  }

  None
}

/// For a set of tsc changes, can them for any that contain something that looks
/// like an import and rewrite the import specifier to include the extension
pub(crate) fn fix_ts_import_changes(
  referrer: &ModuleSpecifier,
  changes: &[tsc::FileTextChanges],
  language_server: &language_server::Inner,
) -> Result<Vec<tsc::FileTextChanges>, AnyError> {
  let mut r = Vec::new();
  let snapshot = language_server.snapshot()?;
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
        if let Some(new_specifier) = check_specifier(
          specifier,
          referrer,
          &snapshot,
          &language_server.maybe_import_map,
        ) {
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
  language_server: &language_server::Inner,
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
      let snapshot = language_server.snapshot()?;
      if let Some(new_specifier) = check_specifier(
        specifier,
        referrer,
        &snapshot,
        &language_server.maybe_import_map,
      ) {
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
    diagnostic: &lsp::Diagnostic,
  ) -> Result<(), AnyError> {
    if let Some(data) = diagnostic.data.clone() {
      let fix_data: DenoFixData = serde_json::from_value(data)?;
      let title = if matches!(&diagnostic.code, Some(lsp::NumberOrString::String(code)) if code == "no-cache-data")
      {
        "Cache the data URL and its dependencies.".to_string()
      } else {
        format!("Cache \"{}\" and its dependencies.", fix_data.specifier)
      };
      let code_action = lsp::CodeAction {
        title,
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

  pub(crate) fn add_deno_lint_ignore_action(
    &mut self,
    specifier: &ModuleSpecifier,
    document: Option<&DocumentData>,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<(), AnyError> {
    let code = diagnostic
      .code
      .as_ref()
      .map(|v| match v {
        lsp::NumberOrString::String(v) => v.to_owned(),
        _ => "".to_string(),
      })
      .unwrap();

    let line_content = if let Some(doc) = document {
      doc
        .content_line(diagnostic.range.start.line as usize)
        .ok()
        .flatten()
    } else {
      None
    };

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
    let action = fix_ts_import_action(specifier, action, language_server)?;
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

/// Get LSP range from the provided start and end locations.
fn get_range_from_location(
  start: &ast::Location,
  end: &ast::Location,
) -> lsp::Range {
  lsp::Range {
    start: lsp::Position {
      line: (start.line - 1) as u32,
      character: start.col as u32,
    },
    end: lsp::Position {
      line: (end.line - 1) as u32,
      character: end.col as u32,
    },
  }
}

/// Narrow the range to only include the text of the specifier, excluding the
/// quotes.
fn narrow_range(range: lsp::Range) -> lsp::Range {
  lsp::Range {
    start: lsp::Position {
      line: range.start.line,
      character: range.start.character + 1,
    },
    end: lsp::Position {
      line: range.end.line,
      character: range.end.character - 1,
    },
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRange {
  /// The LSP Range is inclusive of the quotes around the specifier.
  pub range: lsp::Range,
  /// The text of the specifier within the document.
  pub specifier: String,
}

impl DependencyRange {
  /// Determine if the position is within the range
  fn within(&self, position: &lsp::Position) -> bool {
    (position.line > self.range.start.line
      || position.line == self.range.start.line
        && position.character >= self.range.start.character)
      && (position.line < self.range.end.line
        || position.line == self.range.end.line
          && position.character <= self.range.end.character)
  }
}

#[derive(Debug, Default, Clone)]
pub struct DependencyRanges(Vec<DependencyRange>);

impl DependencyRanges {
  pub fn contains(&self, position: &lsp::Position) -> Option<DependencyRange> {
    self.0.iter().find(|r| r.within(position)).cloned()
  }
}

struct DependencyRangeCollector<'a> {
  import_ranges: DependencyRanges,
  parsed_module: &'a ast::ParsedModule,
}

impl<'a> DependencyRangeCollector<'a> {
  pub fn new(parsed_module: &'a ast::ParsedModule) -> Self {
    Self {
      import_ranges: DependencyRanges::default(),
      parsed_module,
    }
  }

  pub fn take(self) -> DependencyRanges {
    self.import_ranges
  }
}

impl<'a> Visit for DependencyRangeCollector<'a> {
  fn visit_import_decl(
    &mut self,
    node: &swc_ast::ImportDecl,
    _parent: &dyn Node,
  ) {
    let start = self.parsed_module.get_location(node.src.span.lo);
    let end = self.parsed_module.get_location(node.src.span.hi);
    self.import_ranges.0.push(DependencyRange {
      range: narrow_range(get_range_from_location(&start, &end)),
      specifier: node.src.value.to_string(),
    });
  }

  fn visit_named_export(
    &mut self,
    node: &swc_ast::NamedExport,
    _parent: &dyn Node,
  ) {
    if let Some(src) = &node.src {
      let start = self.parsed_module.get_location(src.span.lo);
      let end = self.parsed_module.get_location(src.span.hi);
      self.import_ranges.0.push(DependencyRange {
        range: narrow_range(get_range_from_location(&start, &end)),
        specifier: src.value.to_string(),
      });
    }
  }

  fn visit_export_all(
    &mut self,
    node: &swc_ast::ExportAll,
    _parent: &dyn Node,
  ) {
    let start = self.parsed_module.get_location(node.src.span.lo);
    let end = self.parsed_module.get_location(node.src.span.hi);
    self.import_ranges.0.push(DependencyRange {
      range: narrow_range(get_range_from_location(&start, &end)),
      specifier: node.src.value.to_string(),
    });
  }

  fn visit_ts_import_type(
    &mut self,
    node: &swc_ast::TsImportType,
    _parent: &dyn Node,
  ) {
    let start = self.parsed_module.get_location(node.arg.span.lo);
    let end = self.parsed_module.get_location(node.arg.span.hi);
    self.import_ranges.0.push(DependencyRange {
      range: narrow_range(get_range_from_location(&start, &end)),
      specifier: node.arg.value.to_string(),
    });
  }
}

/// Analyze a document for import ranges, which then can be used to identify if
/// a particular position within the document as inside an import range.
pub fn analyze_dependency_ranges(
  parsed_module: &ast::ParsedModule,
) -> Result<DependencyRanges, AnyError> {
  let mut collector = DependencyRangeCollector::new(parsed_module);
  parsed_module
    .module
    .visit_with(&swc_ast::Invalid { span: DUMMY_SP }, &mut collector);
  Ok(collector.take())
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url;

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
          severity: Some(lsp::DiagnosticSeverity::Warning),
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
          severity: Some(lsp::DiagnosticSeverity::Warning),
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
  fn test_get_lint_references() {
    let specifier = resolve_url("file:///a.ts").expect("bad specifier");
    let source = "const foo = 42;";
    let actual =
      get_lint_references(&specifier, &MediaType::TypeScript, source).unwrap();

    assert_eq!(
      actual,
      vec![Reference {
        category: Category::Lint {
          message: "`foo` is never used".to_string(),
          code: "no-unused-vars".to_string(),
          hint: Some(
            "If this is intentional, prefix it with an underscore like `_foo`"
              .to_string()
          ),
        },
        range: Range {
          start: Position {
            line: 0,
            character: 6,
          },
          end: Position {
            line: 0,
            character: 9,
          }
        }
      }]
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

    import type { Component } from "https://esm.sh/preact";
    import { h, Fragment } from "https://esm.sh/preact";

    // @deno-types="https://deno.land/x/types/react/index.d.ts";
    import React from "https://cdn.skypack.dev/react";
    "#;
    let parsed_module =
      parse_module(&specifier, source, &MediaType::TypeScript).unwrap();
    let (actual, maybe_type) = analyze_dependencies(
      &specifier,
      &MediaType::TypeScript,
      &parsed_module,
      &None,
    );
    assert!(maybe_type.is_none());
    assert_eq!(actual.len(), 3);
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
            line: 11,
            character: 22,
          },
          end: Position {
            line: 11,
            character: 53,
          }
        }),
        maybe_type_specifier_range: Some(Range {
          start: Position {
            line: 10,
            character: 20,
          },
          end: Position {
            line: 10,
            character: 62,
          }
        })
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
        maybe_type_specifier_range: None,
      })
    );
    assert_eq!(
      actual.get("https://esm.sh/preact").cloned(),
      Some(Dependency {
        is_dynamic: false,
        maybe_code: Some(ResolvedDependency::Resolved(
          resolve_url("https://esm.sh/preact").unwrap()
        )),
        maybe_type: None,
        maybe_code_specifier_range: Some(Range {
          start: Position {
            line: 8,
            character: 32
          },
          end: Position {
            line: 8,
            character: 55
          }
        }),
        maybe_type_specifier_range: None,
      }),
    );
  }

  #[test]
  fn test_analyze_dependency_ranges() {
    let specifier = resolve_url("file:///a.ts").unwrap();
    let source =
      "import * as a from \"./b.ts\";\nexport * as a from \"./c.ts\";\n";
    let media_type = MediaType::TypeScript;
    let parsed_module = parse_module(&specifier, source, &media_type).unwrap();
    let result = analyze_dependency_ranges(&parsed_module);
    assert!(result.is_ok());
    let actual = result.unwrap();
    assert_eq!(
      actual.contains(&lsp::Position {
        line: 0,
        character: 0,
      }),
      None
    );
    assert_eq!(
      actual.contains(&lsp::Position {
        line: 0,
        character: 22,
      }),
      Some(DependencyRange {
        range: lsp::Range {
          start: lsp::Position {
            line: 0,
            character: 20,
          },
          end: lsp::Position {
            line: 0,
            character: 26,
          },
        },
        specifier: "./b.ts".to_string(),
      })
    );
    assert_eq!(
      actual.contains(&lsp::Position {
        line: 1,
        character: 22,
      }),
      Some(DependencyRange {
        range: lsp::Range {
          start: lsp::Position {
            line: 1,
            character: 20,
          },
          end: lsp::Position {
            line: 1,
            character: 26,
          },
        },
        specifier: "./c.ts".to_string(),
      })
    );
  }
}
