// Copyright 2018-2026 the Deno authors. MIT license.

//! It would be best to move these utilities out of this
//! crate as this is not specific to resolution, but for
//! the time being it's fine for this to live here.
use std::fmt::Write as _;

use deno_terminal::colors;
use imara_diff::Diff;
use imara_diff::InternedInput;

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

struct DiffBuilder<'a> {
  input: InternedInput<&'a str>,
  output: String,
  line_number_width: usize,
  orig_line: usize,
  edit_line: usize,
}

impl<'a> DiffBuilder<'a> {
  pub fn build(orig_text: &'a str, edit_text: &'a str) -> String {
    let input = InternedInput::new(orig_text, edit_text);
    let mut diff = Diff::compute(imara_diff::Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    let diff_builder = DiffBuilder {
      input,
      output: String::new(),
      orig_line: 1,
      edit_line: 1,
      line_number_width: {
        let line_count = std::cmp::max(
          orig_text.split('\n').count(),
          edit_text.split('\n').count(),
        );
        line_count.to_string().chars().count()
      },
    };
    diff_builder.handle_diff(diff)
  }

  fn handle_diff(mut self, diff: Diff) -> String {
    let mut prev_before_end: u32 = 0;
    let mut is_first_hunk = true;

    for hunk in diff.hunks() {
      // Skip unchanged lines between hunks
      let gap_len = (hunk.before.start - prev_before_end) as usize;
      if gap_len > 0 && !is_first_hunk {
        writeln!(
          self.output,
          "{:width$}{} {}",
          "",
          colors::gray(" |"),
          colors::gray("..."),
          width = self.line_number_width
        )
        .unwrap();
      }
      self.orig_line += gap_len;
      self.edit_line += gap_len;
      is_first_hunk = false;

      // Interleave deleted/inserted line pairs, then emit remaining
      let del_count = hunk.before.len();
      let ins_count = hunk.after.len();
      let paired = std::cmp::min(del_count, ins_count);

      for i in 0..paired {
        let del_idx = hunk.before.start + i as u32;
        let s = self.input.interner[self.input.before[del_idx as usize]];
        self.write_rem_line(s);
        let ins_idx = hunk.after.start + i as u32;
        let s = self.input.interner[self.input.after[ins_idx as usize]];
        self.write_add_line(s);
      }
      // Remaining unpaired deletes
      for del_idx in (hunk.before.start + paired as u32)..hunk.before.end {
        let s = self.input.interner[self.input.before[del_idx as usize]];
        self.write_rem_line(s);
      }
      // Remaining unpaired inserts
      for ins_idx in (hunk.after.start + paired as u32)..hunk.after.end {
        let s = self.input.interner[self.input.after[ins_idx as usize]];
        self.write_add_line(s);
      }

      prev_before_end = hunk.before.end;
    }

    self.output
  }

  fn write_rem_line(&mut self, text: &str) {
    let (text, has_newline) = match text.strip_suffix('\n') {
      Some(t) => (t, true),
      None => (text, false),
    };
    write!(
      self.output,
      "{:width$}{} ",
      self.orig_line,
      colors::gray(" |"),
      width = self.line_number_width
    )
    .unwrap();
    self.output.push_str(&fmt_rem());
    self.output.push_str(&fmt_rem_text_highlight(text));
    self.output.push('\n');
    if !has_newline {
      self.write_no_newline_marker();
    }
    self.orig_line += 1;
  }

  fn write_add_line(&mut self, text: &str) {
    let (text, has_newline) = match text.strip_suffix('\n') {
      Some(t) => (t, true),
      None => (text, false),
    };
    write!(
      self.output,
      "{:width$}{} ",
      self.edit_line,
      colors::gray(" |"),
      width = self.line_number_width
    )
    .unwrap();
    self.output.push_str(&fmt_add());
    self.output.push_str(&fmt_add_text_highlight(text));
    self.output.push('\n');
    if !has_newline {
      self.write_no_newline_marker();
    }
    self.edit_line += 1;
  }

  fn write_no_newline_marker(&mut self) {
    writeln!(
      self.output,
      "{:width$}{} {}",
      "",
      colors::gray(" |"),
      colors::gray("\\ No newline at end of file"),
      width = self.line_number_width
    )
    .unwrap();
  }
}

fn fmt_add() -> String {
  colors::green_bold("+").to_string()
}

fn fmt_add_text_highlight(x: &str) -> String {
  colors::black_on_green(x).to_string()
}

fn fmt_rem() -> String {
  colors::red_bold("-").to_string()
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
        "  | \\ No newline at end of file\n",
        "1 | +console.log(\"Hello World\");\n",
        "  | \\ No newline at end of file\n",
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
        "  | ...\n",
        "6 | -'Hello World'\n",
        "2 | +\"Hello World\"\n",
        "7 | -)\n",
        "  | \\ No newline at end of file\n",
        "3 | +);\n",
        "  | \\ No newline at end of file\n",
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
        "  | \\ No newline at end of file\n",
        "2 | +some line text test\n",
      ),
    );
  }

  #[test]
  fn test_newlines_differing() {
    run_test("test\n", "test\r\n", " | Text differed by line endings.\n");
  }

  #[test]
  fn test_lockfile_diff() {
    // Simulates the frozen lockfile diff scenario where adding a new
    // dependency inserts lines while matching braces remain unchanged.
    let before = r#"{
  "version": "5",
  "packages": {
    "npm:@denotest/add@1": "1.0.0"
  },
  "npm": {
    "@denotest/add@1.0.0": {
      "integrity": "abc",
      "tarball": "http://localhost/add/1.0.0.tgz"
    }
  }
}"#;
    let after = r#"{
  "version": "5",
  "packages": {
    "npm:@denotest/add@1": "1.0.0",
    "npm:@denotest/subtract@1": "1.0.0"
  },
  "npm": {
    "@denotest/add@1.0.0": {
      "integrity": "abc",
      "tarball": "http://localhost/add/1.0.0.tgz"
    },
    "@denotest/subtract@1.0.0": {
      "integrity": "def",
      "tarball": "http://localhost/subtract/1.0.0.tgz"
    }
  }
}"#;
    run_test(
      before,
      after,
      concat!(
        " 4 | -    \"npm:@denotest/add@1\": \"1.0.0\"\n",
        " 4 | +    \"npm:@denotest/add@1\": \"1.0.0\",\n",
        " 5 | +    \"npm:@denotest/subtract@1\": \"1.0.0\"\n",
        "   | ...\n",
        "11 | +    },\n",
        "12 | +    \"@denotest/subtract@1.0.0\": {\n",
        "13 | +      \"integrity\": \"def\",\n",
        "14 | +      \"tarball\": \"http://localhost/subtract/1.0.0.tgz\"\n",
      ),
    );
  }

  fn run_test(diff_text1: &str, diff_text2: &str, expected_output: &str) {
    assert_eq!(
      test_util::strip_ansi_codes(&diff(diff_text1, diff_text2,)),
      expected_output,
    );
  }
}
