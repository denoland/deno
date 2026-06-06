// Copyright 2018-2026 the Deno authors. MIT license.

//! Translation from npm's `package-lock.json` to a deno.lock v5 JSON string.
//!
//! Only npm packages are translated. The produced lockfile contains the
//! `specifiers` and `npm` sections derived from the package-lock; everything
//! else (workspace, jsr, redirects, remote) is left empty for later
//! population by Deno during resolution.

use std::collections::BTreeMap;
use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum NpmLockfileImportError {
  #[error("Failed to parse package-lock.json")]
  Parse(#[source] serde_json::Error),
  #[error(
    "Unsupported package-lock.json `lockfileVersion`: {0}. Only versions 2 and 3 are supported."
  )]
  UnsupportedVersion(u32),
}

#[derive(Debug, Deserialize)]
struct NpmLockfile {
  #[serde(rename = "lockfileVersion")]
  lockfile_version: u32,
  #[serde(default)]
  packages: BTreeMap<String, NpmLockPackage>,
}

#[derive(Debug, Default, Deserialize)]
struct NpmLockPackage {
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  version: Option<String>,
  #[serde(default)]
  integrity: Option<String>,
  #[serde(default)]
  link: bool,
  #[serde(default)]
  dependencies: BTreeMap<String, String>,
  #[serde(default, rename = "devDependencies")]
  dev_dependencies: BTreeMap<String, String>,
  #[serde(default, rename = "optionalDependencies")]
  optional_dependencies: BTreeMap<String, String>,
  #[serde(default, rename = "peerDependencies")]
  peer_dependencies: BTreeMap<String, String>,
  #[serde(default, rename = "peerDependenciesMeta")]
  peer_dependencies_meta: BTreeMap<String, Value>,
  #[serde(default)]
  os: Vec<String>,
  #[serde(default)]
  cpu: Vec<String>,
}

/// Convert a `package-lock.json` (lockfileVersion 2 or 3) JSON string into a
/// deno.lock v5 JSON string. Only the npm subset is populated.
pub fn package_lock_to_deno_lock_v5(
  json_text: &str,
) -> Result<String, NpmLockfileImportError> {
  let lockfile: NpmLockfile =
    serde_json::from_str(json_text).map_err(NpmLockfileImportError::Parse)?;

  if lockfile.lockfile_version < 2 {
    return Err(NpmLockfileImportError::UnsupportedVersion(
      lockfile.lockfile_version,
    ));
  }

  // Build path -> (name, version) for every non-link, non-root entry that
  // has a version. The name is taken from the explicit `name` field when
  // present, falling back to the trailing path segment (which is what
  // npm produces).
  let mut resolved: HashMap<&str, (String, String)> = HashMap::new();
  for (path, pkg) in lockfile.packages.iter() {
    if path.is_empty() || pkg.link {
      continue;
    }
    let Some(version) = pkg.version.as_deref() else {
      continue;
    };
    let name = match pkg.name.as_deref() {
      Some(n) => n.to_string(),
      None => match package_name_from_path(path) {
        Some(n) => n.to_string(),
        None => continue,
      },
    };
    resolved.insert(path.as_str(), (name, version.to_string()));
  }

  let resolve_dep =
    |from_path: &str, dep_name: &str| -> Option<&(String, String)> {
      if from_path.is_empty() {
        let candidate = format!("node_modules/{}", dep_name);
        return resolved.get(candidate.as_str());
      }
      let mut prefix = from_path.to_string();
      loop {
        let candidate = format!("{}/node_modules/{}", prefix, dep_name);
        if let Some(entry) = resolved.get(candidate.as_str()) {
          return Some(entry);
        }
        match prefix.rfind("/node_modules/") {
          Some(idx) => prefix.truncate(idx),
          None => break,
        }
      }
      let candidate = format!("node_modules/{}", dep_name);
      resolved.get(candidate.as_str())
    };

  // Build npm section.
  let mut npm = BTreeMap::<String, Value>::new();
  for (path, pkg) in lockfile.packages.iter() {
    if path.is_empty() || pkg.link {
      continue;
    }
    let Some((name, version)) = resolved.get(path.as_str()) else {
      continue;
    };
    let Some(integrity) = pkg.integrity.as_deref() else {
      // Skip packages without integrity (bundled, git, file, workspace).
      continue;
    };

    let (regular_deps, optional_peers_from_peers) = {
      // Combine `dependencies` and (non-optional) `peerDependencies` into the
      // single deno.lock `dependencies` list. Optional peers go in their own
      // bucket.
      let mut regular: Vec<String> = Vec::new();
      let mut opt_peers: Vec<String> = Vec::new();
      for dep_name in pkg.dependencies.keys() {
        if let Some((n, v)) = resolve_dep(path, dep_name) {
          regular.push(format_dep_entry(dep_name, n, v));
        }
      }
      for dep_name in pkg.peer_dependencies.keys() {
        let is_optional = pkg
          .peer_dependencies_meta
          .get(dep_name)
          .and_then(|v| v.get("optional"))
          .and_then(|v| v.as_bool())
          .unwrap_or(false);
        if let Some((n, v)) = resolve_dep(path, dep_name) {
          let entry = format_dep_entry(dep_name, n, v);
          if is_optional {
            opt_peers.push(entry);
          } else {
            regular.push(entry);
          }
        }
      }
      regular.sort();
      regular.dedup();
      opt_peers.sort();
      opt_peers.dedup();
      (regular, opt_peers)
    };

    let optional_deps: Vec<String> = {
      let mut v: Vec<String> = pkg
        .optional_dependencies
        .keys()
        .filter_map(|dep_name| {
          resolve_dep(path, dep_name)
            .map(|(n, ver)| format_dep_entry(dep_name, n, ver))
        })
        .collect();
      v.sort();
      v.dedup();
      v
    };

    let mut entry = serde_json::Map::new();
    entry.insert(
      "integrity".to_string(),
      Value::String(integrity.to_string()),
    );
    if !regular_deps.is_empty() {
      entry.insert(
        "dependencies".to_string(),
        Value::Array(regular_deps.into_iter().map(Value::String).collect()),
      );
    }
    if !optional_deps.is_empty() {
      entry.insert(
        "optionalDependencies".to_string(),
        Value::Array(optional_deps.into_iter().map(Value::String).collect()),
      );
    }
    if !optional_peers_from_peers.is_empty() {
      entry.insert(
        "optionalPeers".to_string(),
        Value::Array(
          optional_peers_from_peers
            .into_iter()
            .map(Value::String)
            .collect(),
        ),
      );
    }
    if !pkg.os.is_empty() {
      entry.insert(
        "os".to_string(),
        Value::Array(pkg.os.iter().cloned().map(Value::String).collect()),
      );
    }
    if !pkg.cpu.is_empty() {
      entry.insert(
        "cpu".to_string(),
        Value::Array(pkg.cpu.iter().cloned().map(Value::String).collect()),
      );
    }

    let key = format!("{}@{}", name, version);
    npm.insert(key, Value::Object(entry));
  }

  // Build specifiers from the root package's dependencies. Each (name, req)
  // pair maps to the resolved version found at `node_modules/<name>`.
  let mut specifiers = BTreeMap::<String, String>::new();
  if let Some(root) = lockfile.packages.get("") {
    let root_dep_iters = [
      &root.dependencies,
      &root.dev_dependencies,
      &root.optional_dependencies,
      &root.peer_dependencies,
    ];
    for deps in root_dep_iters {
      for (dep_name, req) in deps.iter() {
        if !is_supported_root_req(req) {
          continue;
        }
        let Some((_n, version)) = resolve_dep("", dep_name) else {
          continue;
        };
        let key = format!("npm:{}@{}", dep_name, req);
        specifiers.entry(key).or_insert_with(|| version.clone());
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

  Ok(serde_json::to_string(&Value::Object(output)).unwrap())
}

fn format_dep_entry(alias: &str, name: &str, version: &str) -> String {
  if alias == name {
    format!("{}@{}", name, version)
  } else {
    // npm aliasing: `alias@npm:name@version`
    format!("{}@npm:{}@{}", alias, name, version)
  }
}

fn package_name_from_path(path: &str) -> Option<&str> {
  if let Some(idx) = path.rfind("/node_modules/") {
    Some(&path[idx + "/node_modules/".len()..])
  } else {
    path.strip_prefix("node_modules/")
  }
}

fn is_supported_root_req(req: &str) -> bool {
  // Skip file:, link:, workspace:, git:, https:// tarballs, etc. These
  // cannot be represented as plain npm specifiers in deno.lock.
  !req.starts_with("file:")
    && !req.starts_with("link:")
    && !req.starts_with("workspace:")
    && !req.starts_with("git+")
    && !req.starts_with("git:")
    && !req.starts_with("github:")
    && !req.starts_with("http:")
    && !req.starts_with("https:")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn translates_simple_package_lock() {
    let input = r#"{
      "name": "myapp",
      "version": "1.0.0",
      "lockfileVersion": 3,
      "requires": true,
      "packages": {
        "": {
          "name": "myapp",
          "version": "1.0.0",
          "dependencies": {
            "lodash": "^4.17.0"
          }
        },
        "node_modules/lodash": {
          "version": "4.17.21",
          "resolved": "https://registry.npmjs.org/lodash/-/lodash-4.17.21.tgz",
          "integrity": "sha512-v2kDEe57lecTulaDIuNTPy3Ry4gLGJ6Z1O3vE1krgXZNrsQ+LFTGHVxVjcXPs17LhbZVGedAJv8XZ1tvj5FvSg=="
        }
      }
    }"#;
    let out = package_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["version"], "5");
    assert_eq!(v["specifiers"]["npm:lodash@^4.17.0"], "4.17.21");
    let lodash = &v["npm"]["lodash@4.17.21"];
    assert!(lodash["integrity"].as_str().unwrap().starts_with("sha512-"));
  }

  #[test]
  fn nested_dependency_resolution() {
    let input = r#"{
      "name": "myapp",
      "version": "1.0.0",
      "lockfileVersion": 3,
      "packages": {
        "": {
          "dependencies": { "a": "^1" }
        },
        "node_modules/a": {
          "version": "1.0.0",
          "integrity": "sha512-AAAAA",
          "dependencies": { "b": "^2" }
        },
        "node_modules/a/node_modules/b": {
          "version": "2.0.0",
          "integrity": "sha512-BBBBB"
        },
        "node_modules/b": {
          "version": "1.0.0",
          "integrity": "sha512-BBB1"
        }
      }
    }"#;
    let out = package_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    // a's dependency on b should resolve to its nested copy (2.0.0)
    let a_deps = v["npm"]["a@1.0.0"]["dependencies"].as_array().unwrap();
    assert_eq!(a_deps[0], "b@2.0.0");
    // both versions of b should be present
    assert!(v["npm"].as_object().unwrap().contains_key("b@1.0.0"));
    assert!(v["npm"].as_object().unwrap().contains_key("b@2.0.0"));
  }

  #[test]
  fn scoped_packages() {
    let input = r#"{
      "lockfileVersion": 3,
      "packages": {
        "": {
          "dependencies": { "@scope/pkg": "^1" }
        },
        "node_modules/@scope/pkg": {
          "version": "1.2.3",
          "integrity": "sha512-XXXX"
        }
      }
    }"#;
    let out = package_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:@scope/pkg@^1"], "1.2.3");
    assert!(
      v["npm"]
        .as_object()
        .unwrap()
        .contains_key("@scope/pkg@1.2.3")
    );
  }

  #[test]
  fn skips_workspace_links() {
    let input = r#"{
      "lockfileVersion": 3,
      "packages": {
        "": {
          "dependencies": { "ws-pkg": "*", "lodash": "^4" }
        },
        "node_modules/ws-pkg": {
          "resolved": "../ws-pkg",
          "link": true
        },
        "node_modules/lodash": {
          "version": "4.17.21",
          "integrity": "sha512-AAA"
        }
      }
    }"#;
    let out = package_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(v["specifiers"].get("npm:ws-pkg@*").is_none());
    assert!(v["npm"].as_object().unwrap().get("ws-pkg").is_none());
    assert_eq!(v["specifiers"]["npm:lodash@^4"], "4.17.21");
  }

  #[test]
  fn rejects_v1_lockfile() {
    let input = r#"{ "lockfileVersion": 1, "packages": {} }"#;
    let err = package_lock_to_deno_lock_v5(input).unwrap_err();
    assert!(matches!(err, NpmLockfileImportError::UnsupportedVersion(1)));
  }
}
