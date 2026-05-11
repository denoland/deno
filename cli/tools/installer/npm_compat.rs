// Copyright 2018-2026 the Deno authors. MIT license.

//! Post-install setup for stock TypeScript compatibility.
//!
//! After `deno install` sets up node_modules/, this module:
//! 1. Installs jsr: packages to node_modules/@jsr/ via npm.jsr.io
//! 2. Mirrors http(s): modules into .deno/remote/<host><path>/...
//! 3. Generates .deno/tsconfig.json with paths mappings for npm:/jsr:/https:
//!
//! This enables stock TypeScript tooling (tsc, tsserver, VS Code) to work
//! with Deno projects that use jsr:, npm:, and http(s): specifiers.

use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_semver::Version;
use deno_semver::VersionReq;

/// Installed JSR package info for reporting.
pub struct InstalledJsrPackage {
  /// e.g. "@jsr/std__assert"
  pub name: String,
  /// e.g. "1.0.19"
  pub version: String,
}

/// Run post-install setup: install jsr packages and generate tsconfig.
///
/// Called after `deno install` completes npm resolution and node_modules setup.
/// Returns the list of newly installed JSR packages for reporting.
pub fn setup_npm_compat(
  project_root: &Path,
) -> Result<Vec<InstalledJsrPackage>, AnyError> {
  let deno_json = read_deno_json(project_root)?;
  let Some(deno_json) = deno_json else {
    return Ok(vec![]);
  };

  let deno_imports = deno_json.get("imports");
  let deno_compiler_options = deno_json.get("compilerOptions");

  // Check if there are any jsr:, npm:, or http(s): specifiers — if not, skip
  let has_special_specifiers = deno_imports
    .and_then(|v| v.as_object())
    .is_some_and(|imports| {
      imports.values().any(|v| {
        v.as_str().is_some_and(|s| {
          s.starts_with("jsr:")
            || s.starts_with("npm:")
            || s.starts_with("http://")
            || s.starts_with("https://")
        })
      })
    });

  if !has_special_specifiers {
    return Ok(vec![]);
  }

  // Install jsr: packages to node_modules/@jsr/
  let installed = install_jsr_packages(project_root, deno_imports)?;

  // Mirror http(s): modules (and their transitive remote/relative imports)
  // into .deno/remote/<host><path>/...
  let http_modules = install_http_modules(project_root, deno_imports)
    .unwrap_or_else(|e| {
      log::warn!("Failed to materialize remote modules: {e}");
      BTreeSet::new()
    });

  // Generate .deno/tsconfig.json and ensure root tsconfig.json extends it
  generate_deno_tsconfig(
    project_root,
    deno_compiler_options,
    deno_imports,
    &http_modules,
  )?;

  Ok(installed)
}

fn read_deno_json(project_root: &Path) -> Result<Option<Value>, AnyError> {
  let deno_json_path = project_root.join("deno.json");
  let deno_jsonc_path = project_root.join("deno.jsonc");

  if deno_json_path.exists() {
    let content = std::fs::read_to_string(&deno_json_path)?;
    Ok(Some(serde_json::from_str(&content)?))
  } else if deno_jsonc_path.exists() {
    let content = std::fs::read_to_string(&deno_jsonc_path)?;
    let parsed: Option<Value> = jsonc_parser::parse_to_serde_value(
      &content,
      &jsonc_parser::ParseOptions::default(),
    )?;
    Ok(Some(parsed.unwrap_or(json!({}))))
  } else {
    Ok(None)
  }
}

/// Generate tsconfig.deno.json at the project root with paths mappings.
fn generate_deno_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  http_modules: &BTreeSet<Url>,
) -> Result<(), AnyError> {
  let generated = crate::tsc::tsconfig_gen::generate_tsconfig(
    project_root,
    deno_compiler_options,
    deno_imports,
    &[],
    http_modules,
  )
  .map_err(|e| anyhow!("Failed to generate tsconfig: {e}"))?;

  log::debug!("Generated {}", generated.tsconfig_path.display());

  Ok(())
}

/// Install jsr: packages to node_modules/@jsr/ by downloading from npm.jsr.io.
fn install_jsr_packages(
  project_root: &Path,
  deno_imports: Option<&Value>,
) -> Result<Vec<InstalledJsrPackage>, AnyError> {
  let mut installed = Vec::new();
  let imports = match deno_imports.and_then(|v| v.as_object()) {
    Some(imports) => imports,
    None => return Ok(installed),
  };

  for (_alias, target) in imports {
    let target_str = match target.as_str() {
      Some(s) if s.starts_with("jsr:") => s,
      _ => continue,
    };

    let Some((scope, name, req_version)) =
      crate::tsc::tsconfig_gen::parse_jsr_specifier(target_str)
    else {
      continue;
    };

    let npm_name = format!("{}__{}", scope.trim_start_matches('@'), name);
    let pkg_dir = project_root
      .join("node_modules")
      .join("@jsr")
      .join(&npm_name);
    if pkg_dir.exists() {
      continue;
    }

    let registry_name = format!("@jsr/{npm_name}");
    let npm_jsr_registry = std::env::var("DENO_NPM_JSR_REGISTRY")
      .unwrap_or_else(|_| "https://npm.jsr.io".to_string());
    let metadata_url = format!(
      "{}/{}",
      npm_jsr_registry.trim_end_matches('/'),
      registry_name.replace('/', "%2f")
    );

    log::debug!("Installing {} from {}", registry_name, npm_jsr_registry);

    let metadata_output = std::process::Command::new("curl")
      .args(["-fsSL", &metadata_url])
      .output()
      .map_err(|e| anyhow!("Failed to fetch jsr package metadata: {e}"))?;

    if !metadata_output.status.success() {
      log::debug!("Failed to fetch metadata for {}", registry_name,);
      continue;
    }

    let metadata: Value = serde_json::from_slice(&metadata_output.stdout)
      .map_err(|e| {
        anyhow!("Failed to parse metadata for {registry_name}: {e}")
      })?;

    let resolved_version =
      resolve_jsr_version(&metadata, req_version.as_deref(), &registry_name)?;

    let tarball_url = metadata
      .get("versions")
      .and_then(|vs| vs.get(&resolved_version))
      .and_then(|v| v.get("dist"))
      .and_then(|d| d.get("tarball"))
      .and_then(|t| t.as_str())
      .ok_or_else(|| {
        anyhow!("No tarball URL for {registry_name}@{resolved_version}")
      })?;

    let temp_dir = tempfile::tempdir()?;
    let tgz_path = temp_dir.path().join("package.tgz");

    let dl_status = std::process::Command::new("curl")
      .args(["-fsSL", "-o", &tgz_path.to_string_lossy(), tarball_url])
      .status()
      .map_err(|e| anyhow!("Failed to download {registry_name}: {e}"))?;

    if !dl_status.success() {
      log::debug!("Failed to download {}", registry_name);
      continue;
    }

    std::fs::create_dir_all(&pkg_dir)?;

    let extract_status = std::process::Command::new("tar")
      .args([
        "xzf",
        &tgz_path.to_string_lossy(),
        "-C",
        &pkg_dir.to_string_lossy(),
        "--strip-components=1",
      ])
      .status()
      .map_err(|e| anyhow!("Failed to extract {registry_name}: {e}"))?;

    if !extract_status.success() {
      log::debug!("Failed to extract {}", registry_name);
      let _ = std::fs::remove_dir_all(&pkg_dir);
      continue;
    }

    installed.push(InstalledJsrPackage {
      name: registry_name,
      version: resolved_version,
    });
  }

  Ok(installed)
}

/// Materialize http(s): modules referenced from `deno.json` `imports` (and their
/// transitive remote/relative imports) into `.deno/remote/<host><path>`.
///
/// This is the prototype scanner: it parses module specifiers with a regex
/// rather than a real JS/TS parser. Good enough to walk a typical Deno-style
/// remote module graph. The production implementation should use deno_graph.
///
/// Returns the set of fully-resolved http(s) URLs that were successfully
/// materialized; the tsconfig generator turns each into a `paths` entry.
pub fn install_http_modules(
  project_root: &Path,
  deno_imports: Option<&Value>,
) -> Result<BTreeSet<Url>, AnyError> {
  let mut fetched: BTreeSet<Url> = BTreeSet::new();
  let mut queue: VecDeque<Url> = VecDeque::new();

  let Some(imports) = deno_imports.and_then(|v| v.as_object()) else {
    return Ok(fetched);
  };

  for (_alias, target) in imports {
    let Some(s) = target.as_str() else {
      continue;
    };
    if !(s.starts_with("http://") || s.starts_with("https://")) {
      continue;
    }
    if let Ok(url) = Url::parse(s) {
      queue.push_back(url);
    }
  }

  if queue.is_empty() {
    return Ok(fetched);
  }

  let remote_root = project_root.join(".deno").join("remote");

  while let Some(url) = queue.pop_front() {
    if fetched.contains(&url) {
      continue;
    }

    let Some(local_path) = url_to_local_path(&remote_root, &url) else {
      continue;
    };

    let body = if local_path.exists() {
      std::fs::read_to_string(&local_path).ok()
    } else {
      log::debug!("Fetching {}", url);
      let out = std::process::Command::new("curl")
        .args(["-fsSL", url.as_str()])
        .output()
        .map_err(|e| anyhow!("curl failed for {url}: {e}"))?;
      if !out.status.success() {
        log::debug!("Skipping {} (fetch failed)", url);
        continue;
      }
      if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
      }
      std::fs::write(&local_path, &out.stdout)?;
      String::from_utf8(out.stdout).ok()
    };

    fetched.insert(url.clone());

    if let Some(source) = body {
      for spec in scan_import_specifiers(&source) {
        // Skip non-relative, non-remote (bare / npm: / jsr: / node:)
        let resolved =
          if spec.starts_with("http://") || spec.starts_with("https://") {
            Url::parse(&spec).ok()
          } else if spec.starts_with("./") || spec.starts_with("../") {
            url.join(&spec).ok()
          } else {
            None
          };
        if let Some(child) = resolved
          && (child.scheme() == "http" || child.scheme() == "https")
          && !fetched.contains(&child)
        {
          queue.push_back(child);
        }
      }
    }
  }

  Ok(fetched)
}

/// Map a URL to its mirror file path under `<remote_root>/<host><path>`.
/// Returns None for URLs that don't yield a sensible file path (e.g. ending
/// in `/`, since we can't infer an index filename without server cooperation).
fn url_to_local_path(remote_root: &Path, url: &Url) -> Option<PathBuf> {
  let host = url.host_str()?;
  let path = url.path();
  if path.ends_with('/') || path.is_empty() {
    // Directory-like URL — would need server to tell us the actual filename.
    // Skip for the prototype; full deno_graph handles this via redirects.
    return None;
  }
  // Strip leading '/', percent-decoding left as-is for the prototype.
  let rel = path.trim_start_matches('/');
  Some(remote_root.join(host).join(rel))
}

/// Extract module specifier string literals from a JS/TS source.
/// Matches:
///   import ... from "X"   |   import "X"   |   import("X")   |   export ... from "X"
/// This is a prototype-grade scanner — it does not handle comments or template
/// strings precisely. The production version should use a real parser.
fn scan_import_specifiers(source: &str) -> Vec<String> {
  use regex::Regex;
  // Match `from "X"`, `import "X"`, `import("X")` (single or double quotes).
  let re = Regex::new(
    r#"(?:\bfrom\s+|\bimport\s*\(\s*|\bimport\s+)["']([^"'\n]+)["']"#,
  )
  .unwrap();
  re.captures_iter(source).map(|c| c[1].to_string()).collect()
}

fn resolve_jsr_version(
  metadata: &Value,
  req_version: Option<&str>,
  registry_name: &str,
) -> Result<String, AnyError> {
  match req_version {
    None => metadata
      .get("dist-tags")
      .and_then(|dt| dt.get("latest"))
      .and_then(|v| v.as_str())
      .map(|s| s.to_string())
      .ok_or_else(|| anyhow!("No latest version for {registry_name}")),
    Some(req_str) => {
      if let Ok(exact) = Version::parse_standard(req_str)
        && metadata
          .get("versions")
          .and_then(|vs| vs.get(exact.to_string()))
          .is_some()
      {
        return Ok(exact.to_string());
      }

      let version_req = VersionReq::parse_from_npm(req_str)
        .map_err(|e| anyhow!("Invalid version req '{req_str}': {e}"))?;

      let versions = metadata
        .get("versions")
        .and_then(|vs| vs.as_object())
        .ok_or_else(|| anyhow!("No versions for {registry_name}"))?;

      let mut best: Option<Version> = None;
      for key in versions.keys() {
        if let Ok(v) = Version::parse_standard(key)
          && version_req.matches(&v)
          && best.as_ref().is_none_or(|b| v > *b)
        {
          best = Some(v);
        }
      }

      best.map(|v| v.to_string()).ok_or_else(|| {
        anyhow!("No version matching '{req_str}' for {registry_name}")
      })
    }
  }
}
