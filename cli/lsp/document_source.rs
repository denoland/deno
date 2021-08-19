use std::sync::Arc;
use deno_core::ModuleSpecifier;
use once_cell::sync::OnceCell;

use crate::ast::{ParsedModule, SourceFileText};
use crate::lsp::analysis;
use crate::media_type::MediaType;

#[derive(Debug)]
struct DocumentSourceInner {
  specifier: ModuleSpecifier,
  media_type: MediaType,
  text: SourceFileText,
  parsed_module: OnceCell<Result<ParsedModule, String>>,
}

/// Immutable information about a document.
#[derive(Debug, Clone)]
pub struct DocumentSource {
  inner: Arc<DocumentSourceInner>,
}

impl DocumentSource {
  pub fn new(
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    text: String,
  ) -> Self {
    Self {
      inner: Arc::new(DocumentSourceInner {
        specifier: specifier.clone(),
        media_type,
        text: text.into(),
        parsed_module: OnceCell::new(),
      }),
    }
  }

  pub fn text(&self) -> &SourceFileText {
    &self.inner.text
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
