// Copyright 2018-2026 the Deno authors. MIT license.

//! Automatic migration of a `pnpm-workspace.yaml` into the equivalent
//! `deno.json` fields.
//!
//! Deno deliberately does not read `pnpm-workspace.yaml` during module
//! resolution, so a project whose workspace members (or catalog versions) live
//! in that file fails to resolve. Rather than adding a dedicated subcommand for
//! a single package manager, we detect this situation on the resolution-failure
//! path: when an error looks like a workspace/npm resolution failure and a
//! `pnpm-workspace.yaml` is found nearby, we convert its `packages`, `catalog`,
//! and `catalogs` into `deno.json` and ask the user to run the command again.
//! The conversion preserves existing comments and formatting via the jsonc CST
//! and never overwrites fields the user already set.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_terminal::colors;
use indexmap::IndexMap;
use jsonc_parser::cst::CstInputValue;
use jsonc_parser::cst::CstRootNode;
use yaml_parser::SyntaxKind;
use yaml_parser::SyntaxNode;
use yaml_parser::ast::AstNode;
use yaml_parser::ast::BlockMap;
use yaml_parser::ast::Document;
use yaml_parser::ast::Root;

/// The subset of `pnpm-workspace.yaml` that maps cleanly onto `deno.json`.
#[derive(Default)]
struct PnpmWorkspace {
  /// `packages:` glob list -> `deno.json` `workspace`.
  packages: Vec<String>,
  /// `catalog:` map -> `deno.json` `catalog`.
  catalog: IndexMap<String, String>,
  /// `catalogs:` map of maps -> `deno.json` `catalogs`.
  catalogs: IndexMap<String, IndexMap<String, String>>,
  /// Top-level keys that have no `deno.json` equivalent (e.g. `overrides`,
  /// `patchedDependencies`, `registries`). We do not translate these, but we
  /// surface them so the user is not misled into thinking the migration was
  /// lossless.
  unsupported_keys: Vec<String>,
}

/// Error message fragments produced when a workspace/npm package fails to
/// resolve. When one of these is shown and a `pnpm-workspace.yaml` is nearby,
/// the user is almost certainly in a pnpm workspace that Deno can't read.
const RESOLUTION_FAILURE_MARKERS: &[&str] = &[
  "Deno expects the node_modules/ directory to be up to date",
  "Could not find package.json with name",
  "Could not find a matching package for",
];

/// If `error_text` looks like a workspace/npm resolution failure and a
/// `pnpm-workspace.yaml` exists in `cwd` or an ancestor, converts it into the
/// equivalent `deno.json` fields (without overwriting anything the user already
/// set) and returns a note telling the user to run the command again.
///
/// Returns `None` when this does not apply: the error is unrelated, no
/// `pnpm-workspace.yaml` is found, or `deno.json` already has the fields the
/// migration would add (so re-running would not change anything and there is
/// nothing to migrate).
///
/// This only ever runs on an error path, so the filesystem work never affects
/// successful resolution.
pub fn maybe_auto_migrate_pnpm_workspace(
  error_text: &str,
  cwd: Option<&Path>,
) -> Option<String> {
  if !RESOLUTION_FAILURE_MARKERS
    .iter()
    .any(|m| error_text.contains(m))
  {
    return None;
  }

  let cwd = crate::util::env::resolve_cwd_or_fallback(cwd);
  let yaml_path = find_pnpm_workspace(&cwd)?;
  let dir = yaml_path.parent().unwrap_or(&cwd).to_path_buf();

  match auto_migrate(&yaml_path, &dir) {
    Ok(Some(message)) => Some(message),
    // Nothing to migrate (e.g. `deno.json` already has these fields): stay
    // silent and let the original error speak for itself.
    Ok(None) => None,
    // We found a `pnpm-workspace.yaml` but couldn't convert it. Point the user
    // at it rather than failing silently; the migration is best-effort.
    Err(err) => Some(format!(
      "\n\n{} Found {} nearby, which Deno does not read directly, but it could not be converted automatically: {}",
      colors::yellow("warning:"),
      colors::cyan("pnpm-workspace.yaml"),
      err,
    )),
  }
}

fn auto_migrate(
  yaml_path: &Path,
  dir: &Path,
) -> Result<Option<String>, AnyError> {
  let yaml_text = std::fs::read_to_string(yaml_path)
    .with_context(|| format!("Reading '{}'", yaml_path.display()))?;
  let parsed = parse_pnpm_workspace(&yaml_text)
    .with_context(|| format!("Parsing '{}'", yaml_path.display()))?;

  let deno_json_path = pick_deno_json_path(dir);
  let Some(new_contents) = apply_to_deno_json(&deno_json_path, &parsed)? else {
    return Ok(None);
  };
  std::fs::write(&deno_json_path, new_contents)
    .with_context(|| format!("Writing '{}'", deno_json_path.display()))?;

  let mut message = format!(
    "\n\n{} Found {} nearby, which Deno does not read directly.\n{} Migrated its workspace configuration into {}. Run the command again.",
    colors::yellow("warning:"),
    colors::cyan("pnpm-workspace.yaml"),
    colors::intense_blue("hint:"),
    colors::cyan(deno_json_path.display().to_string()),
  );
  if !parsed.unsupported_keys.is_empty() {
    message.push_str(&format!(
      "\n{} these pnpm-workspace.yaml keys have no deno.json equivalent and were not migrated: {}",
      colors::yellow("note:"),
      parsed.unsupported_keys.join(", "),
    ));
  }
  Ok(Some(message))
}

/// Walks up from `start` looking for a `pnpm-workspace.yaml`.
fn find_pnpm_workspace(start: &Path) -> Option<PathBuf> {
  let mut dir = Some(start);
  while let Some(current) = dir {
    let candidate = current.join("pnpm-workspace.yaml");
    if candidate.is_file() {
      return Some(candidate);
    }
    dir = current.parent();
  }
  None
}

fn pick_deno_json_path(dir: &Path) -> PathBuf {
  let jsonc = dir.join("deno.jsonc");
  if jsonc.is_file() {
    return jsonc;
  }
  dir.join("deno.json")
}

/// Merges the parsed pnpm workspace into an existing `deno.json` (preserving
/// comments and formatting via the jsonc CST) or creates a new one. Fields the
/// user has already set are left untouched. Returns the new file contents, or
/// `None` when there is nothing to add (so the caller can avoid a no-op write).
fn apply_to_deno_json(
  path: &Path,
  parsed: &PnpmWorkspace,
) -> Result<Option<String>, AnyError> {
  let existing =
    std::fs::read_to_string(path).unwrap_or_else(|_| "{}\n".to_string());
  let cst = CstRootNode::parse(&existing, &Default::default())
    .with_context(|| format!("Parsing existing '{}'", path.display()))?;
  let root = cst.object_value_or_set();

  let mut changed = false;

  if !parsed.packages.is_empty() && root.get("workspace").is_none() {
    let array = CstInputValue::Array(
      parsed
        .packages
        .iter()
        .map(|p| CstInputValue::String(p.clone()))
        .collect(),
    );
    root.append("workspace", array);
    changed = true;
  }

  if !parsed.catalog.is_empty() && root.get("catalog").is_none() {
    root.append("catalog", string_map_to_value(&parsed.catalog));
    changed = true;
  }

  if !parsed.catalogs.is_empty() && root.get("catalogs").is_none() {
    let obj = CstInputValue::Object(
      parsed
        .catalogs
        .iter()
        .map(|(name, entries)| (name.clone(), string_map_to_value(entries)))
        .collect(),
    );
    root.append("catalogs", obj);
    changed = true;
  }

  if !changed {
    return Ok(None);
  }

  root.ensure_multiline();
  Ok(Some(cst.to_string()))
}

fn string_map_to_value(map: &IndexMap<String, String>) -> CstInputValue {
  CstInputValue::Object(
    map
      .iter()
      .map(|(k, v)| (k.clone(), CstInputValue::String(v.clone())))
      .collect(),
  )
}

// ===========================================================================
// pnpm-workspace.yaml parsing (via the yaml_parser CST already used by
// `deno fmt`, so no new YAML dependency is introduced).
// ===========================================================================

fn parse_pnpm_workspace(text: &str) -> Result<PnpmWorkspace, AnyError> {
  let tree = yaml_parser::parse(text)
    .map_err(|e| deno_core::anyhow::anyhow!("Invalid YAML: {e}"))?;
  let Some(root) = Root::cast(tree) else {
    return Ok(PnpmWorkspace::default());
  };
  let Some(block_map) = root
    .documents()
    .next()
    .and_then(|doc: Document| doc.block())
    .and_then(|block| block.block_map())
  else {
    return Ok(PnpmWorkspace::default());
  };

  let mut result = PnpmWorkspace::default();
  for entry in block_map.entries() {
    let Some(key) = entry.key().map(|k| k.syntax().clone()) else {
      continue;
    };
    let Some(key_name) = scalar_text(&key) else {
      continue;
    };
    let value = entry.value().map(|v| v.syntax().clone());
    match key_name.as_str() {
      "packages" => {
        if let Some(value) = &value {
          result.packages = collect_string_seq(value);
        }
      }
      "catalog" => {
        if let Some(value) = &value {
          result.catalog = collect_string_map(value);
        }
      }
      "catalogs" => {
        if let Some(value) = &value {
          result.catalogs = collect_catalogs(value);
        }
      }
      other => result.unsupported_keys.push(other.to_string()),
    }
  }
  Ok(result)
}

/// Collects a YAML block/flow sequence of scalars into a `Vec<String>`.
fn collect_string_seq(value: &SyntaxNode) -> Vec<String> {
  let mut out = Vec::new();
  // Block sequence: each `BLOCK_SEQ_ENTRY` holds one scalar.
  for node in value.descendants() {
    match node.kind() {
      SyntaxKind::BLOCK_SEQ_ENTRY | SyntaxKind::FLOW_SEQ_ENTRY => {
        if let Some(s) = scalar_text(&node) {
          out.push(s);
        }
      }
      _ => {}
    }
  }
  out
}

/// Collects a YAML block/flow mapping of scalar -> scalar into an ordered map.
fn collect_string_map(value: &SyntaxNode) -> IndexMap<String, String> {
  let mut out = IndexMap::new();
  for node in value.descendants() {
    if matches!(
      node.kind(),
      SyntaxKind::BLOCK_MAP_ENTRY | SyntaxKind::FLOW_MAP_ENTRY
    ) {
      let mut children = node.children();
      let key = children.next().and_then(|n| scalar_text(&n));
      let val = children.next().and_then(|n| scalar_text(&n));
      if let (Some(key), Some(val)) = (key, val) {
        out.insert(key, val);
      }
    }
  }
  out
}

/// Collects `catalogs:` (a mapping of catalog-name -> mapping of dep -> req).
fn collect_catalogs(
  value: &SyntaxNode,
) -> IndexMap<String, IndexMap<String, String>> {
  let mut out = IndexMap::new();
  let Some(map) = value
    .descendants()
    .find(|n| n.kind() == SyntaxKind::BLOCK_MAP)
    .and_then(BlockMap::cast)
  else {
    return out;
  };
  for entry in map.entries() {
    let Some(name) = entry.key().and_then(|k| scalar_text(k.syntax())) else {
      continue;
    };
    if let Some(value) = entry.value() {
      out.insert(name, collect_string_map(value.syntax()));
    }
  }
  out
}

/// Returns the first scalar token's decoded text within `node`, or its direct
/// scalar if `node` is itself a scalar holder.
fn scalar_text(node: &SyntaxNode) -> Option<String> {
  node
    .descendants_with_tokens()
    .filter_map(|el| el.into_token())
    .find_map(|tok| match tok.kind() {
      SyntaxKind::PLAIN_SCALAR => Some(tok.text().trim().to_string()),
      SyntaxKind::SINGLE_QUOTED_SCALAR => Some(unquote(tok.text(), '\'')),
      SyntaxKind::DOUBLE_QUOTED_SCALAR => Some(unquote(tok.text(), '"')),
      _ => None,
    })
}

fn unquote(text: &str, quote: char) -> String {
  text
    .strip_prefix(quote)
    .and_then(|t| t.strip_suffix(quote))
    .unwrap_or(text)
    .to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_packages() {
    let yaml = "packages:\n  - \"packages/*\"\n  - 'apps/*'\n  - tools/cli\n";
    let parsed = parse_pnpm_workspace(yaml).unwrap();
    assert_eq!(parsed.packages, vec!["packages/*", "apps/*", "tools/cli"]);
  }

  #[test]
  fn parses_catalog_and_catalogs() {
    let yaml = "packages:\n  - \"a\"\ncatalog:\n  react: ^18.0.0\n  zod: ^3.0.0\ncatalogs:\n  react17:\n    react: ^17.0.0\n";
    let parsed = parse_pnpm_workspace(yaml).unwrap();
    assert_eq!(
      parsed.catalog.get("react").map(|s| s.as_str()),
      Some("^18.0.0")
    );
    assert_eq!(
      parsed.catalog.get("zod").map(|s| s.as_str()),
      Some("^3.0.0")
    );
    assert_eq!(
      parsed
        .catalogs
        .get("react17")
        .and_then(|m| m.get("react"))
        .map(|s| s.as_str()),
      Some("^17.0.0")
    );
  }

  #[test]
  fn flags_unsupported_keys() {
    let yaml = "packages:\n  - \"a\"\noverrides:\n  foo: 1.0.0\nshamefully-hoist: true\n";
    let parsed = parse_pnpm_workspace(yaml).unwrap();
    assert!(parsed.unsupported_keys.contains(&"overrides".to_string()));
    assert!(
      parsed
        .unsupported_keys
        .contains(&"shamefully-hoist".to_string())
    );
  }

  #[test]
  fn writes_workspace_into_empty_deno_json() {
    let parsed = PnpmWorkspace {
      packages: vec!["packages/*".to_string()],
      ..Default::default()
    };
    let cst = CstRootNode::parse("{}\n", &Default::default()).unwrap();
    let root = cst.object_value_or_set();
    let array = CstInputValue::Array(
      parsed
        .packages
        .iter()
        .map(|p| CstInputValue::String(p.clone()))
        .collect(),
    );
    root.append("workspace", array);
    root.ensure_multiline();
    let out = cst.to_string();
    assert!(out.contains("\"workspace\""));
    assert!(out.contains("\"packages/*\""));
  }

  #[test]
  fn apply_skips_existing_fields() {
    // `deno.json` already declares `workspace`; nothing to add -> no-op.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deno.json");
    std::fs::write(&path, "{ \"workspace\": [\"a\"] }\n").unwrap();
    let parsed = PnpmWorkspace {
      packages: vec!["packages/*".to_string()],
      ..Default::default()
    };
    assert!(apply_to_deno_json(&path, &parsed).unwrap().is_none());
  }

  #[test]
  fn apply_adds_only_missing_fields() {
    // `workspace` is present but `catalog` is not: only `catalog` is added.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deno.json");
    std::fs::write(&path, "{ \"workspace\": [\"a\"] }\n").unwrap();
    let mut catalog = IndexMap::new();
    catalog.insert("react".to_string(), "^18.0.0".to_string());
    let parsed = PnpmWorkspace {
      packages: vec!["packages/*".to_string()],
      catalog,
      ..Default::default()
    };
    let out = apply_to_deno_json(&path, &parsed).unwrap().unwrap();
    assert!(out.contains("\"catalog\""));
    assert!(out.contains("\"react\""));
    // The existing workspace was left as-is (not duplicated).
    assert_eq!(out.matches("\"workspace\"").count(), 1);
  }
}
