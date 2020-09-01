// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module provides file formating utilities using
//! [`deno_lint`](https://github.com/denoland/deno_lint).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.
use crate::colors;
use crate::file_fetcher::map_file_extension;
use crate::fmt::collect_files;
use crate::fmt::run_parallelized;
use crate::fmt_errors;
use crate::msg;
use crate::swc_util;
use deno_core::ErrBox;
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
  args: Vec<String>,
  ignore: Vec<String>,
  json: bool,
) -> Result<(), ErrBox> {
  if args.len() == 1 && args[0] == "-" {
    return lint_stdin(json);
  }
  let mut target_files = collect_files(args)?;
  if !ignore.is_empty() {
    // collect all files to be ignored
    // and retain only files that should be linted.
    let ignore_files = collect_files(ignore)?;
    target_files.retain(|f| !ignore_files.contains(&f));
  }
  debug!("Found {} files", target_files.len());

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
        Ok(file_diagnostics) => {
          for d in file_diagnostics.iter() {
            has_error.store(true, Ordering::Relaxed);
            reporter.visit(&d);
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

  reporter_lock.lock().unwrap().close();

  if has_error {
    std::process::exit(1);
  }

  Ok(())
}

pub fn print_rules_list() {
  let lint_rules = rules::get_recommended_rules();

  println!("Available rules:");
  for rule in lint_rules {
    println!(" - {}", rule.code());
  }
}

fn create_linter(syntax: Syntax, rules: Vec<Box<dyn LintRule>>) -> Linter {
  LinterBuilder::default()
    .ignore_file_directives(vec!["deno-lint-ignore-file"])
    .ignore_diagnostic_directives(vec![
      "deno-lint-ignore",
      "eslint-disable-next-line",
    ])
    .lint_unused_ignore_directives(true)
    // TODO(bartlomieju): switch to true
    .lint_unknown_rules(false)
    .syntax(syntax)
    .rules(rules)
    .build()
}

fn lint_file(file_path: PathBuf) -> Result<Vec<LintDiagnostic>, ErrBox> {
  let file_name = file_path.to_string_lossy().to_string();
  let source_code = fs::read_to_string(&file_path)?;
  let media_type = map_file_extension(&file_path);
  let syntax = swc_util::get_syntax_for_media_type(media_type);

  let lint_rules = rules::get_recommended_rules();
  let mut linter = create_linter(syntax, lint_rules);

  let file_diagnostics = linter.lint(file_name, source_code)?;

  Ok(file_diagnostics)
}

/// Lint stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--json` flag.
fn lint_stdin(json: bool) -> Result<(), ErrBox> {
  let mut source = String::new();
  if stdin().read_to_string(&mut source).is_err() {
    return Err(ErrBox::error("Failed to read from stdin"));
  }

  let reporter_kind = if json {
    LintReporterKind::Json
  } else {
    LintReporterKind::Pretty
  };
  let mut reporter = create_reporter(reporter_kind);
  let lint_rules = rules::get_recommended_rules();
  let syntax = swc_util::get_syntax_for_media_type(msg::MediaType::TypeScript);
  let mut linter = create_linter(syntax, lint_rules);
  let mut has_error = false;
  let pseudo_file_name = "_stdin.ts";
  match linter
    .lint(pseudo_file_name.to_string(), source)
    .map_err(|e| e.into())
  {
    Ok(diagnostics) => {
      for d in diagnostics {
        has_error = true;
        reporter.visit(&d);
      }
    }
    Err(err) => {
      has_error = true;
      reporter.visit_error(pseudo_file_name, &err);
    }
  }

  reporter.close();

  if has_error {
    std::process::exit(1);
  }

  Ok(())
}

trait LintReporter {
  fn visit(&mut self, d: &LintDiagnostic);
  fn visit_error(&mut self, file_path: &str, err: &ErrBox);
  fn close(&mut self);
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
  fn visit(&mut self, d: &LintDiagnostic) {
    self.lint_count += 1;

    let pretty_message =
      format!("({}) {}", colors::gray(&d.code), d.message.clone());

    let message = fmt_errors::format_stack(
      true,
      &pretty_message,
      Some(&d.line_src),
      Some(d.location.col as i64),
      Some((d.location.col + d.snippet_length) as i64),
      &[fmt_errors::format_location(
        &d.filename,
        d.location.line as i64,
        d.location.col as i64,
      )],
      0,
    );

    eprintln!("{}\n", message);
  }

  fn visit_error(&mut self, file_path: &str, err: &ErrBox) {
    eprintln!("Error linting: {}", file_path);
    eprintln!("   {}", err);
  }

  fn close(&mut self) {
    match self.lint_count {
      1 => eprintln!("Found 1 problem"),
      n if n > 1 => eprintln!("Found {} problems", self.lint_count),
      _ => (),
    }
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
  fn visit(&mut self, d: &LintDiagnostic) {
    self.diagnostics.push(d.clone());
  }

  fn visit_error(&mut self, file_path: &str, err: &ErrBox) {
    self.errors.push(LintError {
      file_path: file_path.to_string(),
      message: err.to_string(),
    });
  }

  fn close(&mut self) {
    // Sort so that we guarantee a deterministic output which is useful for tests
    self.diagnostics.sort_by_key(|key| get_sort_key(&key));

    let json = serde_json::to_string_pretty(&self);
    eprintln!("{}", json.unwrap());
  }
}

pub fn get_sort_key(a: &LintDiagnostic) -> String {
  let location = &a.location;

  return format!("{}:{}:{}", a.filename, location.line, location.col);
}
