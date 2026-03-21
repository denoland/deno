// Copyright 2018-2026 the Deno authors. MIT license.

//! Per-API granular permission system.
//!
//! This module provides a fine-grained permission layer that sits above
//! the category-level permission system (read, write, net, env, sys, run,
//! ffi, import). Each individual API (e.g., `Deno.readFile()`,
//! `Deno.connect()`, `fetch()`) can have its own permission rule.
//!
//! A compatibility layer ensures that when no per-API rules are configured,
//! the existing category-level permission system is used unchanged.
//!
//! ## Architecture
//!
//! ```text
//! API call (e.g., Deno.readFile("/etc/passwd"))
//!   │
//!   ▼
//! PermissionsContainer::check_open(path, Read, Some("Deno.readFile()"))
//!   │
//!   ├─► ApiPermissionResolver::check("Deno.readFile()", "read", "/etc/passwd")
//!   │     │
//!   │     ├─► Allow  → skip category check, return Ok
//!   │     ├─► Deny   → return Err immediately
//!   │     └─► Defer  → fall through to category check
//!   │
//!   └─► Category-level check (existing UnaryPermission<ReadDescriptor>)
//! ```

use std::collections::HashMap;
use std::fmt::Debug;

/// Result of an API-level permission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiCheckResult {
  /// This specific API call is allowed; skip category-level checks.
  Allow,
  /// This specific API call is denied.
  Deny { reason: Option<String> },
  /// No per-API rule exists; defer to the category-level permission system.
  Defer,
}

/// A rule for a specific API, as stored in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ApiRule {
  Allow,
  Deny {
    #[serde(default)]
    reason: Option<String>,
  },
}

/// Trait for resolving per-API permissions.
///
/// Implementations can be backed by a JSON manifest, an external broker
/// process, or any other mechanism.
pub trait ApiPermissionResolver: Send + Sync + Debug {
  /// Check whether a specific API call should be allowed, denied, or
  /// deferred to the category-level permission system.
  ///
  /// # Arguments
  /// * `api_name` - The API being called (e.g., `"Deno.readFile()"`)
  /// * `category` - The permission category (e.g., `"read"`, `"net"`)
  /// * `value_fn` - Lazy function returning the stringified resource
  ///   descriptor (e.g., a file path or hostname). Only called if
  ///   the resolver needs the value.
  fn check(
    &self,
    api_name: &str,
    category: &str,
    value_fn: &dyn Fn() -> Option<String>,
  ) -> ApiCheckResult;
}

/// A simple HashMap-based resolver for manifest-driven configuration.
///
/// Rules are keyed by API name. API names must match exactly what is
/// passed in `check_*` calls on `PermissionsContainer` (e.g.,
/// `"Deno.readFile()"`, `"Deno.connect()"`, `"fetch()"`, `"node:fs.open"`).
///
/// # Example manifest JSON
///
/// ```json
/// {
///   "Deno.readFile()": { "state": "allow" },
///   "Deno.readDir()": { "state": "deny", "reason": "directory listing not permitted" },
///   "fetch()": { "state": "allow" }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HashMapApiPermissionResolver {
  rules: HashMap<String, ApiRule>,
}

impl HashMapApiPermissionResolver {
  pub fn new(rules: HashMap<String, ApiRule>) -> Self {
    Self { rules }
  }

  pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
    let rules: HashMap<String, ApiRule> = serde_json::from_str(json)?;
    Ok(Self { rules })
  }
}

impl ApiPermissionResolver for HashMapApiPermissionResolver {
  fn check(
    &self,
    api_name: &str,
    _category: &str,
    _value_fn: &dyn Fn() -> Option<String>,
  ) -> ApiCheckResult {
    match self.rules.get(api_name) {
      Some(ApiRule::Allow) => ApiCheckResult::Allow,
      Some(ApiRule::Deny { reason }) => ApiCheckResult::Deny {
        reason: reason.clone(),
      },
      None => ApiCheckResult::Defer,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_hashmap_resolver_basic() {
    let mut rules = HashMap::new();
    rules.insert("Deno.readFile()".to_string(), ApiRule::Allow);
    rules.insert(
      "Deno.readDir()".to_string(),
      ApiRule::Deny {
        reason: Some("not allowed".to_string()),
      },
    );

    let resolver = HashMapApiPermissionResolver::new(rules);

    assert_eq!(
      resolver.check("Deno.readFile()", "read", &|| None),
      ApiCheckResult::Allow,
    );
    assert_eq!(
      resolver.check("Deno.readDir()", "read", &|| None),
      ApiCheckResult::Deny {
        reason: Some("not allowed".to_string()),
      },
    );
    assert_eq!(
      resolver.check("Deno.writeFile()", "write", &|| None),
      ApiCheckResult::Defer,
    );
  }

  #[test]
  fn test_hashmap_resolver_from_json() {
    let json = r#"{
      "Deno.readFile()": { "state": "allow" },
      "fetch()": { "state": "deny", "reason": "no fetch allowed" }
    }"#;

    let resolver = HashMapApiPermissionResolver::from_json(json).unwrap();

    assert_eq!(
      resolver.check("Deno.readFile()", "read", &|| None),
      ApiCheckResult::Allow,
    );
    assert_eq!(
      resolver.check("fetch()", "net", &|| None),
      ApiCheckResult::Deny {
        reason: Some("no fetch allowed".to_string()),
      },
    );
    assert_eq!(
      resolver.check("Deno.connect()", "net", &|| None),
      ApiCheckResult::Defer,
    );
  }

  #[test]
  fn test_value_fn_not_called_when_unnecessary() {
    let mut rules = HashMap::new();
    rules.insert("Deno.readFile()".to_string(), ApiRule::Allow);

    let resolver = HashMapApiPermissionResolver::new(rules);

    // The value_fn panics, but since the resolver doesn't need it
    // for a simple HashMap lookup, it should not be called.
    let result =
      resolver.check("Deno.readFile()", "read", &|| panic!("should not call"));
    assert_eq!(result, ApiCheckResult::Allow);
  }
}
