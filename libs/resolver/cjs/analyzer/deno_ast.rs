// Copyright 2018-2025 the Deno authors. MIT license.

use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_error::JsErrorBox;
use deno_graph::ast::ParsedSourceStore;
use url::Url;

use super::ModuleExportAnalyzer;
use crate::cache::ParsedSourceCacheRc;

pub struct DenoAstModuleExportAnalyzer {
  parsed_source_cache: ParsedSourceCacheRc,
}

impl DenoAstModuleExportAnalyzer {
  pub fn new(parsed_source_cache: ParsedSourceCacheRc) -> Self {
    Self {
      parsed_source_cache,
    }
  }
}

#[allow(clippy::disallowed_types)]
type ArcStr = std::sync::Arc<str>;

impl ModuleExportAnalyzer for DenoAstModuleExportAnalyzer {
  fn parse_module(
    &self,
    specifier: Url,
    media_type: MediaType,
    source: ArcStr,
  ) -> Result<Box<dyn super::ModuleForExportAnalysis>, JsErrorBox> {
    let maybe_parsed_source =
      self.parsed_source_cache.remove_parsed_source(&specifier);
    let parsed_source = maybe_parsed_source
      .map(Ok)
      .unwrap_or_else(|| {
        deno_ast::parse_program(deno_ast::ParseParams {
          specifier,
          text: source,
          media_type,
          capture_tokens: true,
          scope_analysis: false,
          maybe_syntax: None,
        })
      })
      .map_err(JsErrorBox::from_err)?;
    Ok(Box::new(parsed_source))
  }
}

impl super::ModuleForExportAnalysis for ParsedSource {
  fn specifier(&self) -> &Url {
    self.specifier()
  }

  fn compute_is_script(&self) -> bool {
    self.compute_is_script()
  }

  fn analyze_cjs(&self) -> super::ModuleExportsAndReExports {
    let analysis = ParsedSource::analyze_cjs(self);
    super::ModuleExportsAndReExports {
      exports: analysis.exports,
      reexports: analysis.reexports,
    }
  }

  fn analyze_es_runtime_exports(&self) -> super::ModuleExportsAndReExports {
    let analysis = ParsedSource::analyze_es_runtime_exports(self);
    super::ModuleExportsAndReExports {
      exports: analysis.exports,
      reexports: analysis.reexports,
    }
  }
}
