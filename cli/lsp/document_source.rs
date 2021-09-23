use deno_ast::Diagnostic;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::ModuleSpecifier;
use once_cell::sync::OnceCell;
use std::sync::Arc;

use super::analysis;
use super::text::LineIndex;

#[derive(Debug)]
struct DocumentSourceInner {
  specifier: ModuleSpecifier,
  media_type: MediaType,
  text_info: SourceTextInfo,
  parsed_module: OnceCell<Result<ParsedSource, Diagnostic>>,
  line_index: LineIndex,
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
    text: Arc<String>,
    line_index: LineIndex,
  ) -> Self {
    Self {
      inner: Arc::new(DocumentSourceInner {
        specifier: specifier.clone(),
        media_type,
        text_info: SourceTextInfo::new(text),
        parsed_module: OnceCell::new(),
        line_index,
      }),
    }
  }

  pub fn text_info(&self) -> &SourceTextInfo {
    &self.inner.text_info
  }

  pub fn line_index(&self) -> &LineIndex {
    &self.inner.line_index
  }

  pub fn module(&self) -> Option<&Result<ParsedSource, Diagnostic>> {
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
          self.inner.text_info.clone(),
          self.inner.media_type,
        )
      }))
    } else {
      None
    }
  }
}
