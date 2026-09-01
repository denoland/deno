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
    .and_then(select_project_document)
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
      let base = normalize_package_key(strip_peer_suffix(&key));
      if !is_package_id(&base) {
        continue;
      }
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
      // Snapshot keys may include peer-suffix parens; for our purposes,
      // collapse to the base `name@version`. First entry wins. The suffix
      // comes off before normalizing because the `@` inside it would
      // otherwise hide the v6 `name/version` form from
      // `normalize_package_key`.
      let base = normalize_package_key(strip_peer_suffix(&raw_key));
      if !is_package_id(&base) || npm.contains_key(&base) {
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

  // pnpm v9 embeds a top-level `catalogs:` block mapping each catalog name to
  // its `dep -> {specifier, version}` entries. Build a lookup so importer deps
  // declared as `catalog:`/`catalog:<name>` can be resolved to a real version
  // requirement.
  let catalogs = collect_catalogs(&root_map);

  // Build specifiers from every importer. The root importer (`.`) feeds the
  // top-level `workspace.packageJson` section; non-root importers map to
  // `workspace.members.<path>.packageJson`. All resolved specifiers end up in
  // the single flat `specifiers` map regardless of which importer declared
  // them.
  let mut specifiers: BTreeMap<String, String> = BTreeMap::new();
  let mut root_dep_keys: Vec<String> = Vec::new();
  let mut member_dep_keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
  if let Some(importers) = root_map.get("importers").and_then(Node::into_map) {
    for (path, importer) in importers.entries() {
      let Some(importer) = importer.into_map() else {
        continue;
      };
      let keys =
        collect_importer_specifiers(&importer, &catalogs, &mut specifiers);
      if path == "." {
        root_dep_keys = keys;
      } else if !keys.is_empty() {
        member_dep_keys.insert(path, keys);
      }
    }
  }
  // pnpm v6 places top-level deps directly on the document root.
  if major == 6 {
    // 6.0 gives each of them the same `{ specifier, version }` shape an
    // importer uses. Older 6.x lockfiles instead put the resolved version
    // here and the requirement in a separate `specifiers` section, which the
    // loop below reads.
    root_dep_keys.extend(collect_importer_specifiers(
      &root_map,
      &catalogs,
      &mut specifiers,
    ));
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
        specifiers.entry(key.clone()).or_insert(resolved);
        root_dep_keys.push(key);
      }
    }
    root_dep_keys.sort();
    root_dep_keys.dedup();
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

/// Build a lookup of pnpm catalogs from the top-level `catalogs:` block:
/// `catalog_name -> (dep_name -> specifier)`. The default catalog is keyed
/// `default`. Returns an empty map when no `catalogs:` block is present (e.g.
/// pnpm v6, which has no catalog support).
fn collect_catalogs(
  root_map: &MapNode,
) -> HashMap<String, HashMap<String, String>> {
  let mut catalogs: HashMap<String, HashMap<String, String>> = HashMap::new();
  let Some(block) = root_map.get("catalogs").and_then(Node::into_map) else {
    return catalogs;
  };
  for (catalog_name, entries) in block.entries() {
    let Some(entries) = entries.into_map() else {
      continue;
    };
    let mut map = HashMap::new();
    for (dep_name, info) in entries.entries() {
      if let Some(spec) = info
        .into_map()
        .and_then(|m| m.get("specifier"))
        .and_then(Node::into_string)
      {
        map.insert(dep_name, spec);
      }
    }
    catalogs.insert(catalog_name, map);
  }
  catalogs
}

/// Collect the supported `npm:<name>@<spec>` specifier keys declared by a
/// single importer, inserting each into the shared `specifiers` map (keyed to
/// the resolved version). `catalog:`/`catalog:<name>` specifiers are resolved
/// to a real version requirement via `catalogs`. Returns the sorted, de-duped
/// list of keys so the caller can record them under the importer's
/// `packageJson.dependencies`.
fn collect_importer_specifiers(
  importer: &MapNode,
  catalogs: &HashMap<String, HashMap<String, String>>,
  specifiers: &mut BTreeMap<String, String>,
) -> Vec<String> {
  let mut keys = Vec::new();
  for section in ["dependencies", "devDependencies", "optionalDependencies"] {
    let Some(deps) = importer.get(section).and_then(Node::into_map) else {
      continue;
    };
    for (name, info) in deps.entries() {
      let Some(info) = info.into_map() else {
        continue;
      };
      let Some(spec) = info.get("specifier").and_then(Node::into_string) else {
        continue;
      };
      let Some(ver) = info.get("version").and_then(Node::into_string) else {
        continue;
      };
      let resolved_spec = if let Some(catalog) = spec.strip_prefix("catalog:") {
        // A bare `catalog:` references the `default` catalog; `catalog:<name>`
        // references a named one. Skip if the catalog entry is missing.
        let catalog_name = if catalog.is_empty() {
          "default"
        } else {
          catalog
        };
        match catalogs.get(catalog_name).and_then(|m| m.get(&name)) {
          // A catalog may itself point at an aliased/unsupported spec (e.g.
          // `npm:other@^1`); guard against producing a malformed key.
          Some(resolved) if is_supported_spec(resolved) => resolved.clone(),
          _ => continue,
        }
      } else if is_supported_spec(&spec) {
        spec.clone()
      } else {
        continue;
      };
      let resolved_ver = strip_peer_suffix(&ver).to_string();
      let key = format!("npm:{}@{}", name, resolved_spec);
      specifiers.entry(key.clone()).or_insert(resolved_ver);
      keys.push(key);
    }
  }
  keys.sort();
  keys.dedup();
  keys
}

/// Build the deno.lock v5 `workspace` object from the root importer's deps and
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

/// Build a sorted list of dependency entries from a pnpm dependency mapping
/// (e.g. `{ ansi-styles: 4.3.0, color-convert: 2.0.1 }`).
///
/// Entries are usually `dep@version`, but an aliased dependency becomes
/// `dep@npm:name@version`. A dependency that deno.lock has no spelling for —
/// a workspace link, a path, a tarball url — is **silently dropped**, so the
/// package ends up described with fewer dependencies than it really has. That
/// is the deliberate trade against emitting an id that cannot be parsed back
/// in, and it matches what `is_supported_spec` already does for root
/// specifiers.
fn collect_deps(node: Option<MapNode>) -> Vec<String> {
  let Some(map) = node else {
    return Vec::new();
  };
  let mut out: Vec<String> = map
    .entries()
    .into_iter()
    .filter_map(|(name, value)| {
      let value = value.into_string()?;
      dep_entry(&name, strip_peer_suffix(&value))
    })
    .collect();
  out.sort();
  out.dedup();
  out
}

/// Build the deno.lock dependency entry for one pnpm dependency mapping.
///
/// The value is normally a bare version (`ansi-styles: 4.3.0`), but an aliased
/// dependency names the package it resolves to instead
/// (`string-width-cjs: string-width@4.2.3`) and a workspace dependency points
/// at a directory (`vite: link:packages/vite`).
///
/// `value` must already have its peer suffix stripped.
fn dep_entry(name: &str, value: &str) -> Option<String> {
  // `link:`/`file:` paths, urls and the other non-registry schemes have no
  // deno.lock spelling, so leave them out and let resolution handle them.
  // This has to happen before `normalize_package_key`, whose `name/version`
  // -> `name@version` fallback would otherwise rewrite such a value into
  // something that reads as a valid alias: `file:vendor/1.0.0.tgz` would
  // become `file:vendor@1.0.0.tgz` and then `name@npm:file:vendor@1.0.0.tgz`.
  if !is_supported_spec(value) {
    return None;
  }
  // pnpm v6 prefixes ids with `/` in dependency values too, not just in the
  // `packages` keys.
  let value = normalize_package_key(value);
  if starts_with_digit(&value) {
    return Some(format!("{}@{}", name, value));
  }
  match value.rfind('@') {
    // A leading `@` is a scope, not a separator. deno.lock spells an alias
    // `key@npm:name@version`. Telling an alias apart from a path rests on npm
    // versions always starting with a digit, which semver guarantees.
    Some(idx) if idx > 0 && starts_with_digit(&value[idx + 1..]) => {
      // Naming the package it already is isn't an alias — that's just the v6
      // `/name/version` id spelled out. Recording it as one would point at
      // `name@npm:name@version`, which the lock has no entry for.
      if &value[..idx] == name {
        Some(format!("{}@{}", name, &value[idx + 1..]))
      } else {
        Some(format!("{}@npm:{}", name, value))
      }
    }
    _ => None,
  }
}

fn starts_with_digit(value: &str) -> bool {
  value.starts_with(|c: char| c.is_ascii_digit())
}

/// Whether a `packages`/`snapshots` key is a `name@version` deno.lock can
/// hold. A package installed from a path or a tarball is keyed by where it
/// came from, e.g. `fake-data-pkg@file:packages/…/fake-data-pkg-1.0.0.tgz`,
/// and has no npm id to record.
fn is_package_id(base: &str) -> bool {
  match base.rfind('@') {
    // A leading `@` is a scope, not a separator.
    Some(idx) if idx > 0 => starts_with_digit(&base[idx + 1..]),
    _ => false,
  }
}

/// Pick the document that describes the project.
///
/// pnpm writes `pnpm-lock.yaml` as more than one YAML document once the
/// project pins its own package manager: the packages that make up pnpm
/// itself come first, in a document whose importers carry
/// `packageManagerDependencies`, and the project's lockfile comes last.
fn select_project_document(root: Root) -> Option<MapNode> {
  let mut package_manager_doc = None;
  let mut project_doc = None;
  for doc in root.documents() {
    let Some(map) = doc
      .block()
      .and_then(|block| block.block_map())
      .map(MapNode::Block)
    else {
      continue;
    };
    if is_package_manager_document(&map) {
      package_manager_doc.get_or_insert(map);
    } else {
      project_doc = Some(map);
    }
  }
  // Fall back to the package manager's document rather than nothing at all,
  // so a lockfile that only has one stays readable whatever it holds.
  project_doc.or(package_manager_doc)
}

fn is_package_manager_document(doc: &MapNode) -> bool {
  let Some(importers) = doc.get("importers").and_then(Node::into_map) else {
    return false;
  };
  importers.entries().into_iter().any(|(_, importer)| {
    importer
      .into_map()
      .and_then(|importer| importer.get("packageManagerDependencies"))
      .is_some()
  })
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
      if starts_with_digit(ver) {
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
    // `runtime:` pins a language runtime, e.g. `node: runtime:26.8.1`.
    && !req.starts_with("runtime:")
  // `catalog:` specifiers are resolved before this check (see
  // `collect_importer_specifiers`), so they never reach here.
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
  fn translates_v6_root_dependencies() {
    // A single-project pnpm 6.0 lockfile has no `specifiers` section: the
    // root deps carry their own `specifier` the way an importer's do.
    let input = r#"
lockfileVersion: '6.0'

settings:
  autoInstallPeers: true

dependencies:
  '@vueuse/core':
    specifier: ^9.13.0
    version: 9.13.0(vue@3.2.47)
  lodash:
    specifier: ^4.17.21
    version: 4.17.21

devDependencies:
  typescript:
    specifier: ^5.0.0
    version: 5.0.4

packages:
  /@vueuse/core@9.13.0:
    resolution: {integrity: sha512-VUSE}
  /lodash@4.17.21:
    resolution: {integrity: sha512-LODASH}
  /typescript@5.0.4:
    resolution: {integrity: sha512-TS}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    // The peer suffix on the resolved version is dropped, as elsewhere.
    assert_eq!(v["specifiers"]["npm:@vueuse/core@^9.13.0"], "9.13.0");
    assert_eq!(v["specifiers"]["npm:lodash@^4.17.21"], "4.17.21");
    assert_eq!(v["specifiers"]["npm:typescript@^5.0.0"], "5.0.4");
    assert_eq!(v["npm"]["@vueuse/core@9.13.0"]["integrity"], "sha512-VUSE");
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
  fn aliased_snapshot_dependency() {
    // `string-width-cjs: string-width@4.2.3` names a package, not a version,
    // so it must not collapse into `string-width-cjs@string-width@4.2.3`.
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      wrap-ansi:
        specifier: ^8.1.0
        version: 8.1.0

packages:
  wrap-ansi@8.1.0:
    resolution: {integrity: sha512-WRAP}
  ansi-styles@6.2.1:
    resolution: {integrity: sha512-ANSI}
  string-width@4.2.3:
    resolution: {integrity: sha512-WIDTH}
  '@scope/pkg@1.0.0':
    resolution: {integrity: sha512-SCOPED}

snapshots:
  wrap-ansi@8.1.0:
    dependencies:
      ansi-styles: 6.2.1
      string-width-cjs: string-width@4.2.3
      scoped-alias: '@scope/pkg@1.0.0'
  ansi-styles@6.2.1: {}
  string-width@4.2.3: {}
  '@scope/pkg@1.0.0': {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    // Every entry has to survive the parsers that read a deno.lock back in.
    let content = deno_lockfile::LockfileContent::from_json(
      serde_json::from_str(&out).unwrap(),
    )
    .unwrap();
    for pkg in content.packages.npm.values() {
      for dep in pkg.dependencies.values() {
        deno_npm::NpmPackageId::from_serialized(dep).unwrap();
      }
    }

    let deps = v["npm"]["wrap-ansi@8.1.0"]["dependencies"]
      .as_array()
      .unwrap();
    assert_eq!(
      deps.as_slice(),
      [
        "ansi-styles@6.2.1",
        "scoped-alias@npm:@scope/pkg@1.0.0",
        "string-width-cjs@npm:string-width@4.2.3",
      ]
    );
  }

  #[test]
  fn workspace_link_snapshot_dependency() {
    // A dependency satisfied by a workspace package is recorded as
    // `vite: link:packages/vite`. That has no deno.lock spelling, so it must
    // be left out rather than turned into `vite@link:packages/vite`.
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      some-plugin:
        specifier: ^1.0.0
        version: 1.0.0

packages:
  some-plugin@1.0.0:
    resolution: {integrity: sha512-PLUGIN}
  cac@7.0.0:
    resolution: {integrity: sha512-CAC}

snapshots:
  some-plugin@1.0.0:
    dependencies:
      cac: 7.0.0
    optionalDependencies:
      vite: link:packages/vite
  cac@7.0.0: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let plugin = &v["npm"]["some-plugin@1.0.0"];
    assert_eq!(
      plugin["dependencies"].as_array().unwrap().as_slice(),
      ["cac@7.0.0"]
    );
    // The only optional dependency was the workspace link.
    assert!(plugin.get("optionalDependencies").is_none());

    let content = deno_lockfile::LockfileContent::from_json(
      serde_json::from_str(&out).unwrap(),
    )
    .unwrap();
    for pkg in content.packages.npm.values() {
      for dep in pkg.dependencies.values() {
        deno_npm::NpmPackageId::from_serialized(dep).unwrap();
      }
    }
  }

  #[test]
  fn path_and_url_snapshot_dependencies() {
    // `normalize_package_key`'s `name/version` -> `name@version` fallback
    // fires on anything whose last `/` segment starts with a digit, which a
    // path or a tarball url can. Those have to be rejected before they reach
    // it, or they come back out dressed as aliases
    // (`tarball-dep@npm:file:vendor@1.0.0.tgz`) — ids that `LockfileContent`
    // accepts but `NpmPackageId` cannot parse.
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      some-plugin:
        specifier: ^1.0.0
        version: 1.0.0

packages:
  some-plugin@1.0.0:
    resolution: {integrity: sha512-PLUGIN}
  cac@7.0.0:
    resolution: {integrity: sha512-CAC}

snapshots:
  some-plugin@1.0.0:
    dependencies:
      cac: 7.0.0
      tarball-dep: file:vendor/1.0.0.tgz
      url-dep: https://host/pkg/1.2.3.tgz
      link-dep: link:packages/2fa-utils
      git-dep: github:owner/repo#1.0.0
  cac@7.0.0: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    // Only the one dependency that has a deno.lock spelling survives.
    assert_eq!(
      v["npm"]["some-plugin@1.0.0"]["dependencies"]
        .as_array()
        .unwrap()
        .as_slice(),
      ["cac@7.0.0"]
    );

    let content = deno_lockfile::LockfileContent::from_json(
      serde_json::from_str(&out).unwrap(),
    )
    .unwrap();
    for pkg in content.packages.npm.values() {
      for dep in pkg.dependencies.values() {
        deno_npm::NpmPackageId::from_serialized(dep).unwrap();
      }
    }
  }

  #[test]
  fn aliased_snapshot_dependency_with_peer_suffix() {
    // An alias can carry a peer suffix. It comes off before the value is
    // normalized, so the entry still resolves to the aliased package.
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      wrap-ansi:
        specifier: ^8.1.0
        version: 8.1.0

packages:
  wrap-ansi@8.1.0:
    resolution: {integrity: sha512-WRAP}
  string-width@4.2.3:
    resolution: {integrity: sha512-WIDTH}
  emoji-regex@8.0.0:
    resolution: {integrity: sha512-EMOJI}

snapshots:
  wrap-ansi@8.1.0:
    dependencies:
      string-width-cjs: string-width@4.2.3(emoji-regex@8.0.0)
  string-width@4.2.3: {}
  emoji-regex@8.0.0: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
      v["npm"]["wrap-ansi@8.1.0"]["dependencies"]
        .as_array()
        .unwrap()
        .as_slice(),
      ["string-width-cjs@npm:string-width@4.2.3"]
    );

    let content = deno_lockfile::LockfileContent::from_json(
      serde_json::from_str(&out).unwrap(),
    )
    .unwrap();
    for (key, pkg) in &content.packages.npm {
      for dep in pkg.dependencies.values() {
        deno_npm::NpmPackageId::from_serialized(dep).unwrap();
        assert!(
          content.packages.npm.contains_key(dep),
          "{key} depends on {dep}, which isn't in the lock"
        );
      }
    }
  }

  #[test]
  fn v6_slash_form_key_with_peer_suffix() {
    // pnpm v6 also wrote ids as `/name/version`. The peer suffix has to come
    // off before `normalize_package_key`, otherwise the `@` inside the parens
    // hides that form and the package is dropped, leaving `specifiers`
    // pointing at an id the `npm` section doesn't have.
    let input = r#"
lockfileVersion: '6.0'

specifiers:
  foo: ^1.0.0

dependencies:
  foo: 1.0.0(bar@2.0.0)

packages:
  /foo/1.0.0(bar@2.0.0):
    resolution: {integrity: sha512-FOO}
    dependencies:
      bar: /bar/2.0.0
  /bar/2.0.0:
    resolution: {integrity: sha512-BAR}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["npm"]["foo@1.0.0"]["integrity"], "sha512-FOO");
    assert_eq!(
      v["npm"]["foo@1.0.0"]["dependencies"]
        .as_array()
        .unwrap()
        .as_slice(),
      ["bar@2.0.0"]
    );
    assert_eq!(v["specifiers"]["npm:foo@^1.0.0"], "1.0.0");

    // Nothing dangles: every specifier and dependency names a package the
    // lock actually holds.
    let content = deno_lockfile::LockfileContent::from_json(
      serde_json::from_str(&out).unwrap(),
    )
    .unwrap();
    for (key, pkg) in &content.packages.npm {
      for dep in pkg.dependencies.values() {
        assert!(
          content.packages.npm.contains_key(dep),
          "{key} depends on {dep}, which isn't in the lock"
        );
      }
    }
  }

  #[test]
  fn v6_aliased_snapshot_dependency() {
    // pnpm v6 prefixes ids with `/` in dependency values too, so
    // `string-width-cjs: /string-width@4.2.3` has to end up pointing at the
    // same package as the `/string-width@4.2.3` key.
    let input = r#"
lockfileVersion: '6.0'

specifiers:
  '@isaacs/cliui': ^8.0.2

dependencies:
  '@isaacs/cliui': 8.0.2

packages:
  /@isaacs/cliui@8.0.2:
    resolution: {integrity: sha512-CLIUI}
    dependencies:
      string-width: 5.1.2
      string-width-cjs: /string-width@4.2.3
  /string-width@5.1.2:
    resolution: {integrity: sha512-W512}
  /string-width@4.2.3:
    resolution: {integrity: sha512-W423}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
      v["npm"]["@isaacs/cliui@8.0.2"]["dependencies"]
        .as_array()
        .unwrap()
        .as_slice(),
      [
        "string-width-cjs@npm:string-width@4.2.3",
        "string-width@5.1.2"
      ]
    );

    // Both point at packages the lock actually has.
    let content = deno_lockfile::LockfileContent::from_json(
      serde_json::from_str(&out).unwrap(),
    )
    .unwrap();
    for (key, pkg) in &content.packages.npm {
      for dep in pkg.dependencies.values() {
        assert!(
          content.packages.npm.contains_key(dep),
          "{key} depends on {dep}, which isn't in the lock"
        );
      }
    }
  }

  #[test]
  fn package_installed_from_a_path_is_left_out() {
    // A package keyed by where it came from has no npm id to record, so it
    // must not reach the npm section as `fake-pkg@file:…`.
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
    resolution: {integrity: sha512-LODASH}
  fake-pkg@file:vendor/fake-pkg-1.0.0.tgz:
    resolution: {integrity: sha512-FAKE, tarball: file:vendor/fake-pkg-1.0.0.tgz}

snapshots:
  lodash@4.17.21: {}
  fake-pkg@file:vendor/fake-pkg-1.0.0.tgz: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["npm"]["lodash@4.17.21"]["integrity"], "sha512-LODASH");
    assert_eq!(v["npm"].as_object().unwrap().len(), 1);

    let content = deno_lockfile::LockfileContent::from_json(
      serde_json::from_str(&out).unwrap(),
    )
    .unwrap();
    for key in content.packages.npm.keys() {
      deno_npm::NpmPackageId::from_serialized(key).unwrap();
    }
  }

  #[test]
  fn multi_document_lockfile() {
    // The packages that make up pnpm itself come first; the project's own
    // lockfile is the document after it.
    let input = r#"---
lockfileVersion: '9.0'

importers:

  .:
    configDependencies: {}
    packageManagerDependencies:
      pnpm:
        specifier: 11.10.0
        version: 11.10.0

packages:

  pnpm@11.10.0:
    resolution: {integrity: sha512-PNPM}
    hasBin: true

snapshots:

  pnpm@11.10.0: {}
---
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      lodash:
        specifier: ^4.17.21
        version: 4.17.21
      node:
        specifier: runtime:26.8.1
        version: runtime:26.8.1

packages:
  lodash@4.17.21:
    resolution: {integrity: sha512-LODASH}

snapshots:
  lodash@4.17.21: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:lodash@^4.17.21"], "4.17.21");
    assert_eq!(v["npm"]["lodash@4.17.21"]["integrity"], "sha512-LODASH");
    // pnpm's own packages are not the project's dependencies.
    assert!(v["npm"].get("pnpm@11.10.0").is_none());
    // A pinned runtime is not an npm package either.
    assert_eq!(v["specifiers"].as_object().unwrap().len(), 1);
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
  fn seeds_workspace_members() {
    // A monorepo with a root dep and a member dep: both end up in the flat
    // `specifiers` map, the root under `workspace.packageJson` and the member
    // under `workspace.members.<path>.packageJson`.
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      is-number:
        specifier: 7.0.0
        version: 7.0.0
  packages/app:
    dependencies:
      is-odd:
        specifier: 3.0.1
        version: 3.0.1

packages:
  is-number@7.0.0:
    resolution: {integrity: sha512-NUM}
  is-odd@3.0.1:
    resolution: {integrity: sha512-ODD}

snapshots:
  is-number@7.0.0: {}
  is-odd@3.0.1: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:is-number@7.0.0"], "7.0.0");
    assert_eq!(v["specifiers"]["npm:is-odd@3.0.1"], "3.0.1");
    // Root dep under the flattened workspace.packageJson.
    assert_eq!(
      v["workspace"]["packageJson"]["dependencies"][0],
      "npm:is-number@7.0.0"
    );
    // Member dep under workspace.members.<path>.packageJson.
    assert_eq!(
      v["workspace"]["members"]["packages/app"]["packageJson"]["dependencies"]
        [0],
      "npm:is-odd@3.0.1"
    );
  }

  #[test]
  fn resolves_default_catalog() {
    // A `catalog:` (default) specifier resolves to its real version
    // requirement via the top-level `catalogs:` block.
    let input = r#"
lockfileVersion: '9.0'

catalogs:
  default:
    is-odd:
      specifier: 3.0.1
      version: 3.0.1

importers:
  .:
    dependencies:
      is-odd:
        specifier: 'catalog:'
        version: 3.0.1

packages:
  is-odd@3.0.1:
    resolution: {integrity: sha512-ODD}

snapshots:
  is-odd@3.0.1: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:is-odd@3.0.1"], "3.0.1");
    assert_eq!(
      v["workspace"]["packageJson"]["dependencies"][0],
      "npm:is-odd@3.0.1"
    );
    assert_eq!(v["npm"]["is-odd@3.0.1"]["integrity"], "sha512-ODD");
  }

  #[test]
  fn resolves_named_catalog() {
    // A named `catalog:<name>` specifier resolves via the matching catalog.
    let input = r#"
lockfileVersion: '9.0'

catalogs:
  react18:
    react:
      specifier: ^18.0.0
      version: 18.3.1

importers:
  .:
    dependencies:
      react:
        specifier: 'catalog:react18'
        version: 18.3.1

packages:
  react@18.3.1:
    resolution: {integrity: sha512-REACT}

snapshots:
  react@18.3.1: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:react@^18.0.0"], "18.3.1");
  }

  #[test]
  fn member_via_catalog() {
    // A workspace member declaring a `catalog:` dep seeds correctly under
    // workspace.members.
    let input = r#"
lockfileVersion: '9.0'

catalogs:
  default:
    is-odd:
      specifier: 3.0.1
      version: 3.0.1

importers:
  .:
    dependencies:
      is-number:
        specifier: 7.0.0
        version: 7.0.0
  packages/app:
    dependencies:
      is-odd:
        specifier: 'catalog:'
        version: 3.0.1

packages:
  is-number@7.0.0:
    resolution: {integrity: sha512-NUM}
  is-odd@3.0.1:
    resolution: {integrity: sha512-ODD}

snapshots:
  is-number@7.0.0: {}
  is-odd@3.0.1: {}
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:is-odd@3.0.1"], "3.0.1");
    assert_eq!(
      v["workspace"]["members"]["packages/app"]["packageJson"]["dependencies"]
        [0],
      "npm:is-odd@3.0.1"
    );
  }

  #[test]
  fn skips_unknown_catalog_entry() {
    // A `catalog:` dep with no matching catalog entry is skipped, producing an
    // empty lockfile (so the caller can suppress the "Seeded" message).
    let input = r#"
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      is-odd:
        specifier: 'catalog:'
        version: 3.0.1
"#;
    let out = pnpm_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("specifiers").is_none());
    assert!(v.get("workspace").is_none());
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
