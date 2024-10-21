// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Extension to path_clean::PathClean
pub trait PathClean<T> {
  fn clean(&self) -> T;
}

impl PathClean<PathBuf> for PathBuf {
  fn clean(&self) -> PathBuf {
    fn is_clean_path(path: &Path) -> bool {
      let path = path.to_string_lossy();
      let mut current_index = 0;
      while let Some(index) = path[current_index..].find("\\.") {
        let trailing_index = index + current_index + 2;
        let mut trailing_chars = path[trailing_index..].chars();
        match trailing_chars.next() {
          Some('.') => match trailing_chars.next() {
            Some('/') | Some('\\') | None => {
              return false;
            }
            _ => {}
          },
          Some('/') | Some('\\') => {
            return false;
          }
          _ => {}
        }
        current_index = trailing_index;
      }
      true
    }

    let path = path_clean::PathClean::clean(self);
    if cfg!(windows) && !is_clean_path(&path) {
      // temporary workaround because path_clean::PathClean::clean is
      // not good enough on windows
      let mut components = Vec::new();

      for component in path.components() {
        match component {
          Component::CurDir => {
            // skip
          }
          Component::ParentDir => {
            let maybe_last_component = components.pop();
            if !matches!(maybe_last_component, Some(Component::Normal(_))) {
              panic!("Error normalizing: {}", path.display());
            }
          }
          Component::Normal(_) | Component::RootDir | Component::Prefix(_) => {
            components.push(component);
          }
        }
      }
      components.into_iter().collect::<PathBuf>()
    } else {
      path
    }
  }
}

#[cfg(test)]
mod test {
  #[cfg(windows)]
  #[test]
  fn test_path_clean() {
    use super::*;

    run_test("C:\\test\\./file.txt", "C:\\test\\file.txt");
    run_test("C:\\test\\../other/file.txt", "C:\\other\\file.txt");
    run_test("C:\\test\\../other\\file.txt", "C:\\other\\file.txt");

    fn run_test(input: &str, expected: &str) {
      assert_eq!(PathBuf::from(input).clean(), PathBuf::from(expected));
    }
  }
}
