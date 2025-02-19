// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceRanged;
use deno_ast::SourceTextInfo;
use deno_ast::TextChange;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::ModuleGraph;
use deno_resolver::workspace::ResolutionKind;
use lazy_regex::Lazy;

use super::diagnostics::PublishDiagnosticsCollector;
use super::unfurl::SpecifierUnfurler;
use super::unfurl::SpecifierUnfurlerDiagnostic;
use crate::args::deno_json::TsConfigFolderInfo;
use crate::args::deno_json::TsConfigResolver;
use crate::cache::LazyGraphSourceParser;
use crate::cache::ParsedSourceCache;
use crate::tools::registry::diagnostics::PublishDiagnostic;

pub struct ModuleContentProvider {
  specifier_unfurler: SpecifierUnfurler,
  parsed_source_cache: Arc<ParsedSourceCache>,
  tsconfig_resolver: Arc<TsConfigResolver>,
}

impl ModuleContentProvider {
  pub fn new(
    parsed_source_cache: Arc<ParsedSourceCache>,
    specifier_unfurler: SpecifierUnfurler,
    tsconfig_resolver: Arc<TsConfigResolver>,
  ) -> Self {
    Self {
      specifier_unfurler,
      parsed_source_cache,
      tsconfig_resolver,
    }
  }

  pub fn resolve_content_maybe_unfurling(
    &self,
    graph: &ModuleGraph,
    diagnostics_collector: &PublishDiagnosticsCollector,
    path: &Path,
    specifier: &Url,
  ) -> Result<Vec<u8>, AnyError> {
    let source_parser =
      LazyGraphSourceParser::new(&self.parsed_source_cache, graph);
    let media_type = MediaType::from_specifier(specifier);
    let parsed_source = match source_parser.get_or_parse_source(specifier)? {
      Some(parsed_source) => parsed_source,
      None => {
        let data = std::fs::read(path).with_context(|| {
          format!("Unable to read file '{}'", path.display())
        })?;

        match media_type {
          MediaType::JavaScript
          | MediaType::Jsx
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::TypeScript
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Dts
          | MediaType::Dmts
          | MediaType::Dcts
          | MediaType::Tsx => {
            // continue
          }
          MediaType::SourceMap
          | MediaType::Unknown
          | MediaType::Json
          | MediaType::Wasm
          | MediaType::Css => {
            // not unfurlable data
            return Ok(data);
          }
        }

        let text = String::from_utf8(data)?;
        deno_ast::parse_module(deno_ast::ParseParams {
          specifier: specifier.clone(),
          text: text.into(),
          media_type,
          capture_tokens: false,
          maybe_syntax: None,
          scope_analysis: false,
        })?
      }
    };

    log::debug!("Unfurling {}", specifier);
    let mut reporter = |diagnostic| {
      diagnostics_collector
        .push(PublishDiagnostic::SpecifierUnfurl(diagnostic));
    };
    let text_info = parsed_source.text_info_lazy();
    let module_info =
      deno_graph::ParserModuleAnalyzer::module_info(&parsed_source);
    let mut text_changes = Vec::new();
    if media_type.is_jsx() {
      self.add_jsx_text_changes(
        specifier,
        &parsed_source,
        text_info,
        &module_info,
        &mut reporter,
        &mut text_changes,
      )?;
    }

    self.specifier_unfurler.unfurl_to_changes(
      specifier,
      &parsed_source,
      &module_info,
      &mut text_changes,
      &mut reporter,
    );
    let rewritten_text =
      deno_ast::apply_text_changes(text_info.text_str(), text_changes);

    Ok(rewritten_text.into_bytes())
  }

  fn add_jsx_text_changes(
    &self,
    specifier: &Url,
    parsed_source: &ParsedSource,
    text_info: &SourceTextInfo,
    module_info: &deno_graph::ModuleInfo,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
    text_changes: &mut Vec<TextChange>,
  ) -> Result<(), AnyError> {
    static JSX_RUNTIME_RE: Lazy<regex::Regex> =
      lazy_regex::lazy_regex!(r"(?i)^[\s*]*@jsxRuntime\s+(\S+)");
    static JSX_FACTORY_RE: Lazy<regex::Regex> =
      lazy_regex::lazy_regex!(r"(?i)^[\s*]*@jsxFactory\s+(\S+)");
    static JSX_FRAGMENT_FACTORY_RE: Lazy<regex::Regex> =
      lazy_regex::lazy_regex!(r"(?i)^[\s*]*@jsxFragmentFactory\s+(\S+)");

    let start_pos = if parsed_source.program_ref().shebang().is_some() {
      match text_info.text_str().find('\n') {
        Some(index) => index + 1,
        None => return Ok(()), // nothing in this file
      }
    } else {
      0
    };
    let mut add_text_change = |text: String| {
      text_changes.push(TextChange {
        range: start_pos..start_pos,
        new_text: text,
      })
    };
    let jsx_options =
      self.resolve_jsx_options(specifier, text_info, diagnostic_reporter)?;
    if module_info.jsx_import_source.is_none() {
      if let Some(import_source) = jsx_options.jsx_import_source {
        add_text_change(format!("/** @jsxImportSource {} */", import_source));
      }
    }
    if module_info.jsx_import_source_types.is_none() {
      if let Some(import_source) = jsx_options.jsx_import_source_types {
        add_text_change(format!(
          "/** @jsxImportSourceTypes {} */",
          import_source
        ));
      }
    }
    let leading_comments = parsed_source.get_leading_comments();
    let leading_comments_has_re = |regex: &regex::Regex| {
      leading_comments
        .as_ref()
        .map(|comments| {
          comments.iter().any(|c| {
            c.kind == deno_ast::swc::common::comments::CommentKind::Block
              && regex.is_match(c.text.as_str())
          })
        })
        .unwrap_or(false)
    };
    if !leading_comments_has_re(&JSX_RUNTIME_RE) {
      add_text_change(format!(
        "/** @jsxRuntime {} */",
        jsx_options.jsx_runtime,
      ));
    }
    if !leading_comments_has_re(&JSX_FACTORY_RE) {
      add_text_change(format!(
        "/** @jsxFactory {} */",
        jsx_options.jsx_factory,
      ));
    }
    if !leading_comments_has_re(&JSX_FRAGMENT_FACTORY_RE) {
      add_text_change(format!(
        "/** @jsxFragmentFactory {} */",
        jsx_options.jsx_fragment_factory,
      ));
    }
    Ok(())
  }

  fn resolve_jsx_options<'a>(
    &'a self,
    specifier: &Url,
    text_info: &SourceTextInfo,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) -> Result<JsxFolderOptions<'a>, AnyError> {
    let tsconfig_folder_info =
      self.tsconfig_resolver.folder_for_specifier(specifier);
    let transpile_options =
      &tsconfig_folder_info.transpile_options()?.transpile;
    let jsx_runtime = if transpile_options.jsx_automatic {
      "automatic"
    } else {
      "classic"
    };
    let mut unfurl_import_source =
      |import_source: &str, resolution_kind: ResolutionKind| {
        let maybe_import_source = self
          .specifier_unfurler
          .unfurl_specifier_reporting_diagnostic(
            &specifier,
            import_source,
            resolution_kind,
            text_info,
            &deno_graph::PositionRange::zeroed(),
            diagnostic_reporter,
          );
        maybe_import_source.unwrap_or_else(|| import_source.to_string())
      };
    let jsx_import_source =
      transpile_options
        .jsx_import_source
        .as_ref()
        .map(|jsx_import_source| {
          unfurl_import_source(jsx_import_source, ResolutionKind::Execution)
        });
    let jsx_import_source_types = tsconfig_folder_info
      .lib_tsconfig(deno_config::deno_json::TsTypeLib::DenoWindow)?
      .0
      .get("jsxImportSourceTypes")
      .and_then(|s| s.as_str())
      .map(|jsx_import_source_types| {
        unfurl_import_source(jsx_import_source_types, ResolutionKind::Types)
      });
    Ok(JsxFolderOptions {
      jsx_runtime,
      jsx_factory: &transpile_options.jsx_factory,
      jsx_fragment_factory: &transpile_options.jsx_fragment_factory,
      jsx_import_source,
      jsx_import_source_types,
    })
  }
}

struct JsxFolderOptions<'a> {
  jsx_factory: &'a str,
  jsx_fragment_factory: &'a str,
  jsx_runtime: &'static str,
  jsx_import_source: Option<String>,
  jsx_import_source_types: Option<String>,
}
