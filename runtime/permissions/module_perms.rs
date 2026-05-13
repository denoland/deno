// Copyright 2018-2026 the Deno authors. MIT license.

//! Per-module permission overlay.
//!
//! Lets a user deny a subset of Deno's runtime permissions to specific
//! modules in the dependency graph using `--deny-module=<pattern>:<kinds>`.
//!
//! When a denied module is found on the JavaScript call stack at the moment
//! a permission-checked op is invoked, the requested permission is rejected
//! regardless of the process-wide allowlist. This is a Stack Inspection style
//! restriction implemented on top of V8's stack-trace API (already plumbed
//! through `OpStackTraceCallback`).

use std::cell::RefCell;
use std::fmt;

use serde::Deserialize;
use serde::Serialize;

/// One restricted permission kind tracked by the per-module overlay.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModulePermissionKind {
  Read,
  Write,
  Net,
  Env,
  Sys,
  Run,
  Ffi,
  Import,
}

impl ModulePermissionKind {
  pub const ALL: &'static [ModulePermissionKind] = &[
    ModulePermissionKind::Read,
    ModulePermissionKind::Write,
    ModulePermissionKind::Net,
    ModulePermissionKind::Env,
    ModulePermissionKind::Sys,
    ModulePermissionKind::Run,
    ModulePermissionKind::Ffi,
    ModulePermissionKind::Import,
  ];

  pub fn as_flag_name(self) -> &'static str {
    match self {
      ModulePermissionKind::Read => "read",
      ModulePermissionKind::Write => "write",
      ModulePermissionKind::Net => "net",
      ModulePermissionKind::Env => "env",
      ModulePermissionKind::Sys => "sys",
      ModulePermissionKind::Run => "run",
      ModulePermissionKind::Ffi => "ffi",
      ModulePermissionKind::Import => "import",
    }
  }

  pub fn from_flag_name(s: &str) -> Option<Self> {
    Some(match s {
      "read" => ModulePermissionKind::Read,
      "write" => ModulePermissionKind::Write,
      "net" => ModulePermissionKind::Net,
      "env" => ModulePermissionKind::Env,
      "sys" => ModulePermissionKind::Sys,
      "run" => ModulePermissionKind::Run,
      "ffi" => ModulePermissionKind::Ffi,
      "import" => ModulePermissionKind::Import,
      _ => return None,
    })
  }
}

impl fmt::Display for ModulePermissionKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_flag_name())
  }
}

#[derive(Debug, thiserror::Error)]
pub enum ModuleDenyRuleParseError {
  #[error(
    "Invalid --deny-module value: expected '<pattern>:<perms>' but got '{0}'"
  )]
  MissingPerms(String),
  #[error("Empty module pattern in --deny-module value: '{0}'")]
  EmptyPattern(String),
  #[error(
    "Unknown permission kind '{kind}' in --deny-module value: '{full}'. \
     Valid kinds are: read, write, net, env, sys, run, ffi, import, all"
  )]
  UnknownKind { kind: String, full: String },
  #[error("Empty permission list in --deny-module value: '{0}'")]
  EmptyKindList(String),
}

/// A pattern matching a module URL or specifier on the call stack.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ModulePattern {
  /// `npm:<name>` — matches `npm:<name>` and `npm:<name>@…` specifiers as
  /// well as file paths under `node_modules/<name>/`.
  NpmPackage(String),
  /// `jsr:<scope>/<name>` — matches `jsr:@scope/name@…` specifiers.
  JsrPackage(String),
  /// Any other prefix is a literal substring match on the script URL.
  Substring(String),
}

impl ModulePattern {
  /// Parse a single pattern string. Currently we only special-case the
  /// `npm:` and `jsr:` schemes; everything else becomes a substring match.
  pub fn parse(pattern: &str) -> Self {
    if let Some(rest) = pattern.strip_prefix("npm:") {
      // strip optional leading slash and trailing version
      let name = rest.trim_start_matches('/').trim_end_matches('/');
      ModulePattern::NpmPackage(name.to_string())
    } else if let Some(rest) = pattern.strip_prefix("jsr:") {
      ModulePattern::JsrPackage(rest.to_string())
    } else {
      ModulePattern::Substring(pattern.to_string())
    }
  }

  pub fn matches(&self, frame_url: &str) -> bool {
    match self {
      ModulePattern::NpmPackage(name) => {
        // `npm:chalk` and `npm:chalk@5.0.0` specifiers
        if let Some(rest) = frame_url.strip_prefix("npm:") {
          let rest = rest.trim_start_matches('/');
          if rest == name.as_str()
            || rest.starts_with(&format!("{name}@"))
            || rest.starts_with(&format!("{name}/"))
          {
            return true;
          }
        }
        // file:///.../node_modules/chalk/...
        let needle_with_slash = format!("/node_modules/{name}/");
        if frame_url.contains(&needle_with_slash) {
          return true;
        }
        // Windows-style separators inside file URLs are still encoded as
        // forward slashes, so no extra handling is needed.
        false
      }
      ModulePattern::JsrPackage(name) => {
        if let Some(rest) = frame_url.strip_prefix("jsr:") {
          let rest = rest.trim_start_matches('/');
          if rest == name.as_str()
            || rest.starts_with(&format!("{name}@"))
            || rest.starts_with(&format!("{name}/"))
          {
            return true;
          }
        }
        false
      }
      ModulePattern::Substring(s) => frame_url.contains(s.as_str()),
    }
  }

  pub fn raw(&self) -> &str {
    match self {
      ModulePattern::NpmPackage(s)
      | ModulePattern::JsrPackage(s)
      | ModulePattern::Substring(s) => s.as_str(),
    }
  }
}

/// Per-module permission denial rule: `pattern` denies the listed `kinds`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleDenyRule {
  pub raw: String,
  pub pattern: ModulePattern,
  pub kinds: Vec<ModulePermissionKind>,
}

impl ModuleDenyRule {
  /// Parses `pattern:kind1,kind2,...` or `pattern:all`.
  ///
  /// The pattern may itself contain colons (URL schemes); we split on the
  /// **last** colon to separate pattern from kinds list.
  pub fn parse(raw: &str) -> Result<Self, ModuleDenyRuleParseError> {
    let raw = raw.trim();
    let (pattern_str, kinds_str) = raw
      .rsplit_once(':')
      .ok_or_else(|| ModuleDenyRuleParseError::MissingPerms(raw.to_string()))?;
    let pattern_str = pattern_str.trim();
    let kinds_str = kinds_str.trim();
    if pattern_str.is_empty() {
      return Err(ModuleDenyRuleParseError::EmptyPattern(raw.to_string()));
    }
    if kinds_str.is_empty() {
      return Err(ModuleDenyRuleParseError::EmptyKindList(raw.to_string()));
    }
    let mut kinds: Vec<ModulePermissionKind> = Vec::new();
    for part in kinds_str.split(',') {
      let part = part.trim();
      if part.is_empty() {
        continue;
      }
      if part.eq_ignore_ascii_case("all") {
        kinds = ModulePermissionKind::ALL.to_vec();
        break;
      }
      let kind =
        ModulePermissionKind::from_flag_name(part).ok_or_else(|| {
          ModuleDenyRuleParseError::UnknownKind {
            kind: part.to_string(),
            full: raw.to_string(),
          }
        })?;
      if !kinds.contains(&kind) {
        kinds.push(kind);
      }
    }
    if kinds.is_empty() {
      return Err(ModuleDenyRuleParseError::EmptyKindList(raw.to_string()));
    }
    let pattern = ModulePattern::parse(pattern_str);
    Ok(ModuleDenyRule {
      raw: raw.to_string(),
      pattern,
      kinds,
    })
  }

  pub fn parse_list(
    items: &[String],
  ) -> Result<Vec<Self>, ModuleDenyRuleParseError> {
    items.iter().map(|s| Self::parse(s)).collect()
  }

  pub fn denies(&self, kind: ModulePermissionKind) -> bool {
    self.kinds.contains(&kind)
  }
}

thread_local! {
  /// File URLs of the frames present on the JS call stack at the moment the
  /// most recently entered `#[op2(stack_trace)]` op was invoked.
  ///
  /// Populated by [`crate::set_current_op_frames`] from the `JsRuntime`'s
  /// op-stack-trace callback. Cleared after the op-permission check
  /// completes.
  ///
  /// Stored as a thread-local because each worker isolate runs on its own
  /// thread and ops are dispatched synchronously inside that thread.
  static CURRENT_OP_FRAMES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Replace the recorded frame list for the current thread. Intended for use
/// from the runtime's `OpStackTraceCallback` only.
pub fn set_current_op_frames(frames: Vec<String>) {
  CURRENT_OP_FRAMES.with(|cell| {
    *cell.borrow_mut() = frames;
  });
}

/// Read-only access to the current frame list.
pub fn with_current_op_frames<R>(f: impl FnOnce(&[String]) -> R) -> R {
  CURRENT_OP_FRAMES.with(|cell| f(&cell.borrow()))
}

/// Check the per-module overlay. Returns `Some(matching_rule_raw)` if any
/// frame on the recorded call stack matches a rule denying `kind`, else
/// `None`.
pub fn matching_denied_module(
  rules: &[ModuleDenyRule],
  kind: ModulePermissionKind,
) -> Option<MatchedDenial> {
  if rules.is_empty() {
    return None;
  }
  with_current_op_frames(|frames| {
    for frame in frames {
      for rule in rules {
        if rule.denies(kind) && rule.pattern.matches(frame) {
          return Some(MatchedDenial {
            rule_raw: rule.raw.clone(),
            module: frame.clone(),
          });
        }
      }
    }
    None
  })
}

#[derive(Debug, Clone)]
pub struct MatchedDenial {
  pub rule_raw: String,
  pub module: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_basic() {
    let r = ModuleDenyRule::parse("npm:chalk:net,run").unwrap();
    assert_eq!(r.pattern, ModulePattern::NpmPackage("chalk".to_string()));
    assert_eq!(
      r.kinds,
      vec![ModulePermissionKind::Net, ModulePermissionKind::Run]
    );
  }

  #[test]
  fn parse_all() {
    let r = ModuleDenyRule::parse("npm:chalk:all").unwrap();
    assert_eq!(r.kinds.len(), ModulePermissionKind::ALL.len());
  }

  #[test]
  fn parse_jsr() {
    let r = ModuleDenyRule::parse("jsr:@scope/pkg:write").unwrap();
    assert_eq!(
      r.pattern,
      ModulePattern::JsrPackage("@scope/pkg".to_string())
    );
  }

  #[test]
  fn parse_url() {
    let r =
      ModuleDenyRule::parse("https://untrusted.example.com/:net").unwrap();
    assert_eq!(
      r.pattern,
      ModulePattern::Substring("https://untrusted.example.com/".to_string())
    );
    assert_eq!(r.kinds, vec![ModulePermissionKind::Net]);
  }

  #[test]
  fn parse_empty_perms() {
    assert!(matches!(
      ModuleDenyRule::parse("npm:chalk:"),
      Err(ModuleDenyRuleParseError::EmptyKindList(_))
    ));
  }

  #[test]
  fn parse_unknown_kind() {
    assert!(matches!(
      ModuleDenyRule::parse("npm:chalk:foo"),
      Err(ModuleDenyRuleParseError::UnknownKind { .. })
    ));
  }

  #[test]
  fn parse_no_colon() {
    assert!(matches!(
      ModuleDenyRule::parse("just-a-pattern"),
      Err(ModuleDenyRuleParseError::MissingPerms(_))
    ));
  }

  #[test]
  fn match_npm_specifier() {
    let p = ModulePattern::parse("npm:chalk");
    assert!(p.matches("npm:chalk"));
    assert!(p.matches("npm:chalk@5.0.0"));
    assert!(p.matches("npm:chalk@5/index.js"));
    assert!(p.matches("npm:/chalk/index.js"));
    assert!(p.matches("file:///x/node_modules/chalk/index.js"));
    assert!(!p.matches("file:///x/node_modules/chalky/index.js"));
    assert!(!p.matches("npm:chalk-utils"));
    assert!(!p.matches("npm:other"));
  }

  #[test]
  fn match_substring() {
    let p = ModulePattern::parse("https://untrusted.example.com/");
    assert!(p.matches("https://untrusted.example.com/lib.js"));
    assert!(!p.matches("https://other.com/lib.js"));
  }

  #[test]
  fn matching_denied_module_uses_thread_local() {
    set_current_op_frames(vec![
      "file:///app/main.ts".into(),
      "file:///app/node_modules/chalk/index.js".into(),
    ]);
    let rules = vec![ModuleDenyRule::parse("npm:chalk:net").unwrap()];
    assert!(
      matching_denied_module(&rules, ModulePermissionKind::Net).is_some()
    );
    assert!(
      matching_denied_module(&rules, ModulePermissionKind::Read).is_none()
    );
    set_current_op_frames(vec![]);
    assert!(
      matching_denied_module(&rules, ModulePermissionKind::Net).is_none()
    );
  }
}
