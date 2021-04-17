// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use dissimilar::diff;
use dissimilar::Chunk;
use lspower::jsonrpc;
use lspower::lsp;
use lspower::lsp::TextEdit;
use std::collections::HashMap;
use std::ops::Bound;
use std::ops::RangeBounds;
use text_size::TextRange;
use text_size::TextSize;

fn partition_point<T, P>(slice: &[T], mut predicate: P) -> usize
where
  P: FnMut(&T) -> bool,
{
  let mut left = 0;
  let mut right = slice.len();

  while left != right {
    let mid = left + (right - left) / 2;
    // SAFETY:
    // When left < right, left <= mid < right.
    // Therefore left always increases and right always decreases,
    // and either of them is selected.
    // In both cases left <= right is satisfied.
    // Therefore if left < right in a step,
    // left <= right is satisfied in the next step.
    // Therefore as long as left != right, 0 <= left < right <= len is satisfied
    // and if this case 0 <= mid < len is satisfied too.
    let value = unsafe { slice.get_unchecked(mid) };
    if predicate(value) {
      left = mid + 1;
    } else {
      right = mid;
    }
  }

  left
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Utf16Char {
  pub start: TextSize,
  pub end: TextSize,
}

impl Utf16Char {
  fn len(&self) -> TextSize {
    self.end - self.start
  }

  fn len_utf16(&self) -> usize {
    if self.len() == TextSize::from(4) {
      2
    } else {
      1
    }
  }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct LineIndex {
  utf8_offsets: Vec<TextSize>,
  utf16_lines: HashMap<u32, Vec<Utf16Char>>,
  utf16_offsets: Vec<TextSize>,
}

impl LineIndex {
  pub fn new(text: &str) -> LineIndex {
    let mut utf16_lines = HashMap::new();
    let mut utf16_chars = Vec::new();

    let mut utf8_offsets = vec![0.into()];
    let mut utf16_offsets = vec![0.into()];
    let mut curr_row = 0.into();
    let mut curr_col = 0.into();
    let mut curr_offset_u16 = 0.into();
    let mut line = 0;
    for c in text.chars() {
      let c_len = TextSize::of(c);
      curr_row += c_len;
      curr_offset_u16 += TextSize::from(c.len_utf16() as u32);
      if c == '\n' {
        utf8_offsets.push(curr_row);
        utf16_offsets.push(curr_offset_u16);

        if !utf16_chars.is_empty() {
          utf16_lines.insert(line, utf16_chars);
          utf16_chars = Vec::new();
        }

        curr_col = 0.into();
        line += 1;
        continue;
      }

      if !c.is_ascii() {
        utf16_chars.push(Utf16Char {
          start: curr_col,
          end: curr_col + c_len,
        });
      }
      curr_col += c_len;
    }

    utf8_offsets.push(curr_row);
    utf16_offsets.push(curr_offset_u16);

    if !utf16_chars.is_empty() {
      utf16_lines.insert(line, utf16_chars);
    }

    LineIndex {
      utf8_offsets,
      utf16_lines,
      utf16_offsets,
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
    let col = self.utf16_to_utf8_col(position.line, position.character);
    if let Some(line_offset) = self.utf8_offsets.get(position.line as usize) {
      Ok(line_offset + col)
    } else {
      Err(custom_error("OutOfRange", "The position is out of range."))
    }
  }

  /// Convert an lsp Position into a tsc/TypeScript "position", which is really
  /// an u16 byte offset from the start of the string represented as an u32.
  pub fn offset_tsc(&self, position: lsp::Position) -> jsonrpc::Result<u32> {
    self
      .offset_utf16(position)
      .map(|ts| ts.into())
      .map_err(|err| jsonrpc::Error::invalid_params(err.to_string()))
  }

  fn offset_utf16(
    &self,
    position: lsp::Position,
  ) -> Result<TextSize, AnyError> {
    if let Some(line_offset) = self.utf16_offsets.get(position.line as usize) {
      Ok(line_offset + TextSize::from(position.character))
    } else {
      Err(custom_error("OutOfRange", "The position is out of range."))
    }
  }

  /// Returns a u16 position based on a u16 offset, which TypeScript offsets are
  /// returned as u16.
  pub fn position_tsc(&self, offset: TextSize) -> lsp::Position {
    let line = partition_point(&self.utf16_offsets, |&it| it <= offset) - 1;
    let line_start_offset = self.utf16_offsets[line];
    let col = offset - line_start_offset;

    lsp::Position {
      line: line as u32,
      character: col.into(),
    }
  }

  /// Returns a u16 position based on a u8 offset.
  pub fn position_utf16(&self, offset: TextSize) -> lsp::Position {
    let line = partition_point(&self.utf16_offsets, |&it| it <= offset) - 1;
    let line_start_offset = self.utf16_offsets[line];
    let col = offset - line_start_offset;

    lsp::Position {
      line: line as u32,
      character: col.into(),
    }
  }

  pub fn line_length_utf16(&self, line: u32) -> Result<TextSize, AnyError> {
    let line_offset = self.utf16_offsets.get(line as usize);
    let next_line_offset = self.utf16_offsets.get((line + 1) as usize);
    if line_offset.is_none() || next_line_offset.is_none() {
      return Err(custom_error("OutOfRange", "The line is out of range."));
    }

    Ok(next_line_offset.unwrap() - line_offset.unwrap())
  }

  pub fn text_content_length_utf16(&self) -> TextSize {
    self.utf16_offsets.last().unwrap().clone()
  }

  fn utf16_to_utf8_col(&self, line: u32, mut col: u32) -> TextSize {
    if let Some(utf16_chars) = self.utf16_lines.get(&line) {
      for c in utf16_chars {
        if col > u32::from(c.start) {
          col += u32::from(c.len()) - c.len_utf16() as u32;
        } else {
          break;
        }
      }
    }

    col.into()
  }
}

/// Compare two strings and return a vector of text edit records which are
/// supported by the Language Server Protocol.
pub fn get_edits(
  a: &str,
  b: &str,
  maybe_line_index: Option<LineIndex>,
) -> Vec<TextEdit> {
  if a == b {
    return vec![];
  }
  let chunks = diff(a, b);
  let mut text_edits = Vec::<TextEdit>::new();
  let line_index = if let Some(line_index) = maybe_line_index {
    line_index
  } else {
    LineIndex::new(a)
  };
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

/// Convert a difference between two strings into a change range used by the
/// TypeScript Language Service.
pub fn get_range_change(a: &str, b: &str) -> Value {
  if a == b {
    return json!(null);
  }
  let chunks = diff(a, b);
  let mut iter = chunks.iter().peekable();
  let mut started = false;
  let mut start = 0;
  let mut end = 0;
  let mut new_length = 0;
  let mut equal = 0;
  let mut a_pos = 0;
  loop {
    let diff = iter.next();
    match diff {
      None => break,
      Some(Chunk::Equal(e)) => {
        a_pos += e.encode_utf16().count();
        equal += e.encode_utf16().count();
      }
      Some(Chunk::Delete(d)) => {
        if !started {
          start = a_pos;
          started = true;
          equal = 0;
        }
        a_pos += d.encode_utf16().count();
        if started {
          end = a_pos;
          new_length += equal;
          equal = 0;
        }
      }
      Some(Chunk::Insert(i)) => {
        if !started {
          start = a_pos;
          end = a_pos;
          started = true;
          equal = 0;
        } else {
          end += equal;
        }
        new_length += i.encode_utf16().count() + equal;
        equal = 0;
      }
    }
  }

  json!({
    "span": {
      "start": start,
      "length": end - start,
    },
    "newLength": new_length,
  })
}

/// Provide a slice of a string based on a character range.
pub fn slice(s: &str, range: impl RangeBounds<usize>) -> &str {
  let start = match range.start_bound() {
    Bound::Included(bound) | Bound::Excluded(bound) => *bound,
    Bound::Unbounded => 0,
  };
  let len = match range.end_bound() {
    Bound::Included(bound) => *bound + 1,
    Bound::Excluded(bound) => *bound,
    Bound::Unbounded => s.encode_utf16().count(),
  } - start;
  substring(s, start, start + len)
}

/// Provide a substring based on the start and end character index positions.
pub fn substring(s: &str, start: usize, end: usize) -> &str {
  let len = end - start;
  let mut char_pos = 0;
  let mut byte_start = 0;
  let mut it = s.chars();
  loop {
    if char_pos == start {
      break;
    }
    if let Some(c) = it.next() {
      char_pos += c.len_utf16();
      byte_start += c.len_utf8();
    } else {
      break;
    }
  }
  char_pos = 0;
  let mut byte_end = byte_start;
  loop {
    if char_pos == len {
      break;
    }
    if let Some(c) = it.next() {
      char_pos += c.len_utf16();
      byte_end += c.len_utf8();
    } else {
      break;
    }
  }
  &s[byte_start..byte_end]
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_line_index() {
    let text = "hello\nworld";
    let index = LineIndex::new(text);
    assert_eq!(
      index.position_utf16(0.into()),
      lsp::Position {
        line: 0,
        character: 0
      }
    );
    assert_eq!(
      index.position_utf16(1.into()),
      lsp::Position {
        line: 0,
        character: 1
      }
    );
    assert_eq!(
      index.position_utf16(5.into()),
      lsp::Position {
        line: 0,
        character: 5
      }
    );
    assert_eq!(
      index.position_utf16(6.into()),
      lsp::Position {
        line: 1,
        character: 0
      }
    );
    assert_eq!(
      index.position_utf16(7.into()),
      lsp::Position {
        line: 1,
        character: 1
      }
    );
    assert_eq!(
      index.position_utf16(8.into()),
      lsp::Position {
        line: 1,
        character: 2
      }
    );
    assert_eq!(
      index.position_utf16(10.into()),
      lsp::Position {
        line: 1,
        character: 4
      }
    );
    assert_eq!(
      index.position_utf16(11.into()),
      lsp::Position {
        line: 1,
        character: 5
      }
    );
    assert_eq!(
      index.position_utf16(12.into()),
      lsp::Position {
        line: 1,
        character: 6
      }
    );

    let text = "\nhello\nworld";
    let index = LineIndex::new(text);
    assert_eq!(
      index.position_utf16(0.into()),
      lsp::Position {
        line: 0,
        character: 0
      }
    );
    assert_eq!(
      index.position_utf16(1.into()),
      lsp::Position {
        line: 1,
        character: 0
      }
    );
    assert_eq!(
      index.position_utf16(2.into()),
      lsp::Position {
        line: 1,
        character: 1
      }
    );
    assert_eq!(
      index.position_utf16(6.into()),
      lsp::Position {
        line: 1,
        character: 5
      }
    );
    assert_eq!(
      index.position_utf16(7.into()),
      lsp::Position {
        line: 2,
        character: 0
      }
    );
  }

  #[test]
  fn test_char_len() {
    assert_eq!('メ'.len_utf8(), 3);
    assert_eq!('メ'.len_utf16(), 1);
    assert_eq!('编'.len_utf8(), 3);
    assert_eq!('编'.len_utf16(), 1);
    assert_eq!('🦕'.len_utf8(), 4);
    assert_eq!('🦕'.len_utf16(), 2);
  }

  #[test]
  fn test_empty_index() {
    let col_index = LineIndex::new(
      "
const C: char = 'x';
",
    );
    assert_eq!(col_index.utf16_lines.len(), 0);
  }

  #[test]
  fn test_single_char() {
    let col_index = LineIndex::new(
      "
const C: char = 'メ';
",
    );

    assert_eq!(col_index.utf16_lines.len(), 1);
    assert_eq!(col_index.utf16_lines[&1].len(), 1);
    assert_eq!(
      col_index.utf16_lines[&1][0],
      Utf16Char {
        start: 17.into(),
        end: 20.into()
      }
    );

    // UTF-16 to UTF-8, no changes
    assert_eq!(col_index.utf16_to_utf8_col(1, 15), TextSize::from(15));

    // UTF-16 to UTF-8
    assert_eq!(col_index.utf16_to_utf8_col(1, 19), TextSize::from(21));

    let col_index = LineIndex::new("a𐐏b");
    assert_eq!(col_index.utf16_to_utf8_col(0, 3), TextSize::from(5));
  }

  #[test]
  fn test_string() {
    let col_index = LineIndex::new(
      "
const C: char = \"メ メ\";
",
    );

    assert_eq!(col_index.utf16_lines.len(), 1);
    assert_eq!(col_index.utf16_lines[&1].len(), 2);
    assert_eq!(
      col_index.utf16_lines[&1][0],
      Utf16Char {
        start: 17.into(),
        end: 20.into()
      }
    );
    assert_eq!(
      col_index.utf16_lines[&1][1],
      Utf16Char {
        start: 21.into(),
        end: 24.into()
      }
    );

    // UTF-16 to UTF-8
    assert_eq!(col_index.utf16_to_utf8_col(1, 15), TextSize::from(15));

    // メ UTF-8: 0xE3 0x83 0xA1, UTF-16: 0x30E1
    assert_eq!(col_index.utf16_to_utf8_col(1, 17), TextSize::from(17)); // first メ at 17..20
    assert_eq!(col_index.utf16_to_utf8_col(1, 18), TextSize::from(20)); // space
    assert_eq!(col_index.utf16_to_utf8_col(1, 19), TextSize::from(21)); // second メ at 21..24

    assert_eq!(col_index.utf16_to_utf8_col(2, 15), TextSize::from(15));
  }

  #[test]
  fn test_get_edits() {
    let a = "abcdefg";
    let b = "a\nb\nchije\nfg\n";
    let actual = get_edits(a, b, None);
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
    let actual = get_edits(a, b, None);
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

  #[test]
  fn test_get_range_change() {
    let a = "abcdefg";
    let b = "abcdefg";
    let actual = get_range_change(a, b);
    assert_eq!(actual, json!(null));

    let a = "abcdefg";
    let b = "abedcfg";
    let actual = get_range_change(a, b);
    assert_eq!(
      actual,
      json!({
        "span": {
          "start": 2,
          "length": 3,
        },
        "newLength": 3
      })
    );

    let a = "abfg";
    let b = "abcdefg";
    let actual = get_range_change(a, b);
    assert_eq!(
      actual,
      json!({
        "span": {
          "start": 2,
          "length": 0,
        },
        "newLength": 3
      })
    );

    let a = "abcdefg";
    let b = "abfg";
    let actual = get_range_change(a, b);
    assert_eq!(
      actual,
      json!({
        "span": {
          "start": 2,
          "length": 3,
        },
        "newLength": 0
      })
    );

    let a = "abcdefg";
    let b = "abfghij";
    let actual = get_range_change(a, b);
    assert_eq!(
      actual,
      json!({
        "span": {
          "start": 2,
          "length": 5,
        },
        "newLength": 5
      })
    );

    let a = "abcdefghijk";
    let b = "axcxexfxixk";
    let actual = get_range_change(a, b);
    assert_eq!(
      actual,
      json!({
        "span": {
          "start": 1,
          "length": 9,
        },
        "newLength": 9
      })
    );

    let a = "abcde";
    let b = "ab(c)de";
    let actual = get_range_change(a, b);
    assert_eq!(
      actual,
      json!({
        "span" : {
          "start": 2,
          "length": 1,
        },
        "newLength": 3
      })
    );

    let a = "hello 🦕!";
    let b = "hello deno!";
    let actual = get_range_change(a, b);
    assert_eq!(
      actual,
      json!({
        "span": {
          "start": 6,
          "length": 2,
        },
        "newLength": 4
      })
    );

    let a = "hello deno!";
    let b = "hello deno🦕!";
    let actual = get_range_change(a, b);
    assert_eq!(
      actual,
      json!({
        "span": {
          "start": 10,
          "length": 0,
        },
        "newLength": 2
      })
    );

    // TODO(@kitsonk): https://github.com/dtolnay/dissimilar/issues/5
    // let a = r#" 🦕🇺🇸👍 "#;
    // let b = r#" 🇺🇸👍 "#;
    // let actual = get_range_change(a, b);
    // assert_eq!(
    //   actual,
    //   json!({
    //     "span": {
    //       "start": 1,
    //       "length": 2,
    //     },
    //     "newLength": 0
    //   })
    // );
  }

  #[test]
  fn test_substring() {
    assert_eq!(substring("Deno", 1, 3), "en");
    assert_eq!(substring("y̆y̆", 2, 4), "y̆");
    assert_eq!(substring("🦕🦕", 2, 4), "🦕");
  }

  #[test]
  fn test_slice() {
    assert_eq!(slice("Deno", 1..3), "en");
    assert_eq!(slice("Deno", 1..=3), "eno");
    assert_eq!(slice("Deno Land", 1..), "eno Land");
    assert_eq!(slice("Deno", ..3), "Den");
    assert_eq!(slice("Hello 🦕", 6..8), "🦕");
  }
}
