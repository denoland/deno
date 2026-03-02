// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use deno_package_json::parse_jsr_dep_value;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_semver::package::PackageName;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NpmOverridesError {
  #[error("Failed to parse override key \"{key}\": {source}")]
  KeyParse {
    key: String,
    source: deno_semver::npm::NpmVersionReqParseError,
  },
  #[error(
    "Failed to parse override value \"{value}\" for key \"{key}\": {source}"
  )]
  ValueParse {
    key: String,
    value: String,
    source: deno_semver::npm::NpmVersionReqParseError,
  },
  #[error(
    "Override uses dollar reference \"${reference}\" but \"{reference}\" is not a direct dependency of the root package"
  )]
  UnresolvedDollarReference { reference: String },
  #[error(
    "Invalid override value type for key \"{key}\": expected a string or object"
  )]
  InvalidValueType { key: String },
  #[error(
    "Invalid value type for \".\" key in override \"{key}\": expected a string"
  )]
  InvalidDotValueType { key: String },
  #[error("Invalid \"overrides\" field in package.json: expected an object")]
  InvalidTopLevelType,
  #[error(
    "jsr: override \"{value}\" for key \"{key}\" requires a scoped package name (e.g. jsr:@scope/name)"
  )]
  JsrRequiresScope { key: String, value: String },
}

/// The value an override resolves to.
#[derive(Debug, Clone)]
pub enum NpmOverrideValue {
  /// A version requirement, e.g. "1.0.0" or "^2.0.0".
  Version(VersionReq),
  /// An npm alias override, e.g. "npm:other-package@^1.0.0".
  /// Replaces the dependency with a different package.
  Alias {
    package: PackageName,
    version_req: VersionReq,
  },
  /// No self-override (only child overrides exist). This corresponds to the
  /// case where the override key maps to an object without a "." key.
  Inherited,
  /// The override was explicitly removed (empty string `""`). This cancels
  /// any outer override for the same package within a scoped context.
  Removed,
}

/// A single override rule parsed from the root package.json "overrides" field.
///
/// Represents entries like:
/// - `"foo": "1.0.0"` (simple, no selector, no children)
/// - `"foo@^2.0.0": { ".": "2.1.0", "bar": "3.0.0" }` (selector + dot + children)
#[derive(Debug, Clone)]
pub struct NpmOverrideRule {
  /// The package name this override targets.
  pub name: PackageName,
  /// Optional version range selector on the key (e.g. `^2.0.0` from
  /// `foo@^2.0.0`). When present, the override only applies when the
  /// resolved version satisfies this range.
  pub selector: Option<VersionReq>,
  /// The override value for this package itself.
  pub value: NpmOverrideValue,
  /// Nested overrides that apply within this package's dependency subtree.
  pub children: Vec<Arc<NpmOverrideRule>>,
}

/// Top-level parsed overrides from the root package.json.
#[derive(Debug, Clone, Default)]
pub struct NpmOverrides {
  pub rules: Vec<Arc<NpmOverrideRule>>,
}

impl NpmOverrides {
  /// Parses overrides from the raw JSON "overrides" object in package.json.
  ///
  /// `root_deps` should contain all direct dependencies of the root package
  /// (merged from `dependencies`, `devDependencies`, etc.) mapping bare
  /// specifier to version string. This is used to resolve `$pkg` references.
  pub fn from_value(
    value: serde_json::Value,
    root_deps: &HashMap<PackageName, StackString>,
  ) -> Result<Self, NpmOverridesError> {
    match value {
      serde_json::Value::Object(map) => {
        let rules = parse_override_rules(&map, root_deps)?;
        Ok(Self { rules })
      }
      serde_json::Value::Null => Ok(Self::default()),
      _ => Err(NpmOverridesError::InvalidTopLevelType),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.rules.is_empty()
  }

  /// Computes the active overrides for a child node in the dependency tree.
  ///
  /// When descending into `child_name@child_version`:
  /// - Rules that target `child_name` and whose selector matches: their
  ///   `children` become active for this subtree. Scoped children are placed
  ///   before passthrough rules so they take precedence in lookups.
  /// - Rules that target other packages (either simple or scoped): pass
  ///   through unchanged so they can match deeper descendants.
  pub fn for_child(
    self: &Rc<Self>,
    child_name: &PackageName,
    child_version: &Version,
  ) -> Rc<Self> {
    if self.rules.is_empty() {
      return self.clone();
    }

    let mut scoped_children: Vec<Arc<NpmOverrideRule>> = Vec::new();
    let mut passthrough_rules: Vec<Arc<NpmOverrideRule>> = Vec::new();
    let mut changed = false;

    for rule in self.rules.iter() {
      if rule.name == *child_name {
        let selector_matches = match &rule.selector {
          Some(selector) => selector.matches(child_version),
          None => true,
        };

        if selector_matches && !rule.children.is_empty() {
          // this rule targets the current child and matches; activate its
          // nested overrides for this subtree
          scoped_children.extend(rule.children.iter().cloned());
        }
        if rule.children.is_empty()
          || (rule.selector.is_some() && !selector_matches)
        {
          // pass through when: the rule is a simple childless override
          // (so it keeps applying deeper), OR the rule has a selector
          // that didn't match this version (so it can still match a
          // deeper occurrence at a different version)
          passthrough_rules.push(rule.clone());
        }
        changed = true;
      } else {
        // this rule targets a different package — keep it active for
        // deeper descendants
        passthrough_rules.push(rule.clone());
      }
    }

    if !changed {
      self.clone()
    } else {
      // scoped children first so they take precedence over passthrough rules
      scoped_children.extend(passthrough_rules);
      Rc::new(Self {
        rules: scoped_children,
      })
    }
  }

  /// Looks up an override for a dependency with the given name.
  ///
  /// When `resolved_version` is `None`, only returns overrides without
  /// selectors (unconditional overrides). When `resolved_version` is
  /// `Some`, also checks selector-based overrides against the version.
  ///
  /// Returns the overridden `VersionReq` if an override applies, or `None`
  /// if the dependency should use its original version requirement.
  /// A `Removed` rule causes an immediate `None` return, cancelling any
  /// further override for this dependency.
  pub fn get_override_for(
    &self,
    dep_name: &PackageName,
    resolved_version: Option<&Version>,
  ) -> Option<&VersionReq> {
    for rule in self.rules.iter() {
      if rule.name != *dep_name {
        continue;
      }
      match &rule.value {
        NpmOverrideValue::Version(req)
        | NpmOverrideValue::Alias {
          version_req: req, ..
        } => {
          match &rule.selector {
            None => return Some(req),
            Some(selector) => {
              if let Some(version) = resolved_version
                && selector.matches(version)
              {
                return Some(req);
              }
              // has selector but no version provided or version doesn't
              // match — skip this rule
            }
          }
        }
        NpmOverrideValue::Removed => match &rule.selector {
          None => return None,
          Some(selector) => {
            if let Some(version) = resolved_version
              && selector.matches(version)
            {
              return None;
            }
            // has selector but no version provided or version doesn't
            // match — skip this removal rule
          }
        },
        NpmOverrideValue::Inherited => {
          // no self-override, only children — skip
        }
      }
    }
    None
  }

  /// Returns the replacement package name if an unconditional alias override
  /// matches this dependency. Used by the graph resolver to fetch the correct
  /// package info before resolution.
  pub fn get_alias_for(&self, dep_name: &PackageName) -> Option<&PackageName> {
    for rule in self.rules.iter() {
      if rule.name != *dep_name || rule.selector.is_some() {
        continue;
      }
      return match &rule.value {
        NpmOverrideValue::Alias { package, .. } => Some(package),
        _ => None,
      };
    }
    None
  }
}

/// Parses a key from the overrides map. The key can be either:
/// - `"package-name"` → (name, None)
/// - `"package-name@version-req"` → (name, Some(version_req))
fn parse_override_key(
  key: &str,
) -> Result<(PackageName, Option<VersionReq>), NpmOverridesError> {
  // handle scoped packages: "@scope/name" or "@scope/name@version"
  let at_index = if let Some(rest) = key.strip_prefix('@') {
    // scoped package — find the second '@' if it exists
    rest.find('@').map(|i| i + 1)
  } else {
    key.find('@')
  };

  match at_index {
    Some(idx) => {
      let name = &key[..idx];
      let version_text = &key[idx + 1..];
      let version_req =
        VersionReq::parse_from_npm(version_text).map_err(|source| {
          NpmOverridesError::KeyParse {
            key: key.to_string(),
            source,
          }
        })?;
      Ok((PackageName::from_str(name), Some(version_req)))
    }
    None => Ok((PackageName::from_str(key), None)),
  }
}

/// Parses an override value, which can be:
/// - A string: version requirement, `$pkg` reference, or `""` (removal)
/// - An object: nested overrides (with optional "." key for self-override)
fn parse_override_value(
  key: &str,
  value: &serde_json::Value,
  root_deps: &HashMap<PackageName, StackString>,
) -> Result<(NpmOverrideValue, Vec<Arc<NpmOverrideRule>>), NpmOverridesError> {
  match value {
    serde_json::Value::String(s) => {
      let value = parse_override_string(key, s, root_deps)?;
      Ok((value, Vec::new()))
    }
    serde_json::Value::Object(map) => {
      let mut self_value = NpmOverrideValue::Inherited;
      let mut children = Vec::new();

      for (child_key, child_value) in map {
        if child_key == "." {
          // the "." key overrides the package itself
          match child_value {
            serde_json::Value::String(s) => {
              self_value = parse_override_string(key, s, root_deps)?;
            }
            _ => {
              return Err(NpmOverridesError::InvalidDotValueType {
                key: key.to_string(),
              });
            }
          }
        } else {
          let (child_name, child_selector) = parse_override_key(child_key)?;
          let (child_val, grandchildren) =
            parse_override_value(child_key, child_value, root_deps)?;
          children.push(Arc::new(NpmOverrideRule {
            name: child_name,
            selector: child_selector,
            value: child_val,
            children: grandchildren,
          }));
        }
      }

      Ok((self_value, children))
    }
    _ => Err(NpmOverridesError::InvalidValueType {
      key: key.to_string(),
    }),
  }
}

/// Parses an override string value, handling empty strings, `npm:` aliases,
/// `$pkg` dollar references, and plain version requirements.
fn parse_override_string(
  key: &str,
  s: &str,
  root_deps: &HashMap<PackageName, StackString>,
) -> Result<NpmOverrideValue, NpmOverridesError> {
  if s.is_empty() {
    Ok(NpmOverrideValue::Removed)
  } else if let Some(rest) = s.strip_prefix("npm:") {
    parse_npm_alias_override(key, rest)
  } else if let Some(rest) = s.strip_prefix("jsr:") {
    parse_jsr_override(key, rest)
  } else {
    let version_req = resolve_override_version_string(key, s, root_deps)?;
    Ok(NpmOverrideValue::Version(version_req))
  }
}

/// Parses an `npm:package@version` alias value into an `Alias` override.
fn parse_npm_alias_override(
  key: &str,
  npm_value: &str,
) -> Result<NpmOverrideValue, NpmOverridesError> {
  let (name, version_str) =
    if let Some((name, version)) = npm_value.rsplit_once('@') {
      if name.is_empty() {
        // scoped package without version: "npm:@scope/package"
        (npm_value, "*")
      } else {
        (name, version)
      }
    } else {
      (npm_value, "*")
    };

  let version_req =
    VersionReq::parse_from_npm(version_str).map_err(|source| {
      NpmOverridesError::ValueParse {
        key: key.to_string(),
        value: format!("npm:{npm_value}"),
        source,
      }
    })?;

  Ok(NpmOverrideValue::Alias {
    package: PackageName::from_str(name),
    version_req,
  })
}

/// Parses a `jsr:` override value into an `Alias` override.
///
/// JSR packages are mapped to npm via the `@jsr/scope__name` convention.
/// Two forms are supported:
/// - Explicit package: `jsr:@scope/name@version` (e.g. `jsr:@std/path@^1.0.0`)
/// - Version only: `jsr:^1` — derives the package name from the override key
///   (e.g. key `@std/path` with value `jsr:1` → `@jsr/std__path` @ `1`)
fn parse_jsr_override(
  key: &str,
  jsr_value: &str,
) -> Result<NpmOverrideValue, NpmOverridesError> {
  // When the value is a bare version (doesn't start with '@'), derive
  // the package name from the key. The key may include a version selector
  // (e.g. "@std/path@^1.0.0"), so strip that first.
  let fallback_name = if let Some(rest) = key.strip_prefix('@') {
    match rest.find('@') {
      Some(idx) => &key[..idx + 1],
      None => key,
    }
  } else {
    match key.find('@') {
      Some(idx) => &key[..idx],
      None => key,
    }
  };

  let (npm_name, version_str) = parse_jsr_dep_value(fallback_name, jsr_value)
    .map_err(|_| {
    NpmOverridesError::JsrRequiresScope {
      key: key.to_string(),
      value: format!("jsr:{jsr_value}"),
    }
  })?;

  let version_req =
    VersionReq::parse_from_npm(version_str).map_err(|source| {
      NpmOverridesError::ValueParse {
        key: key.to_string(),
        value: format!("jsr:{jsr_value}"),
        source,
      }
    })?;

  Ok(NpmOverrideValue::Alias {
    package: PackageName::from_str(&npm_name),
    version_req,
  })
}

/// Resolves a version string value, handling `$pkg` dollar references.
fn resolve_override_version_string(
  key: &str,
  value: &str,
  root_deps: &HashMap<PackageName, StackString>,
) -> Result<VersionReq, NpmOverridesError> {
  if let Some(ref_name) = value.strip_prefix('$') {
    // dollar reference: look up the root dependency's version
    let dep_version = root_deps.get(ref_name).ok_or_else(|| {
      NpmOverridesError::UnresolvedDollarReference {
        reference: ref_name.to_string(),
      }
    })?;
    VersionReq::parse_from_npm(dep_version).map_err(|source| {
      NpmOverridesError::ValueParse {
        key: key.to_string(),
        value: dep_version.to_string(),
        source,
      }
    })
  } else {
    VersionReq::parse_from_npm(value).map_err(|source| {
      NpmOverridesError::ValueParse {
        key: key.to_string(),
        value: value.to_string(),
        source,
      }
    })
  }
}

/// Parses the top-level override rules from a JSON object.
fn parse_override_rules(
  map: &serde_json::Map<String, serde_json::Value>,
  root_deps: &HashMap<PackageName, StackString>,
) -> Result<Vec<Arc<NpmOverrideRule>>, NpmOverridesError> {
  let mut rules = Vec::with_capacity(map.len());
  for (key, value) in map {
    let (name, selector) = parse_override_key(key)?;
    let (override_value, children) =
      parse_override_value(key, value, root_deps)?;
    rules.push(Arc::new(NpmOverrideRule {
      name,
      selector,
      value: override_value,
      children,
    }));
  }
  Ok(rules)
}

#[cfg(test)]
mod test {
  use super::*;

  fn empty_root_deps() -> HashMap<StackString, StackString> {
    HashMap::new()
  }

  #[test]
  fn parse_simple_override() {
    let raw = serde_json::json!({
      "foo": "1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "foo");
    assert!(rule.selector.is_none());
    assert!(rule.children.is_empty());
    match &rule.value {
      NpmOverrideValue::Version(req) => {
        assert_eq!(req.version_text(), "1.0.0");
      }
      _ => panic!("expected Version"),
    }
  }

  #[test]
  fn parse_override_with_version_selector() {
    let raw = serde_json::json!({
      "foo@^2.0.0": "2.1.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "foo");
    assert!(rule.selector.is_some());
    assert_eq!(rule.selector.as_ref().unwrap().version_text(), "^2.0.0");
    match &rule.value {
      NpmOverrideValue::Version(req) => {
        assert_eq!(req.version_text(), "2.1.0");
      }
      _ => panic!("expected Version"),
    }
  }

  #[test]
  fn parse_scoped_package_override() {
    let raw = serde_json::json!({
      "@scope/pkg": "3.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "@scope/pkg");
    assert!(rule.selector.is_none());
  }

  #[test]
  fn parse_scoped_package_with_selector() {
    let raw = serde_json::json!({
      "@scope/pkg@^1.0.0": "1.2.3"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "@scope/pkg");
    assert_eq!(rule.selector.as_ref().unwrap().version_text(), "^1.0.0");
  }

  #[test]
  fn parse_nested_override() {
    let raw = serde_json::json!({
      "foo": {
        "bar": "2.0.0"
      }
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "foo");
    assert!(matches!(rule.value, NpmOverrideValue::Inherited));
    assert_eq!(rule.children.len(), 1);
    assert_eq!(rule.children[0].name.as_str(), "bar");
    match &rule.children[0].value {
      NpmOverrideValue::Version(req) => {
        assert_eq!(req.version_text(), "2.0.0");
      }
      _ => panic!("expected Version"),
    }
  }

  #[test]
  fn parse_nested_with_dot_key() {
    let raw = serde_json::json!({
      "foo@2.x": {
        ".": "2.1.0",
        "bar": "3.0.0"
      }
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "foo");
    assert!(rule.selector.is_some());
    match &rule.value {
      NpmOverrideValue::Version(req) => {
        assert_eq!(req.version_text(), "2.1.0");
      }
      _ => panic!("expected Version from dot key"),
    }
    assert_eq!(rule.children.len(), 1);
    assert_eq!(rule.children[0].name.as_str(), "bar");
  }

  #[test]
  fn parse_dollar_reference() {
    let mut root_deps = HashMap::new();
    root_deps.insert(PackageName::from("foo"), StackString::from("^1.0.0"));
    let raw = serde_json::json!({
      "bar": "$foo"
    });
    let overrides = NpmOverrides::from_value(raw, &root_deps).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "bar");
    match &rule.value {
      NpmOverrideValue::Version(req) => {
        assert_eq!(req.version_text(), "^1.0.0");
      }
      _ => panic!("expected Version from dollar reference"),
    }
  }

  #[test]
  fn parse_dollar_reference_unresolved() {
    let raw = serde_json::json!({
      "bar": "$nonexistent"
    });
    let result = NpmOverrides::from_value(raw, &empty_root_deps());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("nonexistent"));
  }

  #[test]
  fn parse_empty_overrides() {
    let raw = serde_json::json!({});
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert!(overrides.is_empty());
  }

  #[test]
  fn parse_null_overrides() {
    let raw = serde_json::Value::Null;
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert!(overrides.is_empty());
  }

  #[test]
  fn parse_invalid_top_level_type() {
    let raw = serde_json::json!(42);
    let result = NpmOverrides::from_value(raw, &empty_root_deps());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("expected an object"));
  }

  #[test]
  fn parse_empty_string_override() {
    let raw = serde_json::json!({
      "foo": ""
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    assert!(matches!(
      overrides.rules[0].value,
      NpmOverrideValue::Removed
    ));
  }

  #[test]
  fn parse_empty_string_dot_key() {
    let raw = serde_json::json!({
      "foo": {
        ".": "",
        "bar": "2.0.0"
      }
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    assert!(matches!(
      overrides.rules[0].value,
      NpmOverrideValue::Removed
    ));
    assert_eq!(overrides.rules[0].children.len(), 1);
  }

  #[test]
  fn parse_invalid_dot_value_type() {
    let raw = serde_json::json!({
      "foo": {
        ".": 42,
        "bar": "2.0.0"
      }
    });
    let result = NpmOverrides::from_value(raw, &empty_root_deps());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("\".\""));
    assert!(err.contains("expected a string"));
  }

  #[test]
  fn overrides_empty() {
    let overrides = NpmOverrides::default();
    assert!(overrides.is_empty());
    assert!(
      overrides
        .get_override_for(&PackageName::from_str("foo"), None)
        .is_none()
    );
  }

  #[test]
  fn overrides_simple_global() {
    let raw = serde_json::json!({
      "foo": "1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();

    // should find override for "foo" (no selector, so None version works)
    let result =
      overrides.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    // should not find override for "bar"
    assert!(
      overrides
        .get_override_for(&PackageName::from_str("bar"), None)
        .is_none()
    );
  }

  #[test]
  fn overrides_for_child_passthrough() {
    // global override should pass through to child contexts (for non-matching packages)
    let raw = serde_json::json!({
      "foo": "1.0.0"
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // descend into "bar@2.0.0" — "foo" override should still be active
    let child = overrides.for_child(
      &PackageName::from_str("bar"),
      &Version::parse_from_npm("2.0.0").unwrap(),
    );
    let result = child.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");
  }

  #[test]
  fn overrides_for_child_simple_passthrough() {
    // a simple (childless) override for "foo" should pass through when
    // descending into "foo" so it keeps applying to deeper re-entrant deps
    let raw = serde_json::json!({
      "foo": "1.0.0"
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // descend into "foo@1.0.0" — the override should still be active
    let child = overrides.for_child(
      &PackageName::from_str("foo"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    let result = child.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");
  }

  #[test]
  fn overrides_for_child_scoped_consumed() {
    // a scoped override (with children) should be consumed when descending
    // into the targeted node, activating its children instead
    let raw = serde_json::json!({
      "foo": {
        "bar": "2.0.0"
      }
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // descend into "foo@1.0.0" — the "foo" rule is consumed, "bar" activated
    let child = overrides.for_child(
      &PackageName::from_str("foo"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    assert!(
      child
        .get_override_for(&PackageName::from_str("foo"), None)
        .is_none()
    );
    let result = child.get_override_for(&PackageName::from_str("bar"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "2.0.0");
  }

  #[test]
  fn overrides_scoped() {
    // scoped override: "parent": { "child": "2.0.0" }
    let raw = serde_json::json!({
      "parent": {
        "child": "2.0.0"
      }
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // at the top level, there's no override for "child"
    assert!(
      overrides
        .get_override_for(&PackageName::from_str("child"), None)
        .is_none()
    );

    // descend into "parent@1.0.0" — child overrides should become active
    let inside_parent = overrides.for_child(
      &PackageName::from_str("parent"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    let result =
      inside_parent.get_override_for(&PackageName::from_str("child"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "2.0.0");

    // descend into "other@1.0.0" — "child" override should NOT be active
    let inside_other = overrides.for_child(
      &PackageName::from_str("other"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    assert!(
      inside_other
        .get_override_for(&PackageName::from_str("child"), None)
        .is_none()
    );
  }

  #[test]
  fn overrides_selector_match() {
    // override with version selector: "foo@^2.0.0": { "bar": "3.0.0" }
    let raw = serde_json::json!({
      "foo@^2.0.0": {
        "bar": "3.0.0"
      }
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // descend into "foo@2.1.0" (matches ^2.0.0) — bar override should be active
    let matching = overrides.for_child(
      &PackageName::from_str("foo"),
      &Version::parse_from_npm("2.1.0").unwrap(),
    );
    let result = matching.get_override_for(&PackageName::from_str("bar"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "3.0.0");

    // descend into "foo@1.0.0" (does NOT match ^2.0.0) — bar override should NOT be active
    let non_matching = overrides.for_child(
      &PackageName::from_str("foo"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    assert!(
      non_matching
        .get_override_for(&PackageName::from_str("bar"), None)
        .is_none()
    );

    // but the rule should still be alive for a deeper foo@2.x occurrence
    // (e.g. foo@1.0.0 → baz → foo@2.1.0)
    let inside_baz = non_matching.for_child(
      &PackageName::from_str("baz"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    let deeper_match = inside_baz.for_child(
      &PackageName::from_str("foo"),
      &Version::parse_from_npm("2.1.0").unwrap(),
    );
    let result =
      deeper_match.get_override_for(&PackageName::from_str("bar"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "3.0.0");
  }

  #[test]
  fn overrides_dot_key_with_selector() {
    // override with "." key and selector:
    // "foo@^2.0.0": { ".": "2.1.0", "bar": "3.0.0" }
    let raw = serde_json::json!({
      "foo@^2.0.0": {
        ".": "2.1.0",
        "bar": "3.0.0"
      }
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();

    // without a resolved version, the selector-based override is not returned
    assert!(
      overrides
        .get_override_for(&PackageName::from_str("foo"), None)
        .is_none()
    );

    // with a matching version, the "." override is returned
    let result = overrides.get_override_for(
      &PackageName::from_str("foo"),
      Some(&Version::parse_from_npm("2.5.0").unwrap()),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "2.1.0");

    // with a non-matching version, the override is not returned
    assert!(
      overrides
        .get_override_for(
          &PackageName::from_str("foo"),
          Some(&Version::parse_from_npm("1.0.0").unwrap()),
        )
        .is_none()
    );
  }

  #[test]
  fn overrides_selector_on_direct_value() {
    // "foo@^2.0.0": "2.1.0" — selector on a direct version override
    let raw = serde_json::json!({
      "foo@^2.0.0": "2.1.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();

    // without version: not returned (has selector)
    assert!(
      overrides
        .get_override_for(&PackageName::from_str("foo"), None)
        .is_none()
    );

    // matching version: returned
    let result = overrides.get_override_for(
      &PackageName::from_str("foo"),
      Some(&Version::parse_from_npm("2.3.0").unwrap()),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "2.1.0");

    // non-matching version: not returned
    assert!(
      overrides
        .get_override_for(
          &PackageName::from_str("foo"),
          Some(&Version::parse_from_npm("1.5.0").unwrap()),
        )
        .is_none()
    );
  }

  #[test]
  fn for_child_scoped_removal_cancels_override() {
    // "foo": "1.0.0" (global) and "parent": { "foo": "" } (scoped removal)
    // after entering "parent", the scoped removal should take precedence
    let raw = serde_json::json!({
      "foo": "1.0.0",
      "parent": {
        "foo": ""
      }
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // at top level, "foo" override is active
    let result =
      overrides.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    // enter "parent@1.0.0" — the scoped removal should cancel the global override
    let inside_parent = overrides.for_child(
      &PackageName::from_str("parent"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    assert!(
      inside_parent
        .get_override_for(&PackageName::from_str("foo"), None)
        .is_none()
    );
  }

  #[test]
  fn overrides_removed_with_selector() {
    // "parent": { "foo@^2.0.0": "" } should only cancel the global "foo"
    // override when the resolved version matches ^2.0.0
    let raw = serde_json::json!({
      "foo": "1.0.0",
      "parent": {
        "foo@^2.0.0": ""
      }
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    let inside_parent = overrides.for_child(
      &PackageName::from_str("parent"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );

    // no resolved version — selector can't match, so Removed is skipped,
    // falls through to the unconditional Version rule
    let result =
      inside_parent.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    // resolved version 1.5.0 — doesn't match ^2.0.0, so Removed is skipped
    let result = inside_parent.get_override_for(
      &PackageName::from_str("foo"),
      Some(&Version::parse_from_npm("1.5.0").unwrap()),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    // resolved version 2.1.0 — matches ^2.0.0, so Removed cancels the override
    let result = inside_parent.get_override_for(
      &PackageName::from_str("foo"),
      Some(&Version::parse_from_npm("2.1.0").unwrap()),
    );
    assert!(result.is_none());
  }

  #[test]
  fn overrides_first_match_precedence() {
    // when multiple rules target the same package, the first match wins
    let raw = serde_json::json!({
      "foo": "1.0.0",
      "foo@^2.0.0": "2.5.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();

    // the unconditional "foo": "1.0.0" is first, so it always wins
    let result =
      overrides.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    // even with a version that matches ^2.0.0, the first rule still wins
    let result = overrides.get_override_for(
      &PackageName::from_str("foo"),
      Some(&Version::parse_from_npm("2.1.0").unwrap()),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");
  }

  #[test]
  fn overrides_first_match_selector_then_unconditional() {
    // reversed order: selector-based rule first, unconditional second
    let raw = serde_json::json!({
      "foo@^2.0.0": "2.5.0",
      "foo": "1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();

    // without resolved version, selector can't match — falls through to
    // the unconditional rule
    let result =
      overrides.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    // with matching version, the selector-based rule matches first
    let result = overrides.get_override_for(
      &PackageName::from_str("foo"),
      Some(&Version::parse_from_npm("2.1.0").unwrap()),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "2.5.0");

    // with non-matching version, selector skipped, unconditional wins
    let result = overrides.get_override_for(
      &PackageName::from_str("foo"),
      Some(&Version::parse_from_npm("1.5.0").unwrap()),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");
  }

  #[test]
  fn overrides_scoped_package_nested() {
    // scoped packages in nested/behavioral tests
    let raw = serde_json::json!({
      "@scope/parent": {
        "@scope/child": "2.0.0"
      }
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // at top level, no override for @scope/child
    assert!(
      overrides
        .get_override_for(&PackageName::from_str("@scope/child"), None)
        .is_none()
    );

    // descend into @scope/parent@1.0.0 — @scope/child override activates
    let inside_parent = overrides.for_child(
      &PackageName::from_str("@scope/parent"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    let result = inside_parent
      .get_override_for(&PackageName::from_str("@scope/child"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "2.0.0");

    // descend into unrelated @other/pkg — @scope/child override not active
    let inside_other = overrides.for_child(
      &PackageName::from_str("@other/pkg"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    assert!(
      inside_other
        .get_override_for(&PackageName::from_str("@scope/child"), None)
        .is_none()
    );
  }

  #[test]
  fn overrides_scoped_package_with_selector_nested() {
    // scoped parent with version selector
    let raw = serde_json::json!({
      "@scope/parent@^2.0.0": {
        "@scope/child": "3.0.0"
      }
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // matching version — children activate
    let matching = overrides.for_child(
      &PackageName::from_str("@scope/parent"),
      &Version::parse_from_npm("2.1.0").unwrap(),
    );
    let result =
      matching.get_override_for(&PackageName::from_str("@scope/child"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "3.0.0");

    // non-matching version — children do not activate
    let non_matching = overrides.for_child(
      &PackageName::from_str("@scope/parent"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    assert!(
      non_matching
        .get_override_for(&PackageName::from_str("@scope/child"), None)
        .is_none()
    );
  }

  #[test]
  fn parse_npm_alias_override_simple() {
    let raw = serde_json::json!({
      "foo": "npm:bar@1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "foo");
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "bar");
        assert_eq!(version_req.version_text(), "1.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_npm_alias_override_scoped() {
    let raw = serde_json::json!({
      "foo": "npm:@scope/bar@^2.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@scope/bar");
        assert_eq!(version_req.version_text(), "^2.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_npm_alias_override_no_version() {
    let raw = serde_json::json!({
      "foo": "npm:bar"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "bar");
        assert_eq!(version_req.version_text(), "*");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_npm_alias_scoped_no_version() {
    // "npm:@scope/bar" — scoped package without version
    let raw = serde_json::json!({
      "foo": "npm:@scope/bar"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@scope/bar");
        assert_eq!(version_req.version_text(), "*");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_npm_alias_dot_key() {
    // alias in the "." key of an object override
    let raw = serde_json::json!({
      "foo": {
        ".": "npm:bar@1.0.0",
        "baz": "2.0.0"
      }
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "bar");
        assert_eq!(version_req.version_text(), "1.0.0");
      }
      _ => panic!("expected Alias from dot key"),
    }
    assert_eq!(rule.children.len(), 1);
  }

  #[test]
  fn overrides_alias_get_override_for() {
    // get_override_for returns the alias's version_req
    let raw = serde_json::json!({
      "foo": "npm:bar@^1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let result =
      overrides.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "^1.0.0");
  }

  #[test]
  fn overrides_alias_get_alias_for() {
    // get_alias_for returns the replacement package name
    let raw = serde_json::json!({
      "foo": "npm:bar@^1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let alias = overrides.get_alias_for(&PackageName::from_str("foo"));
    assert!(alias.is_some());
    assert_eq!(alias.unwrap().as_str(), "bar");

    // non-aliased override returns None for alias lookup
    let raw2 = serde_json::json!({
      "foo": "1.0.0"
    });
    let overrides2 =
      NpmOverrides::from_value(raw2, &empty_root_deps()).unwrap();
    assert!(
      overrides2
        .get_alias_for(&PackageName::from_str("foo"))
        .is_none()
    );

    // non-existent package returns None
    assert!(
      overrides
        .get_alias_for(&PackageName::from_str("baz"))
        .is_none()
    );
  }

  #[test]
  fn overrides_alias_passthrough_for_child() {
    // alias overrides should pass through to child contexts
    let raw = serde_json::json!({
      "foo": "npm:bar@1.0.0"
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    let child = overrides.for_child(
      &PackageName::from_str("baz"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    let result = child.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    let alias = child.get_alias_for(&PackageName::from_str("foo"));
    assert!(alias.is_some());
    assert_eq!(alias.unwrap().as_str(), "bar");
  }

  // --- jsr: override tests ---

  #[test]
  fn parse_jsr_override_basic() {
    let raw = serde_json::json!({
      "foo": "jsr:@std/path@1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "foo");
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path");
        assert_eq!(version_req.version_text(), "1.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_with_range() {
    let raw = serde_json::json!({
      "foo": "jsr:@std/path@^1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path");
        assert_eq!(version_req.version_text(), "^1.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_tilde_range() {
    let raw = serde_json::json!({
      "foo": "jsr:@foo/bar@~2.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/foo__bar");
        assert_eq!(version_req.version_text(), "~2.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_no_version() {
    let raw = serde_json::json!({
      "foo": "jsr:@std/path"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path");
        assert_eq!(version_req.version_text(), "*");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_hyphenated_name() {
    let raw = serde_json::json!({
      "foo": "jsr:@std/path-utils@1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path-utils");
        assert_eq!(version_req.version_text(), "1.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_complex_scope() {
    let raw = serde_json::json!({
      "foo": "jsr:@my-org/my-pkg@>=1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/my-org__my-pkg");
        assert_eq!(version_req.version_text(), ">=1.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_error_no_scope() {
    // "jsr:foo" — missing @ scope prefix
    let raw = serde_json::json!({
      "bar": "jsr:foo"
    });
    let result = NpmOverrides::from_value(raw, &empty_root_deps());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("jsr:"));
    assert!(err.contains("requires a scoped package name"));
  }

  #[test]
  fn parse_jsr_override_error_no_slash() {
    // "jsr:@foo" — scope without /name
    let raw = serde_json::json!({
      "bar": "jsr:@foo"
    });
    let result = NpmOverrides::from_value(raw, &empty_root_deps());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("requires a scoped package name"));
  }

  #[test]
  fn parse_jsr_override_error_unscoped_with_version() {
    // "jsr:foo@1.0.0" — no scope
    let raw = serde_json::json!({
      "bar": "jsr:foo@1.0.0"
    });
    let result = NpmOverrides::from_value(raw, &empty_root_deps());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("requires a scoped package name"));
  }

  #[test]
  fn parse_jsr_override_dot_key() {
    // jsr: in the "." key of an object override
    let raw = serde_json::json!({
      "foo": {
        ".": "jsr:@std/path@1.0.0",
        "baz": "2.0.0"
      }
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path");
        assert_eq!(version_req.version_text(), "1.0.0");
      }
      _ => panic!("expected Alias from dot key"),
    }
    assert_eq!(rule.children.len(), 1);
  }

  #[test]
  fn jsr_override_get_override_for() {
    let raw = serde_json::json!({
      "foo": "jsr:@std/path@^1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let result =
      overrides.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "^1.0.0");
  }

  #[test]
  fn jsr_override_get_alias_for() {
    let raw = serde_json::json!({
      "foo": "jsr:@std/path@^1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let alias = overrides.get_alias_for(&PackageName::from_str("foo"));
    assert!(alias.is_some());
    assert_eq!(alias.unwrap().as_str(), "@jsr/std__path");
  }

  #[test]
  fn jsr_override_passthrough_for_child() {
    let raw = serde_json::json!({
      "foo": "jsr:@std/path@1.0.0"
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    let child = overrides.for_child(
      &PackageName::from_str("baz"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    let result = child.get_override_for(&PackageName::from_str("foo"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    let alias = child.get_alias_for(&PackageName::from_str("foo"));
    assert!(alias.is_some());
    assert_eq!(alias.unwrap().as_str(), "@jsr/std__path");
  }

  #[test]
  fn jsr_override_scoped_to_parent() {
    // jsr override scoped within a parent
    let raw = serde_json::json!({
      "parent": {
        "child": "jsr:@std/path@1.0.0"
      }
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    // at top level, no override for child
    assert!(
      overrides
        .get_override_for(&PackageName::from_str("child"), None)
        .is_none()
    );

    // enter parent — child override activates
    let inside_parent = overrides.for_child(
      &PackageName::from_str("parent"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    let result =
      inside_parent.get_override_for(&PackageName::from_str("child"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "1.0.0");

    let alias = inside_parent.get_alias_for(&PackageName::from_str("child"));
    assert!(alias.is_some());
    assert_eq!(alias.unwrap().as_str(), "@jsr/std__path");
  }

  #[test]
  fn parse_jsr_override_version_only() {
    // "jsr:1" — derive package name from key "@std/path"
    let raw = serde_json::json!({
      "@std/path": "jsr:1"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    assert_eq!(overrides.rules.len(), 1);
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "@std/path");
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path");
        assert_eq!(version_req.version_text(), "1");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_version_only_caret() {
    let raw = serde_json::json!({
      "@std/path": "jsr:^1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path");
        assert_eq!(version_req.version_text(), "^1.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_version_only_tilde() {
    let raw = serde_json::json!({
      "@foo/bar": "jsr:~2.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/foo__bar");
        assert_eq!(version_req.version_text(), "~2.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_version_only_star() {
    let raw = serde_json::json!({
      "@std/path": "jsr:*"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path");
        assert_eq!(version_req.version_text(), "*");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_version_only_key_with_selector() {
    // key has a version selector: "@std/path@^1.0.0"
    // the selector is on the key (for scoping), not the version override
    let raw = serde_json::json!({
      "@std/path@^1.0.0": "jsr:2.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let rule = &overrides.rules[0];
    assert_eq!(rule.name.as_str(), "@std/path");
    assert!(rule.selector.is_some());
    match &rule.value {
      NpmOverrideValue::Alias {
        package,
        version_req,
      } => {
        assert_eq!(package.as_str(), "@jsr/std__path");
        assert_eq!(version_req.version_text(), "2.0.0");
      }
      _ => panic!("expected Alias"),
    }
  }

  #[test]
  fn parse_jsr_override_version_only_error_unscoped_key() {
    // key is "foo" (unscoped) — can't derive a valid JSR name
    let raw = serde_json::json!({
      "foo": "jsr:1.0.0"
    });
    let result = NpmOverrides::from_value(raw, &empty_root_deps());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("requires a scoped package name"));
  }

  #[test]
  fn jsr_override_version_only_get_alias_for() {
    let raw = serde_json::json!({
      "@std/path": "jsr:^1.0.0"
    });
    let overrides = NpmOverrides::from_value(raw, &empty_root_deps()).unwrap();
    let result =
      overrides.get_override_for(&PackageName::from_str("@std/path"), None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().version_text(), "^1.0.0");

    let alias = overrides.get_alias_for(&PackageName::from_str("@std/path"));
    assert!(alias.is_some());
    assert_eq!(alias.unwrap().as_str(), "@jsr/std__path");
  }

  #[test]
  fn jsr_override_version_only_passthrough() {
    let raw = serde_json::json!({
      "@std/path": "jsr:1.0.0"
    });
    let overrides =
      Rc::new(NpmOverrides::from_value(raw, &empty_root_deps()).unwrap());

    let child = overrides.for_child(
      &PackageName::from_str("other"),
      &Version::parse_from_npm("1.0.0").unwrap(),
    );
    let alias = child.get_alias_for(&PackageName::from_str("@std/path"));
    assert!(alias.is_some());
    assert_eq!(alias.unwrap().as_str(), "@jsr/std__path");
  }
}
