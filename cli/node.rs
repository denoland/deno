// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;

use deno_ast::CjsAnalysis;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::analyze::CjsAnalysis as ExtNodeCjsAnalysis;
use deno_runtime::deno_node::analyze::CjsCodeAnalyzer;
use deno_runtime::deno_node::analyze::NodeCodeTranslator;

use crate::cache::NodeAnalysisCache;
use crate::util::fs::canonicalize_path_maybe_not_exists;

pub type CliNodeCodeTranslator = NodeCodeTranslator<CliCjsCodeAnalyzer>;

/// Resolves a specifier that is pointing into a node_modules folder.
///
/// Note: This should be called whenever getting the specifier from
/// a Module::External(module) reference because that module might
/// not be fully resolved at the time deno_graph is analyzing it
/// because the node_modules folder might not exist at that time.
pub fn resolve_specifier_into_node_modules(
  specifier: &ModuleSpecifier,
) -> ModuleSpecifier {
  specifier
    .to_file_path()
    .ok()
    // this path might not exist at the time the graph is being created
    // because the node_modules folder might not yet exist
    .and_then(|path| canonicalize_path_maybe_not_exists(&path).ok())
    .and_then(|path| ModuleSpecifier::from_file_path(path).ok())
    .unwrap_or_else(|| specifier.clone())
}

pub struct CliCjsCodeAnalyzer {
  cache: NodeAnalysisCache,
  fs: deno_fs::FileSystemRc,
}

impl CliCjsCodeAnalyzer {
  pub fn new(cache: NodeAnalysisCache, fs: deno_fs::FileSystemRc) -> Self {
    Self { cache, fs }
  }

  fn inner_cjs_analysis(
    &self,
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> Result<CjsAnalysis, AnyError> {
    let source_hash = NodeAnalysisCache::compute_source_hash(source);
    if let Some(analysis) = self
      .cache
      .get_cjs_analysis(specifier.as_str(), &source_hash)
    {
      return Ok(analysis);
    }

    let media_type = MediaType::from_specifier(specifier);
    if media_type == MediaType::Json {
      return Ok(CjsAnalysis {
        exports: vec![],
        reexports: vec![],
      });
    }

    let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
      specifier: specifier.clone(),
      text_info: deno_ast::SourceTextInfo::new(source.into()),
      media_type,
      capture_tokens: true,
      scope_analysis: false,
      maybe_syntax: None,
    })?;
    let analysis = parsed_source.analyze_cjs();
    self
      .cache
      .set_cjs_analysis(specifier.as_str(), &source_hash, &analysis);

    Ok(analysis)
  }
}

impl CjsCodeAnalyzer for CliCjsCodeAnalyzer {
  fn analyze_cjs(
    &self,
    specifier: &ModuleSpecifier,
    source: Option<&str>,
  ) -> Result<ExtNodeCjsAnalysis, AnyError> {
    let source = match source {
      Some(source) => Cow::Borrowed(source),
      None => Cow::Owned(
        self
          .fs
          .read_text_file_sync(&specifier.to_file_path().unwrap())?,
      ),
    };
    let analysis = self.inner_cjs_analysis(specifier, &source)?;
    Ok(ExtNodeCjsAnalysis {
      exports: analysis.exports,
      reexports: analysis.reexports,
    })
  }
}
