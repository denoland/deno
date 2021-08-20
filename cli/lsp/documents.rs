// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::text::LineIndex;
use super::tsc;

use crate::media_type::MediaType;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Range;
use std::str::FromStr;

/// A representation of the language id sent from the LSP client, which is used
/// to determine how the document is handled within the language server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageId {
  JavaScript,
  Jsx,
  TypeScript,
  Tsx,
  Json,
  JsonC,
  Markdown,
  Unknown,
}

impl FromStr for LanguageId {
  type Err = AnyError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "javascript" => Ok(Self::JavaScript),
      "javascriptreact" => Ok(Self::Jsx),
      "jsx" => Ok(Self::Jsx),
      "typescript" => Ok(Self::TypeScript),
      "typescriptreact" => Ok(Self::Tsx),
      "tsx" => Ok(Self::Tsx),
      "json" => Ok(Self::Json),
      "jsonc" => Ok(Self::JsonC),
      "markdown" => Ok(Self::Markdown),
      _ => Ok(Self::Unknown),
    }
  }
}

impl<'a> From<&'a LanguageId> for MediaType {
  fn from(id: &'a LanguageId) -> MediaType {
    match id {
      LanguageId::JavaScript => MediaType::JavaScript,
      LanguageId::Json => MediaType::Json,
      LanguageId::JsonC => MediaType::Json,
      LanguageId::Jsx => MediaType::Jsx,
      LanguageId::Markdown => MediaType::Unknown,
      LanguageId::Tsx => MediaType::Tsx,
      LanguageId::TypeScript => MediaType::TypeScript,
      LanguageId::Unknown => MediaType::Unknown,
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
enum IndexValid {
  All,
  UpTo(u32),
}

impl IndexValid {
  fn covers(&self, line: u32) -> bool {
    match *self {
      IndexValid::UpTo(to) => to > line,
      IndexValid::All => true,
    }
  }
}

#[derive(Debug, Clone)]
pub struct DocumentData {
  bytes: Option<Vec<u8>>,
  dependencies: Option<HashMap<String, analysis::Dependency>>,
  dependency_ranges: Option<analysis::DependencyRanges>,
  pub(crate) language_id: LanguageId,
  line_index: Option<LineIndex>,
  maybe_navigation_tree: Option<tsc::NavigationTree>,
  specifier: ModuleSpecifier,
  version: Option<i32>,
}

impl DocumentData {
  pub fn new(
    specifier: ModuleSpecifier,
    version: i32,
    language_id: LanguageId,
    source: &str,
  ) -> Self {
    Self {
      bytes: Some(source.as_bytes().to_owned()),
      dependencies: None,
      dependency_ranges: None,
      language_id,
      line_index: Some(LineIndex::new(source)),
      maybe_navigation_tree: None,
      specifier,
      version: Some(version),
    }
  }

  pub fn apply_content_changes(
    &mut self,
    content_changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<(), AnyError> {
    if self.bytes.is_none() {
      return Ok(());
    }
    let content = &mut String::from_utf8(self.bytes.clone().unwrap())
      .context("unable to parse bytes to string")?;
    let mut line_index = if let Some(line_index) = &self.line_index {
      line_index.clone()
    } else {
      LineIndex::new(content)
    };
    let mut index_valid = IndexValid::All;
    for change in content_changes {
      if let Some(range) = change.range {
        if !index_valid.covers(range.start.line) {
          line_index = LineIndex::new(content);
        }
        index_valid = IndexValid::UpTo(range.start.line);
        let range = line_index.get_text_range(range)?;
        content.replace_range(Range::<usize>::from(range), &change.text);
      } else {
        *content = change.text;
        index_valid = IndexValid::UpTo(0);
      }
    }
    self.bytes = Some(content.as_bytes().to_owned());
    self.line_index = if index_valid == IndexValid::All {
      Some(line_index)
    } else {
      Some(LineIndex::new(content))
    };
    self.maybe_navigation_tree = None;
    Ok(())
  }

  pub fn content(&self) -> Result<Option<String>, AnyError> {
    if let Some(bytes) = &self.bytes {
      Ok(Some(
        String::from_utf8(bytes.clone())
          .context("cannot decode bytes to string")?,
      ))
    } else {
      Ok(None)
    }
  }

  pub fn content_line(&self, line: usize) -> Result<Option<String>, AnyError> {
    let content = self.content().ok().flatten();
    if let Some(content) = content {
      let lines = content.lines().into_iter().collect::<Vec<&str>>();
      Ok(Some(lines[line].to_string()))
    } else {
      Ok(None)
    }
  }

  /// Determines if a position within the document is within a dependency range
  /// and if so, returns the range with the text of the specifier.
  fn is_specifier_position(
    &self,
    position: &lsp::Position,
  ) -> Option<analysis::DependencyRange> {
    let import_ranges = self.dependency_ranges.as_ref()?;
    import_ranges.contains(position)
  }
}

#[derive(Debug, Clone, Default)]
pub struct DocumentCache {
  dependents_graph: HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>>,
  pub(crate) docs: HashMap<ModuleSpecifier, DocumentData>,
}

impl DocumentCache {
  /// Calculate a graph of dependents and set it on the structure.
  fn calculate_dependents(&mut self) {
    let mut dependents_graph: HashMap<
      ModuleSpecifier,
      HashSet<ModuleSpecifier>,
    > = HashMap::new();
    for (specifier, data) in &self.docs {
      if let Some(dependencies) = &data.dependencies {
        for dependency in dependencies.values() {
          if let Some(analysis::ResolvedDependency::Resolved(dep_specifier)) =
            &dependency.maybe_code
          {
            dependents_graph
              .entry(dep_specifier.clone())
              .or_default()
              .insert(specifier.clone());
          }
          if let Some(analysis::ResolvedDependency::Resolved(dep_specifier)) =
            &dependency.maybe_type
          {
            dependents_graph
              .entry(dep_specifier.clone())
              .or_default()
              .insert(specifier.clone());
          }
        }
      }
    }
    self.dependents_graph = dependents_graph;
  }

  pub fn change(
    &mut self,
    specifier: &ModuleSpecifier,
    version: i32,
    content_changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<Option<String>, AnyError> {
    if !self.contains_key(specifier) {
      return Err(custom_error(
        "NotFound",
        format!(
          "The specifier (\"{}\") does not exist in the document cache.",
          specifier
        ),
      ));
    }

    let doc = self.docs.get_mut(specifier).unwrap();
    doc.apply_content_changes(content_changes)?;
    doc.version = Some(version);
    doc.content()
  }

  pub fn close(&mut self, specifier: &ModuleSpecifier) {
    self.docs.remove(specifier);
    self.calculate_dependents();
  }

  pub fn contains_key(&self, specifier: &ModuleSpecifier) -> bool {
    self.docs.contains_key(specifier)
  }

  pub fn content(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<String>, AnyError> {
    if let Some(doc) = self.docs.get(specifier) {
      doc.content()
    } else {
      Ok(None)
    }
  }

  // For a given specifier, get all open documents which directly or indirectly
  // depend upon the specifier.
  pub fn dependents(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Vec<ModuleSpecifier> {
    let mut dependents = HashSet::new();
    self.recurse_dependents(specifier, &mut dependents);
    dependents.into_iter().collect()
  }

  pub fn dependencies(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<HashMap<String, analysis::Dependency>> {
    self
      .docs
      .get(specifier)
      .map(|doc| doc.dependencies.clone())
      .flatten()
  }

  pub fn get_language_id(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<LanguageId> {
    self.docs.get(specifier).map(|doc| doc.language_id.clone())
  }

  pub fn get_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<tsc::NavigationTree> {
    self
      .docs
      .get(specifier)
      .map(|doc| doc.maybe_navigation_tree.clone())
      .flatten()
  }

  /// Determines if the specifier should be processed for diagnostics and other
  /// related language server features.
  pub fn is_diagnosable(&self, specifier: &ModuleSpecifier) -> bool {
    if specifier.scheme() != "file" {
      // otherwise we look at the media type for the specifier.
      matches!(
        MediaType::from(specifier),
        MediaType::JavaScript
          | MediaType::Jsx
          | MediaType::TypeScript
          | MediaType::Tsx
          | MediaType::Dts
      )
    } else if let Some(doc_data) = self.docs.get(specifier) {
      // if the document is in the document cache, then use the client provided
      // language id to determine if the specifier is diagnosable.
      matches!(
        doc_data.language_id,
        LanguageId::JavaScript
          | LanguageId::Jsx
          | LanguageId::TypeScript
          | LanguageId::Tsx
      )
    } else {
      false
    }
  }

  /// Determines if the specifier can be processed for formatting.
  pub fn is_formattable(&self, specifier: &ModuleSpecifier) -> bool {
    self.docs.contains_key(specifier)
  }

  /// Determines if the position in the document is within a range of a module
  /// specifier, returning the text range if true.
  pub fn is_specifier_position(
    &self,
    specifier: &ModuleSpecifier,
    position: &lsp::Position,
  ) -> Option<analysis::DependencyRange> {
    let document = self.docs.get(specifier)?;
    document.is_specifier_position(position)
  }

  pub fn len(&self) -> usize {
    self.docs.len()
  }

  pub fn line_index(&self, specifier: &ModuleSpecifier) -> Option<LineIndex> {
    let doc = self.docs.get(specifier)?;
    doc.line_index.clone()
  }

  pub fn open(
    &mut self,
    specifier: ModuleSpecifier,
    version: i32,
    language_id: LanguageId,
    source: &str,
  ) {
    self.docs.insert(
      specifier.clone(),
      DocumentData::new(specifier, version, language_id, source),
    );
  }

  pub fn open_specifiers(&self) -> Vec<&ModuleSpecifier> {
    self
      .docs
      .iter()
      .filter_map(|(key, data)| {
        if data.version.is_some() {
          Some(key)
        } else {
          None
        }
      })
      .collect()
  }

  fn recurse_dependents(
    &self,
    specifier: &ModuleSpecifier,
    dependents: &mut HashSet<ModuleSpecifier>,
  ) {
    if let Some(deps) = self.dependents_graph.get(specifier) {
      for dep in deps {
        if !dependents.contains(dep) {
          dependents.insert(dep.clone());
          self.recurse_dependents(dep, dependents);
        }
      }
    }
  }

  pub fn set_dependencies(
    &mut self,
    specifier: &ModuleSpecifier,
    maybe_dependencies: Option<HashMap<String, analysis::Dependency>>,
    maybe_dependency_ranges: Option<analysis::DependencyRanges>,
  ) -> Result<(), AnyError> {
    if let Some(doc) = self.docs.get_mut(specifier) {
      doc.dependencies = maybe_dependencies;
      doc.dependency_ranges = maybe_dependency_ranges;
      self.calculate_dependents();
      Ok(())
    } else {
      Err(custom_error(
        "NotFound",
        format!(
          "The specifier (\"{}\") does not exist in the document cache.",
          specifier
        ),
      ))
    }
  }

  pub fn set_navigation_tree(
    &mut self,
    specifier: &ModuleSpecifier,
    navigation_tree: tsc::NavigationTree,
  ) -> Result<(), AnyError> {
    if let Some(doc) = self.docs.get_mut(specifier) {
      doc.maybe_navigation_tree = Some(navigation_tree);
      Ok(())
    } else {
      Err(custom_error(
        "NotFound",
        format!(
          "The specifier (\"{}\") does not exist in the document cache.",
          specifier
        ),
      ))
    }
  }

  pub fn specifiers(&self) -> Vec<ModuleSpecifier> {
    self.docs.keys().cloned().collect()
  }

  pub fn version(&self, specifier: &ModuleSpecifier) -> Option<i32> {
    self.docs.get(specifier).and_then(|doc| doc.version)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url;
  use lspower::lsp;

  #[test]
  fn test_language_id() {
    assert_eq!(
      "javascript".parse::<LanguageId>().unwrap(),
      LanguageId::JavaScript
    );
    assert_eq!(
      "javascriptreact".parse::<LanguageId>().unwrap(),
      LanguageId::Jsx
    );
    assert_eq!("jsx".parse::<LanguageId>().unwrap(), LanguageId::Jsx);
    assert_eq!(
      "typescript".parse::<LanguageId>().unwrap(),
      LanguageId::TypeScript
    );
    assert_eq!(
      "typescriptreact".parse::<LanguageId>().unwrap(),
      LanguageId::Tsx
    );
    assert_eq!("tsx".parse::<LanguageId>().unwrap(), LanguageId::Tsx);
    assert_eq!("json".parse::<LanguageId>().unwrap(), LanguageId::Json);
    assert_eq!("jsonc".parse::<LanguageId>().unwrap(), LanguageId::JsonC);
    assert_eq!(
      "markdown".parse::<LanguageId>().unwrap(),
      LanguageId::Markdown
    );
    assert_eq!("rust".parse::<LanguageId>().unwrap(), LanguageId::Unknown);
  }

  #[test]
  fn test_document_cache_contains() {
    let mut document_cache = DocumentCache::default();
    let specifier = resolve_url("file:///a/b.ts").unwrap();
    let missing_specifier = resolve_url("file:///a/c.ts").unwrap();
    document_cache.open(
      specifier.clone(),
      1,
      LanguageId::TypeScript,
      "console.log(\"Hello Deno\");\n",
    );
    assert!(document_cache.contains_key(&specifier));
    assert!(!document_cache.contains_key(&missing_specifier));
  }

  #[test]
  fn test_document_cache_change() {
    let mut document_cache = DocumentCache::default();
    let specifier = resolve_url("file:///a/b.ts").unwrap();
    document_cache.open(
      specifier.clone(),
      1,
      LanguageId::TypeScript,
      "console.log(\"Hello deno\");\n",
    );
    document_cache
      .change(
        &specifier,
        2,
        vec![lsp::TextDocumentContentChangeEvent {
          range: Some(lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 19,
            },
            end: lsp::Position {
              line: 0,
              character: 20,
            },
          }),
          range_length: Some(1),
          text: "D".to_string(),
        }],
      )
      .expect("failed to make changes");
    let actual = document_cache
      .content(&specifier)
      .expect("failed to get content");
    assert_eq!(actual, Some("console.log(\"Hello Deno\");\n".to_string()));
  }

  #[test]
  fn test_document_cache_change_utf16() {
    let mut document_cache = DocumentCache::default();
    let specifier = resolve_url("file:///a/b.ts").unwrap();
    document_cache.open(
      specifier.clone(),
      1,
      LanguageId::TypeScript,
      "console.log(\"Hello 🦕\");\n",
    );
    document_cache
      .change(
        &specifier,
        2,
        vec![lsp::TextDocumentContentChangeEvent {
          range: Some(lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 19,
            },
            end: lsp::Position {
              line: 0,
              character: 21,
            },
          }),
          range_length: Some(2),
          text: "Deno".to_string(),
        }],
      )
      .expect("failed to make changes");
    let actual = document_cache
      .content(&specifier)
      .expect("failed to get content");
    assert_eq!(actual, Some("console.log(\"Hello Deno\");\n".to_string()));
  }

  #[test]
  fn test_is_diagnosable() {
    let mut document_cache = DocumentCache::default();
    let specifier = resolve_url("file:///a/file.ts").unwrap();
    assert!(!document_cache.is_diagnosable(&specifier));
    document_cache.open(
      specifier.clone(),
      1,
      "typescript".parse().unwrap(),
      "console.log(\"hello world\");\n",
    );
    assert!(document_cache.is_diagnosable(&specifier));
    let specifier = resolve_url("file:///a/file.rs").unwrap();
    document_cache.open(
      specifier.clone(),
      1,
      "rust".parse().unwrap(),
      "pub mod a;",
    );
    assert!(!document_cache.is_diagnosable(&specifier));
    let specifier =
      resolve_url("asset:///lib.es2015.symbol.wellknown.d.ts").unwrap();
    assert!(document_cache.is_diagnosable(&specifier));
    let specifier = resolve_url("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    assert!(document_cache.is_diagnosable(&specifier));
    let specifier = resolve_url("data:application/json;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    assert!(!document_cache.is_diagnosable(&specifier));
  }
}
