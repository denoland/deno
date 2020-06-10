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
use crate::fmt_errors;
use crate::swc_util;
use deno_core::ErrBox;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::Linter;
use deno_lint::rules;
use std::fs;
use std::path::PathBuf;

pub fn lint_files(args: Vec<String>) -> Result<(), ErrBox> {
  let target_files = collect_files(args)?;

  let mut error_counts = 0;

  for file_path in target_files {
    let file_diagnostics = lint_file(file_path)?;
    error_counts += file_diagnostics.len();
    for d in file_diagnostics.iter() {
      let fmt_diagnostic = format_diagnostic(d);
      eprintln!("{}\n", fmt_diagnostic);
    }
  }

  if error_counts > 0 {
    eprintln!("Found {} problems", error_counts);
    std::process::exit(1);
  }

  Ok(())
}

fn lint_file(file_path: PathBuf) -> Result<Vec<LintDiagnostic>, ErrBox> {
  let file_name = file_path.to_string_lossy().to_string();
  let source_code = fs::read_to_string(&file_path)?;
  let media_type = map_file_extension(&file_path);
  let syntax = swc_util::get_syntax_for_media_type(media_type);

  let mut linter = Linter::default();
  let lint_rules = rules::get_recommended_rules();

  let file_diagnostics =
    linter.lint(file_name, source_code, syntax, lint_rules)?;

  Ok(file_diagnostics)
}

fn format_diagnostic(d: &LintDiagnostic) -> String {
  let pretty_message = format!(
    "({}) {}",
    colors::gray(d.code.to_string()),
    d.message.clone()
  );

  fmt_errors::format_stack(
    true,
    pretty_message,
    Some(d.line_src.clone()),
    Some(d.location.col as i64),
    Some((d.location.col + d.snippet_length) as i64),
    &[fmt_errors::format_location(
      d.location.filename.clone(),
      d.location.line as i64,
      d.location.col as i64,
    )],
    0,
  )
}
