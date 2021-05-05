// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use dissimilar::{diff as difference, Chunk};

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

fn write_line_diff(
  diff: &mut String,
  orig_line: &mut usize,
  edit_line: &mut usize,
  line_number_width: usize,
  orig: &mut String,
  edit: &mut String,
) {
  let split = orig.split('\n').enumerate();
  for (i, s) in split {
    diff.push_str(&format!(
      "{:width$}{} ",
      *orig_line + i,
      colors::gray(" |"),
      width = line_number_width
    ));
    diff.push_str(&fmt_rem());
    diff.push_str(s);
    diff.push('\n');
  }

  let split = edit.split('\n').enumerate();
  for (i, s) in split {
    diff.push_str(&format!(
      "{:width$}{} ",
      *edit_line + i,
      colors::gray(" |"),
      width = line_number_width
    ));
    diff.push_str(&fmt_add());
    diff.push_str(s);
    diff.push('\n');
  }

  *orig_line += orig.split('\n').count();
  *edit_line += edit.split('\n').count();

  orig.clear();
  edit.clear();
}

/// Print diff of the same file_path, before and after formatting.
///
/// Diff format is loosely based on Github diff formatting.
pub fn diff(orig_text: &str, edit_text: &str) -> String {
  let lines = edit_text.split('\n').count();
  let line_number_width = lines.to_string().chars().count();

  let mut diff = String::new();

  let mut text1 = orig_text.to_string();
  let mut text2 = edit_text.to_string();

  if !text1.ends_with('\n') {
    text1.push('\n');
  }
  if !text2.ends_with('\n') {
    text2.push('\n');
  }

  let mut orig_line: usize = 1;
  let mut edit_line: usize = 1;
  let mut orig: String = String::new();
  let mut edit: String = String::new();
  let mut changes = false;

  let chunks = difference(&text1, &text2);
  for chunk in chunks {
    match chunk {
      Chunk::Delete(s) => {
        let split = s.split('\n').enumerate();
        for (i, s) in split {
          if i > 0 {
            orig.push('\n');
          }
          orig.push_str(&fmt_rem_text_highlight(s));
        }
        changes = true
      }
      Chunk::Insert(s) => {
        let split = s.split('\n').enumerate();
        for (i, s) in split {
          if i > 0 {
            edit.push('\n');
          }
          edit.push_str(&fmt_add_text_highlight(s));
        }
        changes = true
      }
      Chunk::Equal(s) => {
        let split = s.split('\n').enumerate();
        for (i, s) in split {
          if i > 0 {
            if changes {
              write_line_diff(
                &mut diff,
                &mut orig_line,
                &mut edit_line,
                line_number_width,
                &mut orig,
                &mut edit,
              );
              changes = false
            } else {
              orig.clear();
              edit.clear();
              orig_line += 1;
              edit_line += 1;
            }
          }
          orig.push_str(&fmt_rem_text(s));
          edit.push_str(&fmt_add_text(s));
        }
      }
    }
  }
  diff
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_diff() {
    let simple_console_log_unfmt = "console.log('Hello World')";
    let simple_console_log_fmt = "console.log(\"Hello World\");";
    assert_eq!(
      colors::strip_ansi_codes(&diff(
        simple_console_log_unfmt,
        simple_console_log_fmt
      )),
      "1 | -console.log('Hello World')\n1 | +console.log(\"Hello World\");\n"
    );

    let line_number_unfmt = "\n\n\n\nconsole.log(\n'Hello World'\n)";
    let line_number_fmt = "console.log(\n\"Hello World\"\n);";
    assert_eq!(
      colors::strip_ansi_codes(&diff(line_number_unfmt, line_number_fmt)),
      "1 | -\n2 | -\n3 | -\n4 | -\n5 | -console.log(\n1 | +console.log(\n6 | -'Hello World'\n2 | +\"Hello World\"\n7 | -)\n3 | +);\n"
    );
  }
}
