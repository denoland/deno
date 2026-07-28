// Copyright 2018-2026 the Deno authors. MIT license.

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
use deno_ast::SourceTextProvider;
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

use super::ConfiguredRules;
use super::plugins;
use super::plugins::PluginHostProxy;
use super::rules::FileOrPackageLintRule;
use super::rules::PackageLintRule;
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

  pub fn has_package_rule(&self, code: &str) -> bool {
    self.package_rules.iter().any(|r| r.code() == code)
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
      None,
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

    let has_processor = if let Some(runner) = &self.maybe_plugin_runner {
      if let Some(file_ext) = file_path.extension().and_then(|e| e.to_str()) {
        let dot_ext = format!(".{file_ext}");
        let infos = runner.plugin_info.lock();
        infos.iter().any(|info| {
          info
            .extensions
            .iter()
            .any(|ext| ext.eq_ignore_ascii_case(&dot_ext))
        })
      } else {
        false
      }
    } else {
      false
    };

    if has_processor {
      self.lint_file_processed(
        file_path,
        source_code,
        external_linter_container,
      )
    } else if self.fix {
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
          source_mapping: None,
        })
        .map_err(AnyError::from)?;

      if let Some(err) = external_linter_container.take_error() {
        return Err(err);
      }

      Ok((source, diagnostics))
    }
  }

  /// Lints a file that has a custom plugin preprocessor/postprocessor (e.g. `.vue` or `.svelte`).
  ///
  /// This method runs the V8 preprocessor to extract script blocks, lints each block using
  /// `deno_lint` with source offset mapping, runs the V8 postprocessor on the aggregated diagnostics,
  /// and executes iterative autofix mapping back to the parent file coordinates.
  fn lint_file_processed(
    &self,
    file_path: &Path,
    source_code: String,
    external_linter_container: ExternalLinterContainer,
  ) -> Result<(ParsedSource, Vec<LintDiagnostic>), AnyError> {
    let runner = self.maybe_plugin_runner.as_ref().unwrap();
    let file_ext = file_path.extension().and_then(|e| e.to_str()).unwrap();
    let dot_ext = format!(".{file_ext}");

    let mut current_source_code = source_code.clone();
    let mut fix_iterations = 0;

    // Loop to apply autofixes iteratively (max 5 times to handle overlapping fixes)
    loop {
      // Preprocess the markup source code to extract script blocks (e.g. from <script> tags)
      let preprocess_fut =
        runner.preprocess(&dot_ext, &current_source_code, file_path);
      let blocks = deno_core::futures::executor::block_on(preprocess_fut)?;

      let mut all_diagnostics = Vec::new();
      let mut parsed_source = None;

      if let Some(ref blocks) = blocks {
        let mut block_diagnostics_batch = Vec::new();
        // Lint each extracted block individually
        for block in blocks {
          let block_media_type = if block.filename.ends_with(".ts")
            || block.filename.ends_with(".tsx")
          {
            MediaType::TypeScript
          } else {
            MediaType::JavaScript
          };

          let block_specifier = specifier_from_file_path(file_path)?;
          let byte_offset = current_source_code.find(&block.text).unwrap_or(0);

          // Build source mapping to allow deno_lint engine to offset coordinates back to original file
          let source_mapping = deno_lint::linter::SourceMapping {
            original_source: current_source_code.clone(),
            byte_offset,
          };

          let (block_source, block_diagnostics) = self
            .linter
            .lint_file(deno_lint::linter::LintFileOptions {
              specifier: block_specifier,
              media_type: block_media_type,
              source_code: block.text.clone(),
              config: self.deno_lint_config.clone(),
              external_linter: external_linter_container.get_callback(),
              source_mapping: Some(source_mapping),
            })
            .map_err(AnyError::from)?;

          if parsed_source.is_none() {
            parsed_source = Some(block_source);
          }
          block_diagnostics_batch.push(block_diagnostics);
        }

        // Run postprocess in V8 to filter and combine all block diagnostics
        let source_text_info =
          deno_ast::SourceTextInfo::new(current_source_code.clone().into());
        let start_pos = (&source_text_info).start_pos();
        let serializable_batch: Vec<Vec<SerializableLintDiagnostic>> =
          block_diagnostics_batch
            .iter()
            .map(|batch| {
              batch
                .iter()
                .map(|d| {
                  SerializableLintDiagnostic::from_diagnostic(d, start_pos)
                })
                .collect()
            })
            .collect();
        let diagnostics_json = serde_json::to_string(&serializable_batch)?;
        let postprocess_fut =
          runner.postprocess(&dot_ext, &diagnostics_json, file_path);
        let postprocessed_json =
          deno_core::futures::executor::block_on(postprocess_fut)?;
        let serializable_diagnostics: Vec<SerializableLintDiagnostic> =
          serde_json::from_str(&postprocessed_json)?;
        all_diagnostics = serializable_diagnostics
          .into_iter()
          .map(|d| d.into_diagnostic(&source_text_info))
          .collect::<Result<Vec<_>, _>>()?;
      }

      let source = parsed_source.unwrap_or_else(|| {
        deno_ast::parse_program(deno_ast::ParseParams {
          specifier: specifier_from_file_path(file_path).unwrap(),
          text: "".into(),
          media_type: MediaType::Unknown,
          capture_tokens: false,
          scope_analysis: false,
          maybe_syntax: None,
        })
        .unwrap()
      });

      // If not running with --fix or no diagnostics to fix, we are done
      if !self.fix || all_diagnostics.is_empty() {
        return Ok((source, all_diagnostics));
      }

      // Apply mapped autofixes relative to the original parent file content
      let text_info = SourceTextInfo::from_string(current_source_code.clone());
      let Some(new_text) = apply_lint_fixes(&text_info, &all_diagnostics)
      else {
        return Ok((source, all_diagnostics));
      };

      current_source_code = new_text;
      fix_iterations += 1;

      if fix_iterations > 5 {
        break;
      }
    }

    // Write updated/fixed text back to disk atomically if changes were made
    if current_source_code != source_code {
      atomic_write_file_with_retries(
        &CliSys::default(),
        file_path,
        current_source_code.as_bytes(),
        crate::cache::CACHE_PERM,
      )?;
    }

    // Run a final lint pass to get accurate ParsedSource and diagnostics after fixing
    let preprocess_fut =
      runner.preprocess(&dot_ext, &current_source_code, file_path);
    let blocks = deno_core::futures::executor::block_on(preprocess_fut)?;
    let mut all_diagnostics = Vec::new();
    let mut parsed_source = None;
    if let Some(ref blocks) = blocks {
      let mut block_diagnostics_batch = Vec::new();
      for block in blocks {
        let block_media_type = if block.filename.ends_with(".ts")
          || block.filename.ends_with(".tsx")
        {
          MediaType::TypeScript
        } else {
          MediaType::JavaScript
        };
        let block_specifier = specifier_from_file_path(file_path)?;
        let byte_offset = current_source_code.find(&block.text).unwrap_or(0);
        let source_mapping = deno_lint::linter::SourceMapping {
          original_source: current_source_code.clone(),
          byte_offset,
        };
        let (block_source, block_diagnostics) = self
          .linter
          .lint_file(deno_lint::linter::LintFileOptions {
            specifier: block_specifier,
            media_type: block_media_type,
            source_code: block.text.clone(),
            config: self.deno_lint_config.clone(),
            external_linter: external_linter_container.get_callback(),
            source_mapping: Some(source_mapping),
          })
          .map_err(AnyError::from)?;
        if parsed_source.is_none() {
          parsed_source = Some(block_source);
        }
        block_diagnostics_batch.push(block_diagnostics);
      }
      let source_text_info =
        deno_ast::SourceTextInfo::new(current_source_code.clone().into());
      let start_pos = (&source_text_info).start_pos();
      let serializable_batch: Vec<Vec<SerializableLintDiagnostic>> =
        block_diagnostics_batch
          .iter()
          .map(|batch| {
            batch
              .iter()
              .map(|d| {
                SerializableLintDiagnostic::from_diagnostic(d, start_pos)
              })
              .collect()
          })
          .collect();
      let diagnostics_json = serde_json::to_string(&serializable_batch)?;
      let postprocess_fut =
        runner.postprocess(&dot_ext, &diagnostics_json, file_path);
      let postprocessed_json =
        deno_core::futures::executor::block_on(postprocess_fut)?;
      let serializable_diagnostics: Vec<SerializableLintDiagnostic> =
        serde_json::from_str(&postprocessed_json)?;
      all_diagnostics = serializable_diagnostics
        .into_iter()
        .map(|d| d.into_diagnostic(&source_text_info))
        .collect::<Result<Vec<_>, _>>()?;
    }

    let source = parsed_source.unwrap_or_else(|| {
      deno_ast::parse_program(deno_ast::ParseParams {
        specifier: specifier_from_file_path(file_path).unwrap(),
        text: "".into(),
        media_type: MediaType::Unknown,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
      })
      .unwrap()
    });

    Ok((source, all_diagnostics))
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
      source_mapping: None,
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
      source_mapping: None,
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
  let quick_fixes = diagnostics
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

  Some(deno_ast::apply_text_changes(
    text_info.text_str(),
    // remove any overlapping text changes, we'll circle
    // back for another pass to fix the remaining
    filter_overlapping_text_changes(quick_fixes),
  ))
}

fn filter_overlapping_text_changes(
  mut text_changes: Vec<deno_ast::TextChange>,
) -> Vec<deno_ast::TextChange> {
  let mut seen_imports = HashSet::new();
  text_changes.sort_by_key(|change| change.range.start);
  let mut filtered: Vec<deno_ast::TextChange> =
    Vec::with_capacity(text_changes.len());
  for change in text_changes.into_iter() {
    let overlaps_last = filtered
      .last()
      .map(|prev| change.range.start <= prev.range.end)
      .unwrap_or(false);
    let is_duplicate_import =
      change.new_text.trim_start().starts_with("import ")
        && seen_imports.contains(change.new_text.trim());

    if overlaps_last || is_duplicate_import {
      // skip this edit
      continue;
    }

    // remember any import we keep so we can drop later duplicates
    if change.new_text.trim_start().starts_with("import ") {
      seen_imports.insert(change.new_text.trim().to_owned());
    }

    filtered.push(change);
  }
  filtered
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

#[cfg(test)]
mod test {
  use deno_ast::TextChange;

  use super::*;

  #[test]
  fn test_filter_overlapping_text_changes() {
    let changes = filter_overlapping_text_changes(vec![
      TextChange {
        range: 0..125,
        new_text: "".into(),
      },
      TextChange {
        range: 0..0,
        new_text: "".into(),
      },
      TextChange {
        range: 81..96,
        new_text: "".into(),
      },
    ]);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].range, 0..125);
  }
}

/// A serializable representation of a `LintDiagnostic` designed for V8/JSON serialization boundaries.
///
/// This struct converts non-serializable Swc types (like `SourcePos` and `SourceRange`) to simple
/// 0-indexed byte offsets relative to the start of the file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializableLintDiagnostic {
  pub specifier: String,
  pub code: String,
  pub message: String,
  pub hint: Option<String>,
  pub range: Option<(usize, usize)>,
  pub severity: String,
  pub fixes: Vec<SerializableLintFix>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializableLintFix {
  pub description: String,
  pub changes: Vec<SerializableLintFixChange>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializableLintFixChange {
  pub new_text: String,
  pub range: (usize, usize),
}

impl SerializableLintDiagnostic {
  /// Converts a `LintDiagnostic` into its serializable format.
  ///
  /// Maps coordinate bounds (e.g. diagnostic ranges and potential code fixes) to 0-indexed byte
  /// offsets using the provided file starting position as a baseline.
  pub fn from_diagnostic(
    d: &LintDiagnostic,
    start_pos: deno_ast::StartSourcePos,
  ) -> Self {
    let range = d.range.as_ref().map(|r| {
      (
        r.range.start.as_byte_index(start_pos),
        r.range.end.as_byte_index(start_pos),
      )
    });
    let fixes = d
      .details
      .fixes
      .iter()
      .map(|f| SerializableLintFix {
        description: f.description.to_string(),
        changes: f
          .changes
          .iter()
          .map(|c| SerializableLintFixChange {
            new_text: c.new_text.to_string(),
            range: (
              c.range.start.as_byte_index(start_pos),
              c.range.end.as_byte_index(start_pos),
            ),
          })
          .collect(),
      })
      .collect();

    Self {
      specifier: d.specifier.to_string(),
      code: d.details.code.clone(),
      message: d.details.message.clone(),
      hint: d.details.hint.clone(),
      range,
      severity: d.severity.as_str().to_string(),
      fixes,
    }
  }

  /// Reconstructs a full `LintDiagnostic` from its serializable representation.
  ///
  /// Recreates Swc coordinate systems and constructs exact diagnostic/fix ranges by offset-shifting
  /// the source text's baseline starting position.
  pub fn into_diagnostic(
    self,
    source_text_info: &deno_ast::SourceTextInfo,
  ) -> Result<LintDiagnostic, deno_core::anyhow::Error> {
    use std::borrow::Cow;

    use deno_ast::ModuleSpecifier;
    use deno_ast::SourceRange;
    use deno_ast::SourceTextProvider;
    use deno_lint::diagnostic::LintDiagnosticDetails;
    use deno_lint::diagnostic::LintDiagnosticRange;
    use deno_lint::diagnostic::LintDiagnosticSeverity;
    use deno_lint::diagnostic::LintDocsUrl;
    use deno_lint::diagnostic::LintFix;
    use deno_lint::diagnostic::LintFixChange;

    let specifier = ModuleSpecifier::parse(&self.specifier)?;
    let start_pos = source_text_info.start_pos().as_source_pos();

    let range = self.range.map(|(start, end)| LintDiagnosticRange {
      text_info: source_text_info.clone(),
      range: SourceRange::new(start_pos + start, start_pos + end),
      description: None,
    });

    let fixes = self
      .fixes
      .into_iter()
      .map(|f| LintFix {
        description: Cow::Owned(f.description),
        changes: f
          .changes
          .into_iter()
          .map(|c| LintFixChange {
            new_text: Cow::Owned(c.new_text),
            range: SourceRange::new(
              start_pos + c.range.0,
              start_pos + c.range.1,
            ),
          })
          .collect(),
      })
      .collect();

    let severity = if self.severity == "warning" {
      LintDiagnosticSeverity::Warning
    } else {
      LintDiagnosticSeverity::Error
    };

    Ok(LintDiagnostic {
      specifier,
      range,
      details: LintDiagnosticDetails {
        message: self.message,
        code: self.code,
        hint: self.hint,
        fixes,
        custom_docs_url: LintDocsUrl::None,
        info: vec![],
      },
      severity,
    })
  }
}
