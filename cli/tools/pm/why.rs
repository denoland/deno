// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_lockfile::NpmPackageInfo;

use crate::args::Flags;
use crate::args::WhyFlags;
use crate::colors;
use crate::factory::CliFactory;

const MAX_PATHS_PER_VERSION: usize = 15;

/// Strip the peer-dependency suffix from an npm lockfile key.
/// The peer suffix `_` only appears after the version, so we find the `@`
/// version separator first, then look for `_` only in the version portion.
/// e.g. "react-dom@18.2.0_react@18.2.0" → "react-dom@18.2.0"
///       "is_odd@3.0.1"                  → "is_odd@3.0.1"
fn strip_peer_suffix(key: &str) -> &str {
  // Find the @ that separates name from version (skip leading @ for scoped pkgs)
  let Some(at_pos) = key[1..].find('@').map(|p| p + 1) else {
    return key;
  };
  // Look for _ only after the version separator
  if let Some(underscore_pos) = key[at_pos + 1..].find('_') {
    &key[..at_pos + 1 + underscore_pos]
  } else {
    key
  }
}

pub async fn why(
  flags: Arc<Flags>,
  why_flags: WhyFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let lockfile = factory.maybe_lockfile().await?.cloned().ok_or_else(|| {
    deno_core::anyhow::anyhow!("No lockfile found. Run `deno install` first.")
  })?;

  let lockfile_guard = lockfile.lock();
  let content = &lockfile_guard.content;

  let query = &why_flags.package;

  // Parse optional version from query (e.g. "express@4.18.2")
  let (query_name, query_version) = if let Some(at_pos) = query[1..].find('@') {
    let at_pos = at_pos + 1;
    (&query[..at_pos], Some(query[at_pos + 1..].to_string()))
  } else {
    (query.as_str(), None)
  };

  // The npm packages in the lockfile are keyed like "express@4.22.1" or
  // "express@4.22.1_peer@1.0.0"
  let npm_packages = &content.packages.npm;

  // Find matching packages
  let matching: Vec<(&str, &NpmPackageInfo)> = npm_packages
    .iter()
    .filter(|(key, _)| {
      let key_str = key.as_str();
      let base = strip_peer_suffix(key_str);
      let Some(at_pos) = base[1..].find('@').map(|p| p + 1) else {
        return false;
      };
      let name = &base[..at_pos];
      let version = &base[at_pos + 1..];
      name == query_name
        && query_version
          .as_ref()
          .map(|v| version == v.as_str())
          .unwrap_or(true)
    })
    .map(|(k, v)| (k.as_str(), v))
    .collect();

  if matching.is_empty() {
    bail!("package '{}' not found in the dependency tree", query);
  }

  // Build reverse dependency map from lockfile
  // key: package_key (e.g. "accepts@1.3.8"), value: list of parent package_keys
  // Using BTreeMap for deterministic iteration order.
  let mut reverse_deps: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
  for (key, info) in npm_packages.iter() {
    // Add reverse edges for dependencies, optional_dependencies, and
    // optional_peers so that `deno why` reflects all reasons a package
    // may be installed.
    for dep_map in [
      &info.dependencies,
      &info.optional_dependencies,
      &info.optional_peers,
    ] {
      for dep_key in dep_map.values() {
        reverse_deps
          .entry(dep_key.as_str())
          .or_default()
          .push(key.as_str());
      }
    }
  }

  // Build specifier map: which top-level specifiers resolve to which packages
  let specifiers = &content.packages.specifiers;
  let mut root_specifier_to_pkg: HashMap<String, Vec<String>> = HashMap::new();
  for (req, resolved) in specifiers.iter() {
    let req_str = req.to_string();
    if !req_str.starts_with("npm:") {
      continue;
    }
    // resolved is like "4.22.1" or "4.22.1_peer@1.0.0"
    // We need to find the matching npm package key
    let npm_name = &req_str["npm:".len()..];
    // Parse the package name from the requirement
    let req_name = if let Some(at_pos) = npm_name[1..].find('@').map(|p| p + 1)
    {
      &npm_name[..at_pos]
    } else {
      npm_name
    };
    // Build the expected key: name@resolved_version
    let pkg_key = format!("{}@{}", req_name, resolved);
    root_specifier_to_pkg
      .entry(pkg_key)
      .or_default()
      .push(req_str);
  }

  for (key, _info) in &matching {
    let base = strip_peer_suffix(key);
    log::info!("{}", colors::bold(base));

    // Show direct specifier(s) if this is a root dependency
    if let Some(specifiers) = root_specifier_to_pkg.get(*key) {
      for spec in specifiers {
        log::info!("  {}", colors::green(spec));
      }
    }

    // Also show transitive paths (even for direct deps that are
    // reachable transitively through other packages)
    let mut paths = find_paths_to_root(
      key,
      &reverse_deps,
      &root_specifier_to_pkg,
      MAX_PATHS_PER_VERSION,
    );

    // Sort by path length so the most direct relationship is shown first
    paths.sort_by_key(|p| p.len());

    if paths.is_empty() && !root_specifier_to_pkg.contains_key(*key) {
      log::info!(
        "  (no dependency path found -- try running `deno install` to refresh the lockfile)"
      );
    }

    if paths.len() > MAX_PATHS_PER_VERSION {
      for path in &paths[..MAX_PATHS_PER_VERSION] {
        log::info!("{}", format_dependency_path(path, &root_specifier_to_pkg));
      }
      log::info!(
        "  ... and {} more paths",
        paths.len() - MAX_PATHS_PER_VERSION
      );
    } else {
      for path in &paths {
        log::info!("{}", format_dependency_path(path, &root_specifier_to_pkg));
      }
    }

    log::info!("");
  }

  Ok(())
}

/// Find dependency paths from root packages to the target.
///
/// `max_paths` caps how many paths are collected; once reached the
/// search short-circuits to avoid exponential expansion on wide
/// diamond dependency graphs (e.g. `ms` reachable via 30+ routes
/// in a Next.js app).
fn find_paths_to_root<'a>(
  target_key: &'a str,
  reverse_deps: &BTreeMap<&'a str, Vec<&'a str>>,
  root_specifiers: &HashMap<String, Vec<String>>,
  max_paths: usize,
) -> Vec<Vec<&'a str>> {
  let mut paths: Vec<Vec<&'a str>> = Vec::new();
  let mut current_path: Vec<&'a str> = vec![target_key];

  fn dfs<'a>(
    current_key: &'a str,
    target_key: &'a str,
    current_path: &mut Vec<&'a str>,
    paths: &mut Vec<Vec<&'a str>>,
    reverse_deps: &BTreeMap<&'a str, Vec<&'a str>>,
    root_specifiers: &HashMap<String, Vec<String>>,
    max_paths: usize,
  ) {
    if paths.len() >= max_paths {
      return;
    }

    // Check if we reached a root (but not the starting node itself,
    // since direct specifiers are printed separately by the caller)
    if current_key != target_key && root_specifiers.contains_key(current_key) {
      paths.push(current_path.iter().rev().copied().collect());
      return;
    }

    if let Some(parents) = reverse_deps.get(current_key) {
      for parent in parents {
        if paths.len() >= max_paths {
          return;
        }
        // Use path-based cycle check: only skip nodes already on the
        // current path. This prevents infinite loops while still
        // allowing a node to appear in multiple independent paths.
        if current_path.contains(parent) {
          continue;
        }
        current_path.push(parent);
        dfs(
          parent,
          target_key,
          current_path,
          paths,
          reverse_deps,
          root_specifiers,
          max_paths,
        );
        current_path.pop();
      }
    }
  }

  dfs(
    target_key,
    target_key,
    &mut current_path,
    &mut paths,
    reverse_deps,
    root_specifiers,
    max_paths,
  );

  paths
}

/// Format a dependency path as a string.
fn format_dependency_path(
  path: &[&str],
  root_specifiers: &HashMap<String, Vec<String>>,
) -> String {
  if path.is_empty() {
    return String::new();
  }

  let root = path[0];
  let base = strip_peer_suffix(root);

  let mut out = String::new();

  // Show the root specifier if available
  if let Some(specs) = root_specifiers.get(root) {
    out.push_str(&format!("  {}", colors::green(&specs[0])));
  } else {
    out.push_str(&format!("  {}", base));
  }

  // Append intermediate and final elements
  for key in &path[1..] {
    let base = strip_peer_suffix(key);
    out.push_str(&format!(" > {}", base));
  }

  out
}
