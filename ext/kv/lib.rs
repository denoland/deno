// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub mod codec;
mod interface;
pub mod sqlite;

use std::borrow::Cow;
use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;

use codec::decode_key;
use codec::encode_key;
use deno_core::anyhow::Context;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8::AnyValue;
use deno_core::serde_v8::BigInt;
use deno_core::ByteString;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;

pub use crate::interface::*;

const MAX_WRITE_KEY_SIZE_BYTES: usize = 2048;
// range selectors can contain 0x00 or 0xff suffixes
const MAX_READ_KEY_SIZE_BYTES: usize = MAX_WRITE_KEY_SIZE_BYTES + 1;
const MAX_VALUE_SIZE_BYTES: usize = 65536;
const MAX_READ_RANGES: usize = 10;
const MAX_READ_ENTRIES: usize = 1000;
const MAX_CHECKS: usize = 10;
const MAX_MUTATIONS: usize = 10;

struct UnstableChecker {
  pub unstable: bool,
}

impl UnstableChecker {
  // NOTE(bartlomieju): keep in sync with `cli/program_state.rs`
  pub fn check_unstable(&self, api_name: &str) {
    if !self.unstable {
      eprintln!(
        "Unstable API '{api_name}'. The --unstable flag must be provided."
      );
      std::process::exit(70);
    }
  }
}

deno_core::extension!(deno_kv,
  // TODO(bartlomieju): specify deps
  deps = [ ],
  parameters = [ DBH: DatabaseHandler ],
  ops = [
    op_kv_database_open<DBH>,
    op_kv_snapshot_read<DBH>,
    op_kv_atomic_write<DBH>,
    op_kv_encode_cursor,
  ],
  esm = [ "01_db.ts" ],
  options = {
    handler: DBH,
    unstable: bool,
  },
  state = |state, options| {
    state.put(Rc::new(options.handler));
    state.put(UnstableChecker { unstable: options.unstable })
  }
);

struct DatabaseResource<DB: Database + 'static> {
  db: Rc<DB>,
}

impl<DB: Database + 'static> Resource for DatabaseResource<DB> {
  fn name(&self) -> Cow<str> {
    "database".into()
  }
}

#[op]
async fn op_kv_database_open<DBH>(
  state: Rc<RefCell<OpState>>,
  path: Option<String>,
) -> Result<ResourceId, AnyError>
where
  DBH: DatabaseHandler + 'static,
{
  let handler = {
    let state = state.borrow();
    state
      .borrow::<UnstableChecker>()
      .check_unstable("Deno.openKv");
    state.borrow::<Rc<DBH>>().clone()
  };
  let db = handler.open(state.clone(), path).await?;
  let rid = state
    .borrow_mut()
    .resource_table
    .add(DatabaseResource { db: Rc::new(db) });
  Ok(rid)
}

type KvKey = Vec<AnyValue>;

impl From<AnyValue> for KeyPart {
  fn from(value: AnyValue) -> Self {
    match value {
      AnyValue::Bool(false) => KeyPart::False,
      AnyValue::Bool(true) => KeyPart::True,
      AnyValue::Number(n) => KeyPart::Float(n),
      AnyValue::BigInt(n) => KeyPart::Int(n),
      AnyValue::String(s) => KeyPart::String(s),
      AnyValue::Buffer(buf) => KeyPart::Bytes(buf.to_vec()),
    }
  }
}

impl From<KeyPart> for AnyValue {
  fn from(value: KeyPart) -> Self {
    match value {
      KeyPart::False => AnyValue::Bool(false),
      KeyPart::True => AnyValue::Bool(true),
      KeyPart::Float(n) => AnyValue::Number(n),
      KeyPart::Int(n) => AnyValue::BigInt(n),
      KeyPart::String(s) => AnyValue::String(s),
      KeyPart::Bytes(buf) => AnyValue::Buffer(buf.into()),
    }
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum V8Value {
  V8(ZeroCopyBuf),
  Bytes(ZeroCopyBuf),
  U64(BigInt),
}

impl TryFrom<V8Value> for Value {
  type Error = AnyError;
  fn try_from(value: V8Value) -> Result<Self, AnyError> {
    Ok(match value {
      V8Value::V8(buf) => Value::V8(buf.to_vec()),
      V8Value::Bytes(buf) => Value::Bytes(buf.to_vec()),
      V8Value::U64(n) => Value::U64(num_bigint::BigInt::from(n).try_into()?),
    })
  }
}

impl From<Value> for V8Value {
  fn from(value: Value) -> Self {
    match value {
      Value::V8(buf) => V8Value::V8(buf.into()),
      Value::Bytes(buf) => V8Value::Bytes(buf.into()),
      Value::U64(n) => V8Value::U64(num_bigint::BigInt::from(n).into()),
    }
  }
}

#[derive(Deserialize, Serialize)]
struct V8KvEntry {
  key: KvKey,
  value: V8Value,
  versionstamp: ByteString,
}

impl TryFrom<KvEntry> for V8KvEntry {
  type Error = AnyError;
  fn try_from(entry: KvEntry) -> Result<Self, AnyError> {
    Ok(V8KvEntry {
      key: decode_key(&entry.key)?
        .0
        .into_iter()
        .map(Into::into)
        .collect(),
      value: entry.value.into(),
      versionstamp: hex::encode(entry.versionstamp).into(),
    })
  }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum V8Consistency {
  Strong,
  Eventual,
}

impl From<V8Consistency> for Consistency {
  fn from(value: V8Consistency) -> Self {
    match value {
      V8Consistency::Strong => Consistency::Strong,
      V8Consistency::Eventual => Consistency::Eventual,
    }
  }
}

// (prefix, start, end, limit, reverse, cursor)
type SnapshotReadRange = (
  Option<KvKey>,
  Option<KvKey>,
  Option<KvKey>,
  u32,
  bool,
  Option<ByteString>,
);

#[op]
async fn op_kv_snapshot_read<DBH>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  ranges: Vec<SnapshotReadRange>,
  consistency: V8Consistency,
) -> Result<Vec<Vec<V8KvEntry>>, AnyError>
where
  DBH: DatabaseHandler + 'static,
{
  let db = {
    let state = state.borrow();
    let resource =
      state.resource_table.get::<DatabaseResource<DBH::DB>>(rid)?;
    resource.db.clone()
  };

  if ranges.len() > MAX_READ_RANGES {
    return Err(type_error(format!(
      "too many ranges (max {})",
      MAX_READ_RANGES
    )));
  }

  let mut total_entries = 0usize;

  let read_ranges = ranges
    .into_iter()
    .map(|(prefix, start, end, limit, reverse, cursor)| {
      let selector = RawSelector::from_tuple(prefix, start, end)?;

      let (start, end) =
        decode_selector_and_cursor(&selector, reverse, cursor.as_ref())?;
      check_read_key_size(&start)?;
      check_read_key_size(&end)?;

      total_entries += limit as usize;
      Ok(ReadRange {
        start,
        end,
        limit: NonZeroU32::new(limit)
          .with_context(|| "limit must be greater than 0")?,
        reverse,
      })
    })
    .collect::<Result<Vec<_>, AnyError>>()?;

  if total_entries > MAX_READ_ENTRIES {
    return Err(type_error(format!(
      "too many entries (max {})",
      MAX_READ_ENTRIES
    )));
  }

  let opts = SnapshotReadOptions {
    consistency: consistency.into(),
  };
  let output_ranges = db.snapshot_read(read_ranges, opts).await?;
  let output_ranges = output_ranges
    .into_iter()
    .map(|x| {
      x.entries
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, AnyError>>()
    })
    .collect::<Result<Vec<_>, AnyError>>()?;
  Ok(output_ranges)
}

type V8KvCheck = (KvKey, Option<ByteString>);

impl TryFrom<V8KvCheck> for KvCheck {
  type Error = AnyError;
  fn try_from(value: V8KvCheck) -> Result<Self, AnyError> {
    let versionstamp = match value.1 {
      Some(data) => {
        let mut out = [0u8; 10];
        hex::decode_to_slice(data, &mut out)
          .map_err(|_| type_error("invalid versionstamp"))?;
        Some(out)
      }
      None => None,
    };
    Ok(KvCheck {
      key: encode_v8_key(value.0)?,
      versionstamp,
    })
  }
}

type V8KvMutation = (KvKey, String, Option<V8Value>);

impl TryFrom<V8KvMutation> for KvMutation {
  type Error = AnyError;
  fn try_from(value: V8KvMutation) -> Result<Self, AnyError> {
    let key = encode_v8_key(value.0)?;
    let kind = match (value.1.as_str(), value.2) {
      ("set", Some(value)) => MutationKind::Set(value.try_into()?),
      ("delete", None) => MutationKind::Delete,
      ("sum", Some(value)) => MutationKind::Sum(value.try_into()?),
      ("min", Some(value)) => MutationKind::Min(value.try_into()?),
      ("max", Some(value)) => MutationKind::Max(value.try_into()?),
      (op, Some(_)) => {
        return Err(type_error(format!("invalid mutation '{op}' with value")))
      }
      (op, None) => {
        return Err(type_error(format!(
          "invalid mutation '{op}' without value"
        )))
      }
    };
    Ok(KvMutation { key, kind })
  }
}

type V8Enqueue = (ZeroCopyBuf, u64, Vec<KvKey>, Option<Vec<u32>>);

impl TryFrom<V8Enqueue> for Enqueue {
  type Error = AnyError;
  fn try_from(value: V8Enqueue) -> Result<Self, AnyError> {
    Ok(Enqueue {
      payload: value.0.to_vec(),
      deadline_ms: value.1,
      keys_if_undelivered: value
        .2
        .into_iter()
        .map(encode_v8_key)
        .collect::<std::io::Result<_>>()?,
      backoff_schedule: value.3,
    })
  }
}

fn encode_v8_key(key: KvKey) -> Result<Vec<u8>, std::io::Error> {
  encode_key(&Key(key.into_iter().map(From::from).collect()))
}

enum RawSelector {
  Prefixed {
    prefix: Vec<u8>,
    start: Option<Vec<u8>>,
    end: Option<Vec<u8>>,
  },
  Range {
    start: Vec<u8>,
    end: Vec<u8>,
  },
}

impl RawSelector {
  fn from_tuple(
    prefix: Option<KvKey>,
    start: Option<KvKey>,
    end: Option<KvKey>,
  ) -> Result<Self, AnyError> {
    let prefix = prefix.map(encode_v8_key).transpose()?;
    let start = start.map(encode_v8_key).transpose()?;
    let end = end.map(encode_v8_key).transpose()?;

    match (prefix, start, end) {
      (Some(prefix), None, None) => Ok(Self::Prefixed {
        prefix,
        start: None,
        end: None,
      }),
      (Some(prefix), Some(start), None) => Ok(Self::Prefixed {
        prefix,
        start: Some(start),
        end: None,
      }),
      (Some(prefix), None, Some(end)) => Ok(Self::Prefixed {
        prefix,
        start: None,
        end: Some(end),
      }),
      (None, Some(start), Some(end)) => Ok(Self::Range { start, end }),
      (None, Some(start), None) => {
        let end = start.iter().copied().chain(Some(0)).collect();
        Ok(Self::Range { start, end })
      }
      _ => Err(type_error("invalid range")),
    }
  }

  fn start(&self) -> Option<&[u8]> {
    match self {
      Self::Prefixed { start, .. } => start.as_deref(),
      Self::Range { start, .. } => Some(start),
    }
  }

  fn end(&self) -> Option<&[u8]> {
    match self {
      Self::Prefixed { end, .. } => end.as_deref(),
      Self::Range { end, .. } => Some(end),
    }
  }

  fn common_prefix(&self) -> &[u8] {
    match self {
      Self::Prefixed { prefix, .. } => prefix,
      Self::Range { start, end } => common_prefix_for_bytes(start, end),
    }
  }

  fn range_start_key(&self) -> Vec<u8> {
    match self {
      Self::Prefixed {
        start: Some(start), ..
      } => start.clone(),
      Self::Range { start, .. } => start.clone(),
      Self::Prefixed { prefix, .. } => {
        prefix.iter().copied().chain(Some(0)).collect()
      }
    }
  }

  fn range_end_key(&self) -> Vec<u8> {
    match self {
      Self::Prefixed { end: Some(end), .. } => end.clone(),
      Self::Range { end, .. } => end.clone(),
      Self::Prefixed { prefix, .. } => {
        prefix.iter().copied().chain(Some(0xff)).collect()
      }
    }
  }
}

fn common_prefix_for_bytes<'a>(a: &'a [u8], b: &'a [u8]) -> &'a [u8] {
  let mut i = 0;
  while i < a.len() && i < b.len() && a[i] == b[i] {
    i += 1;
  }
  &a[..i]
}

fn encode_cursor(
  selector: &RawSelector,
  boundary_key: &[u8],
) -> Result<String, AnyError> {
  let common_prefix = selector.common_prefix();
  if !boundary_key.starts_with(common_prefix) {
    return Err(type_error("invalid boundary key"));
  }

  Ok(base64::encode_config(
    &boundary_key[common_prefix.len()..],
    base64::URL_SAFE,
  ))
}

fn decode_selector_and_cursor(
  selector: &RawSelector,
  reverse: bool,
  cursor: Option<&ByteString>,
) -> Result<(Vec<u8>, Vec<u8>), AnyError> {
  let Some(cursor) = cursor else {
    return Ok((selector.range_start_key(), selector.range_end_key()));
  };

  let common_prefix = selector.common_prefix();
  let cursor = base64::decode_config(cursor, base64::URL_SAFE)
    .map_err(|_| type_error("invalid cursor"))?;

  let first_key: Vec<u8>;
  let last_key: Vec<u8>;

  if reverse {
    first_key = selector.range_start_key();
    last_key = common_prefix
      .iter()
      .copied()
      .chain(cursor.iter().copied())
      .collect();
  } else {
    first_key = common_prefix
      .iter()
      .copied()
      .chain(cursor.iter().copied())
      .chain(Some(0))
      .collect();
    last_key = selector.range_end_key();
  }

  // Defend against out-of-bounds reading
  if let Some(start) = selector.start() {
    if &first_key[..] < start {
      return Err(type_error("cursor out of bounds"));
    }
  }

  if let Some(end) = selector.end() {
    if &last_key[..] > end {
      return Err(type_error("cursor out of bounds"));
    }
  }

  Ok((first_key, last_key))
}

#[op]
async fn op_kv_atomic_write<DBH>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  checks: Vec<V8KvCheck>,
  mutations: Vec<V8KvMutation>,
  enqueues: Vec<V8Enqueue>,
) -> Result<Option<String>, AnyError>
where
  DBH: DatabaseHandler + 'static,
{
  let db = {
    let state = state.borrow();
    let resource =
      state.resource_table.get::<DatabaseResource<DBH::DB>>(rid)?;
    resource.db.clone()
  };

  if checks.len() > MAX_CHECKS {
    return Err(type_error(format!("too many checks (max {})", MAX_CHECKS)));
  }

  if mutations.len() + enqueues.len() > MAX_MUTATIONS {
    return Err(type_error(format!(
      "too many mutations (max {})",
      MAX_MUTATIONS
    )));
  }

  let checks = checks
    .into_iter()
    .map(TryInto::try_into)
    .collect::<Result<Vec<KvCheck>, AnyError>>()
    .with_context(|| "invalid check")?;
  let mutations = mutations
    .into_iter()
    .map(TryInto::try_into)
    .collect::<Result<Vec<KvMutation>, AnyError>>()
    .with_context(|| "invalid mutation")?;
  let enqueues = enqueues
    .into_iter()
    .map(TryInto::try_into)
    .collect::<Result<Vec<Enqueue>, AnyError>>()
    .with_context(|| "invalid enqueue")?;

  for key in checks
    .iter()
    .map(|c| &c.key)
    .chain(mutations.iter().map(|m| &m.key))
  {
    if key.is_empty() {
      return Err(type_error("key cannot be empty"));
    }

    check_write_key_size(key)?;
  }

  for value in mutations.iter().flat_map(|m| m.kind.value()) {
    check_value_size(value)?;
  }

  for enqueue in &enqueues {
    check_enqueue_payload_size(&enqueue.payload)?;
  }

  let atomic_write = AtomicWrite {
    checks,
    mutations,
    enqueues,
  };

  let result = db.atomic_write(atomic_write).await?;

  Ok(result.map(|res| hex::encode(res.versionstamp)))
}

// (prefix, start, end)
type EncodeCursorRangeSelector = (Option<KvKey>, Option<KvKey>, Option<KvKey>);

#[op]
fn op_kv_encode_cursor(
  (prefix, start, end): EncodeCursorRangeSelector,
  boundary_key: KvKey,
) -> Result<String, AnyError> {
  let selector = RawSelector::from_tuple(prefix, start, end)?;
  let boundary_key = encode_v8_key(boundary_key)?;
  let cursor = encode_cursor(&selector, &boundary_key)?;
  Ok(cursor)
}

fn check_read_key_size(key: &[u8]) -> Result<(), AnyError> {
  if key.len() > MAX_READ_KEY_SIZE_BYTES {
    Err(type_error(format!(
      "key too large for read (max {} bytes)",
      MAX_READ_KEY_SIZE_BYTES
    )))
  } else {
    Ok(())
  }
}

fn check_write_key_size(key: &[u8]) -> Result<(), AnyError> {
  if key.len() > MAX_WRITE_KEY_SIZE_BYTES {
    Err(type_error(format!(
      "key too large for write (max {} bytes)",
      MAX_WRITE_KEY_SIZE_BYTES
    )))
  } else {
    Ok(())
  }
}

fn check_value_size(value: &Value) -> Result<(), AnyError> {
  let payload = match value {
    Value::Bytes(x) => x,
    Value::V8(x) => x,
    Value::U64(_) => return Ok(()),
  };

  if payload.len() > MAX_VALUE_SIZE_BYTES {
    Err(type_error(format!(
      "value too large (max {} bytes)",
      MAX_VALUE_SIZE_BYTES
    )))
  } else {
    Ok(())
  }
}

fn check_enqueue_payload_size(payload: &[u8]) -> Result<(), AnyError> {
  if payload.len() > MAX_VALUE_SIZE_BYTES {
    Err(type_error(format!(
      "enqueue payload too large (max {} bytes)",
      MAX_VALUE_SIZE_BYTES
    )))
  } else {
    Ok(())
  }
}
