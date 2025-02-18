// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::SourceRanged;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::ModuleGraph;
use deno_resolver::workspace::ResolutionKind;

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
    let mut text_changes = Vec::new();
    if media_type.is_jsx() {
      let jsx_options =
        self.resolve_jsx_options(text_info, &mut reporter, specifier);
      // todo: update the text with these jsx options, but only if no
      // existing pragma exists
    }
    self.specifier_unfurler.unfurl_to_changes(
      specifier,
      &parsed_source,
      &mut text_changes,
      &mut reporter,
    );
    let rewritten_text =
      deno_ast::apply_text_changes(text_info.text_str(), text_changes);

    Ok(rewritten_text.into_bytes())
  }

  fn resolve_jsx_options<'a>(
    &self,
    text_info: &SourceTextInfo,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
    specifier: &Url,
  ) -> JsxFolderOptions<'a> {
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
    JsxFolderOptions {
      jsx_runtime,
      jsx_factory: &transpile_options.jsx_factory,
      jsx_fragment_factory: &transpile_options.jsx_fragment_factory,
      jsx_import_source,
      jsx_import_source_types,
    }
  }
}

struct JsxFolderOptions<'a> {
  jsx_factory: &'a str,
  jsx_fragment_factory: &'a str,
  jsx_runtime: &'static str,
  jsx_import_source: Option<String>,
  jsx_import_source_types: Option<String>,
}
