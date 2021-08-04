// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::Location;

use swc_common::BytePos;

pub struct SourceFileInfo {
  pub specifier: String,
  pub text: String,
  line_start_byte_positions: Vec<BytePos>,
}

impl SourceFileInfo {
  pub fn new(specifier: &str, text: &str) -> SourceFileInfo {
    SourceFileInfo {
      line_start_byte_positions: get_line_start_positions(text),
      specifier: specifier.to_string(),
      text: text.to_string(),
    }
  }

  pub fn get_location(&self, pos: BytePos) -> Location {
    let line_index = self.get_line_index_at_pos(pos);
    let col = self.get_column_on_line_index_at_pos(line_index, pos);

    Location {
      specifier: self.specifier.clone(),
      // todo(dsherret): this is temporarily 1-indexed in order to have
      // the same behaviour as swc, but we should change this to be 0-indexed
      // in order to be the same as the LSP.
      line: line_index + 1,
      col,
    }
  }

  fn get_line_index_at_pos(&self, pos: BytePos) -> usize {
    match self.line_start_byte_positions.binary_search(&pos) {
      Ok(index) => index,
      Err(insert_index) => insert_index - 1,
    }
  }

  fn get_column_on_line_index_at_pos(
    &self,
    line_index: usize,
    pos: BytePos,
  ) -> usize {
    assert!(line_index < self.line_start_byte_positions.len());
    let pos = pos.0 as usize;
    let line_start_pos = self.line_start_byte_positions[line_index].0 as usize;
    let line_end_pos = self
      .line_start_byte_positions
      .get(line_index + 1)
      // may include line feed chars at the end, but in that case the pos should be less
      .map(|p| p.0 as usize)
      .unwrap_or_else(|| self.text.len());
    let line_text = &self.text[line_start_pos..line_end_pos];

    if pos < line_start_pos {
      panic!(
        "byte position {} was less than the start line position of {}",
        pos, line_start_pos
      );
    } else if pos > line_end_pos {
      panic!(
        "byte position {} exceeded the end line position of {}",
        pos, line_end_pos
      );
    } else if pos == line_end_pos {
      line_text.chars().count()
    } else {
      line_text
        .char_indices()
        .position(|(c_pos, _)| line_start_pos + c_pos >= pos)
        .unwrap()
    }
  }
}

fn get_line_start_positions(text: &str) -> Vec<BytePos> {
  let mut result = vec![BytePos(0)];
  for (pos, c) in text.char_indices() {
    if c == '\n' {
      let line_start_pos = BytePos((pos + 1) as u32);
      result.push(line_start_pos);
    }
  }
  result
}

#[cfg(test)]
mod test {
  use super::SourceFileInfo;
  use crate::ast::Location;

  use swc_common::BytePos;

  #[test]
  fn should_provide_locations() {
    let text = "12\n3\r\n4\n5";
    let specifier = "file:///file.ts";
    let info = SourceFileInfo::new(specifier, text);
    assert_pos_line_and_col(&info, 0, 1, 0); // 1
    assert_pos_line_and_col(&info, 1, 1, 1); // 2
    assert_pos_line_and_col(&info, 2, 1, 2); // \n
    assert_pos_line_and_col(&info, 3, 2, 0); // 3
    assert_pos_line_and_col(&info, 4, 2, 1); // \r
    assert_pos_line_and_col(&info, 5, 2, 2); // \n
    assert_pos_line_and_col(&info, 6, 3, 0); // 4
    assert_pos_line_and_col(&info, 7, 3, 1); // \n
    assert_pos_line_and_col(&info, 8, 4, 0); // 5
    assert_pos_line_and_col(&info, 9, 4, 1); // <EOF>
  }

  fn assert_pos_line_and_col(
    info: &SourceFileInfo,
    pos: u32,
    line: usize,
    col: usize,
  ) {
    assert_eq!(
      info.get_location(BytePos(pos)),
      Location {
        specifier: info.specifier.clone(),
        line,
        col,
      }
    );
  }
}
