// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! This module provides file linting utilities using
//! [`deno_lint`](https://github.com/denoland/deno_lint).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.
use crate::config_file::LintConfig;
use crate::file_watcher::ResolutionResult;
use crate::flags::{Flags, LintFlags};
use crate::fmt_errors;
use crate::fs_util::{collect_files, is_supported_ext, specifier_to_file_path};
use crate::proc_state::ProcState;
use crate::tools::fmt::run_parallelized;
use crate::{colors, file_watcher};
use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsStackFrame;
use deno_core::serde_json;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::Linter;
use deno_lint::linter::LinterBuilder;
use deno_lint::rules;
use deno_lint::rules::LintRule;
use log::debug;
use log::info;
use serde::Serialize;
use std::fs;
use std::io::{stdin, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

static STDIN_FILE_NAME: &str = "_stdin.ts";

#[derive(Clone, Debug)]
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

pub async fn lint(flags: Flags, lint_flags: LintFlags) -> Result<(), AnyError> {
  let flags = Arc::new(flags);
  let LintFlags {
    maybe_rules_tags,
    maybe_rules_include,
    maybe_rules_exclude,
    files: args,
    ignore,
    json,
    ..
  } = lint_flags;
  // First, prepare final configuration.
  // Collect included and ignored files. CLI flags take precendence
  // over config file, ie. if there's `files.ignore` in config file
  // and `--ignore` CLI flag, only the flag value is taken into account.
  let mut include_files = args.clone();
  let mut exclude_files = ignore.clone();

  let ps = ProcState::build(flags.clone()).await?;
  let maybe_lint_config = if let Some(config_file) = &ps.maybe_config_file {
    config_file.to_lint_config()?
  } else {
    None
  };

  if let Some(lint_config) = maybe_lint_config.as_ref() {
    if include_files.is_empty() {
      include_files = lint_config
        .files
        .include
        .iter()
        .filter_map(|s| specifier_to_file_path(s).ok())
        .collect::<Vec<_>>();
    }

    if exclude_files.is_empty() {
      exclude_files = lint_config
        .files
        .exclude
        .iter()
        .filter_map(|s| specifier_to_file_path(s).ok())
        .collect::<Vec<_>>();
    }
  }

  if include_files.is_empty() {
    include_files = [std::env::current_dir()?].to_vec();
  }

  let reporter_kind = if json {
    LintReporterKind::Json
  } else {
    LintReporterKind::Pretty
  };

  let has_error = Arc::new(AtomicBool::new(false));
  // Try to get configured rules. CLI flags take precendence
  // over config file, ie. if there's `rules.include` in config file
  // and `--rules-include` CLI flag, only the flag value is taken into account.
  let lint_rules = get_configured_rules(
    maybe_lint_config.as_ref(),
    maybe_rules_tags,
    maybe_rules_include,
    maybe_rules_exclude,
  )?;

  let resolver = |changed: Option<Vec<PathBuf>>| {
    let files_changed = changed.is_some();
    let result =
      collect_files(&include_files, &exclude_files, is_supported_ext).map(
        |files| {
          if let Some(paths) = changed {
            files
              .iter()
              .any(|path| paths.contains(path))
              .then(|| files)
              .unwrap_or_else(|| [].to_vec())
          } else {
            files
          }
        },
      );

    let paths_to_watch = include_files.clone();

    async move {
      if files_changed && matches!(result, Ok(ref files) if files.is_empty()) {
        ResolutionResult::Ignore
      } else {
        ResolutionResult::Restart {
          paths_to_watch,
          result,
        }
      }
    }
  };

  let operation = |paths: Vec<PathBuf>| async {
    let target_files_len = paths.len();
    let reporter_kind = reporter_kind.clone();
    let reporter_lock = Arc::new(Mutex::new(create_reporter(reporter_kind)));
    run_parallelized(paths, {
      let has_error = has_error.clone();
      let lint_rules = lint_rules.clone();
      let reporter_lock = reporter_lock.clone();
      move |file_path| {
        let r = lint_file(file_path.clone(), lint_rules.clone());
        handle_lint_result(
          &file_path.to_string_lossy(),
          r,
          reporter_lock.clone(),
          has_error,
        );

        Ok(())
      }
    })
    .await?;
    reporter_lock.lock().unwrap().close(target_files_len);

    Ok(())
  };
  if flags.watch.is_some() {
    if args.len() == 1 && args[0].to_string_lossy() == "-" {
      return Err(generic_error(
        "Lint watch on standard input is not supported.",
      ));
    }
    file_watcher::watch_func(
      resolver,
      operation,
      file_watcher::PrintConfig {
        job_name: "Lint".to_string(),
        clear_screen: !flags.no_clear_screen,
      },
    )
    .await?;
  } else {
    if args.len() == 1 && args[0].to_string_lossy() == "-" {
      let reporter_lock =
        Arc::new(Mutex::new(create_reporter(reporter_kind.clone())));
      let r = lint_stdin(lint_rules);
      handle_lint_result(
        STDIN_FILE_NAME,
        r,
        reporter_lock.clone(),
        has_error.clone(),
      );
      reporter_lock.lock().unwrap().close(1);
    } else {
      let target_files =
        collect_files(&include_files, &exclude_files, is_supported_ext)
          .and_then(|files| {
            if files.is_empty() {
              Err(generic_error("No target files found."))
            } else {
              Ok(files)
            }
          })?;
      debug!("Found {} files", target_files.len());
      operation(target_files).await?;
    };
    let has_error = has_error.load(Ordering::Relaxed);
    if has_error {
      std::process::exit(1);
    }
  }

  Ok(())
}

pub fn print_rules_list(json: bool) {
  let lint_rules = rules::get_recommended_rules();

  if json {
    let json_rules: Vec<serde_json::Value> = lint_rules
      .iter()
      .map(|rule| {
        serde_json::json!({
          "code": rule.code(),
          "tags": rule.tags(),
          "docs": rule.docs(),
        })
      })
      .collect();
    let json_str = serde_json::to_string_pretty(&json_rules).unwrap();
    println!("{}", json_str);
  } else {
    // The rules should still be printed even if `--quiet` option is enabled,
    // so use `println!` here instead of `info!`.
    println!("Available rules:");
    for rule in lint_rules.iter() {
      println!(" - {}", rule.code());
      println!("   help: https://lint.deno.land/#{}", rule.code());
      println!();
    }
  }
}

pub fn create_linter(
  media_type: MediaType,
  rules: Vec<Arc<dyn LintRule>>,
) -> Linter {
  LinterBuilder::default()
    .ignore_file_directive("deno-lint-ignore-file")
    .ignore_diagnostic_directive("deno-lint-ignore")
    .media_type(media_type)
    .rules(rules)
    .build()
}

fn lint_file(
  file_path: PathBuf,
  lint_rules: Vec<Arc<dyn LintRule>>,
) -> Result<(Vec<LintDiagnostic>, String), AnyError> {
  let file_name = file_path.to_string_lossy().to_string();
  let source_code = fs::read_to_string(&file_path)?;
  let media_type = MediaType::from(&file_path);

  let linter = create_linter(media_type, lint_rules);

  let (_, file_diagnostics) = linter.lint(file_name, source_code.clone())?;

  Ok((file_diagnostics, source_code))
}

/// Lint stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--json` flag.
fn lint_stdin(
  lint_rules: Vec<Arc<dyn LintRule>>,
) -> Result<(Vec<LintDiagnostic>, String), AnyError> {
  let mut source_code = String::new();
  if stdin().read_to_string(&mut source_code).is_err() {
    return Err(generic_error("Failed to read from stdin"));
  }

  let linter = create_linter(MediaType::TypeScript, lint_rules);

  let (_, file_diagnostics) =
    linter.lint(STDIN_FILE_NAME.to_string(), source_code.clone())?;

  Ok((file_diagnostics, source_code))
}

fn handle_lint_result(
  file_path: &str,
  result: Result<(Vec<LintDiagnostic>, String), AnyError>,
  reporter_lock: Arc<Mutex<Box<dyn LintReporter + Send>>>,
  has_error: Arc<AtomicBool>,
) {
  let mut reporter = reporter_lock.lock().unwrap();

  match result {
    Ok((mut file_diagnostics, source)) => {
      sort_diagnostics(&mut file_diagnostics);
      for d in file_diagnostics.iter() {
        has_error.store(true, Ordering::Relaxed);
        reporter.visit_diagnostic(d, source.split('\n').collect());
      }
    }
    Err(err) => {
      has_error.store(true, Ordering::Relaxed);
      reporter.visit_error(file_path, &err);
    }
  }
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

    let pretty_message = format!("({}) {}", colors::red(&d.code), &d.message);

    let message = format_diagnostic(
      &d.code,
      &pretty_message,
      &source_lines,
      d.range.clone(),
      d.hint.as_ref(),
      &fmt_errors::format_location(&JsStackFrame::from_location(
        Some(d.filename.clone()),
        Some(d.range.start.line_index as i64 + 1), // 1-indexed
        // todo(#11111): make 1-indexed as well
        Some(d.range.start.column_index as i64),
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
  diagnostic_code: &str,
  message_line: &str,
  source_lines: &[&str],
  range: deno_lint::diagnostic::Range,
  maybe_hint: Option<&String>,
  formatted_location: &str,
) -> String {
  let mut lines = vec![];

  for (i, line) in source_lines
    .iter()
    .enumerate()
    .take(range.end.line_index + 1)
    .skip(range.start.line_index)
  {
    lines.push(line.to_string());
    if range.start.line_index == range.end.line_index {
      lines.push(format!(
        "{}{}",
        " ".repeat(range.start.column_index),
        colors::red(
          &"^".repeat(range.end.column_index - range.start.column_index)
        )
      ));
    } else {
      let line_len = line.len();
      if range.start.line_index == i {
        lines.push(format!(
          "{}{}",
          " ".repeat(range.start.column_index),
          colors::red(&"^".repeat(line_len - range.start.column_index))
        ));
      } else if range.end.line_index == i {
        lines
          .push(colors::red(&"^".repeat(range.end.column_index)).to_string());
      } else if line_len != 0 {
        lines.push(colors::red(&"^".repeat(line_len)).to_string());
      }
    }
  }

  let hint = if let Some(hint) = maybe_hint {
    format!("    {} {}\n", colors::cyan("hint:"), hint)
  } else {
    "".to_string()
  };
  let help = format!(
    "    {} for further information visit https://lint.deno.land/#{}",
    colors::cyan("help:"),
    diagnostic_code
  );

  format!(
    "{message_line}\n{snippets}\n    at {formatted_location}\n\n{hint}{help}",
    message_line = message_line,
    snippets = lines.join("\n"),
    formatted_location = formatted_location,
    hint = hint,
    help = help
  )
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
    println!("{}", json.unwrap());
  }
}

fn sort_diagnostics(diagnostics: &mut Vec<LintDiagnostic>) {
  // Sort so that we guarantee a deterministic output which is useful for tests
  diagnostics.sort_by(|a, b| {
    use std::cmp::Ordering;
    let file_order = a.filename.cmp(&b.filename);
    match file_order {
      Ordering::Equal => {
        let line_order =
          a.range.start.line_index.cmp(&b.range.start.line_index);
        match line_order {
          Ordering::Equal => {
            a.range.start.column_index.cmp(&b.range.start.column_index)
          }
          _ => line_order,
        }
      }
      _ => file_order,
    }
  });
}

pub(crate) fn get_configured_rules(
  maybe_lint_config: Option<&LintConfig>,
  maybe_rules_tags: Option<Vec<String>>,
  maybe_rules_include: Option<Vec<String>>,
  maybe_rules_exclude: Option<Vec<String>>,
) -> Result<Vec<Arc<dyn LintRule>>, AnyError> {
  if maybe_lint_config.is_none()
    && maybe_rules_tags.is_none()
    && maybe_rules_include.is_none()
    && maybe_rules_exclude.is_none()
  {
    return Ok(rules::get_recommended_rules());
  }

  let (config_file_tags, config_file_include, config_file_exclude) =
    if let Some(lint_config) = maybe_lint_config {
      (
        lint_config.rules.tags.clone(),
        lint_config.rules.include.clone(),
        lint_config.rules.exclude.clone(),
      )
    } else {
      (None, None, None)
    };

  let maybe_configured_include = if maybe_rules_include.is_some() {
    maybe_rules_include
  } else {
    config_file_include
  };

  let maybe_configured_exclude = if maybe_rules_exclude.is_some() {
    maybe_rules_exclude
  } else {
    config_file_exclude
  };

  let maybe_configured_tags = if maybe_rules_tags.is_some() {
    maybe_rules_tags
  } else {
    config_file_tags
  };

  let configured_rules = rules::get_filtered_rules(
    maybe_configured_tags.or_else(|| Some(vec!["recommended".to_string()])),
    maybe_configured_exclude,
    maybe_configured_include,
  );

  if configured_rules.is_empty() {
    anyhow!("No rules have been configured");
  }

  Ok(configured_rules)
}

#[cfg(test)]
mod test {
  use deno_lint::rules::get_recommended_rules;

  use super::*;
  use crate::config_file::LintRulesConfig;

  #[test]
  fn recommended_rules_when_no_tags_in_config() {
    let lint_config = LintConfig {
      rules: LintRulesConfig {
        exclude: Some(vec!["no-debugger".to_string()]),
        ..Default::default()
      },
      ..Default::default()
    };
    let rules =
      get_configured_rules(Some(&lint_config), None, None, None).unwrap();
    let mut rule_names = rules
      .into_iter()
      .map(|r| r.code().to_string())
      .collect::<Vec<_>>();
    rule_names.sort();
    let mut recommended_rule_names = get_recommended_rules()
      .into_iter()
      .map(|r| r.code().to_string())
      .filter(|n| n != "no-debugger")
      .collect::<Vec<_>>();
    recommended_rule_names.sort();
    assert_eq!(rule_names, recommended_rule_names);
  }
}
