// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_core::parking_lot::Mutex;
use deno_graph::CapturingModuleParser;
use deno_graph::ModuleParser;

#[derive(Default)]
pub struct ParsedSourceCache {
  sources: Mutex<HashMap<ModuleSpecifier, ParsedSource>>,
}

impl ParsedSourceCache {
  pub fn get_parsed_source_from_esm_module(
    &self,
    module: &deno_graph::EsmModule,
  ) -> Result<ParsedSource, deno_ast::Diagnostic> {
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
  ) -> deno_core::anyhow::Result<ParsedSource, deno_ast::Diagnostic> {
    let parser = self.as_capturing_parser();
    // this will conditionally parse because it's using a CapturingModuleParser
    parser.parse_module(specifier, source, media_type)
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

  pub fn as_store(self: &Arc<Self>) -> Arc<dyn deno_graph::ParsedSourceStore> {
    self.clone()
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
}
