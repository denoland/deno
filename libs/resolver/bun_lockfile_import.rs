// Copyright 2018-2026 the Deno authors. MIT license.

//! Translation from bun's text lockfile (`bun.lock`) to a deno.lock v5 JSON
//! string.
//!
//! Supports the text lockfile (`bun.lock`) introduced in Bun v1.1.39, which is
//! JSONC. The legacy binary lockfile (`bun.lockb`) is not handled: it has no
//! textual form we can parse without bun itself, so callers should run
//! `bun install --save-text-lockfile` (or upgrade to a bun that writes
//! `bun.lock` by default) to produce a compatible lockfile first.
//!
//! `bun.lock` records every resolved package flat under `packages`, where each
//! entry is a tuple `["<name>@<version>", "<registry>", { ...info... },
//! "<integrity>"]`. Per-workspace dependency requirements live under
//! `workspaces` (the root keyed by the empty string, members keyed by their
//! path), so the workspace section can be reconstructed faithfully here rather
//! than relying on Deno to rebuild it during resolution.

use std::collections::BTreeMap;
use std::collections::HashMap;

use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum BunLockfileImportError {
  #[error("Failed to parse bun.lock")]
  Parse(#[source] jsonc_parser::errors::ParseError),
  #[error("bun.lock is empty or not an object")]
  EmptyOrInvalid,
  #[error(
    "Unsupported bun.lock `lockfileVersion`: {0}. Only versions 0 and 1 are supported."
  )]
  UnsupportedVersion(String),
}

/// Convert a `bun.lock` (text lockfile) string into a deno.lock v5 JSON string.
/// Only the npm subset is populated.
pub fn bun_lock_to_deno_lock_v5(
  text: &str,
) -> Result<String, BunLockfileImportError> {
  let value =
    jsonc_parser::parse_to_serde_value::<Value>(text, &Default::default())
      .map_err(BunLockfileImportError::Parse)?;
  let root = value
    .as_object()
    .ok_or(BunLockfileImportError::EmptyOrInvalid)?;

  match root.get("lockfileVersion").and_then(Value::as_u64) {
    Some(0) | Some(1) => {}
    other => {
      return Err(BunLockfileImportError::UnsupportedVersion(
        other.map(|v| v.to_string()).unwrap_or_default(),
      ));
    }
  }

  let packages = root.get("packages").and_then(Value::as_object);

  // First pass: index resolved versions. `versions_by_key` maps every package
  // key to its version (bun keys a hoisted package by its bare name and a
  // nested duplicate by a `<parent-key>/<name>` path), while `hoisted` is the
  // subset keyed by the bare name. A sibling dependency resolves to the nested
  // entry under its requester's key when present, else to the hoisted one.
  let mut versions_by_key: HashMap<String, String> = HashMap::new();
  let mut hoisted: HashMap<String, String> = HashMap::new();
  if let Some(packages) = packages {
    for (key, entry) in packages {
      let Some((name, version)) = entry_ident(entry) else {
        continue;
      };
      // Skip non-registry resolutions (`workspace:`, `git+...`, tarballs, …);
      // their "version" carries a protocol and has no npm integrity.
      if version.contains(':') {
        continue;
      }
      versions_by_key.insert(key.clone(), version.to_string());
      if key == name {
        hoisted.insert(name.to_string(), version.to_string());
      }
    }
  }

  // Second pass: build the npm map keyed by `name@version`, resolving each
  // package's dependency requirements to concrete versions. The package's own
  // key anchors the nested lookup so a dependency pinned to a non-hoisted
  // version (stored under `<key>/<dep>`) resolves to that version, not the
  // hoisted one.
  let mut npm: BTreeMap<String, Value> = BTreeMap::new();
  if let Some(packages) = packages {
    for (key, entry) in packages {
      let Some((name, version)) = entry_ident(entry) else {
        continue;
      };
      if version.contains(':') {
        continue;
      }
      let arr = entry.as_array().unwrap();
      // A registry package is the 4-tuple form with an integrity in slot 3.
      let Some(integrity) = arr.get(3).and_then(Value::as_str) else {
        continue;
      };
      let info = arr.get(2).and_then(Value::as_object);
      let deps =
        collect_deps(info, "dependencies", key, &versions_by_key, &hoisted);
      let opt_deps = collect_deps(
        info,
        "optionalDependencies",
        key,
        &versions_by_key,
        &hoisted,
      );

      let mut obj = serde_json::Map::new();
      obj.insert(
        "integrity".to_string(),
        Value::String(integrity.to_string()),
      );
      if !deps.is_empty() {
        obj.insert(
          "dependencies".to_string(),
          Value::Array(deps.into_iter().map(Value::String).collect()),
        );
      }
      if !opt_deps.is_empty() {
        obj.insert(
          "optionalDependencies".to_string(),
          Value::Array(opt_deps.into_iter().map(Value::String).collect()),
        );
      }
      npm
        .entry(format!("{}@{}", name, version))
        .or_insert(Value::Object(obj));
    }
  }

  // Build specifiers and the workspace section from every workspace's declared
  // dependency requirements. The root is keyed by the empty string and maps to
  // the top-level `workspace.packageJson`; members map to
  // `workspace.members.<path>`.
  let mut specifiers: BTreeMap<String, String> = BTreeMap::new();
  let mut root_dep_keys: Vec<String> = Vec::new();
  let mut member_dep_keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
  if let Some(workspaces) = root.get("workspaces").and_then(Value::as_object) {
    for (path, ws) in workspaces {
      let Some(ws) = ws.as_object() else {
        continue;
      };
      let keys = collect_workspace_specifiers(ws, &hoisted, &mut specifiers);
      if path.is_empty() {
        root_dep_keys = keys;
      } else if !keys.is_empty() {
        member_dep_keys.insert(path.clone(), keys);
      }
    }
  }

  let mut output = serde_json::Map::new();
  output.insert("version".to_string(), Value::String("5".to_string()));
  if !specifiers.is_empty() {
    output.insert(
      "specifiers".to_string(),
      Value::Object(
        specifiers
          .into_iter()
          .map(|(k, v)| (k, Value::String(v)))
          .collect(),
      ),
    );
  }
  if !npm.is_empty() {
    output.insert("npm".to_string(), Value::Object(npm.into_iter().collect()));
  }
  if let Some(workspace) = build_workspace(root_dep_keys, member_dep_keys) {
    output.insert("workspace".to_string(), workspace);
  }

  Ok(
    serde_json::to_string(&Value::Object(output))
      .expect("serializing deno.lock v5"),
  )
}

/// Parse the `name@version` identifier from a `packages` entry (its first
/// tuple element). Returns `None` for non-array entries or malformed idents.
fn entry_ident(entry: &Value) -> Option<(&str, &str)> {
  let ident = entry.as_array()?.first()?.as_str()?;
  split_ident(ident)
}

/// Split a `name@version` identifier, accounting for a leading `@` in scoped
/// names (e.g. `@scope/pkg@1.2.3` -> `("@scope/pkg", "1.2.3")`).
fn split_ident(ident: &str) -> Option<(&str, &str)> {
  let bytes = ident.as_bytes();
  if bytes.is_empty() {
    return None;
  }
  let start = if bytes[0] == b'@' { 1 } else { 0 };
  let idx = bytes[start..].iter().position(|&b| b == b'@')? + start;
  Some((&ident[..idx], &ident[idx + 1..]))
}

/// Resolve a package's `dependencies`/`optionalDependencies` requirement map
/// into a sorted, de-duped list of `dep@version` strings, dropping any
/// requirement that does not resolve to a known version. `parent_key` is the
/// requesting package's `packages` key, used to find a nested resolution
/// before falling back to the hoisted one.
fn collect_deps(
  info: Option<&serde_json::Map<String, Value>>,
  section: &str,
  parent_key: &str,
  versions_by_key: &HashMap<String, String>,
  hoisted: &HashMap<String, String>,
) -> Vec<String> {
  let Some(deps) = info.and_then(|m| m.get(section)).and_then(Value::as_object)
  else {
    return Vec::new();
  };
  let mut out: Vec<String> = deps
    .keys()
    .filter_map(|name| {
      resolve_dep_version(versions_by_key, hoisted, parent_key, name)
        .map(|ver| format!("{}@{}", name, ver))
    })
    .collect();
  out.sort();
  out.dedup();
  out
}

/// Resolve a single dependency to its installed version: prefer the entry
/// nested directly under the requester (`<parent_key>/<dep_name>`, how bun
/// stores a version that could not be hoisted), then fall back to the hoisted
/// top-level version.
fn resolve_dep_version<'a>(
  versions_by_key: &'a HashMap<String, String>,
  hoisted: &'a HashMap<String, String>,
  parent_key: &str,
  dep_name: &str,
) -> Option<&'a str> {
  if !parent_key.is_empty()
    && let Some(version) =
      versions_by_key.get(&format!("{}/{}", parent_key, dep_name))
  {
    return Some(version);
  }
  hoisted.get(dep_name).map(String::as_str)
}

/// Collect the supported `npm:<name>@<req>` specifier keys declared by a single
/// workspace, inserting each into the shared `specifiers` map (keyed to the
/// resolved version). Returns the sorted, de-duped list of keys for the
/// caller to record under the workspace's `packageJson.dependencies`.
fn collect_workspace_specifiers(
  ws: &serde_json::Map<String, Value>,
  resolved: &HashMap<String, String>,
  specifiers: &mut BTreeMap<String, String>,
) -> Vec<String> {
  let mut keys = Vec::new();
  for section in ["dependencies", "devDependencies", "optionalDependencies"] {
    let Some(deps) = ws.get(section).and_then(Value::as_object) else {
      continue;
    };
    for (name, req) in deps {
      let Some(req) = req.as_str() else {
        continue;
      };
      if !is_supported_req(req) {
        continue;
      }
      let Some(version) = resolved.get(name) else {
        continue;
      };
      let key = format!("npm:{}@{}", name, req);
      specifiers
        .entry(key.clone())
        .or_insert_with(|| version.clone());
      keys.push(key);
    }
  }
  keys.sort();
  keys.dedup();
  keys
}

/// Build the deno.lock v5 `workspace` object from the root workspace's deps and
/// the per-member dep lists. Returns `None` when nothing was collected so the
/// caller can omit the section entirely.
fn build_workspace(
  root_dep_keys: Vec<String>,
  member_dep_keys: BTreeMap<String, Vec<String>>,
) -> Option<Value> {
  fn package_json_deps(keys: Vec<String>) -> Value {
    let mut package_json = serde_json::Map::new();
    package_json.insert(
      "dependencies".to_string(),
      Value::Array(keys.into_iter().map(Value::String).collect()),
    );
    let mut obj = serde_json::Map::new();
    obj.insert("packageJson".to_string(), Value::Object(package_json));
    Value::Object(obj)
  }

  let mut workspace = serde_json::Map::new();
  if !root_dep_keys.is_empty() {
    // The root member is flattened onto the `workspace` object, so lift its
    // `packageJson` up a level.
    if let Value::Object(root) = package_json_deps(root_dep_keys) {
      workspace.extend(root);
    }
  }
  if !member_dep_keys.is_empty() {
    let members = member_dep_keys
      .into_iter()
      .map(|(path, keys)| (path, package_json_deps(keys)))
      .collect();
    workspace.insert("members".to_string(), Value::Object(members));
  }
  if workspace.is_empty() {
    None
  } else {
    Some(Value::Object(workspace))
  }
}

/// Requirements using a protocol other than a plain npm range are skipped from
/// `specifiers` (Deno resolves those during install). `npm:` aliases would
/// produce a malformed `npm:foo@npm:bar@^1` key, and `catalog`/`catalog:` reqs
/// have no version requirement of their own to record.
fn is_supported_req(req: &str) -> bool {
  !req.starts_with("file:")
    && !req.starts_with("link:")
    && !req.starts_with("workspace:")
    && !req.starts_with("git+")
    && !req.starts_with("git:")
    && !req.starts_with("github:")
    && !req.starts_with("http:")
    && !req.starts_with("https:")
    && !req.starts_with("npm:")
    && !req.starts_with("catalog")
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = r#"{
  "lockfileVersion": 1,
  "configVersion": 1,
  "workspaces": {
    "": {
      "name": "root",
      "dependencies": {
        "chalk": "^4.0.0",
      },
    },
  },
  "packages": {
    "ansi-styles": ["ansi-styles@4.3.0", "", { "dependencies": { "color-convert": "^2.0.1" } }, "sha512-ANSI"],

    "chalk": ["chalk@4.1.2", "", { "dependencies": { "ansi-styles": "^4.1.0", "supports-color": "^7.1.0" } }, "sha512-CHALK"],

    "color-convert": ["color-convert@2.0.1", "", { "dependencies": { "color-name": "~1.1.4" } }, "sha512-CC"],

    "color-name": ["color-name@1.1.4", "", {}, "sha512-CN"],

    "has-flag": ["has-flag@4.0.0", "", {}, "sha512-HF"],

    "supports-color": ["supports-color@7.2.0", "", { "dependencies": { "has-flag": "^4.0.0" } }, "sha512-SC"],
  }
}
"#;

  #[test]
  fn translates_bun_text_lock() {
    let out = bun_lock_to_deno_lock_v5(SAMPLE).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["version"], "5");
    assert_eq!(v["specifiers"]["npm:chalk@^4.0.0"], "4.1.2");
    assert_eq!(v["npm"]["chalk@4.1.2"]["integrity"], "sha512-CHALK");
    let chalk_deps =
      v["npm"]["chalk@4.1.2"]["dependencies"].as_array().unwrap();
    assert!(chalk_deps.iter().any(|d| d == "ansi-styles@4.3.0"));
    assert!(chalk_deps.iter().any(|d| d == "supports-color@7.2.0"));
    assert_eq!(
      v["workspace"]["packageJson"]["dependencies"][0],
      "npm:chalk@^4.0.0"
    );
  }

  #[test]
  fn seeds_workspace_members() {
    let input = r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": {
      "name": "root",
      "dependencies": { "is-number": "^7.0.0" },
    },
    "packages/app": {
      "name": "app",
      "dependencies": { "is-odd": "^3.0.0" },
    },
  },
  "packages": {
    "app": ["app@workspace:packages/app"],
    "is-number": ["is-number@7.0.0", "", {}, "sha512-NUM"],
    "is-odd": ["is-odd@3.0.1", "", { "dependencies": { "is-number": "^6.0.0" } }, "sha512-ODD"],
  }
}
"#;
    let out = bun_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:is-number@^7.0.0"], "7.0.0");
    assert_eq!(v["specifiers"]["npm:is-odd@^3.0.0"], "3.0.1");
    // Workspace package entries (1-element tuples) are not npm packages.
    assert!(v["npm"].as_object().unwrap().get("app").is_none());
    assert_eq!(
      v["workspace"]["packageJson"]["dependencies"][0],
      "npm:is-number@^7.0.0"
    );
    assert_eq!(
      v["workspace"]["members"]["packages/app"]["packageJson"]["dependencies"]
        [0],
      "npm:is-odd@^3.0.0"
    );
  }

  #[test]
  fn nested_version_conflict() {
    // Root pins is-number@7, but is-odd needs is-number@^6, so bun nests the
    // older copy under `is-odd/is-number`. is-odd's dependency must resolve to
    // the nested 6.0.0, not the hoisted 7.0.0.
    let input = r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": { "name": "root", "dependencies": { "is-number": "7.0.0", "is-odd": "3.0.1" } },
  },
  "packages": {
    "is-number": ["is-number@7.0.0", "", {}, "sha512-NUM7"],
    "is-odd": ["is-odd@3.0.1", "", { "dependencies": { "is-number": "^6.0.0" } }, "sha512-ODD"],
    "is-odd/is-number": ["is-number@6.0.0", "", {}, "sha512-NUM6"],
  }
}
"#;
    let out = bun_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let is_odd_deps =
      v["npm"]["is-odd@3.0.1"]["dependencies"].as_array().unwrap();
    assert_eq!(is_odd_deps, &["is-number@6.0.0"]);
    // Both versions are present in the npm section.
    let npm = v["npm"].as_object().unwrap();
    assert!(npm.contains_key("is-number@6.0.0"));
    assert!(npm.contains_key("is-number@7.0.0"));
    // The hoisted version still backs the top-level specifier.
    assert_eq!(v["specifiers"]["npm:is-number@7.0.0"], "7.0.0");
  }

  #[test]
  fn scoped_packages() {
    let input = r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": { "name": "root", "dependencies": { "@scope/pkg": "^1.0.0" } },
  },
  "packages": {
    "@scope/pkg": ["@scope/pkg@1.2.3", "", {}, "sha512-SP"],
  }
}
"#;
    let out = bun_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:@scope/pkg@^1.0.0"], "1.2.3");
    assert!(
      v["npm"]
        .as_object()
        .unwrap()
        .contains_key("@scope/pkg@1.2.3")
    );
  }

  #[test]
  fn skips_unsupported_reqs() {
    let input = r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": {
      "name": "root",
      "dependencies": {
        "local": "file:../local",
        "from-git": "git+https://example.com/x.git",
        "ws": "workspace:*",
      },
    },
  },
  "packages": {}
}
"#;
    let out = bun_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("specifiers").is_none());
    assert!(v.get("workspace").is_none());
  }

  #[test]
  fn rejects_unsupported_version() {
    let input = r#"{ "lockfileVersion": 99, "packages": {} }"#;
    let err = bun_lock_to_deno_lock_v5(input).unwrap_err();
    assert!(matches!(
      err,
      BunLockfileImportError::UnsupportedVersion(v) if v == "99"
    ));
  }
}
