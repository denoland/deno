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
use crate::swc_util;
use deno_core::ErrBox;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::Linter;
use deno_lint::linter::LinterBuilder;
use deno_lint::rules;
use deno_lint::rules::LintRule;
use serde::Serialize;
use std::fs;
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

/// List of lint rules used available in "deno lint" subcommand
fn get_rules() -> Vec<Box<dyn LintRule>> {
  vec![
    rules::ban_ts_comment::BanTsComment::new(),
    rules::ban_untagged_ignore::BanUntaggedIgnore::new(),
    rules::constructor_super::ConstructorSuper::new(),
    rules::for_direction::ForDirection::new(),
    rules::getter_return::GetterReturn::new(),
    rules::no_array_constructor::NoArrayConstructor::new(),
    rules::no_async_promise_executor::NoAsyncPromiseExecutor::new(),
    rules::no_case_declarations::NoCaseDeclarations::new(),
    rules::no_class_assign::NoClassAssign::new(),
    rules::no_compare_neg_zero::NoCompareNegZero::new(),
    rules::no_cond_assign::NoCondAssign::new(),
    rules::no_debugger::NoDebugger::new(),
    rules::no_delete_var::NoDeleteVar::new(),
    rules::no_dupe_args::NoDupeArgs::new(),
    rules::no_dupe_class_members::NoDupeClassMembers::new(),
    rules::no_dupe_else_if::NoDupeElseIf::new(),
    rules::no_dupe_keys::NoDupeKeys::new(),
    rules::no_duplicate_case::NoDuplicateCase::new(),
    rules::no_empty_character_class::NoEmptyCharacterClass::new(),
    rules::no_empty_interface::NoEmptyInterface::new(),
    rules::no_empty_pattern::NoEmptyPattern::new(),
    rules::no_empty::NoEmpty::new(),
    rules::no_ex_assign::NoExAssign::new(),
    rules::no_explicit_any::NoExplicitAny::new(),
    rules::no_extra_boolean_cast::NoExtraBooleanCast::new(),
    rules::no_extra_non_null_assertion::NoExtraNonNullAssertion::new(),
    rules::no_extra_semi::NoExtraSemi::new(),
    rules::no_func_assign::NoFuncAssign::new(),
    rules::no_misused_new::NoMisusedNew::new(),
    rules::no_namespace::NoNamespace::new(),
    rules::no_new_symbol::NoNewSymbol::new(),
    rules::no_obj_calls::NoObjCalls::new(),
    rules::no_octal::NoOctal::new(),
    rules::no_prototype_builtins::NoPrototypeBuiltins::new(),
    rules::no_regex_spaces::NoRegexSpaces::new(),
    rules::no_setter_return::NoSetterReturn::new(),
    rules::no_this_alias::NoThisAlias::new(),
    rules::no_this_before_super::NoThisBeforeSuper::new(),
    rules::no_unsafe_finally::NoUnsafeFinally::new(),
    rules::no_unsafe_negation::NoUnsafeNegation::new(),
    rules::no_with::NoWith::new(),
    rules::prefer_as_const::PreferAsConst::new(),
    rules::prefer_namespace_keyword::PreferNamespaceKeyword::new(),
    rules::require_yield::RequireYield::new(),
    rules::triple_slash_reference::TripleSlashReference::new(),
    rules::use_isnan::UseIsNaN::new(),
    rules::valid_typeof::ValidTypeof::new(),
    rules::no_inferrable_types::NoInferrableTypes::new(),
    rules::no_unused_labels::NoUnusedLabels::new(),
    rules::no_shadow_restricted_names::NoShadowRestrictedNames::new(),
  ]
}

pub fn print_rules_list() {
  let lint_rules = get_rules();

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

  let lint_rules = get_rules();
  let mut linter = create_linter(syntax, lint_rules);

  let file_diagnostics = linter.lint(file_name, source_code)?;

  Ok(file_diagnostics)
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
        &d.location.filename,
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

  return format!("{}:{}:{}", location.filename, location.line, location.col);
}
