use std::sync::Arc;
use swc_ast_view::BytePos;
use swc_ast_view::LineAndColumnIndex;
use swc_ast_view::SourceFile;
use swc_ast_view::Span;

use crate::text_encoding::strip_bom_mut;

#[derive(Debug, Clone)]
pub struct SourceFileText {
  inner: Arc<swc_ast_view::SourceFileTextInfo>,
}

impl SourceFileText {
  pub fn new(mut text: String) -> Self {
    strip_bom_mut(&mut text);

    Self {
      inner: Arc::new(swc_ast_view::SourceFileTextInfo::new(BytePos(0), text)),
    }
  }

  pub fn as_str(&self) -> &str {
    self.inner.text()
  }

  pub fn to_string(&self) -> String {
    self.inner.text().to_string()
  }

  pub fn span(&self) -> Span {
    self.inner.span()
  }

  pub fn info(&self) -> &swc_ast_view::SourceFileTextInfo {
    &self.inner
  }

  pub fn line_text(&self, line_index: usize) -> &str {
    let line_start = self.inner.line_start(line_index).0 as usize;
    let line_end = self.inner.line_end(line_index).0 as usize;
    &self.as_str()[line_start..line_end]
  }

  pub fn line_and_column_index(&self, pos: BytePos) -> LineAndColumnIndex {
    self.inner.line_and_column_index(pos)
  }
}

impl From<&str> for SourceFileText {
  fn from(text: &str) -> Self {
    SourceFileText::new(text.to_string())
  }
}

impl From<String> for SourceFileText {
  fn from(text: String) -> Self {
    SourceFileText::new(text)
  }
}
