// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use dissimilar::diff;
use dissimilar::Chunk;
use lspower::lsp_types;
use lspower::lsp_types::TextEdit;
use std::ops::Bound;
use std::ops::Range;
use std::ops::RangeBounds;

// TODO(@kitson) in general all of these text handling routines don't handle
// JavaScript encoding in the same way and likely cause issues when trying to
// arbitrate between chars and Unicode graphemes.  There be dragons.

/// Generate a character position for the start of each line.  For example:
///
/// ```rust
/// let actual = index_lines("a\nb\n");
/// assert_eq!(actual, vec![0, 2, 4]);
/// ```
///
pub fn index_lines(text: &str) -> Vec<u32> {
  let mut indexes = vec![0_u32];
  for (i, c) in text.chars().enumerate() {
    if c == '\n' {
      indexes.push((i + 1) as u32);
    }
  }
  indexes
}

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

fn to_range(line_index: &[u32], range: lsp_types::Range) -> Range<usize> {
  let start =
    (line_index[range.start.line as usize] + range.start.character) as usize;
  let end =
    (line_index[range.end.line as usize] + range.end.character) as usize;
  Range { start, end }
}

pub fn to_position(line_index: &[u32], char_pos: u32) -> lsp_types::Position {
  let mut line = 0_usize;
  let mut line_start = 0_u32;
  for (pos, v) in line_index.iter().enumerate() {
    if char_pos < *v {
      break;
    }
    line_start = *v;
    line = pos;
  }

  lsp_types::Position {
    line: line as u32,
    character: char_pos - line_start,
  }
}

pub fn to_char_pos(line_index: &[u32], position: lsp_types::Position) -> u32 {
  if let Some(line_start) = line_index.get(position.line as usize) {
    line_start + position.character
  } else {
    0_u32
  }
}

/// Apply a vector of document changes to the supplied string.
pub fn apply_content_changes(
  content: &mut String,
  content_changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) {
  let mut line_index = index_lines(&content);
  let mut index_valid = IndexValid::All;
  for change in content_changes {
    if let Some(range) = change.range {
      if !index_valid.covers(range.start.line) {
        line_index = index_lines(&content);
      }
      let range = to_range(&line_index, range);
      content.replace_range(range, &change.text);
    } else {
      *content = change.text;
      index_valid = IndexValid::UpTo(0);
    }
  }
}

/// Compare two strings and return a vector of text edit records which are
/// supported by the Language Server Protocol.
pub fn get_edits(a: &str, b: &str) -> Vec<TextEdit> {
  let chunks = diff(a, b);
  let mut text_edits = Vec::<TextEdit>::new();
  let line_index = index_lines(a);
  let mut iter = chunks.iter().peekable();
  let mut a_pos = 0_u32;
  loop {
    let chunk = iter.next();
    match chunk {
      None => break,
      Some(Chunk::Equal(e)) => {
        a_pos += e.chars().count() as u32;
      }
      Some(Chunk::Delete(d)) => {
        let start = to_position(&line_index, a_pos);
        a_pos += d.chars().count() as u32;
        let end = to_position(&line_index, a_pos);
        let range = lsp_types::Range { start, end };
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
        let pos = to_position(&line_index, a_pos);
        let range = lsp_types::Range {
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
  let chunks = diff(a, b);
  let mut iter = chunks.iter().peekable();
  let mut started = false;
  let mut start = 0;
  let mut end = 0;
  let mut new_length = 0;
  let mut equal = 0;
  let mut a_pos = 0;
  loop {
    let chunk = iter.next();
    match chunk {
      None => break,
      Some(Chunk::Equal(e)) => {
        a_pos += e.chars().count();
        equal += e.chars().count();
      }
      Some(Chunk::Delete(d)) => {
        if !started {
          start = a_pos;
          started = true;
          equal = 0;
        }
        a_pos += d.chars().count();
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
        new_length += i.chars().count() + equal;
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
    Bound::Unbounded => s.len(),
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
      char_pos += 1;
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
      char_pos += 1;
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
  fn test_apply_content_changes() {
    let mut content = "a\nb\nc\nd".to_string();
    let content_changes = vec![lsp_types::TextDocumentContentChangeEvent {
      range: Some(lsp_types::Range {
        start: lsp_types::Position {
          line: 1,
          character: 0,
        },
        end: lsp_types::Position {
          line: 1,
          character: 1,
        },
      }),
      range_length: Some(1),
      text: "e".to_string(),
    }];
    apply_content_changes(&mut content, content_changes);
    assert_eq!(content, "a\ne\nc\nd");
  }

  #[test]
  fn test_get_edits() {
    let a = "abcdefg";
    let b = "a\nb\nchije\nfg\n";
    let actual = get_edits(a, b);
    assert_eq!(
      actual,
      vec![
        TextEdit {
          range: lsp_types::Range {
            start: lsp_types::Position {
              line: 0,
              character: 1
            },
            end: lsp_types::Position {
              line: 0,
              character: 5
            }
          },
          new_text: "\nb\nchije\n".to_string()
        },
        TextEdit {
          range: lsp_types::Range {
            start: lsp_types::Position {
              line: 0,
              character: 7
            },
            end: lsp_types::Position {
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
  fn test_get_range_change() {
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
  }

  #[test]
  fn test_index_lines() {
    let actual = index_lines("a\nb\r\nc");
    assert_eq!(actual, vec![0, 2, 5]);
  }

  #[test]
  fn test_to_position() {
    let line_index = index_lines("a\nb\r\nc\n");
    assert_eq!(
      to_position(&line_index, 6),
      lsp_types::Position {
        line: 2,
        character: 1,
      }
    );
    assert_eq!(
      to_position(&line_index, 0),
      lsp_types::Position {
        line: 0,
        character: 0,
      }
    );
    assert_eq!(
      to_position(&line_index, 3),
      lsp_types::Position {
        line: 1,
        character: 1,
      }
    );
  }

  #[test]
  fn test_to_position_mbc() {
    let line_index = index_lines("y̆\n😱🦕\n🤯\n");
    assert_eq!(
      to_position(&line_index, 0),
      lsp_types::Position {
        line: 0,
        character: 0,
      }
    );
    assert_eq!(
      to_position(&line_index, 2),
      lsp_types::Position {
        line: 0,
        character: 2,
      }
    );
    assert_eq!(
      to_position(&line_index, 3),
      lsp_types::Position {
        line: 1,
        character: 0,
      }
    );
    assert_eq!(
      to_position(&line_index, 4),
      lsp_types::Position {
        line: 1,
        character: 1,
      }
    );
    assert_eq!(
      to_position(&line_index, 5),
      lsp_types::Position {
        line: 1,
        character: 2,
      }
    );
    assert_eq!(
      to_position(&line_index, 6),
      lsp_types::Position {
        line: 2,
        character: 0,
      }
    );
    assert_eq!(
      to_position(&line_index, 7),
      lsp_types::Position {
        line: 2,
        character: 1,
      }
    );
    assert_eq!(
      to_position(&line_index, 8),
      lsp_types::Position {
        line: 3,
        character: 0,
      }
    );
  }

  #[test]
  fn test_substring() {
    assert_eq!(substring("Deno", 1, 3), "en");
    assert_eq!(substring("y̆y̆", 2, 4), "y̆");
    // this doesn't work like JavaScript, as 🦕 is treated as a single char in
    // Rust, but as two chars in JavaScript.
    // assert_eq!(substring("🦕🦕", 2, 4), "🦕");
  }

  #[test]
  fn test_slice() {
    assert_eq!(slice("Deno", 1..3), "en");
    assert_eq!(slice("Deno", 1..=3), "eno");
    assert_eq!(slice("Deno Land", 1..), "eno Land");
    assert_eq!(slice("Deno", ..3), "Den");
  }
}
