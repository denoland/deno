// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::ModuleGraph;

use super::diagnostics::PublishDiagnosticsCollector;
use super::unfurl::SpecifierUnfurler;
use crate::cache::LazyGraphSourceParser;
use crate::cache::ParsedSourceCache;
use crate::tools::registry::diagnostics::PublishDiagnostic;

pub struct ModuleContentProvider {
  unfurler: SpecifierUnfurler,
  parsed_source_cache: Arc<ParsedSourceCache>,
}

impl ModuleContentProvider {
  pub fn new(
    unfurler: SpecifierUnfurler,
    parsed_source_cache: Arc<ParsedSourceCache>,
  ) -> Self {
    Self {
      unfurler,
      parsed_source_cache,
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
    let parsed_source = match source_parser.get_or_parse_source(specifier)? {
      Some(parsed_source) => parsed_source,
      None => {
        let data = std::fs::read(path).with_context(|| {
          format!("Unable to read file '{}'", path.display())
        })?;
        let media_type = MediaType::from_specifier(specifier);

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
    self.unfurler.unfurl_to_changes(
      specifier,
      &parsed_source,
      &mut text_changes,
      &mut reporter,
    );
    let rewritten_text =
      deno_ast::apply_text_changes(text_info.text_str(), text_changes);

    Ok(rewritten_text.into_bytes())
  }
}
