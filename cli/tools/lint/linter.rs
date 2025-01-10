// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use ::tokio_util::sync::CancellationToken;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt as _;
use deno_core::parking_lot::Mutex;
use deno_graph::ModuleGraph;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::ExternalLinterCb;
use deno_lint::linter::ExternalLinterResult;
use deno_lint::linter::LintConfig as DenoLintConfig;
use deno_lint::linter::LintFileOptions;
use deno_lint::linter::Linter as DenoLintLinter;
use deno_lint::linter::LinterOptions;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_runtime::tokio_util;

use super::plugins;
use super::plugins::PluginHostProxy;
use super::rules::FileOrPackageLintRule;
use super::rules::PackageLintRule;
use super::ConfiguredRules;
use crate::sys::CliSys;
use crate::util::fs::specifier_from_file_path;

pub struct CliLinterOptions {
  pub configured_rules: ConfiguredRules,
  pub fix: bool,
  pub deno_lint_config: DenoLintConfig,
  pub maybe_plugin_runner: Option<Arc<Mutex<PluginHostProxy>>>,
}

#[derive(Debug)]
pub struct CliLinter {
  fix: bool,
  package_rules: Vec<Box<dyn PackageLintRule>>,
  linter: DenoLintLinter,
  deno_lint_config: DenoLintConfig,
  maybe_plugin_runner: Option<Arc<Mutex<PluginHostProxy>>>,
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
      maybe_plugin_runner: options.maybe_plugin_runner,
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
    token: CancellationToken,
  ) -> Result<Vec<LintDiagnostic>, AnyError> {
    // TODO(bartlomieju): surface error is running plugin fails

    let mut external_linter_error = None;
    let external_linter: Option<ExternalLinterCb> =
      if let Some(plugin_runner) = self.maybe_plugin_runner.clone() {
        external_linter_error = Some(Arc::new(Mutex::new(None)));
        let external_linter_error_ = external_linter_error.clone();
        Some(Arc::new(move |parsed_source: ParsedSource| {
          // TODO: clean this up
          let file_path = parsed_source.specifier().to_file_path().unwrap();
          let r = run_plugins(
            plugin_runner.clone(),
            parsed_source,
            file_path,
            Some(token.clone()),
          );

          match r {
            Ok(d) => Some(d),
            Err(err) => {
              *external_linter_error_.as_ref().unwrap().lock() = Some(err);
              None
            }
          }
        }))
      } else {
        None
      };

    let d = self.linter.lint_with_ast(
      parsed_source,
      self.deno_lint_config.clone(),
      external_linter,
    );
    if let Some(maybe_external_linter_error) = external_linter_error.as_ref() {
      if let Some(err) = maybe_external_linter_error.lock().take() {
        return Err(err);
      }
    }
    Ok(d)
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

    let mut external_linter_error = None;
    let external_linter: Option<ExternalLinterCb> =
      if let Some(plugin_runner) = self.maybe_plugin_runner.clone() {
        external_linter_error = Some(Arc::new(Mutex::new(None)));
        let external_linter_error_ = external_linter_error.clone();

        Some(Arc::new(move |parsed_source: ParsedSource| {
          // TODO: clean this up
          let file_path = parsed_source.specifier().to_file_path().unwrap();
          let r =
            run_plugins(plugin_runner.clone(), parsed_source, file_path, None);

          match r {
            Ok(d) => Some(d),
            Err(err) => {
              *external_linter_error_.as_ref().unwrap().lock() = Some(err);
              None
            }
          }
        }))
      } else {
        None
      };

    if self.fix {
      self.lint_file_and_fix(
        &specifier,
        media_type,
        source_code,
        file_path,
        external_linter,
        external_linter_error,
      )
    } else {
      let (source, diagnostics) = self
        .linter
        .lint_file(LintFileOptions {
          specifier,
          media_type,
          source_code,
          config: self.deno_lint_config.clone(),
          external_linter,
        })
        .map_err(AnyError::from)?;

      if let Some(maybe_external_linter_error) = external_linter_error.as_ref()
      {
        if let Some(err) = maybe_external_linter_error.lock().take() {
          return Err(err);
        }
      }

      Ok((source, diagnostics))
    }
  }

  fn lint_file_and_fix(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source_code: String,
    file_path: &Path,
    external_linter: Option<ExternalLinterCb>,
    external_linter_error: Option<Arc<Mutex<Option<AnyError>>>>,
  ) -> Result<(ParsedSource, Vec<LintDiagnostic>), deno_core::anyhow::Error> {
    // initial lint
    let (source, diagnostics) = self.linter.lint_file(LintFileOptions {
      specifier: specifier.clone(),
      media_type,
      source_code,
      config: self.deno_lint_config.clone(),
      external_linter: external_linter.clone(),
    })?;

    if let Some(maybe_external_linter_error) = external_linter_error.as_ref() {
      if let Some(err) = maybe_external_linter_error.lock().take() {
        return Err(err);
      }
    }

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
        external_linter.clone(),
        external_linter_error.clone(),
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
        &CliSys::default(),
        file_path,
        source.text().as_bytes(),
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
  external_linter: Option<ExternalLinterCb>,
  external_linter_error: Option<Arc<Mutex<Option<AnyError>>>>,
) -> Result<Option<(ParsedSource, Vec<LintDiagnostic>)>, AnyError> {
  let Some(new_text) = apply_lint_fixes(text_info, diagnostics) else {
    return Ok(None);
  };

  let (source, diagnostics) = linter
    .lint_file(LintFileOptions {
      specifier: specifier.clone(),
      source_code: new_text,
      media_type,
      config,
      external_linter,
    })
    .context(
      "An applied lint fix caused a syntax error. Please report this bug.",
    )?;

  if let Some(maybe_external_linter_error) = external_linter_error.as_ref() {
    if let Some(err) = maybe_external_linter_error.lock().take() {
      return Err(err);
    }
  }

  Ok(Some((source, diagnostics)))
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

fn run_plugins(
  plugin_runner: Arc<Mutex<PluginHostProxy>>,
  parsed_source: ParsedSource,
  file_path: PathBuf,
  maybe_token: Option<CancellationToken>,
) -> Result<ExternalLinterResult, AnyError> {
  let source_text_info = parsed_source.text_info_lazy().clone();
  let plugin_info = plugin_runner.lock().get_plugin_rules();

  #[allow(clippy::await_holding_lock)]
  let fut = async move {
    let mut plugin_runner = plugin_runner.lock();
    let serialized_ast = plugin_runner.serialize_ast(parsed_source)?;

    plugins::run_rules_for_ast(
      &mut plugin_runner,
      &file_path,
      serialized_ast,
      source_text_info,
      maybe_token,
    )
    .await
  }
  .boxed_local();

  let plugin_diagnostics = tokio_util::create_and_run_current_thread(fut)?;

  Ok(ExternalLinterResult {
    diagnostics: plugin_diagnostics,
    rules: plugin_info,
  })
}
