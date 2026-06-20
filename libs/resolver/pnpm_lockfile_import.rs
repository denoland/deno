// Copyright 2018-2026 the Deno authors. MIT license.

//! Translation from pnpm's `pnpm-lock.yaml` to a deno.lock v5 JSON string.
//!
//! Only the npm subset is translated. Targets pnpm lockfileVersion 6.x and
//! 9.x (the formats produced by pnpm v8 and pnpm v9+ respectively).
//!
//! YAML is parsed with `yaml_parser` (the same parser `deno fmt` already
//! depends on) to avoid pulling a new YAML crate into the dependency tree.
//! `yaml_parser` is a lossless CST parser, so the helpers below adapt its
//! syntax tree into a small `Node`/`MapNode` value model that is convenient
//! for the lookups this translation needs.

use std::collections::BTreeMap;
use std::collections::HashMap;

use serde_json::Value;
use yaml_parser::SyntaxError;
use yaml_parser::ast::AstNode;
use yaml_parser::ast::BlockMap;
use yaml_parser::ast::BlockMapKey;
use yaml_parser::ast::BlockMapValue;
use yaml_parser::ast::Flow;
use yaml_parser::ast::FlowMap;
use yaml_parser::ast::Root;

#[derive(Debug, thiserror::Error)]
pub enum PnpmLockfileImportError {
  #[error("Failed to parse pnpm-lock.yaml")]
  Parse(#[source] SyntaxError),
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
  let syntax =
    yaml_parser::parse(yaml_text).map_err(PnpmLockfileImportError::Parse)?;
  let root_map = Root::cast(syntax)
    .and_then(|root| root.documents().next())
    .and_then(|doc| doc.block())
    .and_then(|block| block.block_map())
    .map(MapNode::Block)
    .ok_or(PnpmLockfileImportError::EmptyOrInvalid)?;

  let version = root_map
    .get("lockfileVersion")
    .and_then(Node::into_string)
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
  if let Some(packages) = root_map.get("packages").and_then(Node::into_map) {
    for (key, value) in packages.entries() {
      let key = normalize_package_key(&key);
      let base = strip_peer_suffix(&key).to_string();
      if let Some(integ) = value
        .into_map()
        .and_then(|m| m.get("resolution"))
        .and_then(Node::into_map)
        .and_then(|m| m.get("integrity"))
        .and_then(Node::into_string)
      {
        integrity.entry(base).or_insert(integ);
      }
    }
  }

  // Snapshots define the resolved dependency tree. In v6 the `packages`
  // section itself carries `dependencies`; in v9 they live under `snapshots`.
  // Walk snapshots first so the dep-bearing entries win when both sections
  // exist (the `packages` pass for v9 only carries metadata we've already
  // captured in `integrity`).
  let mut npm: BTreeMap<String, Value> = BTreeMap::new();
  for section in ["snapshots", "packages"] {
    let Some(snaps) = root_map.get(section).and_then(Node::into_map) else {
      continue;
    };
    for (raw_key, value) in snaps.entries() {
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

      let value_map = value.into_map();
      let deps = collect_deps(
        value_map
          .as_ref()
          .and_then(|m| m.get("dependencies"))
          .and_then(Node::into_map),
      );
      let optional_deps = collect_deps(
        value_map
          .as_ref()
          .and_then(|m| m.get("optionalDependencies"))
          .and_then(Node::into_map),
      );

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
  if let Some(root_importer) = root_map
    .get("importers")
    .and_then(Node::into_map)
    .and_then(|m| m.get("."))
    .and_then(Node::into_map)
  {
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
      let Some(deps) = root_importer.get(section).and_then(Node::into_map)
      else {
        continue;
      };
      for (name, info) in deps.entries() {
        let Some(info) = info.into_map() else {
          continue;
        };
        let Some(spec) = info.get("specifier").and_then(Node::into_string)
        else {
          continue;
        };
        let Some(ver) = info.get("version").and_then(Node::into_string) else {
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
    let specifiers_section =
      root_map.get("specifiers").and_then(Node::into_map);
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
      let Some(deps) = root_map.get(section).and_then(Node::into_map) else {
        continue;
      };
      for (name, ver_node) in deps.entries() {
        let Some(ver) = ver_node.into_string() else {
          continue;
        };
        let spec = specifiers_section
          .as_ref()
          .and_then(|s| s.get(&name))
          .and_then(Node::into_string)
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

  Ok(
    serde_json::to_string(&Value::Object(output))
      .expect("serializing deno.lock v5"),
  )
}

/// A minimal value model over `yaml_parser`'s CST, covering the node shapes
/// `pnpm-lock.yaml` uses: scalars and mappings (block or flow style).
enum Node {
  Scalar(String),
  Map(MapNode),
  Other,
}

impl Node {
  fn into_string(self) -> Option<String> {
    match self {
      Node::Scalar(s) => Some(s),
      _ => None,
    }
  }

  fn into_map(self) -> Option<MapNode> {
    match self {
      Node::Map(m) => Some(m),
      _ => None,
    }
  }
}

enum MapNode {
  Block(BlockMap),
  Flow(FlowMap),
}

impl MapNode {
  /// Materialize the mapping's entries as `(key, value)` pairs. Entries whose
  /// key is not a scalar are skipped.
  fn entries(&self) -> Vec<(String, Node)> {
    match self {
      MapNode::Block(block_map) => block_map
        .entries()
        .filter_map(|entry| {
          let key = entry.key().and_then(|k| block_key_text(&k))?;
          let value = entry
            .value()
            .map(|v| block_value_to_node(&v))
            .unwrap_or(Node::Other);
          Some((key, value))
        })
        .collect(),
      MapNode::Flow(flow_map) => {
        let Some(entries) = flow_map.entries() else {
          return Vec::new();
        };
        entries
          .entries()
          .filter_map(|entry| {
            let key = entry
              .key()
              .and_then(|k| k.flow())
              .and_then(|f| flow_text(&f))?;
            let value = entry
              .value()
              .and_then(|v| v.flow())
              .map(|f| flow_to_node(&f))
              .unwrap_or(Node::Other);
            Some((key, value))
          })
          .collect()
      }
    }
  }

  fn get(&self, key: &str) -> Option<Node> {
    self
      .entries()
      .into_iter()
      .find(|(k, _)| k == key)
      .map(|(_, v)| v)
  }
}

fn block_key_text(key: &BlockMapKey) -> Option<String> {
  key.flow().and_then(|f| flow_text(&f))
}

fn block_value_to_node(value: &BlockMapValue) -> Node {
  if let Some(block_map) = value.block().and_then(|b| b.block_map()) {
    return Node::Map(MapNode::Block(block_map));
  }
  if let Some(flow) = value.flow() {
    return flow_to_node(&flow);
  }
  Node::Other
}

fn flow_to_node(flow: &Flow) -> Node {
  if let Some(text) = flow_text(flow) {
    return Node::Scalar(text);
  }
  if let Some(flow_map) = flow.flow_map() {
    return Node::Map(MapNode::Flow(flow_map));
  }
  Node::Other
}

/// Extract the string content of a scalar `Flow`, unquoting single/double
/// quoted forms. Returns `None` for non-scalar flows (maps, sequences).
fn flow_text(flow: &Flow) -> Option<String> {
  if let Some(token) = flow.plain_scalar() {
    return Some(token.text().trim().to_string());
  }
  if let Some(token) = flow.single_quoted_scalar() {
    return Some(unquote_single(token.text()));
  }
  if let Some(token) = flow.double_qouted_scalar() {
    return Some(unquote_double(token.text()));
  }
  None
}

fn unquote_single(raw: &str) -> String {
  let inner = raw
    .strip_prefix('\'')
    .and_then(|s| s.strip_suffix('\''))
    .unwrap_or(raw);
  // In single-quoted YAML scalars the only escape is a doubled quote.
  inner.replace("''", "'")
}

fn unquote_double(raw: &str) -> String {
  let inner = raw
    .strip_prefix('"')
    .and_then(|s| s.strip_suffix('"'))
    .unwrap_or(raw);
  let mut out = String::with_capacity(inner.len());
  let mut chars = inner.chars();
  while let Some(c) = chars.next() {
    if c != '\\' {
      out.push(c);
      continue;
    }
    match chars.next() {
      Some('n') => out.push('\n'),
      Some('t') => out.push('\t'),
      Some('r') => out.push('\r'),
      Some('"') => out.push('"'),
      Some('\\') => out.push('\\'),
      Some('0') => out.push('\0'),
      Some(other) => out.push(other),
      None => {}
    }
  }
  out
}

/// Build a sorted list of `dep@version` strings from a pnpm dependency
/// mapping (e.g. `{ ansi-styles: 4.3.0, color-convert: 2.0.1 }`).
fn collect_deps(node: Option<MapNode>) -> Vec<String> {
  let Some(map) = node else {
    return Vec::new();
  };
  let mut out: Vec<String> = map
    .entries()
    .into_iter()
    .filter_map(|(name, value)| {
      let ver = value.into_string()?;
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
  // `npm:` reqs are aliased dependencies (e.g. `foo: npm:bar@^1`). Building a
  // specifier from those would produce `npm:foo@npm:bar@^1`, which isn't a
  // valid deno.lock specifier, so skip them and let resolution handle aliases.
  !req.starts_with("file:")
    && !req.starts_with("link:")
    && !req.starts_with("workspace:")
    && !req.starts_with("git+")
    && !req.starts_with("git:")
    && !req.starts_with("github:")
    && !req.starts_with("http:")
    && !req.starts_with("https:")
    && !req.starts_with("npm:")
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
  fn skips_aliased_specifier() {
    // An aliased dependency (`my-lodash: npm:lodash@^4`) must not produce a
    // malformed `npm:my-lodash@npm:lodash@^4` specifier.
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      my-lodash:
        specifier: npm:lodash@^4.17.21
        version: lodash@4.17.21

packages:
  lodash@4.17.21:
    resolution: {integrity: sha512-AAA}

snapshots:
  lodash@4.17.21: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    // No specifier is emitted for the aliased dep.
    assert!(v.get("specifiers").is_none());
    // The resolved package itself is still captured in the npm section.
    assert_eq!(v["npm"]["lodash@4.17.21"]["integrity"], "sha512-AAA");
  }

  #[test]
  fn captures_optional_dependencies() {
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      pkg:
        specifier: ^1.0.0
        version: 1.0.0

packages:
  pkg@1.0.0:
    resolution: {integrity: sha512-PKG}
  fsevents@2.3.3:
    resolution: {integrity: sha512-FS}

snapshots:
  pkg@1.0.0:
    optionalDependencies:
      fsevents: 2.3.3
  fsevents@2.3.3: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let opt = v["npm"]["pkg@1.0.0"]["optionalDependencies"]
      .as_array()
      .unwrap();
    assert_eq!(opt[0], "fsevents@2.3.3");
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
