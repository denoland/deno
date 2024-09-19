// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::path::Path;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_graph::ModuleGraph;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::LintConfig as DenoLintConfig;
use deno_lint::linter::LintFileOptions;
use deno_lint::linter::Linter as DenoLintLinter;
use deno_lint::linter::LinterOptions;

use crate::util::fs::atomic_write_file_with_retries;
use crate::util::fs::specifier_from_file_path;

use super::rules::FileOrPackageLintRule;
use super::rules::PackageLintRule;
use super::ConfiguredRules;

pub struct CliLinterOptions {
  pub configured_rules: ConfiguredRules,
  pub fix: bool,
  pub deno_lint_config: DenoLintConfig,
}

#[derive(Debug)]
pub struct CliLinter {
  fix: bool,
  package_rules: Vec<Box<dyn PackageLintRule>>,
  linter: DenoLintLinter,
  deno_lint_config: DenoLintConfig,
}

impl CliLinter {
  pub fn new(options: CliLinterOptions) -> Self {
    let rules = options.configured_rules.rules;
    let mut deno_lint_rules = Vec::with_capacity(rules.len());
    let mut package_rules = Vec::with_capacity(rules.len());
    for rule in rules {
      match rule.into_file_or_pkg_rule() {
        FileOrPackageLintRule::File(rule) => {
          deno_lint_rules.push(rule);
        }
        FileOrPackageLintRule::Package(rule) => {
          package_rules.push(rule);
        }
      }
    }
    Self {
      fix: options.fix,
      package_rules,
      linter: DenoLintLinter::new(LinterOptions {
        rules: deno_lint_rules,
        all_rule_codes: options.configured_rules.all_rule_codes,
        custom_ignore_file_directive: None,
        custom_ignore_diagnostic_directive: None,
      }),
      deno_lint_config: options.deno_lint_config,
    }
  }

  pub fn has_package_rules(&self) -> bool {
    !self.package_rules.is_empty()
  }

  pub fn lint_package(
    &self,
    graph: &ModuleGraph,
    entrypoints: &[ModuleSpecifier],
  ) -> Vec<LintDiagnostic> {
    let mut diagnostics = Vec::new();
    for rule in &self.package_rules {
      diagnostics.extend(rule.lint_package(graph, entrypoints));
    }
    diagnostics
  }

  pub fn lint_with_ast(
    &self,
    parsed_source: &ParsedSource,
  ) -> Vec<LintDiagnostic> {
    self
      .linter
      .lint_with_ast(parsed_source, self.deno_lint_config.clone())
  }

  pub fn lint_file(
    &self,
    file_path: &Path,
    source_code: String,
    ext: Option<&str>,
  ) -> Result<(ParsedSource, Vec<LintDiagnostic>), AnyError> {
    let specifier = specifier_from_file_path(file_path)?;
    let media_type = if let Some(ext) = ext {
      MediaType::from_str(&format!("placeholder.{ext}"))
    } else if file_path.extension().is_none() {
      MediaType::TypeScript
    } else {
      MediaType::from_specifier(&specifier)
    };

    if self.fix {
      self.lint_file_and_fix(&specifier, media_type, source_code, file_path)
    } else {
      self
        .linter
        .lint_file(LintFileOptions {
          specifier,
          media_type,
          source_code,
          config: self.deno_lint_config.clone(),
        })
        .map_err(AnyError::from)
    }
  }

  fn lint_file_and_fix(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source_code: String,
    file_path: &Path,
  ) -> Result<(ParsedSource, Vec<LintDiagnostic>), deno_core::anyhow::Error> {
    // initial lint
    let (source, diagnostics) = self.linter.lint_file(LintFileOptions {
      specifier: specifier.clone(),
      media_type,
      source_code,
      config: self.deno_lint_config.clone(),
    })?;

    // Try applying fixes repeatedly until the file has none left or
    // a maximum number of iterations is reached. This is necessary
    // because lint fixes may overlap and so we can't always apply
    // them in one pass.
    let mut source = source;
    let mut diagnostics = diagnostics;
    let mut fix_iterations = 0;
    loop {
      let change = apply_lint_fixes_and_relint(
        specifier,
        media_type,
        &self.linter,
        self.deno_lint_config.clone(),
        source.text_info_lazy(),
        &diagnostics,
      )?;
      match change {
        Some(change) => {
          source = change.0;
          diagnostics = change.1;
        }
        None => {
          break;
        }
      }
      fix_iterations += 1;
      if fix_iterations > 5 {
        log::warn!(
          concat!(
            "Reached maximum number of fix iterations for '{}'. There's ",
            "probably a bug in Deno. Please fix this file manually.",
          ),
          specifier,
        );
        break;
      }
    }

    if fix_iterations > 0 {
      // everything looks good and the file still parses, so write it out
      atomic_write_file_with_retries(
        file_path,
        source.text().as_ref(),
        crate::cache::CACHE_PERM,
      )
      .context("Failed writing fix to file.")?;
    }

    Ok((source, diagnostics))
  }
}

fn apply_lint_fixes_and_relint(
  specifier: &ModuleSpecifier,
  media_type: MediaType,
  linter: &DenoLintLinter,
  config: DenoLintConfig,
  text_info: &SourceTextInfo,
  diagnostics: &[LintDiagnostic],
) -> Result<Option<(ParsedSource, Vec<LintDiagnostic>)>, AnyError> {
  let Some(new_text) = apply_lint_fixes(text_info, diagnostics) else {
    return Ok(None);
  };
  linter
    .lint_file(LintFileOptions {
      specifier: specifier.clone(),
      source_code: new_text,
      media_type,
      config,
    })
    .map(Some)
    .context(
      "An applied lint fix caused a syntax error. Please report this bug.",
    )
}

fn apply_lint_fixes(
  text_info: &SourceTextInfo,
  diagnostics: &[LintDiagnostic],
) -> Option<String> {
  if diagnostics.is_empty() {
    return None;
  }

  let file_start = text_info.range().start;
  let mut quick_fixes = diagnostics
    .iter()
    // use the first quick fix
    .filter_map(|d| d.details.fixes.first())
    .flat_map(|fix| fix.changes.iter())
    .map(|change| deno_ast::TextChange {
      range: change.range.as_byte_range(file_start),
      new_text: change.new_text.to_string(),
    })
    .collect::<Vec<_>>();
  if quick_fixes.is_empty() {
    return None;
  }

  let mut import_fixes = HashSet::new();
  // remove any overlapping text changes, we'll circle
  // back for another pass to fix the remaining
  quick_fixes.sort_by_key(|change| change.range.start);
  for i in (1..quick_fixes.len()).rev() {
    let cur = &quick_fixes[i];
    let previous = &quick_fixes[i - 1];
    // hack: deduplicate import fixes to avoid creating errors
    if previous.new_text.trim_start().starts_with("import ") {
      import_fixes.insert(previous.new_text.trim().to_string());
    }
    let is_overlapping = cur.range.start <= previous.range.end;
    if is_overlapping
      || (cur.new_text.trim_start().starts_with("import ")
        && import_fixes.contains(cur.new_text.trim()))
    {
      quick_fixes.remove(i);
    }
  }
  let new_text =
    deno_ast::apply_text_changes(text_info.text_str(), quick_fixes);
  Some(new_text)
}
