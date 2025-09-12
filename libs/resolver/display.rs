// Copyright 2018-2025 the Deno authors. MIT license.

//! It would be best to move these utilities out of this
//! crate as this is not specific to resolution, but for
//! the time being it's fine for this to live here.
use deno_terminal::colors;
use imara_diff::BasicLineDiffPrinter;
use imara_diff::Diff;
use imara_diff::InternedInput;
use imara_diff::UnifiedDiffConfig;

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

  build(&orig_text, &edit_text)
}

pub fn build(orig_text: &str, edit_text: &str) -> String {
  let input = InternedInput::new(orig_text, edit_text);
  let mut diff = Diff::compute(imara_diff::Algorithm::Histogram, &input);
  diff.postprocess_lines(&input);

  diff
    .unified_diff(
      &BasicLineDiffPrinter(&input.interner),
      UnifiedDiffConfig::default(),
      &input,
    )
    .to_string()
}

// fn handle_diff(&mut self, diff: &Diff, input: &InternedInput<&str>) {
//   let mut old_line = 0u32;
//   let mut new_line = 0u32;

//   for hunk in diff.hunks() {
//     // Process unchanged lines before this hunk
//     if old_line < hunk.before.start || new_line < hunk.after.start {
//       let unchanged_start = std::cmp::max(old_line, new_line);
//       let unchanged_end = std::cmp::min(hunk.before.start, hunk.after.start);

//       for line_idx in unchanged_start..unchanged_end {
//         if line_idx > unchanged_start {
//           self.flush_changes();
//         }
//         if (line_idx as usize) < input.before.len() {
//           let line_text = &input.interner[input.before[line_idx as usize]];
//           self.orig.push_str(&fmt_rem_text(line_text));
//         }
//         if (line_idx as usize) < input.after.len() {
//           let line_text = &input.interner[input.after[line_idx as usize]];
//           self.edit.push_str(&fmt_add_text(line_text));
//         }
//       }
//     }

//     // Process deletions (lines only in before)
//     if hunk.before.start < hunk.before.end {
//       for line_idx in hunk.before.start..hunk.before.end {
//         if line_idx > hunk.before.start {
//           self.orig.push('\n');
//         }
//         let line_text = &input.interner[input.before[line_idx as usize]];
//         self.orig.push_str(&fmt_rem_text_highlight(line_text));
//       }
//       self.has_changes = true;
//     }

//     // Process insertions (lines only in after)
//     if hunk.after.start < hunk.after.end {
//       for line_idx in hunk.after.start..hunk.after.end {
//         if line_idx > hunk.after.start {
//           self.edit.push('\n');
//         }
//         let line_text = &input.interner[input.after[line_idx as usize]];
//         self.edit.push_str(&fmt_add_text_highlight(line_text));
//       }
//       self.has_changes = true;
//     }

//     old_line = hunk.before.end;
//     new_line = hunk.after.end;
//   }

//   // Process any remaining unchanged lines
//   let max_lines = std::cmp::max(input.before.len(), input.after.len()) as u32;
//   if old_line < max_lines || new_line < max_lines {
//     for line_idx in std::cmp::max(old_line, new_line)..max_lines {
//       if line_idx > std::cmp::max(old_line, new_line) {
//         self.flush_changes();
//       }
//       if (line_idx as usize) < input.before.len() {
//         let line_text = &input.interner[input.before[line_idx as usize]];
//         self.orig.push_str(&fmt_rem_text(line_text));
//       }
//       if (line_idx as usize) < input.after.len() {
//         let line_text = &input.interner[input.after[line_idx as usize]];
//         self.edit.push_str(&fmt_add_text(line_text));
//       }
//     }
//   }

//   self.flush_changes();
// }

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
