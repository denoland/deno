use deno_core::ModuleSpecifier;
use once_cell::sync::OnceCell;
use std::sync::Arc;

use crate::ast::ParsedModule;
use crate::ast::SourceFileText;
use crate::media_type::MediaType;
use super::analysis;
use super::text::LineIndex;

#[derive(Debug)]
struct DocumentSourceInner {
  specifier: ModuleSpecifier,
  media_type: MediaType,
  text: SourceFileText,
  parsed_module: OnceCell<Result<ParsedModule, String>>,
  line_index: LineIndex,
}

/// Immutable information about a document that can be cheaply cloned.
#[derive(Debug, Clone)]
pub struct DocumentSource {
  inner: Arc<DocumentSourceInner>,
}

impl DocumentSource {
  pub fn new(
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    text: String,
    line_index: LineIndex,
  ) -> Self {
    Self {
      inner: Arc::new(DocumentSourceInner {
        specifier: specifier.clone(),
        media_type,
        text: text.into(),
        parsed_module: OnceCell::new(),
        line_index,
      }),
    }
  }

  pub fn text(&self) -> &SourceFileText {
    &self.inner.text
  }

  pub fn line_index(&self) -> &LineIndex {
    &self.inner.line_index
  }

  pub fn module(&self) -> Option<&Result<ParsedModule, String>> {
    let is_parsable = matches!(
      self.inner.media_type,
      MediaType::JavaScript
        | MediaType::Jsx
        | MediaType::TypeScript
        | MediaType::Tsx
        | MediaType::Dts,
    );
    if is_parsable {
      // lazily parse the module
      Some(self.inner.parsed_module.get_or_init(|| {
        analysis::parse_module(
          &self.inner.specifier,
          self.inner.text.clone(),
          self.inner.media_type,
        )
        .map_err(|e| e.to_string())
      }))
    } else {
      None
    }
  }
}
