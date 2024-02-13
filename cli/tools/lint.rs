// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

//! This module provides file linting utilities using
//! [`deno_lint`](https://github.com/denoland/deno_lint).
use crate::args::Flags;
use crate::args::LintFlags;
use crate::args::LintOptions;
use crate::args::LintReporterKind;
use crate::args::LintRulesConfig;
use crate::colors;
use crate::factory::CliFactory;
use crate::tools::fmt::run_parallelized;
use crate::util::file_watcher;
use crate::util::fs::canonicalize_path;
use crate::util::fs::specifier_from_file_path;
use crate::util::fs::FileCollector;
use crate::util::path::is_script_ext;
use crate::util::sync::AtomicFlag;
use deno_ast::diagnostics::Diagnostic;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_config::glob::FilePatterns;
use deno_core::anyhow::bail;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::LintFileOptions;
use deno_lint::linter::Linter;
use deno_lint::linter::LinterBuilder;
use deno_lint::rules;
use deno_lint::rules::LintRule;
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

static STDIN_FILE_NAME: &str = "$deno$stdin.ts";

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
          let files = collect_lint_files(lint_options.files.clone()).and_then(
            |files| {
              if files.is_empty() {
                Err(generic_error("No target files found."))
              } else {
                Ok(files)
              }
            },
          )?;
          _ = watcher_communicator.watch_paths(files.clone());

          let lint_paths = if let Some(paths) = changed_paths {
            // lint all files on any changed (https://github.com/denoland/deno/issues/12446)
            files
              .iter()
              .any(|path| {
                canonicalize_path(path)
                  .map(|p| paths.contains(&p))
                  .unwrap_or(false)
              })
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
      let file_path = cli_options.initial_cwd().join(STDIN_FILE_NAME);
      let r = lint_stdin(&file_path, lint_rules);
      let success = handle_lint_result(
        &file_path.to_string_lossy(),
        r,
        reporter_lock.clone(),
      );
      reporter_lock.lock().unwrap().close(1);
      success
    } else {
      let target_files =
        collect_lint_files(files.clone()).and_then(|files| {
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
      if let Ok((file_diagnostics, file_source)) = &r {
        if file_diagnostics.is_empty() {
          // update the incremental cache if there were no diagnostics
          incremental_cache
            .update_file(&file_path, file_source.text_info().text_str())
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

fn collect_lint_files(files: FilePatterns) -> Result<Vec<PathBuf>, AnyError> {
  FileCollector::new(|path, _| is_script_ext(path))
    .ignore_git_folder()
    .ignore_node_modules()
    .ignore_vendor_folder()
    .collect_file_patterns(files)
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

pub fn create_linter(rules: Vec<&'static dyn LintRule>) -> Linter {
  LinterBuilder::default()
    .ignore_file_directive("deno-lint-ignore-file")
    .ignore_diagnostic_directive("deno-lint-ignore")
    .rules(rules)
    .build()
}

fn lint_file(
  file_path: &Path,
  source_code: String,
  lint_rules: Vec<&'static dyn LintRule>,
) -> Result<(Vec<LintDiagnostic>, ParsedSource), AnyError> {
  let specifier = specifier_from_file_path(file_path)?;
  let media_type = MediaType::from_specifier(&specifier);

  let linter = create_linter(lint_rules);

  let (source, file_diagnostics) = linter.lint_file(LintFileOptions {
    specifier,
    media_type,
    source_code: source_code.clone(),
  })?;

  Ok((file_diagnostics, source))
}

/// Lint stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--json` flag.
fn lint_stdin(
  file_path: &Path,
  lint_rules: Vec<&'static dyn LintRule>,
) -> Result<(Vec<LintDiagnostic>, ParsedSource), AnyError> {
  let mut source_code = String::new();
  if stdin().read_to_string(&mut source_code).is_err() {
    return Err(generic_error("Failed to read from stdin"));
  }

  let linter = create_linter(lint_rules);

  let (source, file_diagnostics) = linter.lint_file(LintFileOptions {
    specifier: specifier_from_file_path(file_path)?,
    source_code: source_code.clone(),
    media_type: MediaType::TypeScript,
  })?;

  Ok((file_diagnostics, source))
}

fn handle_lint_result(
  file_path: &str,
  result: Result<(Vec<LintDiagnostic>, ParsedSource), AnyError>,
  reporter_lock: Arc<Mutex<Box<dyn LintReporter + Send>>>,
) -> bool {
  let mut reporter = reporter_lock.lock().unwrap();

  match result {
    Ok((mut file_diagnostics, source)) => {
      file_diagnostics.sort_by(|a, b| match a.specifier.cmp(&b.specifier) {
        std::cmp::Ordering::Equal => a.range.start.cmp(&b.range.start),
        file_order => file_order,
      });
      for d in file_diagnostics.iter() {
        reporter.visit_diagnostic(d, &source);
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
  fn visit_diagnostic(&mut self, d: &LintDiagnostic, source: &ParsedSource);
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
  fn visit_diagnostic(&mut self, d: &LintDiagnostic, _source: &ParsedSource) {
    self.lint_count += 1;

    eprintln!("{}", d.display());
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
  fn visit_diagnostic(&mut self, d: &LintDiagnostic, _source: &ParsedSource) {
    self.lint_count += 1;

    let line_and_column = d.text_info.line_and_column_display(d.range.start);
    eprintln!(
      "{}: line {}, col {} - {} ({})",
      d.specifier,
      line_and_column.line_number,
      line_and_column.column_number,
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
  pub range: JsonLintDiagnosticRange,
  pub message: String,
  pub code: String,
  pub hint: Option<String>,
}

#[derive(Serialize)]
struct JsonLintReporter {
  diagnostics: Vec<JsonLintDiagnostic>,
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
  fn visit_diagnostic(&mut self, d: &LintDiagnostic, _source: &ParsedSource) {
    self.diagnostics.push(JsonLintDiagnostic {
      filename: d.specifier.to_string(),
      range: JsonLintDiagnosticRange {
        start: JsonDiagnosticLintPosition::new(
          d.range.start.as_byte_index(d.text_info.range().start),
          d.text_info.line_and_column_index(d.range.start),
        ),
        end: JsonDiagnosticLintPosition::new(
          d.range.end.as_byte_index(d.text_info.range().start),
          d.text_info.line_and_column_index(d.range.end),
        ),
      },
      message: d.message.clone(),
      code: d.code.clone(),
      hint: d.hint.clone(),
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
    println!("{}", json.unwrap());
  }
}

fn sort_diagnostics(diagnostics: &mut [JsonLintDiagnostic]) {
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
