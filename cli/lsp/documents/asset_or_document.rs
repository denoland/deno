// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::super::text::LineIndex;
use super::super::tsc;
use super::super::tsc::AssetDocument;
use super::document::Document;
use deno_ast::MediaType;
use deno_core::ModuleSpecifier;
use deno_graph::Dependency;
use indexmap::IndexMap;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

/// Represents either a document or an asset.
///
/// Functions are basically just passthroughs to the underlying document.
#[derive(Debug, Clone)]
pub enum AssetOrDocument {
  Document(Document),
  Asset(AssetDocument),
}

impl AssetOrDocument {
  pub fn specifier(&self) -> &ModuleSpecifier {
    match self {
      AssetOrDocument::Asset(asset) => asset.specifier(),
      AssetOrDocument::Document(doc) => doc.specifier(),
    }
  }

  pub fn document(&self) -> Option<&Document> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => Some(doc),
    }
  }

  pub fn text(&self) -> Arc<str> {
    match self {
      AssetOrDocument::Asset(asset) => asset.text(),
      AssetOrDocument::Document(doc) => doc.text(),
    }
  }

  pub fn line_index(&self) -> Arc<LineIndex> {
    match self {
      AssetOrDocument::Asset(asset) => asset.line_index(),
      AssetOrDocument::Document(doc) => doc.line_index(),
    }
  }

  pub fn maybe_navigation_tree(&self) -> Option<Arc<tsc::NavigationTree>> {
    match self {
      AssetOrDocument::Asset(asset) => asset.maybe_navigation_tree(),
      AssetOrDocument::Document(doc) => doc.maybe_navigation_tree(),
    }
  }

  pub fn media_type(&self) -> MediaType {
    match self {
      AssetOrDocument::Asset(_) => MediaType::TypeScript, // assets are always TypeScript
      AssetOrDocument::Document(doc) => doc.media_type(),
    }
  }

  pub fn get_maybe_dependency(
    &self,
    position: &lsp::Position,
  ) -> Option<(String, deno_graph::Dependency, deno_graph::Range)> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => doc.get_maybe_dependency(position),
    }
  }

  pub fn dependencies(&self) -> Option<&IndexMap<String, Dependency>> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => Some(doc.dependencies()),
    }
  }

  pub fn maybe_parsed_source(
    &self,
  ) -> Option<Result<deno_ast::ParsedSource, deno_ast::Diagnostic>> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => doc.maybe_parsed_source(),
    }
  }

  pub fn document_lsp_version(&self) -> Option<i32> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => doc.maybe_lsp_version(),
    }
  }

  pub fn is_open(&self) -> bool {
    match self {
      AssetOrDocument::Asset(_) => false,
      AssetOrDocument::Document(doc) => doc.is_open(),
    }
  }
}
