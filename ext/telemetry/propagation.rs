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

use deno_core::FastStaticString;
use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8_static_strings;
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
fn tracestate_parse(raw: &str) -> Vec<(String, String)> {
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

/// `Map.delete` + `Map.set` moves an existing key to the end, which we
/// replicate by removing any existing entry and pushing the new one. Returns a
/// fresh `Vec` (the JS `set` cloned the internal `Map`).
fn tracestate_set(
  entries: &[(String, String)],
  key: String,
  value: String,
) -> Vec<(String, String)> {
  let mut entries: Vec<(String, String)> =
    entries.iter().filter(|(k, _)| *k != key).cloned().collect();
  entries.push((key, value));
  entries
}

fn tracestate_unset(
  entries: &[(String, String)],
  key: &str,
) -> Vec<(String, String)> {
  entries.iter().filter(|(k, _)| k != key).cloned().collect()
}

fn tracestate_get<'a>(
  entries: &'a [(String, String)],
  key: &str,
) -> Option<&'a str> {
  entries
    .iter()
    .find(|(k, _)| k == key)
    .map(|(_, v)| v.as_str())
}

/// Members are stored in the same insertion order as the JS `Map`
/// (members reversed so the last-seen value wins); serialize walks them in
/// reverse to restore the original left-to-right header order, mirroring the
/// `_keys()` reversal in the JS code.
fn tracestate_serialize(entries: &[(String, String)]) -> String {
  let mut out = String::new();
  for (key, value) in entries.iter().rev() {
    if !out.is_empty() {
      out.push(LIST_MEMBERS_SEPARATOR);
    }
    out.push_str(key);
    out.push(LIST_MEMBER_KEY_VALUE_SPLITTER);
    out.push_str(value);
  }
  out
}

/// Rust-backed implementation of the W3C `TraceState`, replacing the
/// `TraceStateClass` that previously lived in `telemetry.ts`.
///
/// Instances are immutable: `set`/`unset` return a new `OtelTraceState` (the
/// JS implementation cloned its internal `Map` for the same reason), so no
/// interior mutability is required.
pub struct OtelTraceState {
  entries: Vec<(String, String)>,
}

// SAFETY: we're sure this can be GCed (it holds only owned `String`s).
unsafe impl GarbageCollected for OtelTraceState {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OtelTraceState"
  }
}

#[op2]
impl OtelTraceState {
  #[constructor]
  #[cppgc]
  fn new(#[string] raw_trace_state: Option<String>) -> OtelTraceState {
    let entries = match raw_trace_state {
      // Matches `if (rawTraceState) this._parse(...)` — empty strings are
      // falsy in JS and were never parsed.
      Some(raw) if !raw.is_empty() => tracestate_parse(&raw),
      _ => Vec::new(),
    };
    OtelTraceState { entries }
  }

  #[cppgc]
  fn set(
    &self,
    #[string] key: String,
    #[string] value: String,
  ) -> OtelTraceState {
    OtelTraceState {
      entries: tracestate_set(&self.entries, key, value),
    }
  }

  #[cppgc]
  fn unset(&self, #[string] key: String) -> OtelTraceState {
    OtelTraceState {
      entries: tracestate_unset(&self.entries, &key),
    }
  }

  // Returns the string value or `undefined` (not `null`) to match the
  // `Map.prototype.get` semantics that `@opentelemetry/api` consumers expect.
  fn get<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    #[string] key: String,
  ) -> v8::Local<'s, v8::Value> {
    match tracestate_get(&self.entries, &key) {
      Some(value) => v8::String::new(scope, value).unwrap().into(),
      None => v8::undefined(scope).into(),
    }
  }

  #[string]
  fn serialize(&self) -> String {
    tracestate_serialize(&self.entries)
  }
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

// === Baggage object (Rust-backed `Baggage`) =================================

/// A single baggage entry. `value` is owned by Rust; `metadata` is the original
/// JavaScript `BaggageEntryMetadata` value (an object with a `toString()`),
/// kept as a traced reference so its identity and behavior are preserved for
/// `@opentelemetry/api` consumers. We deliberately do not stringify it early.
struct StoredEntry {
  key: String,
  value: String,
  metadata: Option<v8::TracedReference<v8::Value>>,
}

/// Rust-backed implementation of the W3C `Baggage`, replacing the `BaggageImpl`
/// that previously lived in `telemetry.ts`.
///
/// Like the JS original, instances are immutable: `setEntry`/`removeEntry`/
/// `removeEntries`/`clear` each return a brand new `OtelBaggage` and never
/// mutate the receiver, so no interior mutability is required.
pub struct OtelBaggage {
  entries: Vec<StoredEntry>,
}

// SAFETY: every stored `v8` reference (the per-entry metadata) is traced.
unsafe impl GarbageCollected for OtelBaggage {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    for entry in &self.entries {
      if let Some(metadata) = &entry.metadata {
        visitor.trace(metadata);
      }
    }
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OtelBaggage"
  }
}

/// Read the `metadata` JavaScript value into a traced reference, treating
/// `null`/`undefined`/absent as "no metadata" (the JS code only set the
/// property when it was present).
fn read_metadata<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
) -> Option<v8::TracedReference<v8::Value>> {
  let key = v8::String::new(scope, "metadata")?;
  match obj.get(scope, key.into()) {
    Some(value) if !value.is_null_or_undefined() => {
      Some(v8::TracedReference::new(scope, value))
    }
    _ => None,
  }
}

/// Read a `{ value, metadata? }` JavaScript entry object into a `StoredEntry`.
fn read_stored_entry<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: String,
  entry: v8::Local<'s, v8::Value>,
) -> StoredEntry {
  let Ok(obj) = v8::Local::<v8::Object>::try_from(entry) else {
    return StoredEntry {
      key,
      value: String::new(),
      metadata: None,
    };
  };
  let value = v8::String::new(scope, "value")
    .and_then(|k| obj.get(scope, k.into()))
    .filter(|v| !v.is_null_or_undefined())
    .map(|v| v.to_rust_string_lossy(scope))
    .unwrap_or_default();
  let metadata = read_metadata(scope, obj);
  StoredEntry {
    key,
    value,
    metadata,
  }
}

/// Build a fresh `{ value, metadata? }` JavaScript object for an entry. A new
/// object is returned each call (matching the JS `ObjectAssign({}, entry)`
/// copy), while `metadata` keeps the original object identity.
fn make_entry_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  entry: &StoredEntry,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  if let Some(key) = v8::String::new(scope, "value") {
    let value = v8::String::new(scope, &entry.value).unwrap();
    obj.set(scope, key.into(), value.into());
  }
  if let Some(metadata) = &entry.metadata
    && let Some(local) = metadata.get(scope)
    && let Some(key) = v8::String::new(scope, "metadata")
  {
    obj.set(scope, key.into(), local);
  }
  obj
}

/// Clone a metadata reference (a traced reference cannot be cloned without a
/// scope, since a fresh `TracedReference` must be created from the live value).
fn clone_metadata<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  metadata: &Option<v8::TracedReference<v8::Value>>,
) -> Option<v8::TracedReference<v8::Value>> {
  metadata
    .as_ref()
    .and_then(|m| m.get(scope))
    .map(|local| v8::TracedReference::new(scope, local))
}

/// Deep-clone the entry list, used when deriving a new immutable baggage.
fn clone_entries<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  entries: &[StoredEntry],
) -> Vec<StoredEntry> {
  entries
    .iter()
    .map(|e| StoredEntry {
      key: e.key.clone(),
      value: e.value.clone(),
      metadata: clone_metadata(scope, &e.metadata),
    })
    .collect()
}

/// Insert or update an entry with `Map.prototype.set` semantics: re-setting an
/// existing key updates its value/metadata in place (keeping its position),
/// otherwise the entry is appended.
fn upsert(entries: &mut Vec<StoredEntry>, entry: StoredEntry) {
  if let Some(existing) = entries.iter_mut().find(|e| e.key == entry.key) {
    existing.value = entry.value;
    existing.metadata = entry.metadata;
  } else {
    entries.push(entry);
  }
}

#[op2]
impl OtelBaggage {
  // Constructed from an optional array of `[key, { value, metadata? }]` pairs
  // (the same shape `getAllEntries` produces), mirroring the JS constructor
  // that took a `Map<string, BaggageEntry>` and copied each entry.
  #[constructor]
  #[cppgc]
  fn new<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    entries: Option<v8::Local<'s, v8::Value>>,
  ) -> OtelBaggage {
    let mut stored: Vec<StoredEntry> = Vec::new();
    if let Some(entries) = entries
      && let Ok(arr) = v8::Local::<v8::Array>::try_from(entries)
    {
      for i in 0..arr.length() {
        let Some(pair) = arr.get_index(scope, i) else {
          continue;
        };
        let Ok(pair) = v8::Local::<v8::Array>::try_from(pair) else {
          continue;
        };
        let Some(key) = pair.get_index(scope, 0) else {
          continue;
        };
        let Some(entry) = pair.get_index(scope, 1) else {
          continue;
        };
        let key = key.to_rust_string_lossy(scope);
        upsert(&mut stored, read_stored_entry(scope, key, entry));
      }
    }
    OtelBaggage { entries: stored }
  }

  // Returns a fresh `{ value, metadata? }` object copy, or `undefined`, matching
  // the `ObjectAssign({}, entry)` copy the JS code returned.
  fn get_entry<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    #[string] key: String,
  ) -> v8::Local<'s, v8::Value> {
    match self.entries.iter().find(|e| e.key == key) {
      Some(entry) => make_entry_object(scope, entry).into(),
      None => v8::undefined(scope).into(),
    }
  }

  // Returns `[[key, { value, metadata? }], ...]` preserving insertion order.
  fn get_all_entries<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Array> {
    let pairs: Vec<v8::Local<v8::Value>> = self
      .entries
      .iter()
      .map(|entry| {
        let key = v8::String::new(scope, &entry.key).unwrap().into();
        let value = make_entry_object(scope, entry).into();
        v8::Array::new_with_elements(scope, &[key, value]).into()
      })
      .collect();
    v8::Array::new_with_elements(scope, &pairs)
  }

  // Returns a new baggage with `key` set, leaving the receiver untouched.
  #[cppgc]
  fn set_entry<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    #[string] key: String,
    entry: v8::Local<'s, v8::Value>,
  ) -> OtelBaggage {
    let mut entries = clone_entries(scope, &self.entries);
    upsert(&mut entries, read_stored_entry(scope, key, entry));
    OtelBaggage { entries }
  }

  // Returns a new baggage without `key`, leaving the receiver untouched.
  #[cppgc]
  fn remove_entry<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    #[string] key: String,
  ) -> OtelBaggage {
    let mut entries = clone_entries(scope, &self.entries);
    entries.retain(|e| e.key != key);
    OtelBaggage { entries }
  }

  // Returns a new baggage without any of the given keys.
  #[cppgc]
  fn remove_entries<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    #[varargs] keys: Option<&v8::FunctionCallbackArguments<'s>>,
  ) -> OtelBaggage {
    let mut entries = clone_entries(scope, &self.entries);
    if let Some(keys) = keys {
      let keys: Vec<String> = (0..keys.length())
        .map(|i| keys.get(i).to_rust_string_lossy(scope))
        .collect();
      entries.retain(|e| !keys.contains(&e.key));
    }
    OtelBaggage { entries }
  }

  // Returns a new empty baggage.
  #[cppgc]
  fn clear(&self) -> OtelBaggage {
    OtelBaggage {
      entries: Vec::new(),
    }
  }
}

// === Context object (Rust-backed `Context`) =================================

/// A single context entry: a symbol `key` and its associated `value`, both kept
/// as traced references so their V8 identity survives garbage collection. Keys
/// are compared by identity (`===`), matching the `Record<symbol, unknown>` the
/// JS `Context` class used for storage.
struct ContextEntry {
  key: v8::TracedReference<v8::Value>,
  value: v8::TracedReference<v8::Value>,
}

/// Rust-backed implementation of the internal `Context`, replacing the JS
/// `class Context` that stored a `{ __proto__: null }` symbol-keyed record.
///
/// Like the JS original, instances are immutable: `setValue` / `deleteValue`
/// each return a brand new `OtelContext` and never mutate the receiver, so no
/// interior mutability is required. `@opentelemetry/api` only ever creates
/// context keys via `Symbol.for(...)`, but identity comparison is correct for
/// any symbol regardless.
pub struct OtelContext {
  entries: Vec<ContextEntry>,
}

// SAFETY: every stored `v8` reference (each entry's key and value) is traced.
unsafe impl GarbageCollected for OtelContext {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    for entry in &self.entries {
      visitor.trace(&entry.key);
      visitor.trace(&entry.value);
    }
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OtelContext"
  }
}

/// Index of the entry whose key is `===` to `key`, if any.
fn context_find<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  entries: &[ContextEntry],
  key: v8::Local<'s, v8::Value>,
) -> Option<usize> {
  entries
    .iter()
    .position(|e| e.key.get(scope).is_some_and(|k| k.strict_equals(key)))
}

/// Deep-clone the entry list (fresh traced references to the same keys/values),
/// used when deriving a new immutable context.
fn clone_context_entries<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  entries: &[ContextEntry],
) -> Vec<ContextEntry> {
  entries
    .iter()
    .filter_map(|e| {
      let key = e.key.get(scope)?;
      let value = e.value.get(scope)?;
      Some(ContextEntry {
        key: v8::TracedReference::new(scope, key),
        value: v8::TracedReference::new(scope, value),
      })
    })
    .collect()
}

#[op2]
impl OtelContext {
  // `new Context()` — the root context starts empty. The JS constructor also
  // accepted a record to copy, but that path was only ever used internally by
  // `setValue`/`deleteValue`, which are now Rust methods that build the new
  // context directly.
  #[constructor]
  #[cppgc]
  fn new() -> OtelContext {
    OtelContext {
      entries: Vec::new(),
    }
  }

  // `getValue(key)` — returns the stored value, or `undefined` when absent.
  fn get_value<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    key: v8::Local<'s, v8::Value>,
  ) -> v8::Local<'s, v8::Value> {
    let Some(i) = context_find(scope, &self.entries, key) else {
      return v8::undefined(scope).into();
    };
    match self.entries[i].value.get(scope) {
      Some(value) => value,
      None => v8::undefined(scope).into(),
    }
  }

  // `setValue(key, value)` — returns a new context with `key` set, leaving the
  // receiver untouched (immutable copy semantics). Re-setting an existing key
  // updates it in place, keeping its position.
  #[cppgc]
  fn set_value<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    key: v8::Local<'s, v8::Value>,
    value: v8::Local<'s, v8::Value>,
  ) -> OtelContext {
    let mut entries = clone_context_entries(scope, &self.entries);
    match context_find(scope, &entries, key) {
      Some(i) => entries[i].value = v8::TracedReference::new(scope, value),
      None => entries.push(ContextEntry {
        key: v8::TracedReference::new(scope, key),
        value: v8::TracedReference::new(scope, value),
      }),
    }
    OtelContext { entries }
  }

  // `deleteValue(key)` — returns a new context without `key`, leaving the
  // receiver untouched.
  #[cppgc]
  fn delete_value<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    key: v8::Local<'s, v8::Value>,
  ) -> OtelContext {
    let mut entries = clone_context_entries(scope, &self.entries);
    if let Some(i) = context_find(scope, &entries, key) {
      entries.remove(i);
    }
    OtelContext { entries }
  }
}

// === W3C propagators (Rust-backed `TextMapPropagator`s) ======================

// The trace-context version we emit.
const VERSION: &str = "00";

// `@opentelemetry/api` context keys (created with `Symbol.for`, so the global
// symbol registry is shared with the JavaScript `SymbolFor(...)` calls).
const SPAN_KEY: &str = "OpenTelemetry Context Key SPAN";
const SUPPRESS_TRACING_KEY: &str =
  "OpenTelemetry SDK Context Key SUPPRESS_TRACING";
const BAGGAGE_KEY: &str = "OpenTelemetry Baggage Key";

// Constant header names, method names and property keys used on the hot
// inject/extract paths. `FastStaticString` builds these as external one-byte
// V8 strings that V8 caches by resource pointer, so reusing them across calls
// avoids the per-call heap allocation + UTF-8 copy that `v8::String::new`
// performs. The originating JS kept all of these as monomorphic, JIT-cached
// literals; this restores equivalent string reuse in the Rust port.
v8_static_strings! {
  TRACEPARENT_HEADER = "traceparent",
  TRACESTATE_HEADER = "tracestate",
  BAGGAGE_HEADER = "baggage",
  GET = "get",
  SET = "set",
  GET_VALUE = "getValue",
  SET_VALUE = "setValue",
  SPAN_CONTEXT = "spanContext",
  SERIALIZE = "serialize",
  GET_ALL_ENTRIES = "getAllEntries",
  TO_STRING = "toString",
  INJECT = "inject",
  EXTRACT = "extract",
  FIELDS_NAME = "fields",
  WARN = "warn",
  CONSTRUCTOR = "constructor",
  NAME = "name",
  CONSOLE = "console",
  TRACE_ID = "traceId",
  SPAN_ID = "spanId",
  TRACE_FLAGS = "traceFlags",
  IS_REMOTE = "isRemote",
  TRACE_STATE = "traceState",
  VALUE = "value",
  METADATA = "metadata",
}

fn v8_str<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  s: &str,
) -> v8::Local<'s, v8::String> {
  v8::String::new(scope, s).unwrap()
}

/// A cached constant V8 string (see [`v8_static_strings`]).
fn cstr<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  s: FastStaticString,
) -> v8::Local<'s, v8::String> {
  s.v8_string(scope).unwrap()
}

/// `Symbol.for(key)` — the same global symbol the JS code obtained via
/// `SymbolFor(key)`. Used once per propagator (at construction) to seed the
/// cached symbols below; the hot paths reuse those cached references via
/// [`resolve_symbol`] instead of re-hitting the global registry.
fn symbol_for<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: &str,
) -> v8::Local<'s, v8::Symbol> {
  let desc = v8_str(scope, key);
  v8::Symbol::for_key(scope, desc)
}

/// Return the cached `Symbol.for(key)` reference, falling back to a fresh
/// registry lookup only if it was somehow collected (it can't be in practice —
/// the global symbol registry keeps it alive and the propagator traces it).
fn resolve_symbol<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  cached: &v8::TracedReference<v8::Symbol>,
  key: &str,
) -> v8::Local<'s, v8::Symbol> {
  match cached.get(scope) {
    Some(symbol) => symbol,
    None => symbol_for(scope, key),
  }
}

/// `recv[name](...args)`. Returns `None` if `name` is not callable or the call
/// throws (leaving the exception pending so V8 propagates it once we return).
fn call_method<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  recv: v8::Local<'s, v8::Value>,
  name: FastStaticString,
  args: &[v8::Local<'s, v8::Value>],
) -> Option<v8::Local<'s, v8::Value>> {
  let obj = recv.to_object(scope)?;
  let key = cstr(scope, name);
  let func = obj.get(scope, key.into())?;
  let func = v8::Local::<v8::Function>::try_from(func).ok()?;
  func.call(scope, recv, args)
}

/// `obj[name]`, or `None` if `obj` is not an object / the get throws.
fn get_prop<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Value>,
  name: FastStaticString,
) -> Option<v8::Local<'s, v8::Value>> {
  let obj = obj.to_object(scope)?;
  let key = cstr(scope, name);
  obj.get(scope, key.into())
}

/// `obj[name]` coerced to a Rust string, or `None` when the property is
/// absent / null / undefined.
fn get_string_prop<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Value>,
  name: FastStaticString,
) -> Option<String> {
  let v = get_prop(scope, obj, name)?;
  if v.is_null_or_undefined() {
    return None;
  }
  Some(v.to_rust_string_lossy(scope))
}

/// `Array.prototype.join(sep)` semantics (null/undefined elements become "").
fn join_array<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
  sep: &str,
) -> String {
  let Ok(arr) = v8::Local::<v8::Array>::try_from(value) else {
    return value.to_rust_string_lossy(scope);
  };
  let mut parts: Vec<String> = Vec::with_capacity(arr.length() as usize);
  for i in 0..arr.length() {
    match arr.get_index(scope, i) {
      Some(v) if !v.is_null_or_undefined() => {
        parts.push(v.to_rust_string_lossy(scope))
      }
      _ => parts.push(String::new()),
    }
  }
  parts.join(sep)
}

/// `context.getValue(SUPPRESS_TRACING) === true`. `None` means the `getValue`
/// call threw. The (cached) suppress-tracing symbol is passed in by the caller.
fn is_tracing_suppressed<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  context: v8::Local<'s, v8::Value>,
  suppress_key: v8::Local<'s, v8::Symbol>,
) -> Option<bool> {
  let value = call_method(scope, context, GET_VALUE, &[suppress_key.into()])?;
  Some(value.is_true())
}

// --- W3CTraceContextPropagator ----------------------------------------------

/// Rust-backed `W3CTraceContextPropagator`, replacing the JS class that lived in
/// `telemetry.ts`. The inject/extract/fields logic and all getter/setter
/// callback invocation now run in Rust; `NonRecordingSpan` is still a small JS
/// class (an inert span shell), so its constructor is passed in and called from
/// Rust on extraction.
pub struct OtelW3CTraceContextPropagator {
  non_recording_span: v8::TracedReference<v8::Value>,
  // Cached `Symbol.for(...)` references so the hot paths don't re-hit the
  // global symbol registry on every inject/extract.
  span_key: v8::TracedReference<v8::Symbol>,
  suppress_key: v8::TracedReference<v8::Symbol>,
}

// SAFETY: the stored `NonRecordingSpan` constructor and cached symbols are
// traced.
unsafe impl GarbageCollected for OtelW3CTraceContextPropagator {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.non_recording_span);
    visitor.trace(&self.span_key);
    visitor.trace(&self.suppress_key);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OtelW3CTraceContextPropagator"
  }
}

#[op2]
impl OtelW3CTraceContextPropagator {
  #[constructor]
  #[cppgc]
  fn new<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    non_recording_span: v8::Local<'s, v8::Value>,
  ) -> OtelW3CTraceContextPropagator {
    let span_key = symbol_for(scope, SPAN_KEY);
    let suppress_key = symbol_for(scope, SUPPRESS_TRACING_KEY);
    OtelW3CTraceContextPropagator {
      non_recording_span: v8::TracedReference::new(scope, non_recording_span),
      span_key: v8::TracedReference::new(scope, span_key),
      suppress_key: v8::TracedReference::new(scope, suppress_key),
    }
  }

  #[nofast]
  #[reentrant]
  fn inject<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Value>,
    carrier: v8::Local<'s, v8::Value>,
    setter: v8::Local<'s, v8::Value>,
  ) {
    let span_key = resolve_symbol(scope, &self.span_key, SPAN_KEY);
    let Some(span) = call_method(scope, context, GET_VALUE, &[span_key.into()])
    else {
      return;
    };
    if span.is_null_or_undefined() {
      return;
    }
    let Some(span_context) = call_method(scope, span, SPAN_CONTEXT, &[]) else {
      return;
    };
    if span_context.is_null_or_undefined() {
      return;
    }
    let suppress_key =
      resolve_symbol(scope, &self.suppress_key, SUPPRESS_TRACING_KEY);
    let Some(suppressed) = is_tracing_suppressed(scope, context, suppress_key)
    else {
      return;
    };
    if suppressed {
      return;
    }
    let Some(trace_id) = get_string_prop(scope, span_context, TRACE_ID) else {
      return;
    };
    let Some(span_id) = get_string_prop(scope, span_context, SPAN_ID) else {
      return;
    };
    if !(is_valid_trace_id(&trace_id) && is_valid_span_id(&span_id)) {
      return;
    }

    // `Number(spanContext.traceFlags || 0).toString(16)`, prefixed with "0".
    let trace_flags = get_prop(scope, span_context, TRACE_FLAGS)
      .and_then(|v| v.number_value(scope))
      .filter(|f| !f.is_nan())
      .unwrap_or(0.0) as i64;
    let trace_parent =
      format!("{VERSION}-{trace_id}-{span_id}-0{trace_flags:x}");

    let header = cstr(scope, TRACEPARENT_HEADER).into();
    let value = v8_str(scope, &trace_parent).into();
    if call_method(scope, setter, SET, &[carrier, header, value]).is_none() {
      return;
    }

    if let Some(trace_state) = get_prop(scope, span_context, TRACE_STATE)
      && !trace_state.is_null_or_undefined()
    {
      let Some(serialized) = call_method(scope, trace_state, SERIALIZE, &[])
      else {
        return;
      };
      let serialized = serialized.to_rust_string_lossy(scope);
      let header = cstr(scope, TRACESTATE_HEADER).into();
      let value = v8_str(scope, &serialized).into();
      call_method(scope, setter, SET, &[carrier, header, value]);
    }
  }

  #[reentrant]
  fn extract<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Value>,
    carrier: v8::Local<'s, v8::Value>,
    getter: v8::Local<'s, v8::Value>,
  ) -> v8::Local<'s, v8::Value> {
    let header = cstr(scope, TRACEPARENT_HEADER).into();
    let Some(trace_parent_header) =
      call_method(scope, getter, GET, &[carrier, header])
    else {
      return context;
    };
    if !trace_parent_header.boolean_value(scope) {
      return context;
    }
    let trace_parent = if trace_parent_header.is_array() {
      let arr = v8::Local::<v8::Array>::try_from(trace_parent_header).unwrap();
      match arr.get_index(scope, 0) {
        Some(v) => v,
        None => return context,
      }
    } else {
      trace_parent_header
    };
    if !trace_parent.is_string() {
      return context;
    }
    let trace_parent = trace_parent.to_rust_string_lossy(scope);
    let Some(parsed) = parse_traceparent(&trace_parent) else {
      return context;
    };

    let span_context = v8::Object::new(scope);
    set_string(scope, span_context, TRACE_ID, &parsed.trace_id);
    set_string(scope, span_context, SPAN_ID, &parsed.span_id);
    let trace_flags = v8::Integer::new(scope, parsed.trace_flags as i32);
    set_value(scope, span_context, TRACE_FLAGS, trace_flags.into());
    let is_remote = v8::Boolean::new(scope, true);
    set_value(scope, span_context, IS_REMOTE, is_remote.into());

    let header = cstr(scope, TRACESTATE_HEADER).into();
    let Some(trace_state_header) =
      call_method(scope, getter, GET, &[carrier, header])
    else {
      return context;
    };
    if trace_state_header.boolean_value(scope) {
      let state = if trace_state_header.is_array() {
        Some(join_array(scope, trace_state_header, ","))
      } else if trace_state_header.is_string() {
        Some(trace_state_header.to_rust_string_lossy(scope))
      } else {
        None
      };
      let entries = match &state {
        Some(s) if !s.is_empty() => tracestate_parse(s),
        _ => Vec::new(),
      };
      let trace_state =
        deno_core::cppgc::make_cppgc_object(scope, OtelTraceState { entries });
      set_value(scope, span_context, TRACE_STATE, trace_state.into());
    }

    let Some(ctor) = self.non_recording_span.get(scope) else {
      return context;
    };
    let Ok(ctor) = v8::Local::<v8::Function>::try_from(ctor) else {
      return context;
    };
    let Some(span) = ctor.new_instance(scope, &[span_context.into()]) else {
      return context;
    };

    let span_key = resolve_symbol(scope, &self.span_key, SPAN_KEY);
    call_method(scope, context, SET_VALUE, &[span_key.into(), span.into()])
      .unwrap_or(context)
  }

  fn fields<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Array> {
    let elements = [
      cstr(scope, TRACEPARENT_HEADER).into(),
      cstr(scope, TRACESTATE_HEADER).into(),
    ];
    v8::Array::new_with_elements(scope, &elements)
  }
}

fn set_value<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: FastStaticString,
  value: v8::Local<'s, v8::Value>,
) {
  let key = cstr(scope, key);
  obj.set(scope, key.into(), value);
}

fn set_string<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: FastStaticString,
  value: &str,
) {
  let value = v8_str(scope, value).into();
  set_value(scope, obj, key, value);
}

// --- W3CBaggagePropagator ---------------------------------------------------

/// Rust-backed `W3CBaggagePropagator`, replacing the JS class. Parsing,
/// percent-(de)coding, de-duplication and serialization already lived in Rust;
/// now the inject/extract/fields control flow and getter/setter invocation do
/// too. `baggageEntryMetadataFromString` is still a tiny JS helper (it builds
/// the `BaggageEntryMetadata` object whose identity/`toString()` must be
/// preserved), so it is passed in and called from Rust on extraction.
pub struct OtelW3CBaggagePropagator {
  metadata_from_string: v8::TracedReference<v8::Value>,
  // Cached `Symbol.for(...)` references (see the trace-context propagator).
  baggage_key: v8::TracedReference<v8::Symbol>,
  suppress_key: v8::TracedReference<v8::Symbol>,
}

// SAFETY: the stored `baggageEntryMetadataFromString` reference and the cached
// symbols are traced.
unsafe impl GarbageCollected for OtelW3CBaggagePropagator {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.metadata_from_string);
    visitor.trace(&self.baggage_key);
    visitor.trace(&self.suppress_key);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OtelW3CBaggagePropagator"
  }
}

#[op2]
impl OtelW3CBaggagePropagator {
  #[constructor]
  #[cppgc]
  fn new<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    metadata_from_string: v8::Local<'s, v8::Value>,
  ) -> OtelW3CBaggagePropagator {
    let baggage_key = symbol_for(scope, BAGGAGE_KEY);
    let suppress_key = symbol_for(scope, SUPPRESS_TRACING_KEY);
    OtelW3CBaggagePropagator {
      metadata_from_string: v8::TracedReference::new(
        scope,
        metadata_from_string,
      ),
      baggage_key: v8::TracedReference::new(scope, baggage_key),
      suppress_key: v8::TracedReference::new(scope, suppress_key),
    }
  }

  #[nofast]
  #[reentrant]
  fn inject<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Value>,
    carrier: v8::Local<'s, v8::Value>,
    setter: v8::Local<'s, v8::Value>,
  ) {
    let baggage_key = resolve_symbol(scope, &self.baggage_key, BAGGAGE_KEY);
    let Some(baggage) =
      call_method(scope, context, GET_VALUE, &[baggage_key.into()])
    else {
      return;
    };
    if !baggage.boolean_value(scope) {
      return;
    }
    let suppress_key =
      resolve_symbol(scope, &self.suppress_key, SUPPRESS_TRACING_KEY);
    let Some(suppressed) = is_tracing_suppressed(scope, context, suppress_key)
    else {
      return;
    };
    if suppressed {
      return;
    }

    let Some(all_entries) = call_method(scope, baggage, GET_ALL_ENTRIES, &[])
    else {
      return;
    };
    let Ok(all_entries) = v8::Local::<v8::Array>::try_from(all_entries) else {
      return;
    };

    let mut entries: Vec<JsBaggageEntry> = Vec::new();
    for i in 0..all_entries.length() {
      let Some(pair) = all_entries.get_index(scope, i) else {
        return;
      };
      let Ok(pair) = v8::Local::<v8::Array>::try_from(pair) else {
        continue;
      };
      let Some(key) = pair.get_index(scope, 0) else {
        return;
      };
      let Some(entry) = pair.get_index(scope, 1) else {
        return;
      };
      let key = key.to_rust_string_lossy(scope);
      let value = get_string_prop(scope, entry, VALUE).unwrap_or_default();
      let metadata = match get_prop(scope, entry, METADATA) {
        Some(metadata) if !metadata.is_undefined() => {
          let Some(s) = call_method(scope, metadata, TO_STRING, &[]) else {
            return;
          };
          Some(s.to_rust_string_lossy(scope))
        }
        _ => None,
      };
      entries.push(JsBaggageEntry {
        key,
        value,
        metadata,
      });
    }

    let header_value = baggage_serialize(&entries);
    if !header_value.is_empty() {
      let header = cstr(scope, BAGGAGE_HEADER).into();
      let value = v8_str(scope, &header_value).into();
      call_method(scope, setter, SET, &[carrier, header, value]);
    }
  }

  #[reentrant]
  fn extract<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Value>,
    carrier: v8::Local<'s, v8::Value>,
    getter: v8::Local<'s, v8::Value>,
  ) -> Result<v8::Local<'s, v8::Value>, JsErrorBox> {
    let header = cstr(scope, BAGGAGE_HEADER).into();
    let Some(header_value) =
      call_method(scope, getter, GET, &[carrier, header])
    else {
      return Ok(context);
    };
    if header_value.is_null_or_undefined() {
      return Ok(context);
    }
    let baggage_string = if header_value.is_array() {
      join_array(scope, header_value, ",")
    } else {
      header_value.to_rust_string_lossy(scope)
    };
    if baggage_string.is_empty() {
      return Ok(context);
    }

    // Parsing / percent-decoding / de-duplication is performed in Rust and may
    // throw on a malformed percent-encoding (`URIError`), matching the JS op.
    let parsed = baggage_parse(&baggage_string)?;
    if parsed.is_empty() {
      return Ok(context);
    }

    let mut entries: Vec<StoredEntry> = Vec::with_capacity(parsed.len());
    for entry in parsed {
      let metadata = match &entry.metadata {
        Some(metadata) => self.make_metadata(scope, metadata),
        None => None,
      };
      entries.push(StoredEntry {
        key: entry.key,
        value: entry.value,
        metadata,
      });
    }
    let baggage =
      deno_core::cppgc::make_cppgc_object(scope, OtelBaggage { entries });

    let baggage_key = resolve_symbol(scope, &self.baggage_key, BAGGAGE_KEY);
    Ok(
      call_method(
        scope,
        context,
        SET_VALUE,
        &[baggage_key.into(), baggage.into()],
      )
      .unwrap_or(context),
    )
  }

  fn fields<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Array> {
    let elements = [cstr(scope, BAGGAGE_HEADER).into()];
    v8::Array::new_with_elements(scope, &elements)
  }
}

impl OtelW3CBaggagePropagator {
  /// `baggageEntryMetadataFromString(value)`, returning a traced reference to
  /// the resulting `BaggageEntryMetadata` object (so its identity / `toString()`
  /// survive, exactly as in the JS implementation).
  fn make_metadata<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    value: &str,
  ) -> Option<v8::TracedReference<v8::Value>> {
    let ctor = self.metadata_from_string.get(scope)?;
    let ctor = v8::Local::<v8::Function>::try_from(ctor).ok()?;
    let undefined = v8::undefined(scope).into();
    let arg = v8_str(scope, value).into();
    let metadata = ctor.call(scope, undefined, &[arg])?;
    Some(v8::TracedReference::new(scope, metadata))
  }
}

// --- CompositePropagator ----------------------------------------------------

/// Rust-backed `CompositePropagator`, replacing the JS class. It fans inject /
/// extract out to the wrapped propagators (each a JS `TextMapPropagator`, in
/// practice the Rust-backed propagators above) and absorbs per-propagator
/// failures with a `console.warn`, matching the JS behavior.
pub struct OtelCompositePropagator {
  propagators: Vec<v8::TracedReference<v8::Value>>,
  fields: Vec<String>,
}

// SAFETY: every wrapped propagator reference is traced.
unsafe impl GarbageCollected for OtelCompositePropagator {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    for propagator in &self.propagators {
      visitor.trace(propagator);
    }
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OtelCompositePropagator"
  }
}

#[op2]
impl OtelCompositePropagator {
  #[constructor]
  #[cppgc]
  #[reentrant]
  fn new<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    propagators: v8::Local<'s, v8::Value>,
  ) -> OtelCompositePropagator {
    let mut stored: Vec<v8::TracedReference<v8::Value>> = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    if let Ok(arr) = v8::Local::<v8::Array>::try_from(propagators) {
      for i in 0..arr.length() {
        let Some(propagator) = arr.get_index(scope, i) else {
          continue;
        };
        if propagator.is_null_or_undefined() {
          continue;
        }
        stored.push(v8::TracedReference::new(scope, propagator));
        // Union of each propagator's `fields()`, de-duplicated in order.
        if let Some(fields_value) =
          call_method(scope, propagator, FIELDS_NAME, &[])
          && let Ok(fields_arr) = v8::Local::<v8::Array>::try_from(fields_value)
        {
          for j in 0..fields_arr.length() {
            if let Some(field) = fields_arr.get_index(scope, j) {
              let field = field.to_rust_string_lossy(scope);
              if !fields.contains(&field) {
                fields.push(field);
              }
            }
          }
        }
      }
    }
    OtelCompositePropagator {
      propagators: stored,
      fields,
    }
  }

  #[nofast]
  #[reentrant]
  fn inject<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Value>,
    carrier: v8::Local<'s, v8::Value>,
    setter: v8::Local<'s, v8::Value>,
  ) {
    for i in 0..self.propagators.len() {
      let Some(propagator) = self.propagators[i].get(scope) else {
        continue;
      };
      v8::tc_scope!(tc, scope);
      if call_method(tc, propagator, INJECT, &[context, carrier, setter])
        .is_none()
        && tc.has_caught()
      {
        let exception = tc.exception().unwrap();
        tc.reset();
        warn_failed(tc, propagator, "inject", exception);
      }
    }
  }

  #[reentrant]
  fn extract<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Value>,
    carrier: v8::Local<'s, v8::Value>,
    getter: v8::Local<'s, v8::Value>,
  ) -> v8::Local<'s, v8::Value> {
    let mut ctx = context;
    for i in 0..self.propagators.len() {
      let Some(propagator) = self.propagators[i].get(scope) else {
        continue;
      };
      v8::tc_scope!(tc, scope);
      match call_method(tc, propagator, EXTRACT, &[ctx, carrier, getter]) {
        Some(result) => ctx = result,
        None => {
          if tc.has_caught() {
            let exception = tc.exception().unwrap();
            tc.reset();
            warn_failed(tc, propagator, "extract", exception);
          }
        }
      }
    }
    ctx
  }

  fn fields<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Array> {
    let mut elements: Vec<v8::Local<v8::Value>> =
      Vec::with_capacity(self.fields.len());
    for field in &self.fields {
      elements.push(v8_str(scope, field).into());
    }
    v8::Array::new_with_elements(scope, &elements)
  }
}

/// `console.warn(\`Failed to ${verb} with ${propagator.constructor.name}.\`, err)`.
fn warn_failed<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  propagator: v8::Local<'s, v8::Value>,
  verb: &str,
  error: v8::Local<'s, v8::Value>,
) {
  let name = get_prop(scope, propagator, CONSTRUCTOR)
    .and_then(|ctor| get_string_prop(scope, ctor, NAME))
    .unwrap_or_default();
  let message = v8_str(scope, &format!("Failed to {verb} with {name}.")).into();
  let global = scope.get_current_context().global(scope);
  if let Some(console) = get_prop(scope, global.into(), CONSOLE) {
    call_method(scope, console, WARN, &[message, error]);
  }
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
  fn tracestate_object_serialize_round_trip() {
    // Stored reversed (last wins), serialize restores left-to-right order.
    let entries = tracestate_parse("foo=1,bar=2");
    assert_eq!(tracestate_serialize(&entries), "foo=1,bar=2");
  }

  #[test]
  fn tracestate_object_empty() {
    assert_eq!(tracestate_serialize(&[]), "");
  }

  #[test]
  fn tracestate_object_set_appends_to_front_of_header() {
    // `set` inserts at the end of the internal (reversed) storage, so the new
    // member appears first in the serialized header, matching the JS behavior.
    let entries =
      tracestate_set(&tracestate_parse("foo=1"), "bar".into(), "2".into());
    assert_eq!(tracestate_serialize(&entries), "bar=2,foo=1");
  }

  #[test]
  fn tracestate_object_set_existing_key_moves_it() {
    // Re-setting an existing key deletes and re-adds it (moving it to the end
    // of the internal storage), exactly like `Map.delete` + `Map.set`.
    let entries = tracestate_set(
      &tracestate_parse("foo=1,bar=2"),
      "foo".into(),
      "9".into(),
    );
    assert_eq!(tracestate_serialize(&entries), "foo=9,bar=2");
  }

  #[test]
  fn tracestate_object_unset_and_get() {
    let parsed = tracestate_parse("foo=1,bar=2");
    assert_eq!(tracestate_get(&parsed, "foo"), Some("1"));
    assert_eq!(tracestate_get(&parsed, "missing"), None);
    let entries = tracestate_unset(&parsed, "foo");
    assert_eq!(tracestate_serialize(&entries), "bar=2");
    assert_eq!(tracestate_get(&entries, "foo"), None);
  }

  #[test]
  fn tracestate_object_clone_is_independent() {
    // `set`/`unset` must not mutate the receiver (the JS code cloned its Map).
    let base = tracestate_parse("foo=1");
    let derived = tracestate_set(&base, "bar".into(), "2".into());
    assert_eq!(tracestate_serialize(&base), "foo=1");
    assert_eq!(tracestate_serialize(&derived), "bar=2,foo=1");
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
  fn baggage_upsert_map_semantics() {
    // `upsert` mirrors `Map.prototype.set`: a new key is appended, while
    // re-setting an existing key updates its value in place (keeping order).
    let mut entries: Vec<StoredEntry> = Vec::new();
    let mk = |k: &str, v: &str| StoredEntry {
      key: k.to_string(),
      value: v.to_string(),
      metadata: None,
    };
    upsert(&mut entries, mk("a", "1"));
    upsert(&mut entries, mk("b", "2"));
    upsert(&mut entries, mk("a", "3"));
    let snapshot: Vec<(&str, &str)> = entries
      .iter()
      .map(|e| (e.key.as_str(), e.value.as_str()))
      .collect();
    // `a` keeps its leading position but takes the new value.
    assert_eq!(snapshot, vec![("a", "3"), ("b", "2")]);
  }

  #[test]
  fn baggage_skips_invalid() {
    // No `=` and empty members are skipped.
    let entries = baggage_parse("novalue,,=noskey,good=1").unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "good");
  }
}
