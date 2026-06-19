// Copyright 2018-2026 the Deno authors. MIT license.

//! Translation from pnpm's `pnpm-lock.yaml` to a deno.lock v5 JSON string.
//!
//! Only the npm subset is translated. Targets pnpm lockfileVersion 6.x and
//! 9.x (the formats produced by pnpm v8 and pnpm v9+ respectively).

use std::collections::BTreeMap;
use std::collections::HashMap;

use saphyr::LoadableYamlNode;
use saphyr::Yaml;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum PnpmLockfileImportError {
  #[error("Failed to parse pnpm-lock.yaml")]
  Parse(#[source] saphyr::ScanError),
  #[error("pnpm-lock.yaml is empty or not a mapping")]
  EmptyOrInvalid,
  #[error(
    "Unsupported pnpm-lock.yaml `lockfileVersion`: {0}. Supported versions are 6.x and 9.x."
  )]
  UnsupportedVersion(String),
}

/// Convert a `pnpm-lock.yaml` (lockfileVersion 6 or 9) string into a
/// deno.lock v5 JSON string. Only the npm subset is populated.
pub fn pnpm_lock_to_deno_lock_v5(
  yaml_text: &str,
) -> Result<String, PnpmLockfileImportError> {
  let mut docs =
    Yaml::load_from_str(yaml_text).map_err(PnpmLockfileImportError::Parse)?;
  let doc = docs
    .pop()
    .filter(|d| d.is_mapping())
    .ok_or(PnpmLockfileImportError::EmptyOrInvalid)?;

  let version = doc
    .as_mapping_get("lockfileVersion")
    .and_then(yaml_to_string)
    .ok_or_else(
      || PnpmLockfileImportError::UnsupportedVersion(String::new()),
    )?;
  let major = version
    .split('.')
    .next()
    .and_then(|s| s.parse::<u32>().ok())
    .ok_or_else(|| {
      PnpmLockfileImportError::UnsupportedVersion(version.clone())
    })?;
  if !matches!(major, 6 | 9) {
    return Err(PnpmLockfileImportError::UnsupportedVersion(version));
  }

  // Build integrity map: `name@version` -> integrity. pnpm v6 keys may be
  // prefixed with `/` (e.g. `/lodash@4.17.21`); v9 keys are bare.
  let mut integrity: HashMap<String, String> = HashMap::new();
  if let Some(packages) = doc.as_mapping_get("packages")
    && let Some(map) = packages.as_mapping()
  {
    for (k, v) in map {
      let Some(key) = yaml_to_string(k) else {
        continue;
      };
      let key = normalize_package_key(&key);
      let base = strip_peer_suffix(&key).to_string();
      if let Some(integ) = v
        .as_mapping_get("resolution")
        .and_then(|r| r.as_mapping_get("integrity"))
        .and_then(yaml_to_string)
      {
        integrity.entry(base).or_insert(integ);
      }
    }
  }

  // Snapshots define the resolved dependency tree. In v6 the `packages`
  // section itself carries `dependencies`; in v9 they live under `snapshots`.
  // We accept whichever is present (or both, with snapshots taking
  // precedence).
  let mut npm: BTreeMap<String, Value> = BTreeMap::new();
  // In v9 dependencies live under `snapshots`; in v6 they live inline under
  // `packages`. Walk snapshots first so the dep-bearing entries win when
  // both sections exist (the `packages` pass for v9 only carries metadata
  // we've already captured in `integrity`).
  let snapshot_sources: [&str; 2] = ["snapshots", "packages"];
  for section in snapshot_sources {
    let Some(snaps) = doc.as_mapping_get(section).and_then(|s| s.as_mapping())
    else {
      continue;
    };
    for (k, v) in snaps {
      let Some(raw_key) = yaml_to_string(k) else {
        continue;
      };
      let normalized = normalize_package_key(&raw_key);
      let base = strip_peer_suffix(&normalized).to_string();
      // Snapshot keys may include peer-suffix parens; for our purposes,
      // collapse to the base `name@version`. First entry wins.
      if npm.contains_key(&base) {
        continue;
      }
      let Some(integ) = integrity.get(&base) else {
        // No integrity for this package — skip.
        continue;
      };

      let deps = collect_deps(v.as_mapping_get("dependencies"));
      let optional_deps =
        collect_deps(v.as_mapping_get("optionalDependencies"));

      let mut entry = serde_json::Map::new();
      entry.insert("integrity".to_string(), Value::String(integ.clone()));
      if !deps.is_empty() {
        entry.insert(
          "dependencies".to_string(),
          Value::Array(deps.into_iter().map(Value::String).collect()),
        );
      }
      if !optional_deps.is_empty() {
        entry.insert(
          "optionalDependencies".to_string(),
          Value::Array(optional_deps.into_iter().map(Value::String).collect()),
        );
      }
      npm.insert(base, Value::Object(entry));
    }
  }

  // Ensure every package with integrity ends up in the npm section even if
  // it has no snapshot entry of its own.
  for (base, integ) in &integrity {
    npm.entry(base.clone()).or_insert_with(|| {
      let mut entry = serde_json::Map::new();
      entry.insert("integrity".to_string(), Value::String(integ.clone()));
      Value::Object(entry)
    });
  }

  // Build specifiers from the root importer (key `.`).
  let mut specifiers: BTreeMap<String, String> = BTreeMap::new();
  if let Some(importers) = doc.as_mapping_get("importers")
    && let Some(root) = importers.as_mapping_get(".")
  {
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
      let Some(deps) =
        root.as_mapping_get(section).and_then(|s| s.as_mapping())
      else {
        continue;
      };
      for (name_node, info) in deps {
        let Some(name) = yaml_to_string(name_node) else {
          continue;
        };
        let Some(spec) =
          info.as_mapping_get("specifier").and_then(yaml_to_string)
        else {
          continue;
        };
        let Some(ver) = info.as_mapping_get("version").and_then(yaml_to_string)
        else {
          continue;
        };
        if !is_supported_spec(&spec) {
          continue;
        }
        let resolved = strip_peer_suffix(&ver).to_string();
        let key = format!("npm:{}@{}", name, spec);
        specifiers.entry(key).or_insert(resolved);
      }
    }
  }
  // pnpm v6 places top-level deps directly on the document root.
  if major == 6 {
    let specifiers_section = doc
      .as_mapping_get("specifiers")
      .and_then(|s| s.as_mapping());
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
      let Some(deps) = doc.as_mapping_get(section).and_then(|s| s.as_mapping())
      else {
        continue;
      };
      for (name_node, ver_node) in deps {
        let Some(name) = yaml_to_string(name_node) else {
          continue;
        };
        let Some(ver) = yaml_to_string(ver_node) else {
          continue;
        };
        let spec = specifiers_section
          .and_then(|s| {
            s.iter().find(|(k, _)| {
              yaml_to_string(k).as_deref() == Some(name.as_str())
            })
          })
          .and_then(|(_, v)| yaml_to_string(v))
          .unwrap_or_else(|| ver.clone());
        if !is_supported_spec(&spec) {
          continue;
        }
        let resolved = strip_peer_suffix(&ver).to_string();
        let key = format!("npm:{}@{}", name, spec);
        specifiers.entry(key).or_insert(resolved);
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

fn yaml_to_string(node: &Yaml) -> Option<String> {
  // Saphyr lazy-parses plain scalars and leaves them in the `Representation`
  // variant when they don't resolve to a typed value (e.g. multi-dot
  // version strings like `4.3.0`). Pull the raw text out for those before
  // falling back to the typed accessors.
  if let Yaml::Representation(raw, _, _) = node {
    return Some(raw.to_string());
  }
  if let Some(s) = node.as_str() {
    return Some(s.to_string());
  }
  if let Some(b) = node.as_bool() {
    return Some(b.to_string());
  }
  if let Some(i) = node.as_integer() {
    return Some(i.to_string());
  }
  node.as_floating_point().map(|f| f.to_string())
}

/// Build a sorted list of `dep@version` strings from a pnpm dependency
/// mapping (e.g. `{ ansi-styles: 4.3.0, color-convert: 2.0.1 }`).
fn collect_deps(node: Option<&Yaml>) -> Vec<String> {
  let Some(map) = node.and_then(|n| n.as_mapping()) else {
    return Vec::new();
  };
  let mut out: Vec<String> = map
    .iter()
    .filter_map(|(k, v)| {
      let name = yaml_to_string(k)?;
      let ver = yaml_to_string(v)?;
      let ver = strip_peer_suffix(&ver);
      Some(format!("{}@{}", name, ver))
    })
    .collect();
  out.sort();
  out.dedup();
  out
}

/// In pnpm v6 the keys in `packages` and reference paths are prefixed with
/// `/`, e.g. `/lodash@4.17.21` or `/@babel/core@7.0.0`. Strip it.
fn normalize_package_key(key: &str) -> String {
  let stripped = key.strip_prefix('/').unwrap_or(key);
  // pnpm v6 sometimes used `/name/version` instead of `/name@version`. We
  // detect the `/version` form by checking whether the last `/` is followed
  // by what looks like a semver number.
  if !stripped.contains('@') || stripped.starts_with('@') {
    // For scoped packages, the only `@` may be at the start. Check the
    // `name/version` form by splitting on the last `/`.
    if let Some(idx) = stripped.rfind('/') {
      let (name, ver) = stripped.split_at(idx);
      let ver = &ver[1..];
      if ver.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return format!("{}@{}", name, ver);
      }
    }
  }
  stripped.to_string()
}

/// Strip pnpm's peer-dependency suffix from a package id. E.g.
/// `chalk@5.0.0(react@18.0.0)` -> `chalk@5.0.0`.
fn strip_peer_suffix(key: &str) -> &str {
  match key.find('(') {
    Some(idx) => &key[..idx],
    None => key,
  }
}

fn is_supported_spec(req: &str) -> bool {
  !req.starts_with("file:")
    && !req.starts_with("link:")
    && !req.starts_with("workspace:")
    && !req.starts_with("git+")
    && !req.starts_with("git:")
    && !req.starts_with("github:")
    && !req.starts_with("http:")
    && !req.starts_with("https:")
    && !req.starts_with("catalog:")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn translates_simple_v9() {
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      lodash:
        specifier: ^4.17.21
        version: 4.17.21

packages:
  lodash@4.17.21:
    resolution: {integrity: sha512-AAA}

snapshots:
  lodash@4.17.21: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["version"], "5");
    assert_eq!(v["specifiers"]["npm:lodash@^4.17.21"], "4.17.21");
    assert_eq!(v["npm"]["lodash@4.17.21"]["integrity"], "sha512-AAA");
  }

  #[test]
  fn translates_v9_with_nested_deps() {
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      chalk:
        specifier: ^4.0.0
        version: 4.1.2

packages:
  chalk@4.1.2:
    resolution: {integrity: sha512-CHALK}
  ansi-styles@4.3.0:
    resolution: {integrity: sha512-ANSI}

snapshots:
  chalk@4.1.2:
    dependencies:
      ansi-styles: 4.3.0
  ansi-styles@4.3.0: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["npm"]["chalk@4.1.2"]["integrity"], "sha512-CHALK");
    let chalk_deps =
      v["npm"]["chalk@4.1.2"]["dependencies"].as_array().unwrap();
    assert_eq!(chalk_deps[0], "ansi-styles@4.3.0");
    assert_eq!(v["npm"]["ansi-styles@4.3.0"]["integrity"], "sha512-ANSI");
  }

  #[test]
  fn strips_peer_suffix() {
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      some-plugin:
        specifier: ^1.0.0
        version: 1.0.0(react@18.3.1)

packages:
  some-plugin@1.0.0:
    resolution: {integrity: sha512-PLUGIN}
  react@18.3.1:
    resolution: {integrity: sha512-REACT}

snapshots:
  some-plugin@1.0.0(react@18.3.1):
    dependencies:
      react: 18.3.1
  react@18.3.1: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:some-plugin@^1.0.0"], "1.0.0");
    let plugin_deps = v["npm"]["some-plugin@1.0.0"]["dependencies"]
      .as_array()
      .unwrap();
    assert_eq!(plugin_deps[0], "react@18.3.1");
  }

  #[test]
  fn scoped_packages_v9() {
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      '@scope/pkg':
        specifier: ^1.0.0
        version: 1.2.3

packages:
  '@scope/pkg@1.2.3':
    resolution: {integrity: sha512-XXX}

snapshots:
  '@scope/pkg@1.2.3': {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
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
  fn translates_v6() {
    let input = r#"
lockfileVersion: '6.0'

specifiers:
  lodash: ^4.17.21

dependencies:
  lodash: 4.17.21

packages:
  /lodash@4.17.21:
    resolution: {integrity: sha512-LODASH}
    dev: false
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:lodash@^4.17.21"], "4.17.21");
    assert_eq!(v["npm"]["lodash@4.17.21"]["integrity"], "sha512-LODASH");
  }

  #[test]
  fn rejects_unsupported_version() {
    let input = r#"lockfileVersion: '4.0'
packages: {}
"#;
    let err = pnpm_lock_to_deno_lock_v5(input).unwrap_err();
    assert!(matches!(
      err,
      PnpmLockfileImportError::UnsupportedVersion(_)
    ));
  }
}
