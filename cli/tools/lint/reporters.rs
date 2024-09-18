// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::diagnostics::Diagnostic;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_lint::diagnostic::LintDiagnostic;
use deno_runtime::colors;
use log::info;
use serde::Serialize;

use crate::args::LintReporterKind;

use super::LintError;

const JSON_SCHEMA_VERSION: u8 = 1;

pub fn create_reporter(kind: LintReporterKind) -> Box<dyn LintReporter + Send> {
  match kind {
    LintReporterKind::Pretty => Box::new(PrettyLintReporter::new()),
    LintReporterKind::Json => Box::new(JsonLintReporter::new()),
    LintReporterKind::Compact => Box::new(CompactLintReporter::new()),
  }
}

pub trait LintReporter {
  fn visit_diagnostic(&mut self, d: &LintDiagnostic);
  fn visit_error(&mut self, file_path: &str, err: &AnyError);
  fn close(&mut self, check_count: usize);
}

struct PrettyLintReporter {
  lint_count: u32,
  fixable_diagnostics: u32,
}

impl PrettyLintReporter {
  fn new() -> PrettyLintReporter {
    PrettyLintReporter {
      lint_count: 0,
      fixable_diagnostics: 0,
    }
  }
}

impl LintReporter for PrettyLintReporter {
  fn visit_diagnostic(&mut self, d: &LintDiagnostic) {
    self.lint_count += 1;
    if !d.details.fixes.is_empty() {
      self.fixable_diagnostics += 1;
    }

    log::error!("{}\n", d.display());
  }

  fn visit_error(&mut self, file_path: &str, err: &AnyError) {
    log::error!("Error linting: {file_path}");
    log::error!("   {err}");
  }

  fn close(&mut self, check_count: usize) {
    let fixable_suffix = if self.fixable_diagnostics > 0 {
      colors::gray(format!(" ({} fixable via --fix)", self.fixable_diagnostics))
        .to_string()
    } else {
      "".to_string()
    };
    match self.lint_count {
      1 => info!("Found 1 problem{}", fixable_suffix),
      n if n > 1 => {
        info!("Found {} problems{}", self.lint_count, fixable_suffix)
      }
      _ => (),
    }

    match check_count {
      1 => info!("Checked 1 file"),
      n => info!("Checked {} files", n),
    }
  }
}

struct CompactLintReporter {
  lint_count: u32,
}

impl CompactLintReporter {
  fn new() -> CompactLintReporter {
    CompactLintReporter { lint_count: 0 }
  }
}

impl LintReporter for CompactLintReporter {
  fn visit_diagnostic(&mut self, d: &LintDiagnostic) {
    self.lint_count += 1;

    match &d.range {
      Some(range) => {
        let text_info = &range.text_info;
        let range = &range.range;
        let line_and_column = text_info.line_and_column_display(range.start);
        log::error!(
          "{}: line {}, col {} - {} ({})",
          d.specifier,
          line_and_column.line_number,
          line_and_column.column_number,
          d.message(),
          d.code(),
        )
      }
      None => {
        log::error!("{}: {} ({})", d.specifier, d.message(), d.code())
      }
    }
  }

  fn visit_error(&mut self, file_path: &str, err: &AnyError) {
    log::error!("Error linting: {file_path}");
    log::error!("   {err}");
  }

  fn close(&mut self, check_count: usize) {
    match self.lint_count {
      1 => info!("Found 1 problem"),
      n if n > 1 => info!("Found {} problems", self.lint_count),
      _ => (),
    }

    match check_count {
      1 => info!("Checked 1 file"),
      n => info!("Checked {} files", n),
    }
  }
}

// WARNING: Ensure doesn't change because it's used in the JSON output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonDiagnosticLintPosition {
  /// The 1-indexed line number.
  pub line: usize,
  /// The 0-indexed column index.
  pub col: usize,
  pub byte_pos: usize,
}

impl JsonDiagnosticLintPosition {
  pub fn new(byte_index: usize, loc: deno_ast::LineAndColumnIndex) -> Self {
    JsonDiagnosticLintPosition {
      line: loc.line_index + 1,
      col: loc.column_index,
      byte_pos: byte_index,
    }
  }
}

// WARNING: Ensure doesn't change because it's used in the JSON output
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct JsonLintDiagnosticRange {
  pub start: JsonDiagnosticLintPosition,
  pub end: JsonDiagnosticLintPosition,
}

// WARNING: Ensure doesn't change because it's used in the JSON output
#[derive(Clone, Serialize)]
struct JsonLintDiagnostic {
  pub filename: String,
  pub range: Option<JsonLintDiagnosticRange>,
  pub message: String,
  pub code: String,
  pub hint: Option<String>,
}

#[derive(Serialize)]
struct JsonLintReporter {
  version: u8,
  diagnostics: Vec<JsonLintDiagnostic>,
  errors: Vec<LintError>,
}

impl JsonLintReporter {
  fn new() -> JsonLintReporter {
    JsonLintReporter {
      version: JSON_SCHEMA_VERSION,
      diagnostics: Vec::new(),
      errors: Vec::new(),
    }
  }
}

impl LintReporter for JsonLintReporter {
  fn visit_diagnostic(&mut self, d: &LintDiagnostic) {
    self.diagnostics.push(JsonLintDiagnostic {
      filename: d.specifier.to_string(),
      range: d.range.as_ref().map(|range| {
        let text_info = &range.text_info;
        let range = range.range;
        JsonLintDiagnosticRange {
          start: JsonDiagnosticLintPosition::new(
            range.start.as_byte_index(text_info.range().start),
            text_info.line_and_column_index(range.start),
          ),
          end: JsonDiagnosticLintPosition::new(
            range.end.as_byte_index(text_info.range().start),
            text_info.line_and_column_index(range.end),
          ),
        }
      }),
      message: d.message().to_string(),
      code: d.code().to_string(),
      hint: d.hint().map(|h| h.to_string()),
    });
  }

  fn visit_error(&mut self, file_path: &str, err: &AnyError) {
    self.errors.push(LintError {
      file_path: file_path.to_string(),
      message: err.to_string(),
    });
  }

  fn close(&mut self, _check_count: usize) {
    sort_diagnostics(&mut self.diagnostics);
    let json = serde_json::to_string_pretty(&self);
    #[allow(clippy::print_stdout)]
    {
      println!("{}", json.unwrap());
    }
  }
}

fn sort_diagnostics(diagnostics: &mut [JsonLintDiagnostic]) {
  // Sort so that we guarantee a deterministic output which is useful for tests
  diagnostics.sort_by(|a, b| {
    use std::cmp::Ordering;
    let file_order = a.filename.cmp(&b.filename);
    match file_order {
      Ordering::Equal => match &a.range {
        Some(a_range) => match &b.range {
          Some(b_range) => {
            let line_order = a_range.start.line.cmp(&b_range.start.line);
            match line_order {
              Ordering::Equal => a_range.start.col.cmp(&b_range.start.col),
              _ => line_order,
            }
          }
          None => Ordering::Less,
        },
        None => match &b.range {
          Some(_) => Ordering::Greater,
          None => Ordering::Equal,
        },
      },
      _ => file_order,
    }
  });
}
