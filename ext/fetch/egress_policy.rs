// Copyright 2018-2026 the Deno authors. MIT license.

//! Operator-controlled egress header policy.
//!
//! An embedder (or the CLI, via the `DENO_EGRESS_HEADER_POLICY` env var) can
//! provide a declarative policy describing headers to enforce on outbound
//! `fetch()` requests. The policy is a JSON object with up to five fields:
//!
//! ```json
//! {
//!   "remove":  ["x-internal-debug"],
//!   "forward": ["cdn-loop"],
//!   "set":     { "user-agent": "example/1.0" },
//!   "append":  { "cdn-loop": "example;d=abc" },
//!   "default": { "x-fallback": "1" }
//! }
//! ```
//!
//! Semantics per outbound request:
//!
//! - Headers named in `remove`, `set`, `append`, or `forward` are
//!   *policy-owned*: values supplied by user code are scrubbed before the
//!   policy applies. Headers named in `default` are not scrubbed.
//! - `forward`: values of these headers on the inbound HTTP request currently
//!   being served (if any) are copied onto the outbound request.
//! - `set`: the header is set to the given value, replacing anything present.
//! - `append`: the given value is appended (after any forwarded values).
//! - `default`: the header is set only when not already present.
//!
//! The ops are split across two application points by idempotency:
//!
//! - `remove`/`set`/`default` are idempotent and applied in `op_fetch` for
//!   every network hop, including redirect hops (see
//!   [`EgressHeaderPolicy::apply_static`]).
//! - `forward`/`append` are not idempotent (re-applying on a redirect hop
//!   would duplicate appended values) and are applied exactly once per
//!   `fetch()` call, in JS, where the inbound-request async context is
//!   available (see `26_fetch.js`).
//!
//! A policy that fails to parse or validate is retained as
//! [`EgressHeaderPolicyState::Invalid`]: every fetch then fails with the
//! parse error instead of proceeding without the policy (fail closed).

use std::collections::BTreeMap;

use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum EgressHeaderPolicyError {
  #[class(type)]
  #[error("egress header policy is not valid JSON: {0}")]
  Json(#[from] serde_json::Error),
  #[class(type)]
  #[error("egress header policy must be a JSON object")]
  NotAnObject,
  #[class(type)]
  #[error(
    "egress header policy has unknown field '{0}' (expected 'remove', 'forward', 'set', 'append', or 'default')"
  )]
  UnknownField(String),
  #[class(type)]
  #[error("egress header policy field '{0}' must be an array of strings")]
  ExpectedStringArray(&'static str),
  #[class(type)]
  #[error(
    "egress header policy field '{0}' must be an object with string values"
  )]
  ExpectedStringMap(&'static str),
  #[class(type)]
  #[error("egress header policy has invalid header name '{0}'")]
  InvalidHeaderName(String),
  #[class(type)]
  #[error("egress header policy has invalid value for header '{0}'")]
  InvalidHeaderValue(String),
  #[class(type)]
  #[error("egress header policy may not manage the '{0}' header")]
  ForbiddenHeader(String),
  #[class(type)]
  #[error(
    "egress header policy names header '{name}' in both '{first}' and '{second}'"
  )]
  ConflictingOps {
    name: String,
    first: &'static str,
    second: &'static str,
  },
}

/// Headers whose manipulation would corrupt the HTTP request itself. The
/// policy exists to manage application-level headers; framing stays with the
/// HTTP stack.
const FORBIDDEN_HEADERS: [HeaderName; 4] = [
  header::HOST,
  header::CONTENT_LENGTH,
  header::TRANSFER_ENCODING,
  header::CONNECTION,
];

#[derive(Debug)]
pub struct EgressHeaderPolicy {
  set: Vec<(HeaderName, HeaderValue)>,
  set_if_absent: Vec<(HeaderName, HeaderValue)>,
  /// Names scrubbed and re-managed per hop in Rust: `remove` ∪ `set`.
  /// (`forward` ∪ `append` names are scrubbed in JS, before forwarded
  /// values are injected.)
  scrub_static: Vec<HeaderName>,
  /// Lowercase names whose inbound values are forwarded, applied in JS.
  forward: Vec<String>,
  /// (lowercase name, value) pairs appended once per fetch() call, in JS.
  append: Vec<(String, String)>,
}

impl EgressHeaderPolicy {
  pub fn parse(json: &str) -> Result<Self, EgressHeaderPolicyError> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let serde_json::Value::Object(map) = value else {
      return Err(EgressHeaderPolicyError::NotAnObject);
    };

    let mut remove: Vec<HeaderName> = Vec::new();
    let mut forward: Vec<HeaderName> = Vec::new();
    let mut set: Vec<(HeaderName, HeaderValue)> = Vec::new();
    let mut append: Vec<(HeaderName, HeaderValue)> = Vec::new();
    let mut set_if_absent: Vec<(HeaderName, HeaderValue)> = Vec::new();

    for (key, field) in map {
      match key.as_str() {
        "remove" => remove = parse_name_list("remove", &field)?,
        "forward" => forward = parse_name_list("forward", &field)?,
        "set" => set = parse_value_map("set", &field)?,
        "append" => append = parse_value_map("append", &field)?,
        "default" => set_if_absent = parse_value_map("default", &field)?,
        _ => return Err(EgressHeaderPolicyError::UnknownField(key)),
      }
    }

    // `forward` + `append` on the same header is the accumulation pattern
    // (forwarded inbound values, then the policy's own entry) and is the one
    // permitted overlap. Every other overlap is ambiguous and rejected. The
    // op lists are operator-authored and tiny, so linear scans suffice.
    let disjoint_groups: [(&'static str, Vec<&str>); 5] = [
      ("remove", remove.iter().map(HeaderName::as_str).collect()),
      ("forward", forward.iter().map(HeaderName::as_str).collect()),
      ("set", op_names(&set)),
      ("append", op_names(&append)),
      ("default", op_names(&set_if_absent)),
    ];
    for (i, (first, first_names)) in disjoint_groups.iter().enumerate() {
      for (second, second_names) in &disjoint_groups[i + 1..] {
        if (*first, *second) == ("forward", "append") {
          continue;
        }
        if let Some(name) =
          first_names.iter().find(|n| second_names.contains(n))
        {
          return Err(EgressHeaderPolicyError::ConflictingOps {
            name: (*name).to_string(),
            first,
            second,
          });
        }
      }
    }

    let mut scrub_static = remove;
    scrub_static.extend(set.iter().map(|(n, _)| n.clone()));

    Ok(Self {
      set,
      set_if_absent,
      scrub_static,
      forward: forward.iter().map(|n| n.as_str().to_string()).collect(),
      append: append
        .into_iter()
        .map(|(n, v)| {
          // Values originate from validated JSON strings, so `to_str`
          // cannot fail.
          (n.as_str().to_string(), v.to_str().unwrap().to_string())
        })
        .collect(),
    })
  }

  /// Applies the idempotent ops (`remove`/`set` scrub, `set`, `default`).
  /// Runs in `op_fetch` for every network hop.
  pub fn apply_static(&self, headers: &mut HeaderMap) {
    for name in &self.scrub_static {
      if let header::Entry::Occupied(entry) = headers.entry(name) {
        entry.remove_entry_mult();
      }
    }
    for (name, value) in &self.set {
      headers.insert(name, value.clone());
    }
    for (name, value) in &self.set_if_absent {
      if !headers.contains_key(name) {
        headers.insert(name, value.clone());
      }
    }
  }

  /// The JS-applied part of the policy: header names to forward from the
  /// inbound request context and (name, value) pairs to append, both
  /// lowercase. Empty lists mean the JS fast path can skip entirely.
  pub fn forward_config(&self) -> (&[String], &[(String, String)]) {
    (&self.forward, &self.append)
  }

  /// Whether the policy names this (lowercase) header in any op. Embedders
  /// with their own header mechanisms use this to yield to the policy — the
  /// CLI's legacy `CDN_LOOP`/`X_DENO_FETCH_TOKEN` handling skips headers the
  /// policy manages, since it runs after the JS-applied `forward`/`append`
  /// ops and would otherwise clobber their values.
  pub fn manages_header(&self, name: &str) -> bool {
    self.scrub_static.iter().any(|n| n.as_str() == name)
      || self.set_if_absent.iter().any(|(n, _)| n.as_str() == name)
      || self.forward.iter().any(|n| n == name)
      || self.append.iter().any(|(n, _)| n == name)
  }
}

/// A parsed-or-poisoned policy. An invalid policy fails every fetch closed
/// rather than silently proceeding without enforcement.
#[derive(Debug)]
pub enum EgressHeaderPolicyState {
  Valid(EgressHeaderPolicy),
  Invalid(String),
}

impl EgressHeaderPolicyState {
  pub fn from_json(json: &str) -> Self {
    match EgressHeaderPolicy::parse(json) {
      Ok(policy) => Self::Valid(policy),
      Err(err) => Self::Invalid(err.to_string()),
    }
  }
}

fn op_names(list: &[(HeaderName, HeaderValue)]) -> Vec<&str> {
  list.iter().map(|(n, _)| n.as_str()).collect()
}

fn validated_name(raw: &str) -> Result<HeaderName, EgressHeaderPolicyError> {
  let name = HeaderName::from_bytes(raw.as_bytes())
    .map_err(|_| EgressHeaderPolicyError::InvalidHeaderName(raw.to_string()))?;
  if FORBIDDEN_HEADERS.contains(&name) {
    return Err(EgressHeaderPolicyError::ForbiddenHeader(
      name.as_str().to_string(),
    ));
  }
  Ok(name)
}

fn parse_name_list(
  field: &'static str,
  value: &serde_json::Value,
) -> Result<Vec<HeaderName>, EgressHeaderPolicyError> {
  let serde_json::Value::Array(items) = value else {
    return Err(EgressHeaderPolicyError::ExpectedStringArray(field));
  };
  items
    .iter()
    .map(|item| {
      let serde_json::Value::String(raw) = item else {
        return Err(EgressHeaderPolicyError::ExpectedStringArray(field));
      };
      validated_name(raw)
    })
    .collect()
}

fn parse_value_map(
  field: &'static str,
  value: &serde_json::Value,
) -> Result<Vec<(HeaderName, HeaderValue)>, EgressHeaderPolicyError> {
  let serde_json::Value::Object(entries) = value else {
    return Err(EgressHeaderPolicyError::ExpectedStringMap(field));
  };
  // Keyed by the normalized (lowercase) name so differently-cased spellings
  // of one header dedupe (last wins) and output order is deterministic.
  let mut parsed: BTreeMap<String, (HeaderName, HeaderValue)> = BTreeMap::new();
  for (raw_name, raw_value) in entries {
    let serde_json::Value::String(raw_value) = raw_value else {
      return Err(EgressHeaderPolicyError::ExpectedStringMap(field));
    };
    let name = validated_name(raw_name)?;
    let value = HeaderValue::from_str(raw_value).map_err(|_| {
      EgressHeaderPolicyError::InvalidHeaderValue(name.as_str().to_string())
    })?;
    parsed.insert(name.as_str().to_string(), (name, value));
  }
  Ok(parsed.into_values().collect())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_full_policy() {
    let policy = EgressHeaderPolicy::parse(
      r#"{
        "remove":  ["x-internal-debug"],
        "forward": ["cdn-loop"],
        "set":     { "User-Agent": "example/1.0" },
        "append":  { "cdn-loop": "example;d=abc" },
        "default": { "x-fallback": "1" }
      }"#,
    )
    .unwrap();
    let (forward, append) = policy.forward_config();
    assert_eq!(forward, &["cdn-loop"]);
    assert_eq!(
      append,
      &[("cdn-loop".to_string(), "example;d=abc".to_string())]
    );
  }

  #[test]
  fn empty_policy_is_valid() {
    let policy = EgressHeaderPolicy::parse("{}").unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("x-user", HeaderValue::from_static("1"));
    policy.apply_static(&mut headers);
    assert_eq!(headers.len(), 1);
  }

  #[test]
  fn rejects_unknown_field() {
    let err = EgressHeaderPolicy::parse(r#"{"sets": {}}"#).unwrap_err();
    assert!(
      matches!(err, EgressHeaderPolicyError::UnknownField(f) if f == "sets")
    );
  }

  #[test]
  fn rejects_non_object() {
    assert!(matches!(
      EgressHeaderPolicy::parse("[]").unwrap_err(),
      EgressHeaderPolicyError::NotAnObject
    ));
  }

  #[test]
  fn rejects_invalid_json() {
    assert!(matches!(
      EgressHeaderPolicy::parse("{").unwrap_err(),
      EgressHeaderPolicyError::Json(_)
    ));
  }

  #[test]
  fn rejects_invalid_header_name() {
    let err =
      EgressHeaderPolicy::parse(r#"{"remove": ["bad header"]}"#).unwrap_err();
    assert!(matches!(err, EgressHeaderPolicyError::InvalidHeaderName(_)));
  }

  #[test]
  fn rejects_invalid_header_value() {
    let err = EgressHeaderPolicy::parse(r#"{"set": {"x-a": "bad\nvalue"}}"#)
      .unwrap_err();
    assert!(matches!(
      err,
      EgressHeaderPolicyError::InvalidHeaderValue(_)
    ));
  }

  #[test]
  fn rejects_forbidden_headers() {
    for json in [
      r#"{"set": {"Host": "example.com"}}"#,
      r#"{"remove": ["content-length"]}"#,
      r#"{"forward": ["transfer-encoding"]}"#,
    ] {
      let err = EgressHeaderPolicy::parse(json).unwrap_err();
      assert!(
        matches!(err, EgressHeaderPolicyError::ForbiddenHeader(_)),
        "expected ForbiddenHeader for {json}"
      );
    }
  }

  #[test]
  fn rejects_conflicting_ops() {
    let err = EgressHeaderPolicy::parse(
      r#"{"set": {"x-a": "1"}, "default": {"x-a": "2"}}"#,
    )
    .unwrap_err();
    assert!(matches!(
      err,
      EgressHeaderPolicyError::ConflictingOps {
        first: "set",
        second: "default",
        ..
      }
    ));

    let err =
      EgressHeaderPolicy::parse(r#"{"remove": ["x-a"], "forward": ["x-a"]}"#)
        .unwrap_err();
    assert!(matches!(
      err,
      EgressHeaderPolicyError::ConflictingOps {
        first: "remove",
        second: "forward",
        ..
      }
    ));
  }

  #[test]
  fn allows_forward_with_append() {
    EgressHeaderPolicy::parse(
      r#"{"forward": ["cdn-loop"], "append": {"cdn-loop": "x;d=1"}}"#,
    )
    .unwrap();
  }

  #[test]
  fn apply_static_scrubs_and_sets() {
    let policy = EgressHeaderPolicy::parse(
      r#"{"remove": ["x-gone"], "set": {"user-agent": "enforced/1.0"}}"#,
    )
    .unwrap();
    let mut headers = HeaderMap::new();
    headers.append("x-gone", HeaderValue::from_static("a"));
    headers.append("x-gone", HeaderValue::from_static("b"));
    headers.insert("user-agent", HeaderValue::from_static("user/9.9"));
    headers.insert("x-kept", HeaderValue::from_static("1"));
    policy.apply_static(&mut headers);

    assert!(!headers.contains_key("x-gone"));
    assert_eq!(headers.get("user-agent").unwrap(), "enforced/1.0");
    assert_eq!(headers.get("x-kept").unwrap(), "1");
  }

  #[test]
  fn apply_static_is_idempotent() {
    let policy = EgressHeaderPolicy::parse(
      r#"{"set": {"x-a": "1"}, "default": {"x-b": "2"}}"#,
    )
    .unwrap();
    let mut headers = HeaderMap::new();
    policy.apply_static(&mut headers);
    policy.apply_static(&mut headers);
    assert_eq!(headers.get_all("x-a").iter().count(), 1);
    assert_eq!(headers.get_all("x-b").iter().count(), 1);
  }

  #[test]
  fn default_respects_existing_value() {
    let policy =
      EgressHeaderPolicy::parse(r#"{"default": {"user-agent": "fallback/1"}}"#)
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static("user/9.9"));
    policy.apply_static(&mut headers);
    assert_eq!(headers.get("user-agent").unwrap(), "user/9.9");

    let mut empty = HeaderMap::new();
    policy.apply_static(&mut empty);
    assert_eq!(empty.get("user-agent").unwrap(), "fallback/1");
  }

  #[test]
  fn invalid_policy_becomes_poisoned_state() {
    let state = EgressHeaderPolicyState::from_json(r#"{"bogus": 1}"#);
    let EgressHeaderPolicyState::Invalid(msg) = state else {
      panic!("expected Invalid");
    };
    assert!(msg.contains("bogus"));
  }
}
