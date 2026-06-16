// Copyright 2018-2026 the Deno authors. MIT license.

//! Pure W3C trace-context / baggage propagation logic, ported from the
//! JavaScript implementation that previously lived in `telemetry.ts`.
//!
//! These helpers are exposed to JavaScript as ops so the heavy string parsing,
//! validation and serialization (regex matching, percent-encoding, trace-state
//! truncation, etc.) runs in Rust instead of being carried in the runtime
//! snapshot. The small amount of remaining JavaScript (the propagator classes
//! themselves) only shuffles values between the JS `Context`/carrier objects
//! and these ops, because it has to invoke user supplied getter/setter
//! callbacks and integrate with `npm:@opentelemetry/api`.
//!
//! The original sources are the OpenTelemetry JS `@opentelemetry/core` and
//! `@opentelemetry/api` packages (Apache-2.0).

use deno_core::op2;
use deno_error::JsErrorBox;
use serde::Deserialize;
use serde::Serialize;

const INVALID_TRACEID: &str = "00000000000000000000000000000000";
const INVALID_SPANID: &str = "0000000000000000";

const MAX_TRACE_STATE_ITEMS: usize = 32;
const MAX_TRACE_STATE_LEN: usize = 512;
const LIST_MEMBERS_SEPARATOR: char = ',';
const LIST_MEMBER_KEY_VALUE_SPLITTER: char = '=';

const BAGGAGE_KEY_PAIR_SEPARATOR: char = '=';
const BAGGAGE_PROPERTIES_SEPARATOR: char = ';';
const BAGGAGE_ITEMS_SEPARATOR: char = ',';
const BAGGAGE_MAX_NAME_VALUE_PAIRS: usize = 180;
const BAGGAGE_MAX_PER_NAME_VALUE_PAIRS: usize = 4096;
const BAGGAGE_MAX_TOTAL_LENGTH: usize = 8192;

/// The number of UTF-16 code units in a string, matching JavaScript's
/// `String.prototype.length`. The original code performed all length checks
/// against this value.
fn utf16_len(s: &str) -> usize {
  s.encode_utf16().count()
}

// === W3C traceparent ========================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsTraceParent {
  trace_id: String,
  span_id: String,
  trace_flags: u8,
}

fn is_lower_hex(s: &str, len: usize) -> bool {
  s.len() == len
    && s
      .bytes()
      .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn all_zero(s: &str) -> bool {
  s.bytes().all(|b| b == b'0')
}

/// Parse a `traceparent` header value, mirroring `TRACE_PARENT_REGEX` and
/// `parseTraceParent` from the OpenTelemetry JS implementation.
pub fn parse_traceparent(traceparent: &str) -> Option<JsTraceParent> {
  // The regex was anchored with an optional single leading/trailing whitespace
  // (`^\s?...\s?$`).
  let trimmed = strip_one_ws(traceparent);

  let mut parts = trimmed.split('-');
  let version = parts.next()?;
  let trace_id = parts.next()?;
  let span_id = parts.next()?;
  let flags = parts.next()?;
  // Whether there is anything after the 4th field (the regex `(-.*)?` group).
  let has_extra = parts.next().is_some();

  // version: `(?!ff)[\da-f]{2}`
  if !is_lower_hex(version, 2) || version == "ff" {
    return None;
  }
  // trace id: `(?![0]{32})[\da-f]{32}`
  if !is_lower_hex(trace_id, 32) || all_zero(trace_id) {
    return None;
  }
  // span id: `(?![0]{16})[\da-f]{16}`
  if !is_lower_hex(span_id, 16) || all_zero(span_id) {
    return None;
  }
  // flags: `[\da-f]{2}`
  if !is_lower_hex(flags, 2) {
    return None;
  }

  // For version "00" any trailing data is rejected (future versions allow it).
  if version == "00" && has_extra {
    return None;
  }

  let trace_flags = u8::from_str_radix(flags, 16).ok()?;
  Some(JsTraceParent {
    trace_id: trace_id.to_string(),
    span_id: span_id.to_string(),
    trace_flags,
  })
}

/// Remove at most one leading and one trailing whitespace character, matching
/// the `\s?` anchors in the original regex (any additional whitespace then
/// fails the hex validation, exactly as the regex would fail to match).
fn strip_one_ws(s: &str) -> &str {
  let mut s = s;
  if let Some(c) = s.chars().next()
    && c.is_whitespace()
  {
    s = &s[c.len_utf8()..];
  }
  if let Some(c) = s.chars().next_back()
    && c.is_whitespace()
  {
    s = &s[..s.len() - c.len_utf8()];
  }
  s
}

// === Trace id / span id validation (case-insensitive) =======================

fn is_hex_ci(s: &str, len: usize) -> bool {
  s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_valid_trace_id(trace_id: &str) -> bool {
  is_hex_ci(trace_id, 32) && trace_id != INVALID_TRACEID
}

fn is_valid_span_id(span_id: &str) -> bool {
  is_hex_ci(span_id, 16) && span_id != INVALID_SPANID
}

// === Trace state ============================================================

const TRACE_STATE_KEY_MAX_CHARS: usize = 256;
const TRACE_STATE_VENDOR_KEY_MAX_CHARS: usize = 241;
const TRACE_STATE_VENDOR_SUFFIX_MAX_CHARS: usize = 14;

fn is_valid_key_char(b: u8) -> bool {
  // VALID_KEY_CHAR_RANGE = `[_0-9a-z-*/]`
  b == b'_'
    || b.is_ascii_digit()
    || b.is_ascii_lowercase()
    || b == b'-'
    || b == b'*'
    || b == b'/'
}

/// Validate a trace-state member key against `VALID_KEY_REGEX`:
/// either `[a-z][_0-9a-z-*/]{0,255}` or
/// `[a-z0-9][_0-9a-z-*/]{0,240}@[a-z][_0-9a-z-*/]{0,13}`.
fn validate_trace_state_key(key: &str) -> bool {
  let bytes = key.as_bytes();
  if let Some(at) = key.find('@') {
    // Vendor key: <prefix>@<suffix>
    let prefix = &bytes[..at];
    let suffix = &bytes[at + 1..];
    if prefix.is_empty() || prefix.len() > TRACE_STATE_VENDOR_KEY_MAX_CHARS {
      return false;
    }
    if !(prefix[0].is_ascii_lowercase() || prefix[0].is_ascii_digit()) {
      return false;
    }
    if !prefix[1..].iter().all(|&b| is_valid_key_char(b)) {
      return false;
    }
    if suffix.is_empty() || suffix.len() > TRACE_STATE_VENDOR_SUFFIX_MAX_CHARS {
      return false;
    }
    if !suffix[0].is_ascii_lowercase() {
      return false;
    }
    suffix[1..].iter().all(|&b| is_valid_key_char(b))
  } else {
    if bytes.is_empty() || bytes.len() > TRACE_STATE_KEY_MAX_CHARS {
      return false;
    }
    if !bytes[0].is_ascii_lowercase() {
      return false;
    }
    bytes[1..].iter().all(|&b| is_valid_key_char(b))
  }
}

/// Validate a trace-state member value against `VALID_VALUE_BASE_REGEX`
/// (`^[ -~]{0,255}[!-~]$`) and the additional constraint that it must not
/// contain `,` or `=`.
fn validate_trace_state_value(value: &str) -> bool {
  let bytes = value.as_bytes();
  // Length 1..=256, every byte in printable ASCII (0x20..=0x7e), the last byte
  // must not be a space (0x21..=0x7e).
  if bytes.is_empty() || bytes.len() > 256 {
    return false;
  }
  if !bytes.iter().all(|&b| (0x20..=0x7e).contains(&b)) {
    return false;
  }
  let last = *bytes.last().unwrap();
  if !(0x21..=0x7e).contains(&last) {
    return false;
  }
  !value.contains(',') && !value.contains('=')
}

/// An insertion-ordered map with `Map.prototype.set` semantics (re-setting an
/// existing key updates its value but keeps its position).
#[derive(Default)]
struct OrderedMap {
  entries: Vec<(String, String)>,
}

impl OrderedMap {
  fn set(&mut self, key: String, value: String) {
    if let Some(e) = self.entries.iter_mut().find(|(k, _)| *k == key) {
      e.1 = value;
    } else {
      self.entries.push((key, value));
    }
  }
}

/// Parse a `tracestate` header value, returning the validated members in the
/// internal storage order used by the original `TraceStateClass`.
///
/// The JavaScript implementation reduced over the reversed list of members into
/// a `Map` (so later members win and the map is in reversed order), then, if
/// more than 32 members remained, reversed and truncated back to the first 32.
pub fn tracestate_parse(raw: &str) -> Vec<(String, String)> {
  if utf16_len(raw) > MAX_TRACE_STATE_LEN {
    return Vec::new();
  }

  let mut map = OrderedMap::default();
  for part in raw.split(LIST_MEMBERS_SEPARATOR).rev() {
    let list_member = part.trim();
    if let Some(i) = list_member.find(LIST_MEMBER_KEY_VALUE_SPLITTER) {
      let key = &list_member[..i];
      let value = &list_member[i + 1..];
      if validate_trace_state_key(key) && validate_trace_state_value(value) {
        map.set(key.to_string(), value.to_string());
      }
    }
  }

  if map.entries.len() > MAX_TRACE_STATE_ITEMS {
    // Reverse back to original order and keep the first 32.
    map.entries.reverse();
    map.entries.truncate(MAX_TRACE_STATE_ITEMS);
  }

  map.entries
}

// === Baggage ================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct JsBaggageEntry {
  key: String,
  value: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  metadata: Option<String>,
}

/// Percent-decode a string with the semantics of `decodeURIComponent`: each
/// `%XX` sequence is a byte, the resulting byte sequence must be valid UTF-8,
/// and a malformed sequence is an error (JS would throw `URIError`).
fn decode_uri_component(input: &str) -> Result<String, JsErrorBox> {
  let bytes = input.as_bytes();
  let mut out = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    let b = bytes[i];
    if b == b'%' {
      if i + 2 >= bytes.len() {
        return Err(JsErrorBox::type_error("URI malformed"));
      }
      let hi = (bytes[i + 1] as char).to_digit(16);
      let lo = (bytes[i + 2] as char).to_digit(16);
      match (hi, lo) {
        (Some(hi), Some(lo)) => {
          out.push((hi * 16 + lo) as u8);
          i += 3;
        }
        _ => return Err(JsErrorBox::type_error("URI malformed")),
      }
    } else {
      out.push(b);
      i += 1;
    }
  }
  String::from_utf8(out).map_err(|_| JsErrorBox::type_error("URI malformed"))
}

/// Percent-encode a string with the semantics of `encodeURIComponent`. Every
/// character except the unreserved set `A-Za-z0-9-_.!~*'()` is escaped.
fn encode_uri_component(input: &str) -> String {
  fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric()
      || matches!(
        b,
        b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
      )
  }
  let mut out = String::with_capacity(input.len());
  for &b in input.as_bytes() {
    if is_unreserved(b) {
      out.push(b as char);
    } else {
      out.push('%');
      out.push(
        char::from_digit((b >> 4) as u32, 16)
          .unwrap()
          .to_ascii_uppercase(),
      );
      out.push(
        char::from_digit((b & 0xf) as u32, 16)
          .unwrap()
          .to_ascii_uppercase(),
      );
    }
  }
  out
}

/// Parse a single baggage list member (`parsePairKeyValue`).
fn parse_pair_key_value(
  entry: &str,
) -> Result<Option<JsBaggageEntry>, JsErrorBox> {
  let mut value_props = entry.split(BAGGAGE_PROPERTIES_SEPARATOR);
  let Some(key_pair_part) = value_props.next() else {
    return Ok(None);
  };
  if key_pair_part.is_empty() {
    return Ok(None);
  }
  let Some(sep) = key_pair_part.find(BAGGAGE_KEY_PAIR_SEPARATOR) else {
    return Ok(None);
  };
  if sep == 0 {
    return Ok(None);
  }
  let key = decode_uri_component(key_pair_part[..sep].trim())?;
  let value = decode_uri_component(key_pair_part[sep + 1..].trim())?;

  // Any remaining `;`-separated properties form the (opaque) metadata.
  let rest: Vec<&str> = value_props.collect();
  let metadata = if rest.is_empty() {
    None
  } else {
    Some(rest.join(&BAGGAGE_PROPERTIES_SEPARATOR.to_string()))
  };
  Ok(Some(JsBaggageEntry {
    key,
    value,
    metadata,
  }))
}

/// Parse a `baggage` header value into entries, de-duplicated by key with the
/// last value winning while preserving the first-seen position (matching the
/// JS implementation which collected pairs into a plain object).
pub fn baggage_parse(header: &str) -> Result<Vec<JsBaggageEntry>, JsErrorBox> {
  let mut entries: Vec<JsBaggageEntry> = Vec::new();
  for pair in header.split(BAGGAGE_ITEMS_SEPARATOR) {
    if let Some(entry) = parse_pair_key_value(pair)? {
      if let Some(existing) = entries.iter_mut().find(|e| e.key == entry.key) {
        existing.value = entry.value;
        existing.metadata = entry.metadata;
      } else {
        entries.push(entry);
      }
    }
  }
  Ok(entries)
}

/// Serialize baggage entries into a `baggage` header value (`getKeyPairs` +
/// `serializeKeyPairs`).
pub fn baggage_serialize(entries: &[JsBaggageEntry]) -> String {
  let key_pairs = entries.iter().map(|entry| {
    let mut s = format!(
      "{}={}",
      encode_uri_component(&entry.key),
      encode_uri_component(&entry.value)
    );
    if let Some(metadata) = &entry.metadata {
      // Metadata is intentionally not URI-encoded.
      s.push(BAGGAGE_PROPERTIES_SEPARATOR);
      s.push_str(metadata);
    }
    s
  });

  let mut header = String::new();
  let mut count = 0;
  for pair in key_pairs {
    if utf16_len(&pair) > BAGGAGE_MAX_PER_NAME_VALUE_PAIRS {
      continue;
    }
    if count >= BAGGAGE_MAX_NAME_VALUE_PAIRS {
      break;
    }
    count += 1;
    let candidate = if header.is_empty() {
      pair
    } else {
      format!("{header}{BAGGAGE_ITEMS_SEPARATOR}{pair}")
    };
    // Drop members that would push the total length over the limit, but keep
    // checking subsequent (possibly shorter) members.
    if utf16_len(&candidate) > BAGGAGE_MAX_TOTAL_LENGTH {
      continue;
    }
    header = candidate;
  }
  header
}

// === Ops ====================================================================

#[op2]
#[serde]
pub fn op_otel_parse_traceparent(
  #[string] traceparent: String,
) -> Option<JsTraceParent> {
  parse_traceparent(&traceparent)
}

#[op2(fast)]
pub fn op_otel_span_context_valid(
  #[string] trace_id: String,
  #[string] span_id: String,
) -> bool {
  is_valid_trace_id(&trace_id) && is_valid_span_id(&span_id)
}

#[op2]
#[serde]
pub fn op_otel_tracestate_parse(
  #[string] raw: String,
) -> Vec<(String, String)> {
  tracestate_parse(&raw)
}

#[op2]
#[serde]
pub fn op_otel_baggage_parse(
  #[string] header: String,
) -> Result<Vec<JsBaggageEntry>, JsErrorBox> {
  baggage_parse(&header)
}

#[op2]
#[string]
pub fn op_otel_baggage_serialize(
  #[serde] entries: Vec<JsBaggageEntry>,
) -> String {
  baggage_serialize(&entries)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parsed(s: &str) -> Option<(String, String, u8)> {
    parse_traceparent(s).map(|p| (p.trace_id, p.span_id, p.trace_flags))
  }

  #[test]
  fn traceparent_valid() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    assert_eq!(
      parsed(tp),
      Some((
        "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
        "00f067aa0ba902b7".to_string(),
        1
      ))
    );
  }

  #[test]
  fn traceparent_optional_whitespace() {
    let tp = " 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01 ";
    assert!(parsed(tp).is_some());
  }

  #[test]
  fn traceparent_invalid() {
    // all-zero trace id
    assert!(
      parsed("00-00000000000000000000000000000000-00f067aa0ba902b7-01")
        .is_none()
    );
    // all-zero span id
    assert!(
      parsed("00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01")
        .is_none()
    );
    // version ff
    assert!(
      parsed("ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
        .is_none()
    );
    // uppercase not allowed in traceparent
    assert!(
      parsed("00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01")
        .is_none()
    );
    // version 00 must not have trailing data
    assert!(
      parsed("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra")
        .is_none()
    );
    // too few fields
    assert!(parsed("00-4bf92f3577b34da6a3ce929d0e0e4736").is_none());
  }

  #[test]
  fn traceparent_future_version_allows_extra() {
    let tp = "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra";
    assert!(parsed(tp).is_some());
  }

  #[test]
  fn span_context_validity() {
    assert!(is_valid_trace_id("4bf92f3577b34da6a3ce929d0e0e4736"));
    // case-insensitive (unlike traceparent parsing)
    assert!(is_valid_trace_id("4BF92F3577B34DA6A3CE929D0E0E4736"));
    assert!(!is_valid_trace_id(INVALID_TRACEID));
    assert!(!is_valid_trace_id("xyz"));
    assert!(is_valid_span_id("00f067aa0ba902b7"));
    assert!(!is_valid_span_id(INVALID_SPANID));
  }

  #[test]
  fn tracestate_basic() {
    let entries = tracestate_parse("foo=1,bar=2");
    // Stored in reversed order (matching the JS Map insertion order).
    assert_eq!(
      entries,
      vec![
        ("bar".to_string(), "2".to_string()),
        ("foo".to_string(), "1".to_string()),
      ]
    );
  }

  #[test]
  fn tracestate_ows_and_invalid() {
    // Optional whitespace is trimmed; invalid members are dropped.
    let entries = tracestate_parse("foo=1 , BADKEY=2 , baz=3");
    assert_eq!(
      entries,
      vec![
        ("baz".to_string(), "3".to_string()),
        ("foo".to_string(), "1".to_string()),
      ]
    );
  }

  #[test]
  fn tracestate_vendor_key() {
    let entries = tracestate_parse("vendor@ext=value");
    assert_eq!(
      entries,
      vec![("vendor@ext".to_string(), "value".to_string())]
    );
  }

  #[test]
  fn tracestate_too_long() {
    let big = "a".repeat(513);
    assert!(tracestate_parse(&big).is_empty());
  }

  #[test]
  fn tracestate_truncates_to_32() {
    let raw = (0..40)
      .map(|i| format!("k{i}=v{i}"))
      .collect::<Vec<_>>()
      .join(",");
    let entries = tracestate_parse(&raw);
    assert_eq!(entries.len(), 32);
    // First 32 in original order are kept.
    assert_eq!(entries[0].0, "k0");
    assert_eq!(entries[31].0, "k31");
  }

  #[test]
  fn baggage_round_trip() {
    let entries = baggage_parse("key1=value1,key2=value2").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "key1");
    assert_eq!(entries[0].value, "value1");
    let header = baggage_serialize(&entries);
    assert_eq!(header, "key1=value1,key2=value2");
  }

  #[test]
  fn baggage_percent_encoding() {
    let entries = baggage_parse("key=val%20ue").unwrap();
    assert_eq!(entries[0].value, "val ue");
    let header = baggage_serialize(&entries);
    assert_eq!(header, "key=val%20ue");
  }

  #[test]
  fn baggage_metadata() {
    let entries = baggage_parse("key=value;metaprop").unwrap();
    assert_eq!(entries[0].metadata.as_deref(), Some("metaprop"));
    let header = baggage_serialize(&entries);
    assert_eq!(header, "key=value;metaprop");
  }

  #[test]
  fn baggage_dedup_last_wins() {
    let entries = baggage_parse("k=1,k=2").unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value, "2");
  }

  #[test]
  fn baggage_malformed_percent() {
    assert!(baggage_parse("key=%zz").is_err());
  }

  #[test]
  fn baggage_skips_invalid() {
    // No `=` and empty members are skipped.
    let entries = baggage_parse("novalue,,=noskey,good=1").unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "good");
  }
}
