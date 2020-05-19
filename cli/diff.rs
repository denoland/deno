use crate::colors;
use difference::{Changeset, Difference};

use std::path::PathBuf;

macro_rules! print_add {
  () => {
    print!("{}", colors::green_bold("+".to_string()));
  };
}

macro_rules! print_add_text {
  ($x:expr) => {
    print!("{}", colors::green_bold($x));
  };
}

macro_rules! print_add_text_highlited {
  ($x:expr) => {
    print!("{}", colors::white_on_green($x));
  };
}

macro_rules! print_rem {
  () => {
    print!("{}", colors::red_bold("-".to_string()));
  };
}

macro_rules! print_rem_text {
  ($x:expr) => {
    print!("{}", colors::red($x));
  };
}

pub fn print_diff(file_path: &PathBuf, orig: &str, edit: &str) {
  let line_number_width = edit.split('\n').count().to_string().chars().count();
  let print_line_number = |line| {
    print!(
      "{:0width$}{} ",
      line,
      colors::gray("|".to_string()),
      width = line_number_width
    );
  };

  println!();
  println!(
    "{} {}:",
    colors::bold("from".to_string()),
    file_path.display().to_string()
  );
  let Changeset { diffs, .. } = Changeset::new(orig, edit, "\n");
  let mut line = 1;
  for i in 0..diffs.len() {
    match diffs[i] {
      Difference::Add(ref x) => {
        match diffs[i - 1] {
          Difference::Rem(ref y) => {
            print_line_number(line);
            print_add!();
            let Changeset { diffs, .. } = Changeset::new(y, x, "");
            let mut inline = line;
            for c in diffs {
              match c {
                Difference::Same(ref z) => {
                  let split = z.split('\n').enumerate();
                  for (i, s) in split {
                    if i > 0 {
                      inline += 1;
                      println!();
                      print_line_number(inline);
                      print_add!();
                    }
                    print_add_text!(s.to_string());
                  }
                }
                Difference::Add(ref z) => {
                  let split = z.split('\n').enumerate();
                  for (i, s) in split {
                    if i > 0 {
                      inline += 1;
                      println!();
                      print_line_number(inline);
                      print_add!();
                    }
                    print_add_text_highlited!(s.to_string());
                  }
                }
                _ => (),
              }
            }
            println!()
          }
          _ => {
            let split = x.split('\n').enumerate();
            for (i, s) in split {
              print_line_number(line + i);
              print_add!();
              print_add_text!(s.to_string());
              println!()
            }
          }
        };
        line += 1 + x.matches('\n').count();
      }
      Difference::Rem(ref x) => {
        let split = x.split('\n').enumerate();
        for (i, s) in split {
          print_line_number(line + i);
          print_rem!();
          print_rem_text!(s.to_string());
          println!()
        }
      }
      Difference::Same(ref x) => {
        line += 1 + x.matches('\n').count();
      }
    }
  }
  println!();
}
