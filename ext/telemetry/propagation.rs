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

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
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
