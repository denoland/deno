// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Component;
use std::path::PathBuf;

/// Extenion to path_clean::PathClean
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
            let poped_component = components.pop();
            if !matches!(poped_component, Some(Component::Normal(_))) {
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
