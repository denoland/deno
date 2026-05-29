// Copyright 2018-2026 the Deno authors. MIT license.

//! Discovery of npm packages that ship native (.node) addons.
//!
//! Used by `deno compile --bundle` to decide which packages must stay
//! external during esbuild bundling (so `require('@napi-rs/whatever')`
//! survives) and which package directories must still be embedded in the
//! VFS so the addon can be located at runtime.

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_npm::NpmPackageId;
use deno_npm::NpmSystemInfo;

use crate::npm::CliNpmResolver;

#[derive(Debug, Clone)]
#[allow(
  dead_code,
  reason = "fields are kept for future precise-embedding work; v1 only uses the existence of any entry"
)]
pub struct NativeAddonPackage {
  /// Package name from the npm registry, e.g. `@napi-rs/clipboard`.
  pub name: String,
  /// Resolved on-disk folder of the installed package.
  pub folder: PathBuf,
  /// Snapshot id used to walk dependency edges.
  pub id: NpmPackageId,
}

/// Returns the set of installed npm packages whose folder contains at least
/// one `.node` native addon. Pure-JS packages are filtered out.
///
/// For managed npm, walks the resolved snapshot; for BYONM, walks the
/// workspace `node_modules` trees. In both cases each top-level package
/// directory is scanned recursively but `node_modules` subdirectories are
/// skipped so transitive deps are reported separately rather than rolled
/// into their parent.
pub fn find_native_addon_packages(
  npm_resolver: &CliNpmResolver,
  npm_system_info: &NpmSystemInfo,
) -> Result<Vec<NativeAddonPackage>, AnyError> {
  match npm_resolver {
    CliNpmResolver::Managed(managed) => {
      let snapshot = managed
        .resolution()
        .snapshot()
        .as_valid_serialized_for_system(npm_system_info);
      let mut packages = Vec::new();
      for pkg in snapshot.as_serialized().packages.iter() {
        let folder = match managed.resolve_pkg_folder_from_pkg_id(&pkg.id) {
          Ok(folder) => folder,
          // Skip packages that aren't actually present on disk (e.g.
          // platform-specific optional deps for other systems).
          Err(
            deno_resolver::npm::managed::ResolvePkgFolderFromPkgIdError::NotFound(_),
          ) => continue,
          Err(err) => return Err(err.into()),
        };
        if folder.exists() && folder_contains_node_addon(&folder) {
          packages.push(NativeAddonPackage {
            name: pkg.id.nv.name.to_string(),
            folder,
            id: pkg.id.clone(),
          });
        }
      }
      Ok(packages)
    }
    CliNpmResolver::Byonm(_) => {
      // TODO(bartlomieju): handle BYONM in a follow-up. Walking arbitrary
      // workspace `node_modules` trees needs to be aware of monorepo layout
      // (hoisted vs nested) before we can ship a sensible default.
      Ok(Vec::new())
    }
  }
}

fn folder_contains_node_addon(folder: &Path) -> bool {
  let mut pending = VecDeque::from([folder.to_path_buf()]);
  while let Some(dir) = pending.pop_front() {
    let Ok(entries) = std::fs::read_dir(&dir) else {
      continue;
    };
    for entry in entries.flatten() {
      let path = entry.path();
      let file_name = path.file_name();
      if path.is_dir() {
        // Don't descend into a nested `node_modules`; transitive deps are
        // reported separately so the caller can decide independently which
        // ones to embed.
        if file_name == Some(OsStr::new("node_modules")) {
          continue;
        }
        pending.push_back(path);
      } else if path.extension() == Some(OsStr::new("node")) {
        return true;
      }
    }
  }
  false
}
