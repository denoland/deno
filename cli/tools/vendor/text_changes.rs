// todo: before merge, delete this file as it's now in deno_ast

use std::ops::Range;

use deno_ast::swc::common::BytePos;
use deno_ast::swc::common::Span;

#[derive(Clone, Debug)]
pub struct TextChange {
  /// Range start to end byte index.
  pub range: Range<usize>,
  /// New text to insert or replace at the provided range.
  pub new_text: String,
}

impl TextChange {
  pub fn new(start: usize, end: usize, new_text: String) -> Self {
    Self {
      range: start..end,
      new_text,
    }
  }

  pub fn from_span_and_text(span: Span, new_text: String) -> Self {
    TextChange::new(span.lo.0 as usize, span.hi.0 as usize, new_text)
  }

  /// Gets an swc span for the provided text change.
  pub fn as_span(&self) -> Span {
    Span::new(
      BytePos(self.range.start as u32),
      BytePos(self.range.end as u32),
      Default::default(),
    )
  }
}

/// Applies the text changes to the given source text.
pub fn apply_text_changes(
  source: &str,
  mut changes: Vec<TextChange>,
) -> String {
  changes.sort_by(|a, b| a.range.start.cmp(&b.range.start));

  let mut last_index = 0;
  let mut final_text = String::new();

  for change in changes {
    if change.range.start > change.range.end {
      panic!(
        "Text change had start index {} greater than end index {}.",
        change.range.start, change.range.end
      )
    }
    if change.range.start < last_index {
      panic!("Text changes were overlapping. Past index was {}, but new change had index {}.", last_index, change.range.start);
    } else if change.range.start > last_index && last_index < source.len() {
      final_text.push_str(
        &source[last_index..std::cmp::min(source.len(), change.range.start)],
      );
    }
    final_text.push_str(&change.new_text);
    last_index = change.range.end;
  }

  if last_index < source.len() {
    final_text.push_str(&source[last_index..]);
  }

  final_text
}
