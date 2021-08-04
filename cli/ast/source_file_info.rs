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
    let mut line_start_byte_positions = Vec::new();
    line_start_byte_positions.push(BytePos(0));
    for (pos, c) in text.char_indices() {
      if c == '\n' {
        let line_start_pos = BytePos((pos + 1) as u32);
        line_start_byte_positions.push(line_start_pos);
      }
    }

    SourceFileInfo {
      line_start_byte_positions,
      specifier: specifier.to_string(),
      text: text.to_string(),
    }
  }

  pub fn get_location(&self, pos: BytePos) -> Location {
    let mut best_line_match_index = 0;
    for (index, line_start_pos) in self.line_start_byte_positions.iter().enumerate() {
      if pos >= *line_start_pos {
        best_line_match_index = index;
      } else {
        break;
      }
    }

    // todo: fix this up
    let pos = pos.0 as usize;
    let line_start_pos = self.line_start_byte_positions[best_line_match_index].0 as usize;
    let line_end_pos = self.line_start_byte_positions.get(best_line_match_index + 1)
      .map(|p| p.0 as usize)
      .unwrap_or_else(|| self.text.len());
    let sub_text = &self.text[line_start_pos..line_end_pos];
    let col = if pos == line_end_pos {
      sub_text.chars().count()
    } else {
      sub_text.char_indices().position(|(c_pos, _)| line_start_pos + c_pos == pos).unwrap()
    };

    Location {
      filename: self.specifier.clone(),
      line: best_line_match_index + 1,
      col,
    }
  }
}