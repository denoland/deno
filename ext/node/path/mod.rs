// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Component;
use std::path::PathBuf;

pub mod constants;
pub mod posix;
pub mod win32;

/// Extension to path_clean::PathClean
pub trait PathClean<T> {
  fn clean(&self) -> T;
}

impl PathClean<PathBuf> for PathBuf {
  fn clean(&self) -> PathBuf {
    let path = path_clean::PathClean::clean(self);
    if cfg!(windows) && path.to_string_lossy().contains("..\\") {
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

pub fn is_windows_device_root(code: u32) -> bool {
  (constants::CHAR_LOWERCASE_A..=constants::CHAR_LOWERCASE_Z).contains(&code)
    || (constants::CHAR_UPPERCASE_A..=constants::CHAR_UPPERCASE_Z)
      .contains(&code)
}

pub fn is_posix_path_separator(code: u32) -> bool {
  code == constants::CHAR_FORWARD_SLASH
}

pub fn is_path_separator(code: u32) -> bool {
  is_posix_path_separator(code) || code == constants::CHAR_BACKWARD_SLASH
}

pub fn char_at_index(path: &str, index: usize) -> Option<char> {
  path.chars().nth(index)
}
