// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_core::parking_lot::Mutex;
use deno_graph::CapturingModuleParser;
use deno_graph::ModuleParser;
use deno_graph::ParseOptions;

/// Lazily parses JS/TS sources from a `deno_graph::ModuleGraph` given
/// a `ParsedSourceCache`. Note that deno_graph doesn't necessarily cause
/// files to end up in the `ParsedSourceCache` because it might have all
/// the information it needs via caching in order to skip parsing.
#[derive(Clone, Copy)]
pub struct LazyGraphSourceParser<'a> {
  cache: &'a ParsedSourceCache,
  graph: &'a deno_graph::ModuleGraph,
}

impl<'a> LazyGraphSourceParser<'a> {
  pub fn new(
    cache: &'a ParsedSourceCache,
    graph: &'a deno_graph::ModuleGraph,
  ) -> Self {
    Self { cache, graph }
  }

  pub fn get_or_parse_source(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<Option<deno_ast::ParsedSource>, deno_ast::ParseDiagnostic> {
    let Some(deno_graph::Module::Js(module)) = self.graph.get(module_specifier)
    else {
      return Ok(None);
    };
    self
      .cache
      .get_parsed_source_from_js_module(module)
      .map(Some)
  }
}

#[derive(Default)]
pub struct ParsedSourceCache {
  sources: Mutex<HashMap<ModuleSpecifier, ParsedSource>>,
}

impl ParsedSourceCache {
  pub fn get_parsed_source_from_js_module(
    &self,
    module: &deno_graph::JsModule,
  ) -> Result<ParsedSource, deno_ast::ParseDiagnostic> {
    self.get_or_parse_module(
      &module.specifier,
      module.source.clone(),
      module.media_type,
    )
  }

  /// Gets the matching `ParsedSource` from the cache
  /// or parses a new one and stores that in the cache.
  pub fn get_or_parse_module(
    &self,
    specifier: &deno_graph::ModuleSpecifier,
    source: Arc<str>,
    media_type: MediaType,
  ) -> deno_core::anyhow::Result<ParsedSource, deno_ast::ParseDiagnostic> {
    let parser = self.as_capturing_parser();
    // this will conditionally parse because it's using a CapturingModuleParser
    parser.parse_module(ParseOptions {
      specifier,
      source,
      media_type,
      // don't bother enabling because this method is currently only used for emitting
      scope_analysis: false,
    })
  }

  /// Frees the parsed source from memory.
  pub fn free(&self, specifier: &ModuleSpecifier) {
    self.sources.lock().remove(specifier);
  }

  /// Creates a parser that will reuse a ParsedSource from the store
  /// if it exists, or else parse.
  pub fn as_capturing_parser(&self) -> CapturingModuleParser {
    CapturingModuleParser::new(None, self)
  }
}

/// It's ok that this is racy since in non-LSP situations
/// this will only ever store one form of a parsed source
/// and in LSP settings the concurrency will be enforced
/// at a higher level to ensure this will have the latest
/// parsed source.
impl deno_graph::ParsedSourceStore for ParsedSourceCache {
  fn set_parsed_source(
    &self,
    specifier: deno_graph::ModuleSpecifier,
    parsed_source: ParsedSource,
  ) -> Option<ParsedSource> {
    self.sources.lock().insert(specifier, parsed_source)
  }

  fn get_parsed_source(
    &self,
    specifier: &deno_graph::ModuleSpecifier,
  ) -> Option<ParsedSource> {
    self.sources.lock().get(specifier).cloned()
  }

  fn get_scope_analysis_parsed_source(
    &self,
    specifier: &deno_graph::ModuleSpecifier,
  ) -> Option<ParsedSource> {
    let mut sources = self.sources.lock();
    let parsed_source = sources.get(specifier)?;
    if parsed_source.has_scope_analysis() {
      Some(parsed_source.clone())
    } else {
      // upgrade to have scope analysis
      let parsed_source = sources.remove(specifier).unwrap();
      let parsed_source = parsed_source.into_with_scope_analysis();
      sources.insert(specifier.clone(), parsed_source.clone());
      Some(parsed_source)
    }
  }
}
