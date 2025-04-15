// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::ops::Range;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_core::error::AnyError;
use deno_core::ModuleSourceCode;
use deno_error::JsErrorBox;
use text_lines::LineAndColumnIndex;
use text_size::TextSize;

static SOURCE_MAP_PREFIX: &[u8] =
  b"//# sourceMappingURL=data:application/json;base64,";

pub fn source_map_from_code(code: &[u8]) -> Option<Vec<u8>> {
  let range = find_source_map_range(code)?;
  let source_map_range = &code[range];
  let input = source_map_range.split_at(SOURCE_MAP_PREFIX.len()).1;
  let decoded_map = BASE64_STANDARD.decode(input).ok()?;
  Some(decoded_map)
}

/// Truncate the source code before the source map.
pub fn code_without_source_map(code: ModuleSourceCode) -> ModuleSourceCode {
  use deno_core::ModuleCodeBytes;

  match code {
    ModuleSourceCode::String(mut code) => {
      if let Some(range) = find_source_map_range(code.as_bytes()) {
        code.truncate(range.start);
      }
      ModuleSourceCode::String(code)
    }
    ModuleSourceCode::Bytes(code) => {
      if let Some(range) = find_source_map_range(code.as_bytes()) {
        let source_map_index = range.start;
        ModuleSourceCode::Bytes(match code {
          ModuleCodeBytes::Static(bytes) => {
            ModuleCodeBytes::Static(&bytes[..source_map_index])
          }
          ModuleCodeBytes::Boxed(bytes) => {
            // todo(dsherret): should be possible without cloning
            ModuleCodeBytes::Boxed(
              bytes[..source_map_index].to_vec().into_boxed_slice(),
            )
          }
          ModuleCodeBytes::Arc(bytes) => ModuleCodeBytes::Boxed(
            bytes[..source_map_index].to_vec().into_boxed_slice(),
          ),
        })
      } else {
        ModuleSourceCode::Bytes(code)
      }
    }
  }
}

fn find_source_map_range(code: &[u8]) -> Option<Range<usize>> {
  fn last_non_blank_line_range(code: &[u8]) -> Option<Range<usize>> {
    let mut hit_non_whitespace = false;
    let mut range_end = code.len();
    for i in (0..code.len()).rev() {
      match code[i] {
        b' ' | b'\t' => {
          if !hit_non_whitespace {
            range_end = i;
          }
        }
        b'\n' | b'\r' => {
          if hit_non_whitespace {
            return Some(i + 1..range_end);
          }
          range_end = i;
        }
        _ => {
          hit_non_whitespace = true;
        }
      }
    }
    None
  }

  let range = last_non_blank_line_range(code)?;
  if code[range.start..range.end].starts_with(SOURCE_MAP_PREFIX) {
    Some(range)
  } else {
    None
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Utf16Char {
  pub start: TextSize,
  pub end: TextSize,
}

impl Utf16Char {
  pub fn len(&self) -> TextSize {
    self.end - self.start
  }

  pub fn len_utf16(&self) -> usize {
    if self.len() == TextSize::from(4) {
      2
    } else {
      1
    }
  }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Utf16Map {
  utf8_offsets: Vec<TextSize>,
  utf16_lines: HashMap<u32, Vec<Utf16Char>>,
  utf16_offsets: Vec<TextSize>,
}

impl Utf16Map {
  pub fn new(text: &str) -> Utf16Map {
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

    // utf8_offsets and utf16_offsets length is equal to (# of lines + 1)
    utf8_offsets.push(curr_row);
    utf16_offsets.push(curr_offset_u16);

    if !utf16_chars.is_empty() {
      utf16_lines.insert(line, utf16_chars);
    }

    Utf16Map {
      utf8_offsets,
      utf16_lines,
      utf16_offsets,
    }
  }

  pub fn text_content_length_utf16(&self) -> TextSize {
    *self.utf16_offsets.last().unwrap()
  }

  pub fn utf8_offsets_len(&self) -> usize {
    self.utf8_offsets.len()
  }

  pub fn line_length_utf16(&self, line: u32) -> TextSize {
    self.utf16_offsets[(line + 1) as usize] - self.utf16_offsets[line as usize]
  }

  pub fn utf16_to_utf8_col(&self, line: u32, mut col: u32) -> TextSize {
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

  /// Return a u8 offset based on a u16 position.
  pub fn offset(&self, line: u32, col: u32) -> Result<TextSize, AnyError> {
    let col = self.utf16_to_utf8_col(line, col);
    if let Some(line_offset) = self.utf8_offsets.get(line as usize) {
      Ok(line_offset + col)
    } else {
      Err(JsErrorBox::new("OutOfRange", "The position is out of range.").into())
    }
  }

  pub fn offset_utf16(
    &self,
    line: u32,
    col: u32,
  ) -> Result<TextSize, AnyError> {
    if let Some(line_offset) = self.utf16_offsets.get(line as usize) {
      Ok(line_offset + TextSize::from(col))
    } else {
      Err(JsErrorBox::new("OutOfRange", "The position is out of range.").into())
    }
  }

  /// Returns a u16 line and column based on a u16 offset, which
  /// TypeScript offsets are returned as u16.
  pub fn position_utf16(&self, offset: TextSize) -> LineAndColumnIndex {
    let line = partition_point(&self.utf16_offsets, |&it| it <= offset) - 1;
    let line_start_offset = self.utf16_offsets[line];
    let col = offset - line_start_offset;

    LineAndColumnIndex {
      line_index: line,
      column_index: col.into(),
    }
  }

  /// Convert a UTF-16 byte offset to UTF-8 byte offset
  pub fn utf16_to_utf8_offset(
    &self,
    utf16_offset: TextSize,
  ) -> Option<TextSize> {
    if utf16_offset > self.text_content_length_utf16() {
      return None;
    }
    let pos = self.position_utf16(utf16_offset);
    let line_start_utf8 = self.utf8_offsets[pos.line_index];
    let col_utf8 =
      self.utf16_to_utf8_col(pos.line_index as u32, pos.column_index as u32);
    Some(line_start_utf8 + col_utf8)
  }

  /// Convert a UTF-8 byte offset to UTF-16 byte offset
  pub fn utf8_to_utf16_offset(
    &self,
    utf8_offset: TextSize,
  ) -> Option<TextSize> {
    if utf8_offset > *self.utf8_offsets.last()? {
      return None;
    }
    let line = partition_point(&self.utf8_offsets, |&it| it <= utf8_offset) - 1;
    let line_start_utf8 = self.utf8_offsets[line];
    let col_utf8 = utf8_offset - line_start_utf8;
    let col_utf16 = self.utf8_to_utf16_col(line as u32, col_utf8);
    Some(self.utf16_offsets[line] + TextSize::from(col_utf16))
  }

  fn utf8_to_utf16_col(&self, line: u32, col: TextSize) -> u32 {
    let mut utf16_col = u32::from(col);

    if let Some(utf16_chars) = self.utf16_lines.get(&line) {
      for c in utf16_chars {
        if col > c.start {
          utf16_col -= u32::from(c.len()) - c.len_utf16() as u32;
        } else {
          break;
        }
      }
    }

    utf16_col
  }
}

fn partition_point<T, P>(slice: &[T], mut predicate: P) -> usize
where
  P: FnMut(&T) -> bool,
{
  let mut left = 0;
  let mut right = slice.len() - 1;

  while left != right {
    let mid = left + (right - left) / 2;
    // SAFETY:
    // When left < right, left <= mid < right.
    // Therefore left always increases and right always decreases,
    // and either of them is selected.
    // In both cases left <= right is satisfied.
    // Therefore if left < right in a step,
    // left <= right is satisfied in the next step.
    // Therefore as long as left != right, 0 <= left < right < len is satisfied
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

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use deno_core::ModuleCodeBytes;
  use deno_core::ModuleCodeString;

  use super::*;

  #[test]
  fn test_source_map_from_code() {
    let to_string =
      |bytes: Vec<u8>| -> String { String::from_utf8(bytes.to_vec()).unwrap() };
    assert_eq!(
      source_map_from_code(
        b"test\n//# sourceMappingURL=data:application/json;base64,dGVzdGluZ3Rlc3Rpbmc="
      ).map(to_string),
      Some("testingtesting".to_string())
    );
    assert_eq!(
      source_map_from_code(
        b"test\n//# sourceMappingURL=data:application/json;base64,dGVzdGluZ3Rlc3Rpbmc=\n  \n"
      ).map(to_string),
      Some("testingtesting".to_string())
    );
    assert_eq!(
      source_map_from_code(
        b"test\n//# sourceMappingURL=data:application/json;base64,dGVzdGluZ3Rlc3Rpbmc=\n  test\n"
      ).map(to_string),
      None
    );
    assert_eq!(
      source_map_from_code(
        b"\"use strict\";

throw new Error(\"Hello world!\");
//# sourceMappingURL=data:application/json;base64,{"
      ),
      None
    );
  }

  #[test]
  fn test_source_without_source_map() {
    run_test("", "");
    run_test("\n", "\n");
    run_test("\r\n", "\r\n");
    run_test("a", "a");
    run_test("a\n", "a\n");
    run_test("a\r\n", "a\r\n");
    run_test("a\r\nb", "a\r\nb");
    run_test("a\nb\n", "a\nb\n");
    run_test("a\r\nb\r\n", "a\r\nb\r\n");
    run_test(
      "test\n//# sourceMappingURL=data:application/json;base64,test",
      "test\n",
    );
    run_test(
      "test\r\n//# sourceMappingURL=data:application/json;base64,test",
      "test\r\n",
    );
    run_test(
      "\n//# sourceMappingURL=data:application/json;base64,test",
      "\n",
    );
    run_test(
      "test\n//# sourceMappingURL=data:application/json;base64,test\n\n",
      "test\n",
    );
    run_test(
      "test\n//# sourceMappingURL=data:application/json;base64,test\n   \n  ",
      "test\n",
    );

    fn run_test(input: &'static str, output: &'static str) {
      let forms = [
        ModuleSourceCode::String(ModuleCodeString::from_static(input)),
        ModuleSourceCode::String({
          let text: Arc<str> = input.into();
          text.into()
        }),
        ModuleSourceCode::String({
          let text: String = input.into();
          text.into()
        }),
        ModuleSourceCode::Bytes(ModuleCodeBytes::Static(input.as_bytes())),
        ModuleSourceCode::Bytes(ModuleCodeBytes::Boxed(
          input.as_bytes().to_vec().into_boxed_slice(),
        )),
        ModuleSourceCode::Bytes(ModuleCodeBytes::Arc(
          input.as_bytes().to_vec().into(),
        )),
      ];
      for form in forms {
        let result = code_without_source_map(form);
        let bytes = result.as_bytes();
        assert_eq!(bytes, output.as_bytes());
      }
    }
  }

  #[test]
  fn test_line_index() {
    let cases = [
      (0, (0, 0)),
      (1, (0, 1)),
      (5, (0, 5)),
      (6, (1, 0)),
      (7, (1, 1)),
      (8, (1, 2)),
      (10, (1, 4)),
      (11, (1, 5)),
      (12, (1, 6)),
    ];
    let text = "hello\nworld";
    let index = Utf16Map::new(text);
    for (input, expected) in cases {
      assert_eq!(
        index.position_utf16(input.into()),
        LineAndColumnIndex {
          line_index: expected.0,
          column_index: expected.1
        }
      );
    }

    let cases = [
      (0, (0, 0)),
      (1, (1, 0)),
      (2, (1, 1)),
      (6, (1, 5)),
      (7, (2, 0)),
    ];
    let text = "\nhello\nworld";
    let index = Utf16Map::new(text);
    for (input, expected) in cases {
      assert_eq!(
        index.position_utf16(input.into()),
        LineAndColumnIndex {
          line_index: expected.0,
          column_index: expected.1
        }
      );
    }
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
    let col_index = Utf16Map::new(
      "
const C: char = 'x';
",
    );
    assert_eq!(col_index.utf16_lines.len(), 0);
  }

  #[test]
  fn test_single_char() {
    let col_index = Utf16Map::new(
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

    let col_index = Utf16Map::new("a𐐏b");
    assert_eq!(col_index.utf16_to_utf8_col(0, 3), TextSize::from(5));
  }

  #[test]
  fn test_string() {
    let col_index = Utf16Map::new(
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
  fn test_offset_out_of_range() {
    let text = "hello";
    let map = Utf16Map::new(text);
    assert_eq!(map.utf8_to_utf16_offset(TextSize::from(10)), None);
    assert_eq!(map.utf16_to_utf8_offset(TextSize::from(10)), None);
  }

  #[test]
  fn test_offset_basic_ascii() {
    let text = "hello\nworld";
    let map = Utf16Map::new(text);

    let utf8_offset = TextSize::from(7);
    let utf16_offset = map.utf8_to_utf16_offset(utf8_offset).unwrap();
    assert_eq!(utf16_offset, TextSize::from(7));

    let result = map.utf16_to_utf8_offset(utf16_offset).unwrap();
    assert_eq!(result, utf8_offset);
  }

  #[test]
  fn test_offset_emoji() {
    let text = "hi 👋\nbye";
    let map = Utf16Map::new(text);

    let utf8_offset = TextSize::from(3);
    let utf16_offset = map.utf8_to_utf16_offset(utf8_offset).unwrap();
    assert_eq!(utf16_offset, TextSize::from(3));

    let utf8_offset_after = TextSize::from(7);
    let utf16_offset_after =
      map.utf8_to_utf16_offset(utf8_offset_after).unwrap();
    assert_eq!(utf16_offset_after, TextSize::from(5));

    for (utf8_offset, _) in text.char_indices() {
      let utf8_offset = TextSize::from(utf8_offset as u32);
      let utf16_offset = map.utf8_to_utf16_offset(utf8_offset).unwrap();
      let reverse_ut8_offset = map.utf16_to_utf8_offset(utf16_offset).unwrap();
      assert_eq!(reverse_ut8_offset, utf8_offset);
    }
  }
}
