// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::text::LineIndex;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::ModuleSpecifier;
use lspower::lsp::TextDocumentContentChangeEvent;
use std::collections::HashMap;
use std::ops::Range;

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

#[derive(Debug, Clone, Default)]
pub struct DocumentData {
  bytes: Option<Vec<u8>>,
  line_index: Option<LineIndex>,
  dependencies: Option<HashMap<String, analysis::Dependency>>,
  version: Option<i32>,
}

impl DocumentData {
  pub fn apply_content_changes(
    &mut self,
    content_changes: Vec<TextDocumentContentChangeEvent>,
  ) -> Result<(), AnyError> {
    if self.bytes.is_none() {
      return Ok(());
    }
    let content = &mut String::from_utf8(self.bytes.clone().unwrap())
      .context("unable to parse bytes to string")?;
    let mut line_index = if let Some(line_index) = &self.line_index {
      line_index.clone()
    } else {
      LineIndex::new(&content)
    };
    let mut index_valid = IndexValid::All;
    for change in content_changes {
      if let Some(range) = change.range {
        if !index_valid.covers(range.start.line) {
          line_index = LineIndex::new(&content);
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
      Some(LineIndex::new(&content))
    };
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
}

#[derive(Debug, Clone, Default)]
pub struct DocumentCache {
  docs: HashMap<ModuleSpecifier, DocumentData>,
}

impl DocumentCache {
  pub fn change(
    &mut self,
    specifier: &ModuleSpecifier,
    version: i32,
    content_changes: Vec<TextDocumentContentChangeEvent>,
  ) -> Result<Option<String>, AnyError> {
    if !self.contains(specifier) {
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
    if let Some(mut doc) = self.docs.get_mut(specifier) {
      doc.version = None;
      doc.dependencies = None;
    }
  }

  pub fn contains(&self, specifier: &ModuleSpecifier) -> bool {
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

  pub fn dependencies(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<HashMap<String, analysis::Dependency>> {
    let doc = self.docs.get(specifier)?;
    doc.dependencies.clone()
  }

  pub fn len(&self) -> usize {
    self.docs.iter().count()
  }

  pub fn line_index(&self, specifier: &ModuleSpecifier) -> Option<LineIndex> {
    let doc = self.docs.get(specifier)?;
    doc.line_index.clone()
  }

  pub fn open(&mut self, specifier: ModuleSpecifier, version: i32, text: &str) {
    self.docs.insert(
      specifier,
      DocumentData {
        bytes: Some(text.as_bytes().to_owned()),
        version: Some(version),
        line_index: Some(LineIndex::new(&text)),
        ..Default::default()
      },
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

  pub fn set_dependencies(
    &mut self,
    specifier: &ModuleSpecifier,
    maybe_dependencies: Option<HashMap<String, analysis::Dependency>>,
  ) -> Result<(), AnyError> {
    if let Some(doc) = self.docs.get_mut(specifier) {
      doc.dependencies = maybe_dependencies;
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

  pub fn version(&self, specifier: &ModuleSpecifier) -> Option<i32> {
    self.docs.get(specifier).and_then(|doc| doc.version)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use lspower::lsp;

  #[test]
  fn test_document_cache_contains() {
    let mut document_cache = DocumentCache::default();
    let specifier = ModuleSpecifier::resolve_url("file:///a/b.ts").unwrap();
    let missing_specifier =
      ModuleSpecifier::resolve_url("file:///a/c.ts").unwrap();
    document_cache.open(specifier.clone(), 1, "console.log(\"Hello Deno\");\n");
    assert!(document_cache.contains(&specifier));
    assert!(!document_cache.contains(&missing_specifier));
  }

  #[test]
  fn test_document_cache_change() {
    let mut document_cache = DocumentCache::default();
    let specifier = ModuleSpecifier::resolve_url("file:///a/b.ts").unwrap();
    document_cache.open(specifier.clone(), 1, "console.log(\"Hello deno\");\n");
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
    let specifier = ModuleSpecifier::resolve_url("file:///a/b.ts").unwrap();
    document_cache.open(specifier.clone(), 1, "console.log(\"Hello 🦕\");\n");
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
}
