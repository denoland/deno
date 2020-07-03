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
use crate::swc_ecma_parser::Syntax;
use crate::swc_util;
use deno_core::ErrBox;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::Linter;
use deno_lint::linter::LinterBuilder;
use deno_lint::rules;
use deno_lint::rules::LintRule;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub async fn lint_files(args: Vec<String>) -> Result<(), ErrBox> {
  let target_files = collect_files(args)?;
  debug!("Found {} files", target_files.len());

  let error_count = Arc::new(AtomicUsize::new(0));

  // prevent threads outputting at the same time
  let output_lock = Arc::new(Mutex::new(0));

  run_parallelized(target_files, {
    let error_count = error_count.clone();
    move |file_path| {
      let r = lint_file(file_path.clone());

      match r {
        Ok(file_diagnostics) => {
          error_count.fetch_add(file_diagnostics.len(), Ordering::SeqCst);
          let _g = output_lock.lock().unwrap();
          for d in file_diagnostics.iter() {
            let fmt_diagnostic = format_diagnostic(d);
            eprintln!("{}\n", fmt_diagnostic);
          }
        }
        Err(err) => {
          eprintln!("Error linting: {}", file_path.to_string_lossy());
          eprintln!("   {}", err);
        }
      }
      Ok(())
    }
  })
  .await?;

  let error_count = error_count.load(Ordering::SeqCst);
  if error_count > 0 {
    eprintln!("Found {} problems", error_count);
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

fn format_diagnostic(d: &LintDiagnostic) -> String {
  let pretty_message =
    format!("({}) {}", colors::gray(&d.code), d.message.clone());

  fmt_errors::format_stack(
    true,
    &pretty_message,
    Some(&d.line_src),
    Some(d.location.col as i64),
    Some((d.location.col + d.snippet_length) as i64),
    &[fmt_errors::format_location(
      &d.location.filename,
      d.location.line as i64,
      d.location.col as i64,
    )],
    0,
  )
}
