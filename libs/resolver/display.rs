// Copyright 2018-2025 the Deno authors. MIT license.

//! It would be best to move these utilities out of this
//! crate as this is not specific to resolution, but for
//! the time being it's fine for this to live here.
use std::fmt::Write as _;

use deno_terminal::colors;
use dissimilar::Chunk;
use dissimilar::diff as difference;

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

pub struct DisplayTreeNode {
  pub text: String,
  pub children: Vec<DisplayTreeNode>,
}

impl DisplayTreeNode {
  pub fn from_text(text: String) -> Self {
    Self {
      text,
      children: Default::default(),
    }
  }

  pub fn print<TWrite: std::fmt::Write>(
    &self,
    writer: &mut TWrite,
  ) -> std::fmt::Result {
    fn print_children<TWrite: std::fmt::Write>(
      writer: &mut TWrite,
      prefix: &str,
      children: &[DisplayTreeNode],
    ) -> std::fmt::Result {
      const SIBLING_CONNECTOR: char = '├';
      const LAST_SIBLING_CONNECTOR: char = '└';
      const CHILD_DEPS_CONNECTOR: char = '┬';
      const CHILD_NO_DEPS_CONNECTOR: char = '─';
      const VERTICAL_CONNECTOR: char = '│';
      const EMPTY_CONNECTOR: char = ' ';

      let child_len = children.len();
      for (index, child) in children.iter().enumerate() {
        let is_last = index + 1 == child_len;
        let sibling_connector = if is_last {
          LAST_SIBLING_CONNECTOR
        } else {
          SIBLING_CONNECTOR
        };
        let child_connector = if child.children.is_empty() {
          CHILD_NO_DEPS_CONNECTOR
        } else {
          CHILD_DEPS_CONNECTOR
        };
        writeln!(
          writer,
          "{} {}",
          colors::gray(format!(
            "{prefix}{sibling_connector}─{child_connector}"
          )),
          child.text
        )?;
        let child_prefix = format!(
          "{}{}{}",
          prefix,
          if is_last {
            EMPTY_CONNECTOR
          } else {
            VERTICAL_CONNECTOR
          },
          EMPTY_CONNECTOR
        );
        print_children(writer, &child_prefix, &child.children)?;
      }

      Ok(())
    }

    writeln!(writer, "{}", self.text)?;
    print_children(writer, "", &self.children)?;
    Ok(())
  }
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
