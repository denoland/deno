// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use dissimilar::diff as difference;
use dissimilar::Chunk;
use std::fmt::Write as _;

/// Print diff of the same file_path, before and after formatting.
///
/// Diff format is loosely based on GitHub diff formatting.
pub fn diff(orig_text: &str, edit_text: &str) -> String {
  if orig_text == edit_text {
    return String::new();
  }

  // normalize newlines as it adds too much noise if they differ
  let orig_text = orig_text.replace("\r\n", "\n");
  let edit_text = edit_text.replace("\r\n", "\n");

  if orig_text == edit_text {
    return " | Text differed by line endings.\n".to_string();
  }

  DiffBuilder::build(&orig_text, &edit_text)
}

struct DiffBuilder {
  output: String,
  line_number_width: usize,
  orig_line: usize,
  edit_line: usize,
  orig: String,
  edit: String,
  has_changes: bool,
}

impl DiffBuilder {
  pub fn build(orig_text: &str, edit_text: &str) -> String {
    let mut diff_builder = DiffBuilder {
      output: String::new(),
      orig_line: 1,
      edit_line: 1,
      orig: String::new(),
      edit: String::new(),
      has_changes: false,
      line_number_width: {
        let line_count = std::cmp::max(
          orig_text.split('\n').count(),
          edit_text.split('\n').count(),
        );
        line_count.to_string().chars().count()
      },
    };

    let chunks = difference(orig_text, edit_text);
    diff_builder.handle_chunks(chunks);
    diff_builder.output
  }

  fn handle_chunks<'a>(&'a mut self, chunks: Vec<Chunk<'a>>) {
    for chunk in chunks {
      match chunk {
        Chunk::Delete(s) => {
          let split = s.split('\n').enumerate();
          for (i, s) in split {
            if i > 0 {
              self.orig.push('\n');
            }
            self.orig.push_str(&fmt_rem_text_highlight(s));
          }
          self.has_changes = true
        }
        Chunk::Insert(s) => {
          let split = s.split('\n').enumerate();
          for (i, s) in split {
            if i > 0 {
              self.edit.push('\n');
            }
            self.edit.push_str(&fmt_add_text_highlight(s));
          }
          self.has_changes = true
        }
        Chunk::Equal(s) => {
          let split = s.split('\n').enumerate();
          for (i, s) in split {
            if i > 0 {
              self.flush_changes();
            }
            self.orig.push_str(&fmt_rem_text(s));
            self.edit.push_str(&fmt_add_text(s));
          }
        }
      }
    }

    self.flush_changes();
  }

  fn flush_changes(&mut self) {
    if self.has_changes {
      self.write_line_diff();

      self.orig_line += self.orig.split('\n').count();
      self.edit_line += self.edit.split('\n').count();
      self.has_changes = false;
    } else {
      self.orig_line += 1;
      self.edit_line += 1;
    }

    self.orig.clear();
    self.edit.clear();
  }

  fn write_line_diff(&mut self) {
    let split = self.orig.split('\n').enumerate();
    for (i, s) in split {
      write!(
        self.output,
        "{:width$}{} ",
        self.orig_line + i,
        colors::gray(" |"),
        width = self.line_number_width
      )
      .unwrap();
      self.output.push_str(&fmt_rem());
      self.output.push_str(s);
      self.output.push('\n');
    }

    let split = self.edit.split('\n').enumerate();
    for (i, s) in split {
      write!(
        self.output,
        "{:width$}{} ",
        self.edit_line + i,
        colors::gray(" |"),
        width = self.line_number_width
      )
      .unwrap();
      self.output.push_str(&fmt_add());
      self.output.push_str(s);
      self.output.push('\n');
    }
  }
}

fn fmt_add() -> String {
  colors::green_bold("+").to_string()
}

fn fmt_add_text(x: &str) -> String {
  colors::green(x).to_string()
}

fn fmt_add_text_highlight(x: &str) -> String {
  colors::black_on_green(x).to_string()
}

fn fmt_rem() -> String {
  colors::red_bold("-").to_string()
}

fn fmt_rem_text(x: &str) -> String {
  colors::red(x).to_string()
}

fn fmt_rem_text_highlight(x: &str) -> String {
  colors::white_on_red(x).to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_diff() {
    run_test(
      "console.log('Hello World')",
      "console.log(\"Hello World\");",
      concat!(
        "1 | -console.log('Hello World')\n",
        "1 | +console.log(\"Hello World\");\n",
      ),
    );

    run_test(
      "\n\n\n\nconsole.log(\n'Hello World'\n)",
      "console.log(\n\"Hello World\"\n);",
      concat!(
        "1 | -\n",
        "2 | -\n",
        "3 | -\n",
        "4 | -\n",
        "5 | -console.log(\n",
        "1 | +console.log(\n",
        "6 | -'Hello World'\n",
        "2 | +\"Hello World\"\n",
        "7 | -)\n3 | +);\n",
      ),
    );
  }

  #[test]
  fn test_eof_newline_missing() {
    run_test(
      "test\nsome line text test",
      "test\nsome line text test\n",
      concat!(
        "2 | -some line text test\n",
        "2 | +some line text test\n",
        "3 | +\n",
      ),
    );
  }

  #[test]
  fn test_newlines_differing() {
    run_test("test\n", "test\r\n", " | Text differed by line endings.\n");
  }

  fn run_test(diff_text1: &str, diff_text2: &str, expected_output: &str) {
    assert_eq!(
      test_util::strip_ansi_codes(&diff(diff_text1, diff_text2,)),
      expected_output,
    );
  }
}
