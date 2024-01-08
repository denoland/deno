// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

//! This module provides file linting utilities using
//! [`deno_lint`](https://github.com/denoland/deno_lint).
use crate::args::FilesConfig;
use crate::args::Flags;
use crate::args::LintFlags;
use crate::args::LintOptions;
use crate::args::LintReporterKind;
use crate::args::LintRulesConfig;
use crate::colors;
use crate::factory::CliFactory;
use crate::tools::fmt::run_parallelized;
use crate::util::file_watcher;
use crate::util::fs::FileCollector;
use crate::util::path::is_script_ext;
use crate::util::sync::AtomicFlag;
use deno_ast::MediaType;
use deno_core::anyhow::bail;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsStackFrame;
use deno_core::serde_json;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::Linter;
use deno_lint::linter::LinterBuilder;
use deno_lint::rules;
use deno_lint::rules::LintRule;
use deno_runtime::fmt_errors::format_location;
use log::debug;
use log::info;
use serde::Serialize;
use std::fs;
use std::io::stdin;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crate::cache::IncrementalCache;

static STDIN_FILE_NAME: &str = "_stdin.ts";

fn create_reporter(kind: LintReporterKind) -> Box<dyn LintReporter + Send> {
  match kind {
    LintReporterKind::Pretty => Box::new(PrettyLintReporter::new()),
    LintReporterKind::Json => Box::new(JsonLintReporter::new()),
    LintReporterKind::Compact => Box::new(CompactLintReporter::new()),
  }
}

pub async fn lint(flags: Flags, lint_flags: LintFlags) -> Result<(), AnyError> {
  if let Some(watch_flags) = &lint_flags.watch {
    if lint_flags.is_stdin() {
      return Err(generic_error(
        "Lint watch on standard input is not supported.",
      ));
    }
    file_watcher::watch_func(
      flags,
      file_watcher::PrintConfig::new("Lint", !watch_flags.no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let lint_flags = lint_flags.clone();
        Ok(async move {
          let factory = CliFactory::from_flags(flags).await?;
          let cli_options = factory.cli_options();
          let lint_options = cli_options.resolve_lint_options(lint_flags)?;
          let files =
            collect_lint_files(&lint_options.files).and_then(|files| {
              if files.is_empty() {
                Err(generic_error("No target files found."))
              } else {
                Ok(files)
              }
            })?;
          _ = watcher_communicator.watch_paths(files.clone());

          let lint_paths = if let Some(paths) = changed_paths {
            // lint all files on any changed (https://github.com/denoland/deno/issues/12446)
            files
              .iter()
              .any(|path| paths.contains(path))
              .then_some(files)
              .unwrap_or_else(|| [].to_vec())
          } else {
            files
          };

          lint_files(factory, lint_options, lint_paths).await?;
          Ok(())
        })
      },
    )
    .await?;
  } else {
    let factory = CliFactory::from_flags(flags).await?;
    let cli_options = factory.cli_options();
    let is_stdin = lint_flags.is_stdin();
    let lint_options = cli_options.resolve_lint_options(lint_flags)?;
    let files = &lint_options.files;
    let success = if is_stdin {
      let reporter_kind = lint_options.reporter_kind;
      let reporter_lock = Arc::new(Mutex::new(create_reporter(reporter_kind)));
      let lint_rules = get_config_rules_err_empty(lint_options.rules)?;
      let r = lint_stdin(lint_rules);
      let success =
        handle_lint_result(STDIN_FILE_NAME, r, reporter_lock.clone());
      reporter_lock.lock().unwrap().close(1);
      success
    } else {
      let target_files = collect_lint_files(files).and_then(|files| {
        if files.is_empty() {
          Err(generic_error("No target files found."))
        } else {
          Ok(files)
        }
      })?;
      debug!("Found {} files", target_files.len());
      lint_files(factory, lint_options, target_files).await?
    };
    if !success {
      std::process::exit(1);
    }
  }

  Ok(())
}

async fn lint_files(
  factory: CliFactory,
  lint_options: LintOptions,
  paths: Vec<PathBuf>,
) -> Result<bool, AnyError> {
  let caches = factory.caches()?;
  let lint_rules = get_config_rules_err_empty(lint_options.rules)?;
  let incremental_cache = Arc::new(IncrementalCache::new(
    caches.lint_incremental_cache_db(),
    // use a hash of the rule names in order to bust the cache
    &{
      // ensure this is stable by sorting it
      let mut names = lint_rules.iter().map(|r| r.code()).collect::<Vec<_>>();
      names.sort_unstable();
      names
    },
    &paths,
  ));
  let target_files_len = paths.len();
  let reporter_kind = lint_options.reporter_kind;
  let reporter_lock =
    Arc::new(Mutex::new(create_reporter(reporter_kind.clone())));
  let has_error = Arc::new(AtomicFlag::default());

  run_parallelized(paths, {
    let has_error = has_error.clone();
    let lint_rules = lint_rules.clone();
    let reporter_lock = reporter_lock.clone();
    let incremental_cache = incremental_cache.clone();
    move |file_path| {
      let file_text = fs::read_to_string(&file_path)?;

      // don't bother rechecking this file if it didn't have any diagnostics before
      if incremental_cache.is_file_same(&file_path, &file_text) {
        return Ok(());
      }

      let r = lint_file(&file_path, file_text, lint_rules);
      if let Ok((file_diagnostics, file_text)) = &r {
        if file_diagnostics.is_empty() {
          // update the incremental cache if there were no diagnostics
          incremental_cache.update_file(&file_path, file_text)
        }
      }

      let success = handle_lint_result(
        &file_path.to_string_lossy(),
        r,
        reporter_lock.clone(),
      );
      if !success {
        has_error.raise();
      }

      Ok(())
    }
  })
  .await?;
  incremental_cache.wait_completion().await;
  reporter_lock.lock().unwrap().close(target_files_len);

  Ok(!has_error.is_raised())
}

fn collect_lint_files(files: &FilesConfig) -> Result<Vec<PathBuf>, AnyError> {
  FileCollector::new(is_script_ext)
    .ignore_git_folder()
    .ignore_node_modules()
    .ignore_vendor_folder()
    .add_ignore_paths(&files.exclude)
    .collect_files(files.include.as_deref())
}

pub fn print_rules_list(json: bool, maybe_rules_tags: Option<Vec<String>>) {
  let lint_rules = if maybe_rules_tags.is_none() {
    rules::get_all_rules()
  } else {
    rules::get_filtered_rules(maybe_rules_tags, None, None)
  };

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
    println!("{json_str}");
  } else {
    // The rules should still be printed even if `--quiet` option is enabled,
    // so use `println!` here instead of `info!`.
    println!("Available rules:");
    for rule in lint_rules.iter() {
      print!(" - {}", colors::cyan(rule.code()));
      if rule.tags().is_empty() {
        println!();
      } else {
        println!(" [{}]", colors::gray(rule.tags().join(", ")))
      }
      println!(
        "{}",
        colors::gray(format!(
          "   help: https://lint.deno.land/#{}",
          rule.code()
        ))
      );
      println!();
    }
  }
}

pub fn create_linter(
  media_type: MediaType,
  rules: Vec<&'static dyn LintRule>,
) -> Linter {
  LinterBuilder::default()
    .ignore_file_directive("deno-lint-ignore-file")
    .ignore_diagnostic_directive("deno-lint-ignore")
    .media_type(media_type)
    .rules(rules)
    .build()
}

fn lint_file(
  file_path: &Path,
  source_code: String,
  lint_rules: Vec<&'static dyn LintRule>,
) -> Result<(Vec<LintDiagnostic>, String), AnyError> {
  let file_name = file_path.to_string_lossy().to_string();
  let media_type = MediaType::from_path(file_path);

  let linter = create_linter(media_type, lint_rules);

  let (_, file_diagnostics) = linter.lint(file_name, source_code.clone())?;

  Ok((file_diagnostics, source_code))
}

/// Lint stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--json` flag.
fn lint_stdin(
  lint_rules: Vec<&'static dyn LintRule>,
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
) -> bool {
  let mut reporter = reporter_lock.lock().unwrap();

  match result {
    Ok((mut file_diagnostics, source)) => {
      sort_diagnostics(&mut file_diagnostics);
      for d in file_diagnostics.iter() {
        reporter.visit_diagnostic(d, source.split('\n').collect());
      }
      file_diagnostics.is_empty()
    }
    Err(err) => {
      reporter.visit_error(file_path, &err);
      false
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
      &d.range,
      d.hint.as_ref(),
      &format_location(&JsStackFrame::from_location(
        Some(d.filename.clone()),
        // todo(dsherret): these should use "display positions"
        // which take into account the added column index of tab
        // indentation
        Some(d.range.start.line_index as i64 + 1),
        Some(d.range.start.column_index as i64 + 1),
      )),
    );

    eprintln!("{message}\n");
  }

  fn visit_error(&mut self, file_path: &str, err: &AnyError) {
    eprintln!("Error linting: {file_path}");
    eprintln!("   {err}");
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

struct CompactLintReporter {
  lint_count: u32,
}

impl CompactLintReporter {
  fn new() -> CompactLintReporter {
    CompactLintReporter { lint_count: 0 }
  }
}

impl LintReporter for CompactLintReporter {
  fn visit_diagnostic(&mut self, d: &LintDiagnostic, _source_lines: Vec<&str>) {
    self.lint_count += 1;

    eprintln!(
      "{}: line {}, col {} - {} ({})",
      d.filename,
      d.range.start.line_index + 1,
      d.range.start.column_index + 1,
      d.message,
      d.code
    )
  }

  fn visit_error(&mut self, file_path: &str, err: &AnyError) {
    eprintln!("Error linting: {file_path}");
    eprintln!("   {err}");
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
  range: &deno_lint::diagnostic::Range,
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

fn sort_diagnostics(diagnostics: &mut [LintDiagnostic]) {
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

fn get_config_rules_err_empty(
  rules: LintRulesConfig,
) -> Result<Vec<&'static dyn LintRule>, AnyError> {
  let lint_rules = get_configured_rules(rules);
  if lint_rules.is_empty() {
    bail!("No rules have been configured")
  }
  Ok(lint_rules)
}

pub fn get_configured_rules(
  rules: LintRulesConfig,
) -> Vec<&'static dyn LintRule> {
  if rules.tags.is_none() && rules.include.is_none() && rules.exclude.is_none()
  {
    rules::get_recommended_rules()
  } else {
    rules::get_filtered_rules(
      rules.tags.or_else(|| Some(vec!["recommended".to_string()])),
      rules.exclude,
      rules.include,
    )
  }
}

#[cfg(test)]
mod test {
  use deno_lint::rules::get_recommended_rules;

  use super::*;
  use crate::args::LintRulesConfig;

  #[test]
  fn recommended_rules_when_no_tags_in_config() {
    let rules_config = LintRulesConfig {
      exclude: Some(vec!["no-debugger".to_string()]),
      include: None,
      tags: None,
    };
    let rules = get_configured_rules(rules_config);
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
