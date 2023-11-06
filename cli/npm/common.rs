// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::ResolvedPackageFolder;

use crate::util::path::specifier_to_file_path;

/// Gets the corresponding @types package for the provided package name.
pub fn types_package_name(package_name: &str) -> String {
  debug_assert!(!package_name.starts_with("@types/"));
  // Scoped packages will get two underscores for each slash
  // https://github.com/DefinitelyTyped/DefinitelyTyped/tree/15f1ece08f7b498f4b9a2147c2a46e94416ca777#what-about-scoped-packages
  format!("@types/{}", package_name.replace('/', "__"))
}

pub fn resolve_node_modules_pkg_folder_from_pkg(
  fs: &dyn FileSystem,
  specifier: &str,
  referrer: &ModuleSpecifier,
  mode: NodeResolutionMode,
  root_folder: Option<&Path>, // folder to stop searching at
) -> Result<ResolvedPackageFolder, AnyError> {
  fn inner(
    fs: &dyn FileSystem,
    specifier: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
    root_folder: Option<&Path>,
  ) -> Result<ResolvedPackageFolder, AnyError> {
    let specifier_parts = specifier.split('/').collect::<Vec<_>>();
    let referrer_file = specifier_to_file_path(referrer)?;
    let types_pkg_name = if mode.is_types() && !specifier.starts_with("@types/")
    {
      deno_runtime::deno_node::parse_npm_pkg_name(specifier, referrer)
        .ok()
        .map(|(package_name, sub_path, _)| {
          (types_package_name(&package_name), sub_path)
        })
    } else {
      None
    };
    let mut current_folder = referrer_file.parent().unwrap();
    loop {
      if let Some(root_folder) = root_folder {
        if current_folder == root_folder {
          break; // stop searching
        }
      }

      let node_modules_folder = if current_folder.ends_with("node_modules") {
        Cow::Borrowed(current_folder)
      } else {
        Cow::Owned(current_folder.join("node_modules"))
      };

      // attempt to resolve the types package first, then fallback to the regular package
      if let Some((types_pkg_name, sub_path)) = &types_pkg_name {
        let sub_dir = join_package_name(&node_modules_folder, types_pkg_name);
        if fs.is_dir_sync(&sub_dir) {
          return Ok(ResolvedPackageFolder::new(sub_dir, sub_path.to_string()));
        }
      }

      {
        let mut found_folder = None;
        let mut folder = node_modules_folder;
        for (i, part) in specifier_parts.iter().enumerate() {
          folder = Cow::Owned(folder.join(part));

          // todo(dsherret): this is kind of hacky in order to not resolve
          // a scope folder. We're fundamentally doing something different
          // from Node and should ideally align in the future
          if i == 0 && part.starts_with('@') && specifier_parts.len() > 1 {
            continue;
          }

          if fs.is_dir_sync(&folder) {
            found_folder = Some((i, folder.clone()));

            // prefer the directory that has a package.json
            let package_json = folder.join("package.json");
            if fs.is_file_sync(&package_json) {
              return Ok(ResolvedPackageFolder::new(
                folder.to_path_buf(),
                specifier_parts[i + 1..].join("/"),
              ));
            }
          }
        }

        if let Some((i, folder)) = found_folder {
          return Ok(ResolvedPackageFolder::new(
            folder.to_path_buf(),
            specifier_parts[i + 1..].join("/"),
          ));
        }
      }

      if let Some(parent) = current_folder.parent() {
        current_folder = parent;
      } else {
        break;
      }
    }

    bail!(
      "could not find specifier '{}' from referrer '{}'.",
      specifier,
      referrer
    );
  }

  let folder = inner(fs, specifier, referrer, mode, root_folder)?;
  Ok(ResolvedPackageFolder {
    folder_path: fs.realpath_sync(&folder.folder_path)?,
    sub_path: folder.sub_path,
  })
}

fn join_package_name(path: &Path, package_name: &str) -> PathBuf {
  let mut path = path.to_path_buf();
  // ensure backslashes are used on windows
  for part in package_name.split('/') {
    path = path.join(part);
  }
  path
}

#[cfg(test)]
mod test {
  use super::types_package_name;

  #[test]
  fn test_types_package_name() {
    assert_eq!(types_package_name("name"), "@types/name");
    assert_eq!(
      types_package_name("@scoped/package"),
      "@types/@scoped__package"
    );
  }
}
