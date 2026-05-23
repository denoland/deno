// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

pub fn relative_path(path: &Path, base: &Path) -> Option<PathBuf> {
  if path.is_absolute() != base.is_absolute() {
    return if path.is_absolute() {
      Some(path.to_path_buf())
    } else {
      None
    };
  }

  let mut path_components = path.components();
  let mut base_components = base.components();
  let mut components = Vec::new();
  loop {
    match (path_components.next(), base_components.next()) {
      (None, None) => break,
      (Some(component), None) => {
        components.push(component);
        components.extend(path_components);
        break;
      }
      (None, Some(_)) => components.push(Component::ParentDir),
      (Some(path_component), Some(base_component))
        if components.is_empty() && path_component == base_component => {}
      (Some(path_component), Some(Component::CurDir)) => {
        components.push(path_component)
      }
      (Some(_), Some(Component::ParentDir)) => return None,
      (Some(path_component), Some(_)) => {
        components.push(Component::ParentDir);
        components.extend(base_components.map(|_| Component::ParentDir));
        components.push(path_component);
        components.extend(path_components);
        break;
      }
    }
  }

  Some(components.iter().map(|c| c.as_os_str()).collect())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn relative_path_matches_installer_cases() {
    assert_eq!(
      relative_path(
        Path::new("/project/node_modules/.deno/pkg/node_modules/pkg/bin.js"),
        Path::new("/project/node_modules/.bin"),
      ),
      Some(PathBuf::from("../.deno/pkg/node_modules/pkg/bin.js"))
    );
    assert_eq!(
      relative_path(
        Path::new("/project/node_modules/.deno/dep/node_modules/dep"),
        Path::new("/project/node_modules/.deno/pkg/node_modules"),
      ),
      Some(PathBuf::from("../../dep/node_modules/dep"))
    );
    assert_eq!(
      relative_path(Path::new("pkg/bin.js"), Path::new("/project/.bin")),
      None
    );
    assert_eq!(
      relative_path(Path::new("/project/pkg/bin.js"), Path::new(".bin")),
      Some(PathBuf::from("/project/pkg/bin.js"))
    );
  }
}
