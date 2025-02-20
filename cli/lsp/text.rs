// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::error::AnyError;
use dissimilar::diff;
use dissimilar::Chunk;
use text_size::TextRange;
use text_size::TextSize;
use tower_lsp::jsonrpc;
use tower_lsp::lsp_types as lsp;
use tower_lsp::lsp_types::TextEdit;

use crate::util::text_encoding::Utf16Map;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct LineIndex {
  inner: Utf16Map,
}

impl LineIndex {
  pub fn new(text: &str) -> LineIndex {
    LineIndex {
      inner: Utf16Map::new(text),
    }
  }

  /// Convert a u16 based range to a u8 TextRange.
  pub fn get_text_range(
    &self,
    range: lsp::Range,
  ) -> Result<TextRange, AnyError> {
    let start = self.offset(range.start)?;
    let end = self.offset(range.end)?;
    Ok(TextRange::new(start, end))
  }

  /// Return a u8 offset based on a u16 position.
  pub fn offset(&self, position: lsp::Position) -> Result<TextSize, AnyError> {
    self.inner.offset(position.line, position.character)
  }

  /// Convert an lsp Position into a tsc/TypeScript "position", which is really
  /// an u16 byte offset from the start of the string represented as an u32.
  pub fn offset_tsc(&self, position: lsp::Position) -> jsonrpc::Result<u32> {
    self
      .inner
      .offset_utf16(position.line, position.character)
      .map(|ts| ts.into())
      .map_err(|err| jsonrpc::Error::invalid_params(err.to_string()))
  }

  /// Returns a u16 position based on a u16 offset, which TypeScript offsets are
  /// returned as u16.
  pub fn position_utf16(&self, offset: TextSize) -> lsp::Position {
    let lc = self.inner.position_utf16(offset);
    lsp::Position {
      line: lc.line_index as u32,
      character: lc.column_index as u32,
    }
  }

  pub fn line_length_utf16(&self, line: u32) -> TextSize {
    self.inner.line_length_utf16(line)
  }

  pub fn text_content_length_utf16(&self) -> TextSize {
    self.inner.text_content_length_utf16()
  }
}

/// Compare two strings and return a vector of text edit records which are
/// supported by the Language Server Protocol.
pub fn get_edits(a: &str, b: &str, line_index: &LineIndex) -> Vec<TextEdit> {
  if a == b {
    return vec![];
  }
  // Heuristic to detect things like minified files. `diff()` is expensive.
  if b.chars().filter(|c| *c == '\n').count()
    > line_index.inner.utf8_offsets_len() * 3
  {
    return vec![TextEdit {
      range: lsp::Range {
        start: lsp::Position::new(0, 0),
        end: line_index.position_utf16(TextSize::from(a.len() as u32)),
      },
      new_text: b.to_string(),
    }];
  }
  let chunks = diff(a, b);
  let mut text_edits = Vec::<TextEdit>::new();
  let mut iter = chunks.iter().peekable();
  let mut a_pos = TextSize::from(0);
  loop {
    let chunk = iter.next();
    match chunk {
      None => break,
      Some(Chunk::Equal(e)) => {
        a_pos += TextSize::from(e.encode_utf16().count() as u32);
      }
      Some(Chunk::Delete(d)) => {
        let start = line_index.position_utf16(a_pos);
        a_pos += TextSize::from(d.encode_utf16().count() as u32);
        let end = line_index.position_utf16(a_pos);
        let range = lsp::Range { start, end };
        match iter.peek() {
          Some(Chunk::Insert(i)) => {
            iter.next();
            text_edits.push(TextEdit {
              range,
              new_text: i.to_string(),
            });
          }
          _ => text_edits.push(TextEdit {
            range,
            new_text: "".to_string(),
          }),
        }
      }
      Some(Chunk::Insert(i)) => {
        let pos = line_index.position_utf16(a_pos);
        let range = lsp::Range {
          start: pos,
          end: pos,
        };
        text_edits.push(TextEdit {
          range,
          new_text: i.to_string(),
        });
      }
    }
  }

  text_edits
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_edits() {
    let a = "abcdefg";
    let b = "a\nb\nchije\nfg\n";
    let actual = get_edits(a, b, &LineIndex::new(a));
    assert_eq!(
      actual,
      vec![
        TextEdit {
          range: lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 1
            },
            end: lsp::Position {
              line: 0,
              character: 5
            }
          },
          new_text: "\nb\nchije\n".to_string()
        },
        TextEdit {
          range: lsp::Range {
            start: lsp::Position {
              line: 0,
              character: 7
            },
            end: lsp::Position {
              line: 0,
              character: 7
            }
          },
          new_text: "\n".to_string()
        },
      ]
    );
  }

  #[test]
  fn test_get_edits_mbc() {
    let a = "const bar = \"👍🇺🇸😃\";\nconsole.log('hello deno')\n";
    let b = "const bar = \"👍🇺🇸😃\";\nconsole.log(\"hello deno\");\n";
    let actual = get_edits(a, b, &LineIndex::new(a));
    assert_eq!(
      actual,
      vec![
        TextEdit {
          range: lsp::Range {
            start: lsp::Position {
              line: 1,
              character: 12
            },
            end: lsp::Position {
              line: 1,
              character: 13
            }
          },
          new_text: "\"".to_string()
        },
        TextEdit {
          range: lsp::Range {
            start: lsp::Position {
              line: 1,
              character: 23
            },
            end: lsp::Position {
              line: 1,
              character: 25
            }
          },
          new_text: "\");".to_string()
        },
      ]
    )
  }
}
