// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;

use deno_ast::swc::common::SyntaxContext;
use deno_ast::view::Node;
use deno_ast::view::NodeTrait;
use deno_ast::CjsAnalysis;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceRanged;
use deno_core::error::AnyError;
use deno_runtime::deno_node::analyze::CjsAnalysis as ExtNodeCjsAnalysis;
use deno_runtime::deno_node::analyze::CjsEsmCodeAnalyzer;
use deno_runtime::deno_node::analyze::NodeCodeTranslator;

use crate::cache::NodeAnalysisCache;
use crate::util::fs::canonicalize_path_maybe_not_exists;

pub type CliNodeCodeTranslator = NodeCodeTranslator<CliCjsEsmCodeAnalyzer>;

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

pub struct CliCjsEsmCodeAnalyzer {
  cache: NodeAnalysisCache,
}

impl CliCjsEsmCodeAnalyzer {
  pub fn new(cache: NodeAnalysisCache) -> Self {
    Self { cache }
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
      specifier: specifier.to_string(),
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

impl CjsEsmCodeAnalyzer for CliCjsEsmCodeAnalyzer {
  fn analyze_cjs(
    &self,
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> Result<ExtNodeCjsAnalysis, AnyError> {
    let analysis = self.inner_cjs_analysis(specifier, source)?;
    Ok(ExtNodeCjsAnalysis {
      exports: analysis.exports,
      reexports: analysis.reexports,
    })
  }

  fn analyze_esm_top_level_decls(
    &self,
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> Result<HashSet<String>, AnyError> {
    // TODO(dsherret): this code is way more inefficient than it needs to be.
    //
    // In the future, we should disable capturing tokens & scope analysis
    // and instead only use swc's APIs to go through the portions of the tree
    // that we know will affect the global scope while still ensuring that
    // `var` decls are taken into consideration.
    let source_hash = NodeAnalysisCache::compute_source_hash(source);
    if let Some(decls) = self
      .cache
      .get_esm_analysis(specifier.as_str(), &source_hash)
    {
      Ok(HashSet::from_iter(decls))
    } else {
      let parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
        specifier: specifier.to_string(),
        text_info: deno_ast::SourceTextInfo::from_string(source.to_string()),
        media_type: deno_ast::MediaType::from_specifier(specifier),
        capture_tokens: true,
        scope_analysis: true,
        maybe_syntax: None,
      })?;
      let top_level_decls = analyze_top_level_decls(&parsed_source)?;
      self.cache.set_esm_analysis(
        specifier.as_str(),
        &source_hash,
        &top_level_decls.clone().into_iter().collect::<Vec<_>>(),
      );
      Ok(top_level_decls)
    }
  }
}

fn analyze_top_level_decls(
  parsed_source: &ParsedSource,
) -> Result<HashSet<String>, AnyError> {
  fn visit_children(
    node: Node,
    top_level_context: SyntaxContext,
    results: &mut HashSet<String>,
  ) {
    if let Node::Ident(ident) = node {
      if ident.ctxt() == top_level_context && is_local_declaration_ident(node) {
        results.insert(ident.sym().to_string());
      }
    }

    for child in node.children() {
      visit_children(child, top_level_context, results);
    }
  }

  let top_level_context = parsed_source.top_level_context();

  parsed_source.with_view(|program| {
    let mut results = HashSet::new();
    visit_children(program.into(), top_level_context, &mut results);
    Ok(results)
  })
}

fn is_local_declaration_ident(node: Node) -> bool {
  if let Some(parent) = node.parent() {
    match parent {
      Node::BindingIdent(decl) => decl.id.range().contains(&node.range()),
      Node::ClassDecl(decl) => decl.ident.range().contains(&node.range()),
      Node::ClassExpr(decl) => decl
        .ident
        .as_ref()
        .map(|i| i.range().contains(&node.range()))
        .unwrap_or(false),
      Node::TsInterfaceDecl(decl) => decl.id.range().contains(&node.range()),
      Node::FnDecl(decl) => decl.ident.range().contains(&node.range()),
      Node::FnExpr(decl) => decl
        .ident
        .as_ref()
        .map(|i| i.range().contains(&node.range()))
        .unwrap_or(false),
      Node::TsModuleDecl(decl) => decl.id.range().contains(&node.range()),
      Node::TsNamespaceDecl(decl) => decl.id.range().contains(&node.range()),
      Node::VarDeclarator(decl) => decl.name.range().contains(&node.range()),
      Node::ImportNamedSpecifier(decl) => {
        decl.local.range().contains(&node.range())
      }
      Node::ImportDefaultSpecifier(decl) => {
        decl.local.range().contains(&node.range())
      }
      Node::ImportStarAsSpecifier(decl) => decl.range().contains(&node.range()),
      Node::KeyValuePatProp(decl) => decl.key.range().contains(&node.range()),
      Node::AssignPatProp(decl) => decl.key.range().contains(&node.range()),
      _ => false,
    }
  } else {
    false
  }
}
