// Copyright 2018-2026 the Deno authors. MIT license.

//! Translation from yarn's `yarn.lock` to a deno.lock v5 JSON string.
//!
//! Supports yarn classic (v1) only. Yarn berry (v2+) uses YAML with
//! `checksum` fields that are typically xxhash rather than SRI-compatible
//! sha-512 digests, so translating berry lockfiles to deno.lock without
//! losing integrity information is not possible without re-fetching the
//! tarballs. Berry lockfiles are detected and rejected with a clear error.

use std::collections::BTreeMap;
use std::collections::HashMap;

use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum YarnLockfileImportError {
  #[error(
    "yarn berry (v2+) lockfiles are not supported. Run `yarn install` with yarn classic to produce a compatible lockfile."
  )]
  BerryUnsupported,
  #[error("yarn.lock is empty or could not be parsed")]
  Empty,
}

/// Convert a yarn v1 `yarn.lock` string into a deno.lock v5 JSON string. Only
/// the npm subset is populated.
pub fn yarn_lock_to_deno_lock_v5(
  text: &str,
) -> Result<String, YarnLockfileImportError> {
  if is_yarn_berry(text) {
    return Err(YarnLockfileImportError::BerryUnsupported);
  }

  let blocks = parse_yarn_v1(text);
  if blocks.is_empty() {
    return Err(YarnLockfileImportError::Empty);
  }

  // Map each pattern (`name@req`) to the resolved `(name, version)`.
  let mut pattern_to_resolved: HashMap<String, (String, String)> =
    HashMap::new();
  for block in &blocks {
    let Some(version) = block.version.as_deref() else {
      continue;
    };
    for pattern in &block.patterns {
      let Some((name, _req)) = split_spec(pattern) else {
        continue;
      };
      pattern_to_resolved
        .insert(pattern.clone(), (name.to_string(), version.to_string()));
    }
  }

  // Build the npm map keyed by `name@version`.
  let mut npm: BTreeMap<String, Value> = BTreeMap::new();
  for block in &blocks {
    let Some(version) = block.version.as_deref() else {
      continue;
    };
    let Some(integrity) = block.integrity.as_deref() else {
      continue;
    };
    // Determine the package name from the first pattern.
    let Some(first_pattern) = block.patterns.first() else {
      continue;
    };
    let Some((name, _)) = split_spec(first_pattern) else {
      continue;
    };

    let mut deps: Vec<String> = block
      .dependencies
      .iter()
      .filter_map(|(dep_name, req)| {
        resolve(&pattern_to_resolved, dep_name, req)
          .map(|(n, v)| format_dep_entry(dep_name, &n, &v))
      })
      .collect();
    deps.sort();
    deps.dedup();

    let mut opt_deps: Vec<String> = block
      .optional_dependencies
      .iter()
      .filter_map(|(dep_name, req)| {
        resolve(&pattern_to_resolved, dep_name, req)
          .map(|(n, v)| format_dep_entry(dep_name, &n, &v))
      })
      .collect();
    opt_deps.sort();
    opt_deps.dedup();

    let mut entry = serde_json::Map::new();
    entry.insert(
      "integrity".to_string(),
      Value::String(integrity.to_string()),
    );
    if !deps.is_empty() {
      entry.insert(
        "dependencies".to_string(),
        Value::Array(deps.into_iter().map(Value::String).collect()),
      );
    }
    if !opt_deps.is_empty() {
      entry.insert(
        "optionalDependencies".to_string(),
        Value::Array(opt_deps.into_iter().map(Value::String).collect()),
      );
    }
    npm.insert(format!("{}@{}", name, version), Value::Object(entry));
  }

  // Build specifiers from every pattern in the lockfile. Unused entries are
  // pruned during install.
  let mut specifiers: BTreeMap<String, String> = BTreeMap::new();
  for (pattern, (_name, version)) in &pattern_to_resolved {
    let Some((name, req)) = split_spec(pattern) else {
      continue;
    };
    if !is_supported_req(req) {
      continue;
    }
    specifiers
      .entry(format!("npm:{}@{}", name, req))
      .or_insert_with(|| version.clone());
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

fn resolve(
  map: &HashMap<String, (String, String)>,
  dep_name: &str,
  req: &str,
) -> Option<(String, String)> {
  let key = format!("{}@{}", dep_name, req);
  map.get(&key).cloned()
}

fn format_dep_entry(alias: &str, name: &str, version: &str) -> String {
  if alias == name {
    format!("{}@{}", name, version)
  } else {
    format!("{}@npm:{}@{}", alias, name, version)
  }
}

fn split_spec(spec: &str) -> Option<(&str, &str)> {
  let bytes = spec.as_bytes();
  if bytes.is_empty() {
    return None;
  }
  let start = if bytes[0] == b'@' { 1 } else { 0 };
  let idx = bytes[start..].iter().position(|&b| b == b'@')? + start;
  Some((&spec[..idx], &spec[idx + 1..]))
}

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
}

fn is_yarn_berry(text: &str) -> bool {
  text
    .lines()
    .take(40)
    .any(|l| l.trim_start().starts_with("__metadata:"))
}

#[derive(Debug)]
struct YarnV1Block {
  patterns: Vec<String>,
  version: Option<String>,
  integrity: Option<String>,
  dependencies: Vec<(String, String)>,
  optional_dependencies: Vec<(String, String)>,
}

fn parse_yarn_v1(text: &str) -> Vec<YarnV1Block> {
  let mut blocks: Vec<YarnV1Block> = Vec::new();
  let mut current: Option<YarnV1Block> = None;
  let mut sub_section: SubSection = SubSection::None;

  for raw_line in text.lines() {
    let line = raw_line.trim_end_matches('\r');
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
      continue;
    }
    let indent = line.len() - line.trim_start().len();

    if indent == 0 {
      if let Some(b) = current.take() {
        blocks.push(b);
      }
      let header = trimmed.trim_end_matches(':');
      current = Some(YarnV1Block {
        patterns: parse_block_header(header),
        version: None,
        integrity: None,
        dependencies: Vec::new(),
        optional_dependencies: Vec::new(),
      });
      sub_section = SubSection::None;
    } else if indent == 2 {
      let Some(b) = current.as_mut() else {
        continue;
      };
      if trimmed == "dependencies:" {
        sub_section = SubSection::Dependencies;
      } else if trimmed == "optionalDependencies:" {
        sub_section = SubSection::OptionalDependencies;
      } else {
        sub_section = SubSection::None;
        let (key, value) = split_key_value(trimmed);
        match key {
          "version" => b.version = Some(value.to_string()),
          "integrity" => b.integrity = Some(value.to_string()),
          _ => {}
        }
      }
    } else if indent >= 4 {
      let Some(b) = current.as_mut() else {
        continue;
      };
      let (key, value) = split_key_value(trimmed);
      match sub_section {
        SubSection::Dependencies => {
          b.dependencies.push((key.to_string(), value.to_string()));
        }
        SubSection::OptionalDependencies => {
          b.optional_dependencies
            .push((key.to_string(), value.to_string()));
        }
        SubSection::None => {}
      }
    }
  }
  if let Some(b) = current.take() {
    blocks.push(b);
  }
  blocks
}

#[derive(Debug, Clone, Copy)]
enum SubSection {
  None,
  Dependencies,
  OptionalDependencies,
}

/// Parse a yarn-v1 block header (the part before the trailing `:`) into its
/// comma-separated, optionally-quoted patterns.
fn parse_block_header(header: &str) -> Vec<String> {
  let mut patterns = Vec::new();
  let mut buf = String::new();
  let mut in_quotes = false;
  for c in header.chars() {
    match c {
      '"' => in_quotes = !in_quotes,
      ',' if !in_quotes => {
        let t = buf.trim().to_string();
        if !t.is_empty() {
          patterns.push(t);
        }
        buf.clear();
      }
      _ => buf.push(c),
    }
  }
  let t = buf.trim().to_string();
  if !t.is_empty() {
    patterns.push(t);
  }
  patterns
}

fn split_key_value(s: &str) -> (&str, &str) {
  let s = s.trim();
  let key_end = s
    .char_indices()
    .find(|(_, c)| c.is_whitespace())
    .map(|(i, _)| i)
    .unwrap_or(s.len());
  let key = &s[..key_end];
  let value = s[key_end..].trim();
  // The key may be quoted too (yarn quotes any package name containing `@` or
  // `/`, so scoped names like `"@babel/code-frame"` arrive quoted). Package
  // names never contain whitespace, so splitting on the first whitespace is
  // still correct for a quoted key.
  (unquote(key), unquote(value))
}

fn unquote(s: &str) -> &str {
  let s = s.trim();
  if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
    &s[1..s.len() - 1]
  } else {
    s
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = r#"# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
# yarn lockfile v1


ansi-styles@^4.1.0:
  version "4.3.0"
  resolved "https://registry.yarnpkg.com/ansi-styles/-/ansi-styles-4.3.0.tgz"
  integrity sha512-ANSI
  dependencies:
    color-convert "^2.0.1"

chalk@^4.0.0:
  version "4.1.2"
  resolved "https://registry.yarnpkg.com/chalk/-/chalk-4.1.2.tgz"
  integrity sha512-CHALK
  dependencies:
    ansi-styles "^4.1.0"
    supports-color "^7.1.0"

color-convert@^2.0.1:
  version "2.0.1"
  resolved "https://registry.yarnpkg.com/color-convert/-/color-convert-2.0.1.tgz"
  integrity sha512-CC
  dependencies:
    color-name "~1.1.4"

color-name@~1.1.4:
  version "1.1.4"
  resolved "https://registry.yarnpkg.com/color-name/-/color-name-1.1.4.tgz"
  integrity sha512-CN

has-flag@^4.0.0:
  version "4.0.0"
  resolved "https://registry.yarnpkg.com/has-flag/-/has-flag-4.0.0.tgz"
  integrity sha512-HF

supports-color@^7.1.0:
  version "7.2.0"
  resolved "https://registry.yarnpkg.com/supports-color/-/supports-color-7.2.0.tgz"
  integrity sha512-SC
  dependencies:
    has-flag "^4.0.0"
"#;

  #[test]
  fn translates_yarn_v1() {
    let out = yarn_lock_to_deno_lock_v5(SAMPLE).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["version"], "5");
    assert_eq!(v["specifiers"]["npm:chalk@^4.0.0"], "4.1.2");
    assert_eq!(v["npm"]["chalk@4.1.2"]["integrity"], "sha512-CHALK");
    let chalk_deps =
      v["npm"]["chalk@4.1.2"]["dependencies"].as_array().unwrap();
    assert!(chalk_deps.iter().any(|d| d == "ansi-styles@4.3.0"));
    assert!(chalk_deps.iter().any(|d| d == "supports-color@7.2.0"));
  }

  #[test]
  fn scoped_patterns() {
    let input = r#"# yarn lockfile v1

"@scope/pkg@^1.0.0":
  version "1.2.3"
  resolved "https://registry.yarnpkg.com/@scope/pkg/-/pkg-1.2.3.tgz"
  integrity sha512-SP
"#;
    let out = yarn_lock_to_deno_lock_v5(input).unwrap();
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
  fn scoped_dependency_entry() {
    // A scoped package name appears quoted in a `dependencies:` block. The
    // quotes must be stripped from the key so the dep resolves to its locked
    // version (otherwise the edge is silently dropped).
    let input = r#"# yarn lockfile v1

"@babel/code-frame@^7.0.0":
  version "7.0.0"
  resolved "https://registry.yarnpkg.com/@babel/code-frame/-/code-frame-7.0.0.tgz"
  integrity sha512-CF

pkg@^1.0.0:
  version "1.0.0"
  resolved "https://registry.yarnpkg.com/pkg/-/pkg-1.0.0.tgz"
  integrity sha512-PKG
  dependencies:
    "@babel/code-frame" "^7.0.0"
"#;
    let out = yarn_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let pkg_deps = v["npm"]["pkg@1.0.0"]["dependencies"].as_array().unwrap();
    assert!(pkg_deps.iter().any(|d| d == "@babel/code-frame@7.0.0"));
  }

  #[test]
  fn multi_pattern_header() {
    let input = r#"# yarn lockfile v1

"chalk@^4.0.0", "chalk@^4.1.0":
  version "4.1.2"
  resolved "..."
  integrity sha512-CHALK
"#;
    let out = yarn_lock_to_deno_lock_v5(input).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["specifiers"]["npm:chalk@^4.0.0"], "4.1.2");
    assert_eq!(v["specifiers"]["npm:chalk@^4.1.0"], "4.1.2");
  }

  #[test]
  fn rejects_berry() {
    let input = r#"# This file is generated by running "yarn install"

__metadata:
  version: 8

"chalk@npm:^4.0.0":
  version: 4.1.2
"#;
    let err = yarn_lock_to_deno_lock_v5(input).unwrap_err();
    assert!(matches!(err, YarnLockfileImportError::BerryUnsupported));
  }
}
