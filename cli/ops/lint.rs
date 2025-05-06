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
use deno_lint::diagnostic::LintDocsUrl;
use deno_lint::diagnostic::LintFix;
use deno_lint::diagnostic::LintFixChange;
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

  fn report(
    &mut self,
    id: String,
    message: String,
    hint: Option<String>,
    start_utf16: usize,
    end_utf16: usize,
    raw_fixes: Vec<LintReportFix>,
  ) -> Result<(), LintReportError> {
    fn out_of_range_err(
      map: &Utf16Map,
      start_utf16: usize,
      end_utf16: usize,
    ) -> LintReportError {
      LintReportError::IncorrectRange {
        start: start_utf16,
        end: end_utf16,
        source_end: map.text_content_length_utf16().into(),
      }
    }

    fn utf16_to_utf8_range(
      utf16_map: &Utf16Map,
      source_text_info: &SourceTextInfo,
      start_utf16: usize,
      end_utf16: usize,
    ) -> Result<SourceRange, LintReportError> {
      let Some(start) =
        utf16_map.utf16_to_utf8_offset((start_utf16 as u32).into())
      else {
        return Err(out_of_range_err(utf16_map, start_utf16, end_utf16));
      };
      let Some(end) = utf16_map.utf16_to_utf8_offset((end_utf16 as u32).into())
      else {
        return Err(out_of_range_err(utf16_map, start_utf16, end_utf16));
      };
      let start_pos = source_text_info.start_pos();
      Ok(SourceRange::new(
        start_pos + start.into(),
        start_pos + end.into(),
      ))
    }

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
  let utf16_map = Utf16Map::new(parsed_source.text().as_ref());
  Ok(lint::serialize_ast_to_buffer(&parsed_source, &utf16_map))
}

#[derive(serde::Deserialize)]
struct LintReportFix {
  text: String,
  range: (usize, usize),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LintReportError {
  #[class(type)]
  #[error("Invalid range [{start}, {end}], the source has a range of [0, {source_end}]")]
  IncorrectRange {
    start: usize,
    end: usize,
    source_end: u32,
  },
}

#[op2]
fn op_lint_report(
  state: &mut OpState,
  #[string] id: String,
  #[string] message: String,
  #[string] hint: Option<String>,
  #[smi] start_utf16: usize,
  #[smi] end_utf16: usize,
  #[serde] fix: Vec<LintReportFix>,
) -> Result<(), LintReportError> {
  let container = state.borrow_mut::<LintPluginContainer>();
  container.report(id, message, hint, start_utf16, end_utf16, fix)?;
  Ok(())
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
