// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! This module provides file formating utilities using
//! [`deno_lint`](https://github.com/denoland/deno_lint).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.
use crate::ast;
use crate::colors;
use crate::fmt::run_parallelized;
use crate::fmt_errors;
use crate::fs_util::{collect_files, is_supported_ext};
use crate::media_type::MediaType;
use deno_core::error::{generic_error, AnyError, JsStackFrame};
use deno_core::serde_json;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::Linter;
use deno_lint::linter::LinterBuilder;
use deno_lint::rules;
use deno_lint::rules::LintRule;
use serde::Serialize;
use std::fs;
use std::io::{stdin, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use swc_ecmascript::parser::Syntax;

pub enum LintReporterKind {
  Pretty,
  Json,
}

fn create_reporter(kind: LintReporterKind) -> Box<dyn LintReporter + Send> {
  match kind {
    LintReporterKind::Pretty => Box::new(PrettyLintReporter::new()),
    LintReporterKind::Json => Box::new(JsonLintReporter::new()),
  }
}

pub async fn lint_files(
  args: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
  json: bool,
) -> Result<(), AnyError> {
  if args.len() == 1 && args[0].to_string_lossy() == "-" {
    return lint_stdin(json);
  }
  let target_files = collect_files(args, ignore, is_supported_ext)?;
  debug!("Found {} files", target_files.len());
  let target_files_len = target_files.len();

  let has_error = Arc::new(AtomicBool::new(false));

  let reporter_kind = if json {
    LintReporterKind::Json
  } else {
    LintReporterKind::Pretty
  };
  let reporter_lock = Arc::new(Mutex::new(create_reporter(reporter_kind)));

  run_parallelized(target_files, {
    let reporter_lock = reporter_lock.clone();
    let has_error = has_error.clone();
    move |file_path| {
      let r = lint_file(file_path.clone());
      let mut reporter = reporter_lock.lock().unwrap();

      match r {
        Ok((mut file_diagnostics, source)) => {
          sort_diagnostics(&mut file_diagnostics);
          for d in file_diagnostics.iter() {
            has_error.store(true, Ordering::Relaxed);
            reporter.visit_diagnostic(&d, source.split('\n').collect());
          }
        }
        Err(err) => {
          has_error.store(true, Ordering::Relaxed);
          reporter.visit_error(&file_path.to_string_lossy().to_string(), &err);
        }
      }
      Ok(())
    }
  })
  .await?;

  let has_error = has_error.load(Ordering::Relaxed);

  reporter_lock.lock().unwrap().close(target_files_len);

  if has_error {
    std::process::exit(1);
  }

  Ok(())
}

fn rule_to_json(rule: Box<dyn LintRule>) -> serde_json::Value {
  serde_json::json!({
    "code": rule.code(),
    "tags": rule.tags(),
    "docs": rule.docs(),
  })
}

pub fn print_rules_list(json: bool) {
  let lint_rules = rules::get_recommended_rules();

  if json {
    let json_rules: Vec<serde_json::Value> =
      lint_rules.into_iter().map(rule_to_json).collect();
    let json_str = serde_json::to_string_pretty(&json_rules).unwrap();
    println!("{}", json_str);
  } else {
    // The rules should still be printed even if `--quiet` option is enabled,
    // so use `println!` here instead of `info!`.
    println!("Available rules:");
    for rule in lint_rules {
      println!(" - {}", rule.code());
    }
  }
}

fn create_linter(syntax: Syntax, rules: Vec<Box<dyn LintRule>>) -> Linter {
  LinterBuilder::default()
    .ignore_file_directive("deno-lint-ignore-file")
    .ignore_diagnostic_directive("deno-lint-ignore")
    .lint_unused_ignore_directives(true)
    // TODO(bartlomieju): switch to true
    .lint_unknown_rules(false)
    .syntax(syntax)
    .rules(rules)
    .build()
}

fn lint_file(
  file_path: PathBuf,
) -> Result<(Vec<LintDiagnostic>, String), AnyError> {
  let file_name = file_path.to_string_lossy().to_string();
  let source_code = fs::read_to_string(&file_path)?;
  let media_type = MediaType::from(&file_path);
  let syntax = ast::get_syntax(&media_type);

  let lint_rules = rules::get_recommended_rules();
  let mut linter = create_linter(syntax, lint_rules);

  let (_, file_diagnostics) = linter.lint(file_name, source_code.clone())?;

  Ok((file_diagnostics, source_code))
}

/// Lint stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--json` flag.
fn lint_stdin(json: bool) -> Result<(), AnyError> {
  let mut source = String::new();
  if stdin().read_to_string(&mut source).is_err() {
    return Err(generic_error("Failed to read from stdin"));
  }

  let reporter_kind = if json {
    LintReporterKind::Json
  } else {
    LintReporterKind::Pretty
  };
  let mut reporter = create_reporter(reporter_kind);
  let lint_rules = rules::get_recommended_rules();
  let syntax = ast::get_syntax(&MediaType::TypeScript);
  let mut linter = create_linter(syntax, lint_rules);
  let mut has_error = false;
  let pseudo_file_name = "_stdin.ts";
  match linter
    .lint(pseudo_file_name.to_string(), source.clone())
    .map_err(|e| e.into())
  {
    Ok((_, diagnostics)) => {
      for d in diagnostics {
        has_error = true;
        reporter.visit_diagnostic(&d, source.split('\n').collect());
      }
    }
    Err(err) => {
      has_error = true;
      reporter.visit_error(pseudo_file_name, &err);
    }
  }

  reporter.close(1);

  if has_error {
    std::process::exit(1);
  }

  Ok(())
}

trait LintReporter {
  fn visit_diagnostic(&mut self, d: &LintDiagnostic, source_lines: Vec<&str>);
  fn visit_error(&mut self, file_path: &str, err: &AnyError);
  fn close(&mut self, check_count: usize);
}

#[derive(Serialize)]
struct LintError {
  file_path: String,
  message: String,
}

struct PrettyLintReporter {
  lint_count: u32,
}

impl PrettyLintReporter {
  fn new() -> PrettyLintReporter {
    PrettyLintReporter { lint_count: 0 }
  }
}

impl LintReporter for PrettyLintReporter {
  fn visit_diagnostic(&mut self, d: &LintDiagnostic, source_lines: Vec<&str>) {
    self.lint_count += 1;

    let pretty_message =
      format!("({}) {}", colors::gray(&d.code), d.message.clone());

    let message = format_diagnostic(
      &pretty_message,
      &source_lines,
      d.range.clone(),
      d.hint.as_ref(),
      &fmt_errors::format_location(&JsStackFrame::from_location(
        Some(d.filename.clone()),
        Some(d.range.start.line as i64),
        Some(d.range.start.col as i64),
      )),
    );

    eprintln!("{}\n", message);
  }

  fn visit_error(&mut self, file_path: &str, err: &AnyError) {
    eprintln!("Error linting: {}", file_path);
    eprintln!("   {}", err);
  }

  fn close(&mut self, check_count: usize) {
    match self.lint_count {
      1 => info!("Found 1 problem"),
      n if n > 1 => info!("Found {} problems", self.lint_count),
      _ => (),
    }

    match check_count {
      n if n <= 1 => info!("Checked {} file", n),
      n if n > 1 => info!("Checked {} files", n),
      _ => unreachable!(),
    }
  }
}

pub fn format_diagnostic(
  message_line: &str,
  source_lines: &[&str],
  range: deno_lint::diagnostic::Range,
  maybe_hint: Option<&String>,
  formatted_location: &str,
) -> String {
  let mut lines = vec![];

  for i in range.start.line..=range.end.line {
    lines.push(source_lines[i - 1].to_string());
    if range.start.line == range.end.line {
      lines.push(format!(
        "{}{}",
        " ".repeat(range.start.col),
        colors::red(&"^".repeat(range.end.col - range.start.col))
      ));
    } else {
      let line_len = source_lines[i - 1].len();
      if range.start.line == i {
        lines.push(format!(
          "{}{}",
          " ".repeat(range.start.col),
          colors::red(&"^".repeat(line_len - range.start.col))
        ));
      } else if range.end.line == i {
        lines.push(colors::red(&"^".repeat(range.end.col)).to_string());
      } else if line_len != 0 {
        lines.push(colors::red(&"^".repeat(line_len)).to_string());
      }
    }
  }

  if let Some(hint) = maybe_hint {
    format!(
      "{}\n{}\n    at {}\n\n    {} {}",
      message_line,
      lines.join("\n"),
      formatted_location,
      colors::gray("hint:"),
      hint,
    )
  } else {
    format!(
      "{}\n{}\n    at {}",
      message_line,
      lines.join("\n"),
      formatted_location
    )
  }
}

#[derive(Serialize)]
struct JsonLintReporter {
  diagnostics: Vec<LintDiagnostic>,
  errors: Vec<LintError>,
}

impl JsonLintReporter {
  fn new() -> JsonLintReporter {
    JsonLintReporter {
      diagnostics: Vec::new(),
      errors: Vec::new(),
    }
  }
}

impl LintReporter for JsonLintReporter {
  fn visit_diagnostic(&mut self, d: &LintDiagnostic, _source_lines: Vec<&str>) {
    self.diagnostics.push(d.clone());
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
    eprintln!("{}", json.unwrap());
  }
}

fn sort_diagnostics(diagnostics: &mut Vec<LintDiagnostic>) {
  // Sort so that we guarantee a deterministic output which is useful for tests
  diagnostics.sort_by(|a, b| {
    use std::cmp::Ordering;
    let file_order = a.filename.cmp(&b.filename);
    match file_order {
      Ordering::Equal => {
        let line_order = a.range.start.line.cmp(&b.range.start.line);
        match line_order {
          Ordering::Equal => a.range.start.col.cmp(&b.range.start.col),
          _ => line_order,
        }
      }
      _ => file_order,
    }
  });
}
