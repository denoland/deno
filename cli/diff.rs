// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use dissimilar::{diff, Chunk};
use std::path::PathBuf;

macro_rules! print_add {
  () => {
    print!("{}", colors::green_bold("+".to_string()));
  };
}

macro_rules! fmt_add_text {
  ($x:expr) => {
    format!("{}", colors::green($x));
  };
}

macro_rules! print_rem {
  () => {
    print!("{}", colors::red_bold("-".to_string()));
  };
}

macro_rules! fmt_rem_text {
  ($x:expr) => {
    format!("{}", colors::red($x));
  };
}

macro_rules! fmt_add_text_highlight {
  ($x:expr) => {
    format!("{}", colors::white_on_green($x));
  };
}

macro_rules! fmt_rem_text_highlight {
  ($x:expr) => {
    format!("{}", colors::white_on_red($x));
  };
}

fn print_line_diff(
  orig_line: &mut usize,
  edit_line: &mut usize,
  line_number_width: usize,
  orig: &mut String,
  edit: &mut String,
) {
  let print_line_number = |line: usize| {
    print!(
      "{:0width$}{} ",
      line,
      colors::gray("|".to_string()),
      width = line_number_width
    );
  };

  let split = orig.split('\n').enumerate();
  for (i, s) in split {
    print_line_number(*orig_line + i);
    print_rem!();
    print!("{}", s);
    println!();
  }

  let split = edit.split('\n').enumerate();
  for (i, s) in split {
    print_line_number(*edit_line + i);
    print_add!();
    print!("{}", s);
    println!();
  }

  *orig_line += orig.split('\n').count();
  *edit_line += edit.split('\n').count();

  orig.clear();
  edit.clear();
}

/// Print diff of the same file_path, before and after formatting.
///
/// Diff format is loosely based on Github diff formatting.
pub fn print_diff(file_path: &PathBuf, orig_text: &str, edit_text: &str) {
  let line_number_width =
    edit_text.split('\n').count().to_string().chars().count();

  println!();
  println!(
    "{} {}:",
    colors::bold("from".to_string()),
    file_path.display().to_string()
  );

  let mut orig_line: usize = 1;
  let mut edit_line: usize = 1;
  let mut orig: String = "".to_string();
  let mut edit: String = "".to_string();
  let mut changes = false;

  let chunks = diff(orig_text, edit_text);
  for chunk in chunks {
    match chunk {
      Chunk::Delete(s) => {
        let split = s.split('\n').enumerate();
        for (i, s) in split {
          if i > 0 {
            edit.push_str("\n");
          }
          edit.push_str(&fmt_rem_text_highlight!(s.to_string()));
        }
        changes = true
      }
      Chunk::Insert(s) => {
        let split = s.split('\n').enumerate();
        for (i, s) in split {
          if i > 0 {
            edit.push_str("\n");
          }
          edit.push_str(&fmt_add_text_highlight!(s.to_string()));
        }
        changes = true
      }
      Chunk::Equal(s) => {
        let split = s.split('\n').enumerate();
        for (i, s) in split {
          if i > 0 {
            if changes {
              print_line_diff(
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
          orig.push_str(&fmt_rem_text!(s.to_string()));
          edit.push_str(&fmt_add_text!(s.to_string()));
        }
      }
    }
  }
  println!();
}
