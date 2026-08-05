// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseDiagnostic;
use deno_ast::SourceRange;
use deno_ast::SourceTextInfo;
use deno_ast::SourceTextProvider;
use deno_core::FromV8;
use deno_core::OpState;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_lint::diagnostic::LintDocsUrl;
use deno_lint::diagnostic::LintFix;
use deno_lint::diagnostic::LintFixChange;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

use crate::tools::lint;
use crate::tools::lint::PluginLogger;
use crate::util::text_encoding::Utf16Map;

deno_core::extension!(
  deno_lint_ext,
  ops = [
    op_lint_create_serialized_ast,
    op_lint_report,
    op_lint_get_source,
    op_is_cancelled
  ],
  options = {
    logger: PluginLogger,
  },
  // TODO(bartlomieju): this should only be done,
  // if not in the "test worker".
  middleware = |op| match op.name {
    "op_print" => op_print(),
    _ => op,
  },
  state = |state, options| {
    state.put(options.logger);
    state.put(LintPluginContainer::default());
  },
);

deno_core::extension!(
  deno_lint_ext_for_test,
  ops = [op_lint_create_serialized_ast, op_is_cancelled],
  state = |state| {
    state.put(LintPluginContainer::default());
  },
);

#[derive(Default)]
pub struct LintPluginContainer {
  pub diagnostics: Vec<LintDiagnostic>,
  pub source_text_info: Option<SourceTextInfo>,
  pub utf_16_map: Option<Utf16Map>,
  pub specifier: Option<ModuleSpecifier>,
  pub token: CancellationToken,
  /// Per-module source info for the graph phase, keyed by specifier. A graph
  /// rule reports against an arbitrary module (not the "current" file), so we
  /// need each module's text info + utf16 map to attribute the diagnostic.
  pub graph_files: HashMap<ModuleSpecifier, (SourceTextInfo, Utf16Map)>,
}

/// Convert a [start, end) UTF-16 range into a UTF-8 `SourceRange` within the
/// given source, shared by the per-file and graph report paths.
fn utf16_to_utf8_range(
  utf16_map: &Utf16Map,
  source_text_info: &SourceTextInfo,
  start_utf16: usize,
  end_utf16: usize,
) -> Result<SourceRange, LintReportError> {
  let out_of_range = || LintReportError::IncorrectRange {
    start: start_utf16,
    end: end_utf16,
    source_end: utf16_map.text_content_length_utf16().into(),
  };
  let Some(start) = utf16_map.utf16_to_utf8_offset((start_utf16 as u32).into())
  else {
    return Err(out_of_range());
  };
  let Some(end) = utf16_map.utf16_to_utf8_offset((end_utf16 as u32).into())
  else {
    return Err(out_of_range());
  };
  let start_pos = source_text_info.start_pos();
  Ok(SourceRange::new(
    start_pos + start.into(),
    start_pos + end.into(),
  ))
}

impl LintPluginContainer {
  pub fn set_info_for_file(
    &mut self,
    specifier: ModuleSpecifier,
    source_text_info: SourceTextInfo,
    utf16_map: Utf16Map,
    maybe_token: Option<CancellationToken>,
  ) {
    self.specifier = Some(specifier);
    self.utf_16_map = Some(utf16_map);
    self.source_text_info = Some(source_text_info);
    self.diagnostics.clear();
    self.token = maybe_token.unwrap_or_default();
  }

  /// Set the per-module source info used by the graph phase.
  pub fn set_graph_files(
    &mut self,
    graph_files: HashMap<ModuleSpecifier, (SourceTextInfo, Utf16Map)>,
  ) {
    self.diagnostics.clear();
    self.graph_files = graph_files;
  }

  /// Report a diagnostic from a graph rule, attributed to an arbitrary module
  /// `specifier` (not the "current" file). The range is UTF-16 offsets within
  /// that module's source. Called from the plugin host after reading the
  /// reports returned by the JS `runGraphRules` function.
  pub fn report_graph(
    &mut self,
    specifier: String,
    id: String,
    message: String,
    hint: Option<String>,
    start_utf16: usize,
    end_utf16: usize,
  ) -> Result<(), LintReportError> {
    let specifier = ModuleSpecifier::parse(&specifier)
      .map_err(|_| LintReportError::UnknownSpecifier(specifier.clone()))?;
    let Some((source_text_info, utf16_map)) = self.graph_files.get(&specifier)
    else {
      return Err(LintReportError::UnknownSpecifier(specifier.to_string()));
    };
    let diagnostic_range =
      utf16_to_utf8_range(utf16_map, source_text_info, start_utf16, end_utf16)?;
    let range = LintDiagnosticRange {
      range: diagnostic_range,
      description: None,
      text_info: source_text_info.clone(),
    };
    let lint_diagnostic = LintDiagnostic {
      specifier,
      range: Some(range),
      details: LintDiagnosticDetails {
        message,
        code: id,
        hint,
        fixes: vec![],
        custom_docs_url: LintDocsUrl::None,
        info: vec![],
      },
    };
    self.diagnostics.push(lint_diagnostic);
    Ok(())
  }

  fn report(
    &mut self,
    id: String,
    message: String,
    hint: Option<String>,
    start_utf16: usize,
    end_utf16: usize,
    raw_fixes: Vec<LintReportFix>,
  ) -> Result<(), LintReportError> {
    let source_text_info = self.source_text_info.as_ref().unwrap();
    let utf16_map = self.utf_16_map.as_ref().unwrap();
    let specifier = self.specifier.clone().unwrap();
    let diagnostic_range =
      utf16_to_utf8_range(utf16_map, source_text_info, start_utf16, end_utf16)?;
    let range = LintDiagnosticRange {
      range: diagnostic_range,
      description: None,
      text_info: source_text_info.clone(),
    };

    let changes = raw_fixes
      .into_iter()
      .map(|fix| {
        let fix_range = utf16_to_utf8_range(
          utf16_map,
          source_text_info,
          fix.range.0,
          fix.range.1,
        )?;

        Ok(LintFixChange {
          new_text: fix.text.into(),
          range: fix_range,
        })
      })
      .collect::<Result<Vec<LintFixChange>, LintReportError>>()?;

    let mut fixes = vec![];

    if !changes.is_empty() {
      fixes.push(LintFix {
        changes,
        description: format!("Fix this {} problem", id).into(),
      });
    }

    let lint_diagnostic = LintDiagnostic {
      specifier,
      range: Some(range),
      details: LintDiagnosticDetails {
        message,
        code: id,
        hint,
        fixes,
        // TODO(bartlomieju): allow plugins to actually specify custom url for docs
        custom_docs_url: LintDocsUrl::None,
        info: vec![],
      },
    };
    self.diagnostics.push(lint_diagnostic);
    Ok(())
  }
}

#[op2(fast)]
pub fn op_print(state: &mut OpState, #[string] msg: &str, is_err: bool) {
  let logger = state.borrow::<PluginLogger>();

  if is_err {
    logger.error(msg);
  } else {
    logger.log(msg);
  }
}

#[op2(fast)]
fn op_is_cancelled(state: &mut OpState) -> bool {
  let container = state.borrow::<LintPluginContainer>();
  container.token.is_cancelled()
}

#[derive(Debug, boxed_error::Boxed, deno_error::JsError)]
pub struct LintError(pub Box<LintErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LintErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  ParseDiagnostic(#[from] ParseDiagnostic),
  #[class(type)]
  #[error("Failed to parse path as URL: {0}")]
  PathParse(std::path::PathBuf),
}

#[op2]
fn op_lint_create_serialized_ast(
  #[string] file_name: &str,
  #[string] source: String,
) -> Result<Uint8Array, LintError> {
  let file_text = deno_ast::strip_bom(source);
  #[allow(clippy::disallowed_methods, reason = "ok for linting")]
  let path = std::env::current_dir()?.join(file_name);
  let specifier = ModuleSpecifier::from_file_path(&path)
    .map_err(|_| LintErrorKind::PathParse(path))?;
  let media_type = MediaType::from_specifier(&specifier);
  let parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
    specifier,
    text: file_text.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })?;
  let utf16_map = Utf16Map::new(parsed_source.text().as_ref());
  Ok(lint::serialize_ast_to_buffer(&parsed_source, &utf16_map).into())
}

#[derive(FromV8)]
struct LintReportFix {
  text: String,
  range: (usize, usize),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LintReportError {
  #[class(type)]
  #[error(
    "Invalid range [{start}, {end}], the source has a range of [0, {source_end}]"
  )]
  IncorrectRange {
    start: usize,
    end: usize,
    source_end: u32,
  },
  #[class(type)]
  #[error("Unknown specifier reported by graph rule: {0}")]
  UnknownSpecifier(String),
}

#[op2]
fn op_lint_report(
  state: &mut OpState,
  #[string] id: String,
  #[string] message: String,
  #[string] hint: Option<String>,
  #[smi] start_utf16: usize,
  #[smi] end_utf16: usize,
  #[scoped] fix: Vec<LintReportFix>,
) -> Result<(), LintReportError> {
  let container = state.borrow_mut::<LintPluginContainer>();
  container.report(id, message, hint, start_utf16, end_utf16, fix)?;
  Ok(())
}

/// A single report emitted by a graph rule, decoded from the array returned by
/// the JS `runGraphRules` function. Using a return value (rather than an op)
/// avoids depending on op-table binding for `40_lint.js`, which is baked into
/// the startup snapshot.
#[derive(Debug, FromV8)]
pub struct GraphReport {
  pub specifier: String,
  pub id: String,
  pub message: String,
  /// Empty string means "no hint".
  pub hint: String,
  pub start: usize,
  pub end: usize,
}

#[op2]
#[string]
fn op_lint_get_source(state: &mut OpState) -> String {
  let container = state.borrow_mut::<LintPluginContainer>();
  container
    .source_text_info
    .as_ref()
    .unwrap()
    .text_str()
    .to_string()
}
