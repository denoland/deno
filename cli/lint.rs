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
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use swc_ecmascript::parser::Syntax;

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
