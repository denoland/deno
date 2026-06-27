// Copyright 2018-2026 the Deno authors. MIT license.

//! Discovery of npm packages that need to ship in the binary built by
//! `deno compile --bundle`. Two complementary signals:
//!
//! 1. Packages with `.node` native addons can't be tree-shaken into the
//!    bundle, so we mark them for inclusion eagerly. For managed npm we
//!    walk the resolved snapshot; for BYONM we scan the workspace
//!    `node_modules` trees that `fill_npm_vfs` embeds.
//! 2. Absolute paths the bundle path-rewriter pointed at — captured at
//!    bundle time — are mapped back to their owning npm package so the
//!    binary writer ships only what's actually reached at runtime.

use std::collections::HashMap;
use std::collections::HashSet;
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
  /// Snapshot id used to walk dependency edges. `None` under BYONM, where
  /// there is no resolved snapshot and the whole tree is embedded wholesale.
  pub id: Option<NpmPackageId>,
}

/// Returns the set of installed npm packages whose folder contains at least
/// one `.node` native addon. Pure-JS packages are filtered out.
///
/// For managed npm, walks the resolved snapshot; for BYONM, walks the
/// workspace `node_modules` trees (`workspace_root` is the workspace root
/// dir). In both cases each top-level package directory is scanned
/// recursively but `node_modules` subdirectories are skipped so transitive
/// deps are reported separately rather than rolled into their parent.
pub fn find_native_addon_packages(
  npm_resolver: &CliNpmResolver,
  npm_system_info: &NpmSystemInfo,
  workspace_root: Option<&Path>,
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
            id: Some(pkg.id.clone()),
          });
        }
      }
      Ok(packages)
    }
    CliNpmResolver::Byonm(_) => {
      // BYONM has no resolved snapshot to walk, and `fill_npm_vfs` embeds
      // the workspace `node_modules` trees wholesale rather than picking
      // individual packages. So the monorepo hoisted-vs-nested layout that
      // would matter for *precise* embedding is irrelevant here: the caller
      // only needs to know whether *any* addon exists. Scan the same trees
      // for a `.node` and report the folders that have one.
      let Some(workspace_root) = workspace_root else {
        return Ok(Vec::new());
      };
      Ok(find_byonm_native_addon_packages(workspace_root))
    }
  }
}

/// Compute the set of npm packages whose folders must ship in the binary
/// produced by `deno compile --bundle`.
///
/// Seeds the set with:
/// - every native-addon package (always needed; bundle can't inline a
///   `.node` file), and
/// - every package whose installed folder contains one of the
///   `referenced_paths` — i.e. paths the bundle path-rewriter resolved
///   at build time, which the compiled binary will require() at runtime.
///
/// Then walks each seed's dependency closure so transitive runtime
/// requires (e.g. a CJS package's own require()s) also have their
/// packages on disk.
///
/// Returns `None` for BYONM; the caller falls back to embedding all
/// workspace `node_modules` trees.
pub fn collect_bundle_required_packages(
  npm_resolver: &CliNpmResolver,
  npm_system_info: &NpmSystemInfo,
  referenced_paths: &[PathBuf],
) -> Result<Option<HashSet<NpmPackageId>>, AnyError> {
  let CliNpmResolver::Managed(managed) = npm_resolver else {
    return Ok(None);
  };

  let snapshot = managed.resolution().snapshot();
  // Managed resolver only: `workspace_root` is unused on this path (it only
  // matters for BYONM, which returns `None` above).
  let native_packages =
    find_native_addon_packages(npm_resolver, npm_system_info, None)?;

  let mut folders: Vec<(NpmPackageId, PathBuf)> = Vec::new();
  for pkg in snapshot.all_packages_for_every_system() {
    if let Ok(folder) = managed.resolve_pkg_folder_from_pkg_id(&pkg.id)
      && folder.exists()
    {
      folders.push((pkg.id.clone(), folder));
    }
  }

  // `id` is always `Some` here: `find_native_addon_packages` only returns
  // `None` ids for BYONM, which this function bails out of above.
  let mut seeds: HashSet<NpmPackageId> =
    native_packages.into_iter().filter_map(|p| p.id).collect();

  // One-level reverse walk: include any package that depends on a
  // native-addon package. NAPI-RS packages are typically split into a
  // JS-only wrapper (e.g. `@mariozechner/clipboard`) and a set of
  // platform-specific binary packages it loads via optional deps
  // (`@mariozechner/clipboard-darwin-arm64`). Only the binary one has
  // a `.node` file, so the forward closure misses the wrapper — which
  // is the package user code actually imports. Pull it in here.
  let native_seeds: HashSet<NpmPackageId> = seeds.iter().cloned().collect();
  for pkg in snapshot.all_packages_for_every_system() {
    if pkg
      .dependencies
      .values()
      .any(|dep_id| native_seeds.contains(dep_id))
    {
      seeds.insert(pkg.id.clone());
    }
  }

  // Map each referenced path to its owning package using longest-prefix
  // matching, so a path inside a nested package directory is attributed
  // to the nested one rather than its parent.
  let mut path_owner_cache: HashMap<&PathBuf, Option<NpmPackageId>> =
    HashMap::new();
  for path in referenced_paths {
    if path_owner_cache.contains_key(path) {
      continue;
    }
    let mut best_match: Option<(&NpmPackageId, usize)> = None;
    for (id, folder) in &folders {
      let folder_str = folder.as_os_str();
      if path.starts_with(folder) {
        let folder_len = folder_str.len();
        if best_match.map(|(_, len)| folder_len > len).unwrap_or(true) {
          best_match = Some((id, folder_len));
        }
      }
    }
    let owner = best_match.map(|(id, _)| id.clone());
    if let Some(ref id) = owner {
      seeds.insert(id.clone());
    }
    path_owner_cache.insert(path, owner);
  }

  // BFS through dependency edges from each seed so runtime requires that
  // happen inside an already-embedded package can resolve.
  let mut closure: HashSet<NpmPackageId> = HashSet::new();
  let mut stack: Vec<NpmPackageId> = seeds.into_iter().collect();
  while let Some(id) = stack.pop() {
    if !closure.insert(id.clone()) {
      continue;
    }
    if let Some(pkg) = snapshot.package_from_id(&id) {
      for dep_id in pkg.dependencies.values() {
        if !closure.contains(dep_id) {
          stack.push(dep_id.clone());
        }
      }
    }
  }

  Ok(Some(closure))
}

/// Returns the subset of `referenced_paths` that live inside an installed npm
/// package folder.
pub fn resolve_bundle_npm_referenced_paths(
  npm_resolver: &CliNpmResolver,
  referenced_paths: &[PathBuf],
) -> Result<HashSet<PathBuf>, AnyError> {
  match npm_resolver {
    CliNpmResolver::Managed(managed) => {
      let snapshot = managed.resolution().snapshot();
      let mut folders: Vec<PathBuf> = Vec::new();
      for pkg in snapshot.all_packages_for_every_system() {
        if let Ok(folder) = managed.resolve_pkg_folder_from_pkg_id(&pkg.id)
          && folder.exists()
        {
          folders.push(folder);
        }
      }
      Ok(referenced_paths
        .iter()
        .filter(|path| folders.iter().any(|folder| path.starts_with(folder)))
        .cloned()
        .collect())
    }
    CliNpmResolver::Byonm(_) => Ok(referenced_paths
      .iter()
      .filter(|path| path_is_in_node_modules(path))
      .cloned()
      .collect()),
  }
}

/// BYONM: walk every `node_modules` directory under `workspace_root` (the
/// same set `fill_npm_vfs` embeds for BYONM) and return each package folder
/// that ships a native addon. Nested `node_modules` are followed so a
/// transitive addon is still detected.
fn find_byonm_native_addon_packages(
  workspace_root: &Path,
) -> Vec<NativeAddonPackage> {
  // First collect every `node_modules` dir in the workspace, mirroring
  // `fill_npm_vfs`'s BYONM traversal (recurse into non-`node_modules` dirs,
  // stop at `node_modules` boundaries).
  let mut node_modules_dirs = VecDeque::new();
  let mut pending = VecDeque::from([workspace_root.to_path_buf()]);
  while let Some(dir) = pending.pop_front() {
    let Ok(entries) = std::fs::read_dir(&dir) else {
      continue;
    };
    for entry in entries.flatten() {
      let path = entry.path();
      if !path.is_dir() {
        continue;
      }
      if path.file_name() == Some(OsStr::new("node_modules")) {
        node_modules_dirs.push_back(path);
      } else {
        pending.push_back(path);
      }
    }
  }
  // Then scan each `node_modules` for package folders with a `.node`,
  // queueing nested `node_modules` (transitive deps) as we go.
  let mut packages = Vec::new();
  while let Some(node_modules) = node_modules_dirs.pop_front() {
    let Ok(entries) = std::fs::read_dir(&node_modules) else {
      continue;
    };
    for entry in entries.flatten() {
      let path = entry.path();
      if !path.is_dir() {
        continue;
      }
      let nested = path.join("node_modules");
      if nested.is_dir() {
        node_modules_dirs.push_back(nested);
      }
      if folder_contains_node_addon(&path) {
        packages.push(NativeAddonPackage {
          name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
          folder: path,
          id: None,
        });
      }
    }
  }
  packages
}

fn path_is_in_node_modules(path: &Path) -> bool {
  path.components().any(|c| c.as_os_str() == "node_modules")
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

#[cfg(test)]
mod tests {
  use super::*;

  fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "").unwrap();
  }

  #[test]
  fn byonm_detects_addon_in_workspace_node_modules() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let pkg = root.join("node_modules").join("pkg");
    touch(&pkg.join("index.js"));

    // Pure-JS package: nothing to embed for.
    assert!(find_byonm_native_addon_packages(root).is_empty());

    // Drop a `.node` in and it's detected.
    touch(&pkg.join("build").join("Release").join("addon.node"));
    let found = find_byonm_native_addon_packages(root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].folder, pkg);
    assert!(found[0].id.is_none());
  }

  #[test]
  fn byonm_detects_transitive_addon() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    // Addon lives only in a transitive dep, nested under a parent package.
    let nested = root
      .join("node_modules")
      .join("parent")
      .join("node_modules")
      .join("dep");
    touch(&nested.join("addon.node"));

    let found = find_byonm_native_addon_packages(root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].folder, nested);
  }
}
