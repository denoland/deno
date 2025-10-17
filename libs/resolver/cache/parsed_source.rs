// Copyright 2018-2025 the Deno authors. MIT license.

use deno_ast::ParsedSource;
use deno_graph::ast::CapturingEsParser;
use deno_graph::ast::DefaultEsParser;
use deno_graph::ast::EsParser;
use deno_graph::ast::ParsedSourceStore;
use deno_media_type::MediaType;
use url::Url;

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

  #[allow(clippy::result_large_err)]
  pub fn get_or_parse_source(
    &self,
    module_specifier: &Url,
  ) -> Result<Option<ParsedSource>, deno_ast::ParseDiagnostic> {
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

#[allow(clippy::disallowed_types)] // ok because we always store source text as Arc<str>
type ArcStr = std::sync::Arc<str>;

#[allow(clippy::disallowed_types)]
pub type ParsedSourceCacheRc = deno_maybe_sync::MaybeArc<ParsedSourceCache>;

#[derive(Debug, Default)]
pub struct ParsedSourceCache {
  sources: deno_maybe_sync::MaybeDashMap<Url, ParsedSource>,
}

impl ParsedSourceCache {
  #[allow(clippy::result_large_err)]
  pub fn get_parsed_source_from_js_module(
    &self,
    module: &deno_graph::JsModule,
  ) -> Result<ParsedSource, deno_ast::ParseDiagnostic> {
    self.get_matching_parsed_source(
      &module.specifier,
      module.media_type,
      module.source.text.clone(),
    )
  }

  #[allow(clippy::result_large_err)]
  pub fn get_matching_parsed_source(
    &self,
    specifier: &Url,
    media_type: MediaType,
    source: ArcStr,
  ) -> Result<ParsedSource, deno_ast::ParseDiagnostic> {
    let parser = self.as_capturing_parser();
    // this will conditionally parse because it's using a CapturingEsParser
    parser.parse_program(deno_graph::ast::ParseOptions {
      specifier,
      source,
      media_type,
      scope_analysis: false,
    })
  }

  #[allow(clippy::result_large_err, clippy::disallowed_types)]
  pub fn remove_or_parse_module(
    &self,
    specifier: &Url,
    media_type: MediaType,
    source: ArcStr,
  ) -> Result<ParsedSource, deno_ast::ParseDiagnostic> {
    if let Some(parsed_source) = self.remove_parsed_source(specifier)
      && parsed_source.media_type() == media_type
      && parsed_source.text().as_ref() == source.as_ref()
    {
      // note: message used tests
      log::debug!("Removed parsed source: {}", specifier);
      return Ok(parsed_source);
    }
    let options = deno_graph::ast::ParseOptions {
      specifier,
      source,
      media_type,
      scope_analysis: false,
    };
    DefaultEsParser.parse_program(options)
  }

  /// Frees the parsed source from memory.
  pub fn free(&self, specifier: &Url) {
    self.sources.remove(specifier);
  }

  /// Fress all parsed sources from memory.
  pub fn free_all(&self) {
    self.sources.clear();
  }

  /// Creates a parser that will reuse a ParsedSource from the store
  /// if it exists, or else parse.
  pub fn as_capturing_parser(&self) -> CapturingEsParser<'_> {
    CapturingEsParser::new(None, self)
  }

  #[allow(clippy::len_without_is_empty)]
  pub fn len(&self) -> usize {
    self.sources.len()
  }
}

/// It's ok that this is racy since in non-LSP situations
/// this will only ever store one form of a parsed source
/// and in LSP settings the concurrency will be enforced
/// at a higher level to ensure this will have the latest
/// parsed source.
impl ParsedSourceStore for ParsedSourceCache {
  fn set_parsed_source(
    &self,
    specifier: Url,
    parsed_source: ParsedSource,
  ) -> Option<ParsedSource> {
    self.sources.insert(specifier, parsed_source)
  }

  fn get_parsed_source(&self, specifier: &Url) -> Option<ParsedSource> {
    self.sources.get(specifier).map(|p| p.clone())
  }

  fn remove_parsed_source(&self, specifier: &Url) -> Option<ParsedSource> {
    self.sources.remove(specifier).map(|(_, p)| p)
  }

  fn get_scope_analysis_parsed_source(
    &self,
    specifier: &Url,
  ) -> Option<ParsedSource> {
    {
      let parsed_source = self.sources.get(specifier)?;
      if parsed_source.has_scope_analysis() {
        return Some(parsed_source.clone());
      }
    }
    // upgrade to have scope analysis
    let (specifier, parsed_source) = self.sources.remove(specifier)?;
    let parsed_source = parsed_source.into_with_scope_analysis();
    self.sources.insert(specifier, parsed_source.clone());
    Some(parsed_source)
  }
}
