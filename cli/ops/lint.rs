// Copyright 2018-2025 the Deno authors. MIT license.

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseDiagnostic;
use deno_ast::SourceRange;
use deno_ast::SourceTextInfo;
use deno_ast::SourceTextProvider;
use deno_core::op2;
use deno_core::OpState;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_lint::diagnostic::LintFix;
use deno_lint::diagnostic::LintFixChange;
use tokio_util::sync::CancellationToken;

use crate::tools::lint;
use crate::tools::lint::PluginLogger;

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

#[derive(Default)]
pub struct LintPluginContainer {
  pub diagnostics: Vec<LintDiagnostic>,
  pub source_text_info: Option<SourceTextInfo>,
  pub specifier: Option<ModuleSpecifier>,
  pub token: CancellationToken,
}

impl LintPluginContainer {
  pub fn set_cancellation_token(
    &mut self,
    maybe_token: Option<CancellationToken>,
  ) {
    let token = maybe_token.unwrap_or_default();
    self.token = token;
  }

  pub fn set_info_for_file(
    &mut self,
    specifier: ModuleSpecifier,
    source_text_info: SourceTextInfo,
  ) {
    self.specifier = Some(specifier);
    self.source_text_info = Some(source_text_info);
  }

  fn report(
    &mut self,
    id: String,
    message: String,
    hint: Option<String>,
    start: usize,
    end: usize,
    fix: Option<LintReportFix>,
  ) {
    let source_text_info = self.source_text_info.as_ref().unwrap();
    let specifier = self.specifier.clone().unwrap();
    let start_pos = source_text_info.start_pos();
    let source_range = SourceRange::new(start_pos + start, start_pos + end);
    // TODO(bartlomieju): validate this is a correct range
    let range = LintDiagnosticRange {
      range: source_range,
      description: None,
      text_info: source_text_info.clone(),
    };

    let mut fixes: Vec<LintFix> = vec![];

    if let Some(fix) = fix {
      fixes.push(LintFix {
        changes: vec![LintFixChange {
          new_text: fix.text.into(),
          // TODO(bartlomieju): validate this is a correct range
          range: SourceRange::new(
            start_pos + fix.range.0,
            start_pos + fix.range.1,
          ),
        }],
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
        custom_docs_url: None,
        info: vec![],
      },
    };
    self.diagnostics.push(lint_diagnostic);
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
  let container = state.borrow_mut::<LintPluginContainer>();
  container.token.is_cancelled()
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LintError {
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
#[buffer]
fn op_lint_create_serialized_ast(
  #[string] file_name: &str,
  #[string] source: String,
) -> Result<Vec<u8>, LintError> {
  let file_text = deno_ast::strip_bom(source);
  let path = std::env::current_dir()?.join(file_name);
  let specifier = ModuleSpecifier::from_file_path(&path)
    .map_err(|_| LintError::PathParse(path))?;
  let media_type = MediaType::from_specifier(&specifier);
  let parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
    specifier,
    text: file_text.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })?;
  Ok(lint::serialize_ast_to_buffer(&parsed_source))
}

#[derive(serde::Deserialize)]
struct LintReportFix {
  text: String,
  range: (usize, usize),
}

#[op2]
fn op_lint_report(
  state: &mut OpState,
  #[string] id: String,
  #[string] message: String,
  #[string] hint: Option<String>,
  #[smi] start: usize,
  #[smi] end: usize,
  #[serde] fix: Option<LintReportFix>,
) {
  let container = state.borrow_mut::<LintPluginContainer>();
  container.report(id, message, hint, start, end, fix);
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
