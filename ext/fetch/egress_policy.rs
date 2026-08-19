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
//! Header values are restricted to visible ASCII (plus tab); see
//! [`validated_value`].
//!
//! Semantics per outbound request:
//!
//! - Headers named in `remove`, `set`, `append`, or `forward` are
//!   *policy-owned*: values supplied by user code are scrubbed before the
//!   policy applies. Headers named in `default` are not owned and not
//!   scrubbed.
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
//!   would duplicate appended values) and are selected in JS, where the
//!   inbound-request async context is available: once per outermost
//!   `mainFetch` call, which covers every entry point into the fetch stack
//!   including `EventSource` (see `26_fetch.js`). `op_fetch` preserves those
//!   selected values, then scrubs and restores them after URL credentials and
//!   the embedder's request hook have run.
//!
//! Once a redirect crosses origins, `set`/`default` stop applying to the
//! credential headers WHATWG fetch drops there; see
//! [`EgressHeaderPolicy::apply_static`].
//!
//! The policy is not the last writer on the wire. `Client::send` runs after
//! it and fills in `user-agent`, `accept` and `accept-encoding` when absent,
//! so `remove` of any of them yields the client default rather than no header
//! at all; it also overwrites `proxy-authorization` with credentials taken
//! from the proxy URL, which beats a `set`. Policies covering those four
//! headers only hold where the client leaves them alone.
//!
//! A policy that fails to parse or validate is retained as
//! [`EgressHeaderPolicyState::Invalid`]: every outbound HTTP(S) `fetch()` then
//! fails with the parse error instead of proceeding without the policy (fail
//! closed). See the scope note below for what that does *not* cover.
//!
//! # Scope and limitations
//!
//! The policy applies at exactly one place: `op_fetch`, the op behind
//! `fetch()` for `http:` and `https:` URLs. That is narrower than "outbound
//! HTTP from this process", and the gap matters for anyone deploying it as a
//! control:
//!
//! - **Other egress paths are untouched.** `node:http`/`node:https` (and the
//!   npm ecosystem built on them), raw `Deno.connect` sockets, and WebSocket
//!   handshakes never reach `op_fetch`. Neither do the CLI's own downloads —
//!   remote module loads, npm/jsr registry traffic, `deno upgrade` — which
//!   build their clients without these [`crate::Options`]. An `Invalid` policy
//!   fails `fetch()` closed but does not stop the process egressing by these
//!   other routes.
//! - **Non-network schemes are unaffected.** `file:`, `data:` and `blob:`
//!   fetches resolve before or outside the HTTP arm of `op_fetch`, so they
//!   succeed even under an `Invalid` policy. There is no egress to govern.
//! - **It is not a sandbox.** The policy governs code that goes through
//!   `fetch()`; it does not confine code that chooses not to. Any program
//!   holding `--allow-net` can bypass it in two lines via `node:http` or a raw
//!   socket. Treat it as an enforcement default for cooperative code — a way
//!   to make the right headers happen by default — not as a boundary against
//!   hostile code. The boundary is the permission system.
//! - **Policy values are not secrets.** `DENO_EGRESS_HEADER_POLICY` is
//!   readable by user code under `--allow-env` and is inherited by every
//!   subprocess. Do not put credentials in it. (The CLI's env-file denylist
//!   stops an `.env` file from *setting* the variable; it does nothing to hide
//!   an already-set one.)

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

/// Headers whose manipulation would corrupt the HTTP request itself: message
/// framing, hop-by-hop connection management, and the declared encoding of the
/// body. The policy exists to manage application-level headers; these stay
/// with the HTTP stack.
///
/// A forbidden name is rejected for every op, `remove` included. Allowing a
/// `remove` of, say, `content-encoding` would be harmless on its own, but the
/// whole list is refused at parse time so an operator gets one loud error
/// rather than a policy that silently applies to some ops and not others.
const FORBIDDEN_HEADERS: [HeaderName; 11] = [
  header::HOST,
  header::CONTENT_LENGTH,
  header::TRANSFER_ENCODING,
  header::CONNECTION,
  header::UPGRADE,
  header::TE,
  header::TRAILER,
  header::EXPECT,
  // Describes how the body is encoded; setting it mislabels every request the
  // policy touches, and the body is not the policy's to describe.
  header::CONTENT_ENCODING,
  // No `http::header` constant for these two.
  HeaderName::from_static("keep-alive"),
  HeaderName::from_static("proxy-connection"),
];

/// Credential-bearing headers that `httpRedirectFetch` drops when a redirect
/// crosses origins. Must stay in sync with `REDIRECT_SENSITIVE_HEADER_NAMES`
/// in `26_fetch.js`; see [`EgressHeaderPolicy::apply_static`].
const REDIRECT_SENSITIVE_HEADERS: [HeaderName; 3] = [
  header::AUTHORIZATION,
  header::PROXY_AUTHORIZATION,
  header::COOKIE,
];

#[derive(Debug)]
pub struct EgressHeaderPolicy {
  set: Vec<(HeaderName, HeaderValue)>,
  set_if_absent: Vec<(HeaderName, HeaderValue)>,
  /// Names scrubbed and re-managed per hop in Rust: `remove` ∪ `set`.
  scrub_static: Vec<HeaderName>,
  /// Names whose JS-selected values are preserved while Rust finishes request
  /// construction, then scrubbed and restored afterwards: `forward` ∪
  /// `append`.
  scrub_dynamic: Vec<HeaderName>,
  /// Lowercase names whose inbound values are forwarded, applied in JS.
  forward: Vec<String>,
  /// (lowercase name, value) pairs appended once per outermost fetch, in JS.
  append: Vec<(String, String)>,
  /// Whether any `set`/`default` entry names a [`REDIRECT_SENSITIVE_HEADERS`]
  /// header. Almost always false, which lets [`Self::apply_static`] skip the
  /// per-entry check entirely.
  writes_redirect_sensitive: bool,
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
    let mut scrub_dynamic = forward.clone();
    for (name, _) in &append {
      if !scrub_dynamic.contains(name) {
        scrub_dynamic.push(name.clone());
      }
    }

    let writes_redirect_sensitive = set
      .iter()
      .chain(set_if_absent.iter())
      .any(|(n, _)| REDIRECT_SENSITIVE_HEADERS.contains(n));

    Ok(Self {
      set,
      set_if_absent,
      scrub_static,
      scrub_dynamic,
      forward: forward.iter().map(|n| n.as_str().to_string()).collect(),
      append: append
        .into_iter()
        .map(|(n, v)| {
          // `validated_value` restricts policy values to visible ASCII, so
          // the bytes are valid UTF-8 and this conversion is exact. It is
          // also total: a lossy conversion cannot panic if that ever changes.
          let value = String::from_utf8_lossy(v.as_bytes()).into_owned();
          (n.as_str().to_string(), value)
        })
        .collect(),
      writes_redirect_sensitive,
    })
  }

  /// Applies the idempotent ops (`remove`/`set` scrub, `set`, `default`).
  /// Runs in `op_fetch` for every network hop.
  ///
  /// `redirect_sensitive_stripped` is set once a redirect has crossed origins
  /// and `httpRedirectFetch` has dropped the credential headers. Per
  /// <https://fetch.spec.whatwg.org/#http-redirect-fetch> those headers must
  /// stay gone for the rest of the chain, so `set`/`default` entries naming
  /// them are skipped from that hop on — otherwise an operator-injected
  /// credential would be re-attached and delivered to the redirect target.
  /// The scrub is unaffected: a `remove` of a credential header is an
  /// explicit instruction that still holds after the origin change.
  pub fn apply_static(
    &self,
    headers: &mut HeaderMap,
    redirect_sensitive_stripped: bool,
  ) {
    scrub_headers(headers, &self.scrub_static);
    let skip_sensitive =
      redirect_sensitive_stripped && self.writes_redirect_sensitive;
    for (name, value) in &self.set {
      if skip_sensitive && REDIRECT_SENSITIVE_HEADERS.contains(name) {
        continue;
      }
      headers.insert(name, value.clone());
    }
    for (name, value) in &self.set_if_absent {
      if skip_sensitive && REDIRECT_SENSITIVE_HEADERS.contains(name) {
        continue;
      }
      if !headers.contains_key(name) {
        headers.insert(name, value.clone());
      }
    }
  }

  /// Whether a header carries a JS-selected `forward`/`append` value that
  /// must be preserved separately while Rust finishes request construction.
  pub(crate) fn owns_dynamic_header(&self, name: &HeaderName) -> bool {
    self.scrub_dynamic.contains(name)
  }

  /// Reasserts the JS-selected `forward`/`append` values after every other
  /// request-construction step. Values in `policy_headers` came from the
  /// already-scrubbed JS header list, before URL credentials and the embedder
  /// hook could add competing values.
  pub(crate) fn apply_dynamic(
    &self,
    headers: &mut HeaderMap,
    policy_headers: &HeaderMap,
  ) {
    scrub_headers(headers, &self.scrub_dynamic);
    for (name, value) in policy_headers {
      headers.append(name, value.clone());
    }
  }

  /// The JS-applied part of the policy: header names to forward from the
  /// inbound request context and (name, value) pairs to append, both
  /// lowercase. Empty lists mean the JS fast path can skip entirely.
  pub fn forward_config(&self) -> (&[String], &[(String, String)]) {
    (&self.forward, &self.append)
  }
}

fn scrub_headers(headers: &mut HeaderMap, names: &[HeaderName]) {
  for name in names {
    if let header::Entry::Occupied(entry) = headers.entry(name) {
      entry.remove_entry_mult();
    }
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

/// Parses a policy header value, restricted to visible ASCII (plus tab).
///
/// [`HeaderValue::from_str`] alone also accepts the deprecated obs-text range
/// (`0x80..=0xFF`), which a non-ASCII character in the policy JSON encodes
/// to. Such a value has no single interpretation across the two application
/// points — the Rust side would put the UTF-8 bytes on the wire while the JS
/// side round-trips through a JS string — so it is rejected here rather than
/// silently differing between `set` and `append`.
fn validated_value(
  name: &HeaderName,
  raw: &str,
) -> Result<HeaderValue, EgressHeaderPolicyError> {
  let invalid =
    || EgressHeaderPolicyError::InvalidHeaderValue(name.as_str().to_string());
  let value = HeaderValue::from_str(raw).map_err(|_| invalid())?;
  value.to_str().map_err(|_| invalid())?;
  Ok(value)
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
    let value = validated_value(&name, raw_value)?;
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
    policy.apply_static(&mut headers, false);
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

  // `HeaderValue::from_str` accepts the deprecated obs-text range, which a
  // non-ASCII character encodes to, while `to_str` does not. Every op has to
  // reject such a value: the JS-applied half converts it back to a string,
  // and only visible ASCII survives that round trip intact.
  #[test]
  fn rejects_non_ascii_header_value() {
    for field in ["set", "append", "default"] {
      let json = format!(r#"{{"{field}": {{"x-a": "café"}}}}"#);
      let err = EgressHeaderPolicy::parse(&json).unwrap_err();
      assert!(
        matches!(err, EgressHeaderPolicyError::InvalidHeaderValue(ref n) if n == "x-a"),
        "expected InvalidHeaderValue for {json}, got {err:?}"
      );
    }
  }

  #[test]
  fn accepts_full_visible_ascii_value() {
    let policy =
      EgressHeaderPolicy::parse(r#"{"append": {"x-a": "a\tb ~!$%^&*()_+-="}}"#)
        .unwrap();
    let (_, append) = policy.forward_config();
    assert_eq!(
      append,
      &[("x-a".to_string(), "a\tb ~!$%^&*()_+-=".to_string())]
    );
  }

  #[test]
  fn rejects_forbidden_headers() {
    for json in [
      r#"{"set": {"Host": "example.com"}}"#,
      r#"{"remove": ["content-length"]}"#,
      r#"{"forward": ["transfer-encoding"]}"#,
      r#"{"set": {"connection": "close"}}"#,
    ] {
      let err = EgressHeaderPolicy::parse(json).unwrap_err();
      assert!(
        matches!(err, EgressHeaderPolicyError::ForbiddenHeader(_)),
        "expected ForbiddenHeader for {json}"
      );
    }
  }

  // Every forbidden name is refused in every op, so an operator cannot corrupt
  // framing, hop-by-hop connection management, or the declared body encoding
  // from any direction. `content-encoding` is the one that bites quietly: it
  // would mislabel the body of every request the policy touches.
  #[test]
  fn rejects_every_forbidden_header_in_every_op() {
    for name in [
      "host",
      "content-length",
      "transfer-encoding",
      "connection",
      "upgrade",
      "te",
      "trailer",
      "expect",
      "content-encoding",
      "keep-alive",
      "proxy-connection",
    ] {
      for json in [
        format!(r#"{{"remove": ["{name}"]}}"#),
        format!(r#"{{"forward": ["{name}"]}}"#),
        format!(r#"{{"set": {{"{name}": "x"}}}}"#),
        format!(r#"{{"append": {{"{name}": "x"}}}}"#),
        format!(r#"{{"default": {{"{name}": "x"}}}}"#),
      ] {
        let err = EgressHeaderPolicy::parse(&json).unwrap_err();
        assert!(
          matches!(err, EgressHeaderPolicyError::ForbiddenHeader(ref n) if n == name),
          "expected ForbiddenHeader({name}) for {json}, got {err:?}"
        );
      }
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
    policy.apply_static(&mut headers, false);

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
    policy.apply_static(&mut headers, false);
    policy.apply_static(&mut headers, false);
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
    policy.apply_static(&mut headers, false);
    assert_eq!(headers.get("user-agent").unwrap(), "user/9.9");

    let mut empty = HeaderMap::new();
    policy.apply_static(&mut empty, false);
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

  #[test]
  fn set_and_default_yield_to_the_cross_origin_redirect_strip() {
    let policy = EgressHeaderPolicy::parse(
      r#"{"set": {"authorization": "Bearer secret"},
          "default": {"cookie": "a=1"},
          "append": {"x-keep": "1"}}"#,
    )
    .unwrap();

    // Before any cross-origin redirect the credentials are applied.
    let mut headers = HeaderMap::new();
    policy.apply_static(&mut headers, false);
    assert_eq!(headers.get("authorization").unwrap(), "Bearer secret");
    assert_eq!(headers.get("cookie").unwrap(), "a=1");

    // Once a hop has crossed origins they stay gone.
    let mut headers = HeaderMap::new();
    policy.apply_static(&mut headers, true);
    assert!(!headers.contains_key("authorization"));
    assert!(!headers.contains_key("cookie"));
  }

  #[test]
  fn redirect_strip_leaves_other_headers_alone() {
    let policy = EgressHeaderPolicy::parse(
      r#"{"set": {"user-agent": "enforced/1.0"}, "remove": ["x-gone"]}"#,
    )
    .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("x-gone", HeaderValue::from_static("1"));
    policy.apply_static(&mut headers, true);
    assert_eq!(headers.get("user-agent").unwrap(), "enforced/1.0");
    assert!(!headers.contains_key("x-gone"));
  }
}
