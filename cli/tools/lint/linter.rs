// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
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
use crate::util::text_encoding::Utf16Map;

pub struct CliLinterOptions {
  pub configured_rules: ConfiguredRules,
  pub fix: bool,
  pub deno_lint_config: DenoLintConfig,
  pub maybe_plugin_runner: Option<Arc<PluginHostProxy>>,
}

#[derive(Debug)]
pub struct CliLinter {
  fix: bool,
  package_rules: Vec<Box<dyn PackageLintRule>>,
  linter: DenoLintLinter,
  deno_lint_config: DenoLintConfig,
  maybe_plugin_runner: Option<Arc<PluginHostProxy>>,
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
    let external_linter_container = ExternalLinterContainer::new(
      self.maybe_plugin_runner.clone(),
      Some(token),
    );

    let d = self.linter.lint_with_ast(
      parsed_source,
      self.deno_lint_config.clone(),
      external_linter_container.get_callback(),
    );
    if let Some(err) = external_linter_container.take_error() {
      return Err(err);
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

    let external_linter_container =
      ExternalLinterContainer::new(self.maybe_plugin_runner.clone(), None);

    if self.fix {
      self.lint_file_and_fix(
        &specifier,
        media_type,
        source_code,
        file_path,
        external_linter_container,
      )
    } else {
      let (source, diagnostics) = self
        .linter
        .lint_file(LintFileOptions {
          specifier,
          media_type,
          source_code,
          config: self.deno_lint_config.clone(),
          external_linter: external_linter_container.get_callback(),
        })
        .map_err(AnyError::from)?;

      if let Some(err) = external_linter_container.take_error() {
        return Err(err);
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
    external_linter_container: ExternalLinterContainer,
  ) -> Result<(ParsedSource, Vec<LintDiagnostic>), deno_core::anyhow::Error> {
    // initial lint
    let (source, diagnostics) = self.linter.lint_file(LintFileOptions {
      specifier: specifier.clone(),
      media_type,
      source_code,
      config: self.deno_lint_config.clone(),
      external_linter: external_linter_container.get_callback(),
    })?;

    if let Some(err) = external_linter_container.take_error() {
      return Err(err);
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
        &source,
        &diagnostics,
        &external_linter_container,
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
            "probably a bug in the lint rule. Please fix this file manually.",
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
  original_source: &ParsedSource,
  diagnostics: &[LintDiagnostic],
  external_linter_container: &ExternalLinterContainer,
) -> Result<Option<(ParsedSource, Vec<LintDiagnostic>)>, AnyError> {
  let text_info = original_source.text_info_lazy();
  let Some(new_text) = apply_lint_fixes(text_info, diagnostics) else {
    return Ok(None);
  };

  let lint_with_text = |new_text: String| {
    let (source, diagnostics) = linter.lint_file(LintFileOptions {
      specifier: specifier.clone(),
      source_code: new_text,
      media_type,
      config: config.clone(),
      external_linter: external_linter_container.get_callback(),
    })?;
    let mut new_diagnostics = source.diagnostics().clone();
    new_diagnostics.retain(|d| !original_source.diagnostics().contains(d));
    if let Some(diagnostic) = new_diagnostics.pop() {
      return Err(AnyError::from(diagnostic));
    }
    Ok((source, diagnostics))
  };

  let (source, diagnostics) = match lint_with_text(new_text) {
    Ok(result) => result,
    Err(err) => {
      let utf16_map = Utf16Map::new(text_info.text_str());
      // figure out which diagnostic caused a syntax error
      let mut diagnostics = diagnostics.to_vec();
      while let Some(last_diagnostic) = diagnostics.pop() {
        let Some(lint_fix) = last_diagnostic.details.fixes.first() else {
          continue;
        };
        let success = match apply_lint_fixes(text_info, &diagnostics) {
          Some(new_text) => lint_with_text(new_text).is_ok(),
          None => true,
        };
        if success {
          let mut changes_text = String::new();
          for change in &lint_fix.changes {
            let utf8_start =
              (change.range.start - text_info.range().start) as u32;
            let utf8_end = (change.range.end - text_info.range().start) as u32;
            let utf16_start = utf16_map
              .utf8_to_utf16_offset(utf8_start.into())
              .unwrap_or(utf8_start.into());
            let utf16_end = utf16_map
              .utf8_to_utf16_offset(utf8_end.into())
              .unwrap_or(utf8_end.into());
            changes_text.push_str(&format!(
              "Range: [{}, {}]\n",
              u32::from(utf16_start),
              u32::from(utf16_end)
            ));
            changes_text.push_str(&format!("Text: {:?}\n\n", &change.new_text));
          }
          return Err(err).context(format!(
            "The '{}' rule caused a syntax error applying '{}'.\n\n{}",
            last_diagnostic.details.code, lint_fix.description, changes_text
          ));
        }
      }
      return Err(err).context(
        "A lint fix caused a syntax error. This is a bug in a lint rule.",
      );
    }
  };

  if let Some(err) = external_linter_container.take_error() {
    return Err(err);
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
  plugin_runner: Arc<PluginHostProxy>,
  parsed_source: ParsedSource,
  file_path: PathBuf,
  maybe_token: Option<CancellationToken>,
) -> Result<ExternalLinterResult, AnyError> {
  let source_text_info = parsed_source.text_info_lazy().clone();
  let plugin_info = plugin_runner
    .get_plugin_rules()
    .into_iter()
    .map(Cow::from)
    .collect();

  let fut = async move {
    let utf16_map = Utf16Map::new(parsed_source.text().as_ref());
    let serialized_ast =
      plugin_runner.serialize_ast(&parsed_source, &utf16_map)?;

    plugins::run_rules_for_ast(
      &plugin_runner,
      &file_path,
      serialized_ast,
      source_text_info,
      utf16_map,
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

struct ExternalLinterContainer {
  cb: Option<ExternalLinterCb>,
  error: Option<Arc<Mutex<Option<AnyError>>>>,
}

impl ExternalLinterContainer {
  pub fn new(
    maybe_plugin_runner: Option<Arc<PluginHostProxy>>,
    maybe_token: Option<CancellationToken>,
  ) -> Self {
    let mut s = Self {
      cb: None,
      error: None,
    };
    if let Some(plugin_runner) = maybe_plugin_runner {
      s.error = Some(Arc::new(Mutex::new(None)));
      let error_ = s.error.clone();
      let cb = Arc::new(move |parsed_source: ParsedSource| {
        let token_ = maybe_token.clone();
        let file_path =
          match deno_path_util::url_to_file_path(parsed_source.specifier()) {
            Ok(path) => path,
            Err(err) => {
              *error_.as_ref().unwrap().lock() = Some(err.into());
              return None;
            }
          };

        let r =
          run_plugins(plugin_runner.clone(), parsed_source, file_path, token_);

        match r {
          Ok(d) => Some(d),
          Err(err) => {
            *error_.as_ref().unwrap().lock() = Some(err);
            None
          }
        }
      });
      s.cb = Some(cb);
    }
    s
  }

  pub fn get_callback(&self) -> Option<ExternalLinterCb> {
    self.cb.clone()
  }

  pub fn take_error(&self) -> Option<AnyError> {
    self.error.as_ref().and_then(|e| e.lock().take())
  }
}
