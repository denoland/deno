// Copyright 2018-2025 the Deno authors. MIT license.

pub mod config;
pub mod dynamic;
mod interface;
pub mod remote;
pub mod sqlite;

use std::borrow::Cow;
use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE;
use boxed_error::Boxed;
use chrono::DateTime;
use chrono::Utc;
use deno_core::AsyncRefCell;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ToJsBuffer;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::serde_v8::AnyValue;
use deno_core::serde_v8::BigInt;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_features::FeatureChecker;
use denokv_proto::AtomicWrite;
use denokv_proto::Check;
use denokv_proto::Consistency;
use denokv_proto::Database;
use denokv_proto::Enqueue;
use denokv_proto::Key;
use denokv_proto::KeyPart;
use denokv_proto::KvEntry;
use denokv_proto::KvValue;
use denokv_proto::Mutation;
use denokv_proto::MutationKind;
use denokv_proto::QueueMessageHandle;
use denokv_proto::ReadRange;
use denokv_proto::SnapshotReadOptions;
use denokv_proto::WatchKeyOutput;
use denokv_proto::WatchStream;
use denokv_proto::decode_key;
use denokv_proto::encode_key;
use log::debug;
use serde::Deserialize;
use serde::Serialize;

pub use crate::config::*;
pub use crate::interface::*;

pub const UNSTABLE_FEATURE_NAME: &str = "kv";

deno_core::extension!(deno_kv,
  deps = [ deno_web ],
  parameters = [ DBH: DatabaseHandler ],
  ops = [
    op_kv_database_open<DBH>,
    op_kv_snapshot_read<DBH>,
    op_kv_atomic_write<DBH>,
    op_kv_encode_cursor,
    op_kv_dequeue_next_message<DBH>,
    op_kv_finish_dequeued_message<DBH>,
    op_kv_watch<DBH>,
    op_kv_watch_next,
  ],
  esm = [ "01_db.ts" ],
  options = {
    handler: DBH,
    config: KvConfig,
  },
  state = |state, options| {
    state.put(Rc::new(options.config));
    state.put(Rc::new(options.handler));
  }
);

struct DatabaseResource<DB: Database + 'static> {
  db: DB,
  cancel_handle: Rc<CancelHandle>,
}

impl<DB: Database + 'static> Resource for DatabaseResource<DB> {
  fn name(&self) -> Cow<'_, str> {
    "database".into()
  }

  fn close(self: Rc<Self>) {
    self.db.close();
    self.cancel_handle.cancel();
  }
}

struct DatabaseWatcherResource {
  stream: AsyncRefCell<WatchStream>,
  db_cancel_handle: Rc<CancelHandle>,
  cancel_handle: Rc<CancelHandle>,
}

impl Resource for DatabaseWatcherResource {
  fn name(&self) -> Cow<'_, str> {
    "databaseWatcher".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel()
  }
}

#[derive(Debug, Boxed, deno_error::JsError)]
pub struct KvError(pub Box<KvErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum KvErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  DatabaseHandler(JsErrorBox),
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(type)]
  #[error("Too many ranges (max {0})")]
  TooManyRanges(usize),
  #[class(type)]
  #[error("Too many entries (max {0})")]
  TooManyEntries(usize),
  #[class(type)]
  #[error("Too many checks (max {0})")]
  TooManyChecks(usize),
  #[class(type)]
  #[error("Too many mutations (max {0})")]
  TooManyMutations(usize),
  #[class(type)]
  #[error("Too many keys (max {0})")]
  TooManyKeys(usize),
  #[class(type)]
  #[error("limit must be greater than 0")]
  InvalidLimit,
  #[class(type)]
  #[error("Invalid boundary key")]
  InvalidBoundaryKey,
  #[class(type)]
  #[error("Key too large for read (max {0} bytes)")]
  KeyTooLargeToRead(usize),
  #[class(type)]
  #[error("Key too large for write (max {0} bytes)")]
  KeyTooLargeToWrite(usize),
  #[class(type)]
  #[error("Total mutation size too large (max {0} bytes)")]
  TotalMutationTooLarge(usize),
  #[class(type)]
  #[error("Total key size too large (max {0} bytes)")]
  TotalKeyTooLarge(usize),
  #[class(inherit)]
  #[error(transparent)]
  Kv(JsErrorBox),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(type)]
  #[error("Queue message not found")]
  QueueMessageNotFound,
  #[class(type)]
  #[error("Start key is not in the keyspace defined by prefix")]
  StartKeyNotInKeyspace,
  #[class(type)]
  #[error("End key is not in the keyspace defined by prefix")]
  EndKeyNotInKeyspace,
  #[class(type)]
  #[error("Start key is greater than end key")]
  StartKeyGreaterThanEndKey,
  #[class(inherit)]
  #[error("Invalid check")]
  InvalidCheck(#[source] KvCheckError),
  #[class(inherit)]
  #[error("Invalid mutation")]
  InvalidMutation(#[source] KvMutationError),
  #[class(inherit)]
  #[error("Invalid enqueue")]
  InvalidEnqueue(#[source] std::io::Error),
  #[class(type)]
  #[error("key cannot be empty")]
  EmptyKey,
  #[class(type)]
  #[error("Value too large (max {0} bytes)")]
  ValueTooLarge(usize),
  #[class(type)]
  #[error("enqueue payload too large (max {0} bytes)")]
  EnqueuePayloadTooLarge(usize),
  #[class(type)]
  #[error("invalid cursor")]
  InvalidCursor,
  #[class(type)]
  #[error("cursor out of bounds")]
  CursorOutOfBounds,
  #[class(type)]
  #[error("Invalid range")]
  InvalidRange,
}

#[op2(async, stack_trace)]
#[smi]
async fn op_kv_database_open<DBH>(
  state: Rc<RefCell<OpState>>,
  #[string] path: Option<String>,
) -> Result<ResourceId, KvError>
where
  DBH: DatabaseHandler + 'static,
{
  let handler = {
    let state = state.borrow();
    state
      .borrow::<Arc<FeatureChecker>>()
      .check_or_exit(UNSTABLE_FEATURE_NAME, "Deno.openKv");
    state.borrow::<Rc<DBH>>().clone()
  };
  let db = handler
    .open(state.clone(), path)
    .await
    .map_err(KvErrorKind::DatabaseHandler)?;
  let rid = state.borrow_mut().resource_table.add(DatabaseResource {
    db,
    cancel_handle: CancelHandle::new_rc(),
  });
  Ok(rid)
}

type KvKey = Vec<AnyValue>;

fn key_part_from_v8(value: AnyValue) -> KeyPart {
  match value {
    AnyValue::Bool(false) => KeyPart::False,
    AnyValue::Bool(true) => KeyPart::True,
    AnyValue::Number(n) => KeyPart::Float(n),
    AnyValue::BigInt(n) => KeyPart::Int(n),
    AnyValue::String(s) => KeyPart::String(s),
    AnyValue::V8Buffer(buf) => KeyPart::Bytes(buf.to_vec()),
    AnyValue::RustBuffer(_) => unreachable!(),
  }
}

fn key_part_to_v8(value: KeyPart) -> AnyValue {
  match value {
    KeyPart::False => AnyValue::Bool(false),
    KeyPart::True => AnyValue::Bool(true),
    KeyPart::Float(n) => AnyValue::Number(n),
    KeyPart::Int(n) => AnyValue::BigInt(n),
    KeyPart::String(s) => AnyValue::String(s),
    KeyPart::Bytes(buf) => AnyValue::RustBuffer(buf.into()),
  }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum FromV8Value {
  V8(JsBuffer),
  Bytes(JsBuffer),
  U64(BigInt),
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum ToV8Value {
  V8(ToJsBuffer),
  Bytes(ToJsBuffer),
  U64(BigInt),
}

impl TryFrom<FromV8Value> for KvValue {
  type Error = num_bigint::TryFromBigIntError<num_bigint::BigInt>;
  fn try_from(value: FromV8Value) -> Result<Self, Self::Error> {
    Ok(match value {
      FromV8Value::V8(buf) => KvValue::V8(buf.to_vec()),
      FromV8Value::Bytes(buf) => KvValue::Bytes(buf.to_vec()),
      FromV8Value::U64(n) => {
        KvValue::U64(num_bigint::BigInt::from(n).try_into()?)
      }
    })
  }
}

impl From<KvValue> for ToV8Value {
  fn from(value: KvValue) -> Self {
    match value {
      KvValue::V8(buf) => ToV8Value::V8(buf.into()),
      KvValue::Bytes(buf) => ToV8Value::Bytes(buf.into()),
      KvValue::U64(n) => ToV8Value::U64(num_bigint::BigInt::from(n).into()),
    }
  }
}

#[derive(Serialize)]
struct ToV8KvEntry {
  key: KvKey,
  value: ToV8Value,
  versionstamp: ByteString,
}

impl TryFrom<KvEntry> for ToV8KvEntry {
  type Error = std::io::Error;
  fn try_from(entry: KvEntry) -> Result<Self, Self::Error> {
    Ok(ToV8KvEntry {
      key: decode_key(&entry.key)?
        .0
        .into_iter()
        .map(key_part_to_v8)
        .collect(),
      value: entry.value.into(),
      versionstamp: faster_hex::hex_string(&entry.versionstamp).into(),
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

#[op2(async)]
#[serde]
async fn op_kv_snapshot_read<DBH>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[serde] ranges: Vec<SnapshotReadRange>,
  #[serde] consistency: V8Consistency,
) -> Result<Vec<Vec<ToV8KvEntry>>, KvError>
where
  DBH: DatabaseHandler + 'static,
{
  let db = {
    let state = state.borrow();
    let resource = state
      .resource_table
      .get::<DatabaseResource<DBH::DB>>(rid)
      .map_err(KvErrorKind::Resource)?;
    resource.db.clone()
  };

  let config = {
    let state = state.borrow();
    state.borrow::<Rc<KvConfig>>().clone()
  };

  if ranges.len() > config.max_read_ranges {
    return Err(KvErrorKind::TooManyRanges(config.max_read_ranges).into_box());
  }

  let mut total_entries = 0usize;

  let read_ranges = ranges
    .into_iter()
    .map(|(prefix, start, end, limit, reverse, cursor)| {
      let selector = RawSelector::from_tuple(prefix, start, end)?;

      let (start, end) =
        decode_selector_and_cursor(&selector, reverse, cursor.as_ref())?;
      check_read_key_size(&start, &config)?;
      check_read_key_size(&end, &config)?;

      total_entries += limit as usize;
      Ok(ReadRange {
        start,
        end,
        limit: NonZeroU32::new(limit).ok_or(KvErrorKind::InvalidLimit)?,
        reverse,
      })
    })
    .collect::<Result<Vec<_>, KvError>>()?;

  if total_entries > config.max_read_entries {
    return Err(
      KvErrorKind::TooManyEntries(config.max_read_entries).into_box(),
    );
  }

  let opts = SnapshotReadOptions {
    consistency: consistency.into(),
  };
  let output_ranges = db
    .snapshot_read(read_ranges, opts)
    .await
    .map_err(KvErrorKind::Kv)?;
  let output_ranges = output_ranges
    .into_iter()
    .map(|x| {
      x.entries
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, std::io::Error>>()
    })
    .collect::<Result<Vec<_>, std::io::Error>>()?;
  Ok(output_ranges)
}

struct QueueMessageResource<QPH: QueueMessageHandle + 'static> {
  handle: QPH,
}

impl<QMH: QueueMessageHandle + 'static> Resource for QueueMessageResource<QMH> {
  fn name(&self) -> Cow<'_, str> {
    "queueMessage".into()
  }
}

#[op2(async)]
#[serde]
async fn op_kv_dequeue_next_message<DBH>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<(ToJsBuffer, ResourceId)>, KvError>
where
  DBH: DatabaseHandler + 'static,
{
  let db = {
    let state = state.borrow();
    let resource =
      match state.resource_table.get::<DatabaseResource<DBH::DB>>(rid) {
        Ok(resource) => resource,
        Err(err) => {
          if err.get_class() == "BadResource" {
            return Ok(None);
          } else {
            return Err(KvErrorKind::Resource(err).into_box());
          }
        }
      };
    resource.db.clone()
  };

  let Some(mut handle) =
    db.dequeue_next_message().await.map_err(KvErrorKind::Kv)?
  else {
    return Ok(None);
  };
  let payload = handle.take_payload().await.map_err(KvErrorKind::Kv)?.into();
  let handle_rid = {
    let mut state = state.borrow_mut();
    state.resource_table.add(QueueMessageResource { handle })
  };
  Ok(Some((payload, handle_rid)))
}

#[op2]
#[smi]
fn op_kv_watch<DBH>(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[serde] keys: Vec<KvKey>,
) -> Result<ResourceId, KvError>
where
  DBH: DatabaseHandler + 'static,
{
  let resource = state
    .resource_table
    .get::<DatabaseResource<DBH::DB>>(rid)
    .map_err(KvErrorKind::Resource)?;
  let config = state.borrow::<Rc<KvConfig>>().clone();

  if keys.len() > config.max_watched_keys {
    return Err(KvErrorKind::TooManyKeys(config.max_watched_keys).into_box());
  }

  let keys: Vec<Vec<u8>> = keys
    .into_iter()
    .map(encode_v8_key)
    .collect::<std::io::Result<_>>()?;

  for k in &keys {
    check_read_key_size(k, &config)?;
  }

  let stream = resource.db.watch(keys);

  let rid = state.resource_table.add(DatabaseWatcherResource {
    stream: AsyncRefCell::new(stream),
    db_cancel_handle: resource.cancel_handle.clone(),
    cancel_handle: CancelHandle::new_rc(),
  });

  Ok(rid)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", untagged)]
enum WatchEntry {
  Changed(Option<ToV8KvEntry>),
  Unchanged,
}

#[op2(async)]
#[serde]
async fn op_kv_watch_next(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<Vec<WatchEntry>>, KvError> {
  let resource = {
    let state = state.borrow();
    let resource = state
      .resource_table
      .get::<DatabaseWatcherResource>(rid)
      .map_err(KvErrorKind::Resource)?;
    resource.clone()
  };

  let db_cancel_handle = resource.db_cancel_handle.clone();
  let cancel_handle = resource.cancel_handle.clone();
  let stream = RcRef::map(resource, |r| &r.stream)
    .borrow_mut()
    .or_cancel(db_cancel_handle.clone())
    .or_cancel(cancel_handle.clone())
    .await;
  let Ok(Ok(mut stream)) = stream else {
    return Ok(None);
  };

  // We hold a strong reference to `resource`, so we can't rely on the stream
  // being dropped when the db connection is closed
  let Ok(Ok(Some(res))) = stream
    .next()
    .or_cancel(db_cancel_handle)
    .or_cancel(cancel_handle)
    .await
  else {
    return Ok(None);
  };

  let entries = res.map_err(KvErrorKind::Kv)?;
  let entries = entries
    .into_iter()
    .map(|entry| {
      Ok(match entry {
        WatchKeyOutput::Changed { entry } => {
          WatchEntry::Changed(entry.map(TryInto::try_into).transpose()?)
        }
        WatchKeyOutput::Unchanged => WatchEntry::Unchanged,
      })
    })
    .collect::<Result<_, KvError>>()?;

  Ok(Some(entries))
}

#[op2(async)]
async fn op_kv_finish_dequeued_message<DBH>(
  state: Rc<RefCell<OpState>>,
  #[smi] handle_rid: ResourceId,
  success: bool,
) -> Result<(), KvError>
where
  DBH: DatabaseHandler + 'static,
{
  let handle = {
    let mut state = state.borrow_mut();
    let handle = state
      .resource_table
      .take::<QueueMessageResource<<<DBH>::DB as Database>::QMH>>(handle_rid)
      .map_err(|_| KvErrorKind::QueueMessageNotFound)?;
    Rc::try_unwrap(handle)
      .map_err(|_| KvErrorKind::QueueMessageNotFound)?
      .handle
  };
  // if we fail to finish the message, there is not much we can do and the
  // message will be retried anyway, so we just ignore the error
  if let Err(err) = handle.finish(success).await {
    debug!("Failed to finish dequeued message: {}", err);
  };
  Ok(())
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum KvCheckError {
  #[class(type)]
  #[error("invalid versionstamp")]
  InvalidVersionstamp,
  #[class(inherit)]
  #[error(transparent)]
  Io(std::io::Error),
}

type V8KvCheck = (KvKey, Option<ByteString>);

fn check_from_v8(value: V8KvCheck) -> Result<Check, KvCheckError> {
  let versionstamp = match value.1 {
    Some(data) => {
      let mut out = [0u8; 10];
      if data.len() != out.len() * 2 {
        return Err(KvCheckError::InvalidVersionstamp);
      }
      faster_hex::hex_decode(&data, &mut out)
        .map_err(|_| KvCheckError::InvalidVersionstamp)?;
      Some(out)
    }
    None => None,
  };
  Ok(Check {
    key: encode_v8_key(value.0).map_err(KvCheckError::Io)?,
    versionstamp,
  })
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum KvMutationError {
  #[class(generic)]
  #[error(transparent)]
  BigInt(#[from] num_bigint::TryFromBigIntError<num_bigint::BigInt>),
  #[class(inherit)]
  #[error(transparent)]
  Io(
    #[from]
    #[inherit]
    std::io::Error,
  ),
  #[class(type)]
  #[error("Invalid mutation '{0}' with value")]
  InvalidMutationWithValue(String),
  #[class(type)]
  #[error("Invalid mutation '{0}' without value")]
  InvalidMutationWithoutValue(String),
}

type V8KvMutation = (KvKey, String, Option<FromV8Value>, Option<u64>);

fn mutation_from_v8(
  (value, current_timstamp): (V8KvMutation, DateTime<Utc>),
) -> Result<Mutation, KvMutationError> {
  let key = encode_v8_key(value.0)?;
  let kind = match (value.1.as_str(), value.2) {
    ("set", Some(value)) => MutationKind::Set(value.try_into()?),
    ("delete", None) => MutationKind::Delete,
    ("sum", Some(value)) => MutationKind::Sum {
      value: value.try_into()?,
      min_v8: vec![],
      max_v8: vec![],
      clamp: false,
    },
    ("min", Some(value)) => MutationKind::Min(value.try_into()?),
    ("max", Some(value)) => MutationKind::Max(value.try_into()?),
    ("setSuffixVersionstampedKey", Some(value)) => {
      MutationKind::SetSuffixVersionstampedKey(value.try_into()?)
    }
    (op, Some(_)) => {
      return Err(KvMutationError::InvalidMutationWithValue(op.to_string()));
    }
    (op, None) => {
      return Err(KvMutationError::InvalidMutationWithoutValue(op.to_string()));
    }
  };
  Ok(Mutation {
    key,
    kind,
    expire_at: value
      .3
      .map(|expire_in| current_timstamp + Duration::from_millis(expire_in)),
  })
}

type V8Enqueue = (JsBuffer, u64, Vec<KvKey>, Option<Vec<u32>>);

fn enqueue_from_v8(
  value: V8Enqueue,
  current_timestamp: DateTime<Utc>,
) -> Result<Enqueue, std::io::Error> {
  Ok(Enqueue {
    payload: value.0.to_vec(),
    deadline: current_timestamp
      + chrono::Duration::milliseconds(value.1 as i64),
    keys_if_undelivered: value
      .2
      .into_iter()
      .map(encode_v8_key)
      .collect::<std::io::Result<_>>()?,
    backoff_schedule: value.3,
  })
}

fn encode_v8_key(key: KvKey) -> Result<Vec<u8>, std::io::Error> {
  encode_key(&Key(key.into_iter().map(key_part_from_v8).collect()))
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
  ) -> Result<Self, KvError> {
    let prefix = prefix.map(encode_v8_key).transpose()?;
    let start = start.map(encode_v8_key).transpose()?;
    let end = end.map(encode_v8_key).transpose()?;

    match (prefix, start, end) {
      (Some(prefix), None, None) => Ok(Self::Prefixed {
        prefix,
        start: None,
        end: None,
      }),
      (Some(prefix), Some(start), None) => {
        if !start.starts_with(&prefix) || start.len() == prefix.len() {
          return Err(KvErrorKind::StartKeyNotInKeyspace.into_box());
        }
        Ok(Self::Prefixed {
          prefix,
          start: Some(start),
          end: None,
        })
      }
      (Some(prefix), None, Some(end)) => {
        if !end.starts_with(&prefix) || end.len() == prefix.len() {
          return Err(KvErrorKind::EndKeyNotInKeyspace.into_box());
        }
        Ok(Self::Prefixed {
          prefix,
          start: None,
          end: Some(end),
        })
      }
      (None, Some(start), Some(end)) => {
        if start > end {
          return Err(KvErrorKind::StartKeyGreaterThanEndKey.into_box());
        }
        Ok(Self::Range { start, end })
      }
      (None, Some(start), None) => {
        let end = start.iter().copied().chain(Some(0)).collect();
        Ok(Self::Range { start, end })
      }
      _ => Err(KvErrorKind::InvalidRange.into_box()),
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
) -> Result<String, KvError> {
  let common_prefix = selector.common_prefix();
  if !boundary_key.starts_with(common_prefix) {
    return Err(KvErrorKind::InvalidBoundaryKey.into_box());
  }
  Ok(BASE64_URL_SAFE.encode(&boundary_key[common_prefix.len()..]))
}

fn decode_selector_and_cursor(
  selector: &RawSelector,
  reverse: bool,
  cursor: Option<&ByteString>,
) -> Result<(Vec<u8>, Vec<u8>), KvError> {
  let Some(cursor) = cursor else {
    return Ok((selector.range_start_key(), selector.range_end_key()));
  };

  let common_prefix = selector.common_prefix();
  let cursor = BASE64_URL_SAFE
    .decode(cursor)
    .map_err(|_| KvErrorKind::InvalidCursor)?;

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
  if let Some(start) = selector.start()
    && &first_key[..] < start
  {
    return Err(KvErrorKind::CursorOutOfBounds.into_box());
  }

  if let Some(end) = selector.end()
    && &last_key[..] > end
  {
    return Err(KvErrorKind::CursorOutOfBounds.into_box());
  }

  Ok((first_key, last_key))
}

#[op2(async)]
#[string]
async fn op_kv_atomic_write<DBH>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[serde] checks: Vec<V8KvCheck>,
  #[serde] mutations: Vec<V8KvMutation>,
  #[serde] enqueues: Vec<V8Enqueue>,
) -> Result<Option<String>, KvError>
where
  DBH: DatabaseHandler + 'static,
{
  let current_timestamp = chrono::Utc::now();
  let db = {
    let state = state.borrow();
    let resource = state
      .resource_table
      .get::<DatabaseResource<DBH::DB>>(rid)
      .map_err(KvErrorKind::Resource)?;
    resource.db.clone()
  };

  let config = {
    let state = state.borrow();
    state.borrow::<Rc<KvConfig>>().clone()
  };

  if checks.len() > config.max_checks {
    return Err(KvErrorKind::TooManyChecks(config.max_checks).into_box());
  }

  if mutations.len() + enqueues.len() > config.max_mutations {
    return Err(KvErrorKind::TooManyMutations(config.max_mutations).into_box());
  }

  let checks = checks
    .into_iter()
    .map(check_from_v8)
    .collect::<Result<Vec<Check>, KvCheckError>>()
    .map_err(KvErrorKind::InvalidCheck)?;
  let mutations = mutations
    .into_iter()
    .map(|mutation| mutation_from_v8((mutation, current_timestamp)))
    .collect::<Result<Vec<Mutation>, KvMutationError>>()
    .map_err(KvErrorKind::InvalidMutation)?;
  let enqueues = enqueues
    .into_iter()
    .map(|e| enqueue_from_v8(e, current_timestamp))
    .collect::<Result<Vec<Enqueue>, std::io::Error>>()
    .map_err(KvErrorKind::InvalidEnqueue)?;

  let mut total_payload_size = 0usize;
  let mut total_key_size = 0usize;

  for key in checks
    .iter()
    .map(|c| &c.key)
    .chain(mutations.iter().map(|m| &m.key))
  {
    if key.is_empty() {
      return Err(KvErrorKind::EmptyKey.into_box());
    }

    total_payload_size += check_write_key_size(key, &config)?;
  }

  for (key, value) in mutations
    .iter()
    .flat_map(|m| m.kind.value().map(|x| (&m.key, x)))
  {
    let key_size = check_write_key_size(key, &config)?;
    total_payload_size += check_value_size(value, &config)? + key_size;
    total_key_size += key_size;
  }

  for enqueue in &enqueues {
    total_payload_size +=
      check_enqueue_payload_size(&enqueue.payload, &config)?;
    if let Some(schedule) = enqueue.backoff_schedule.as_ref() {
      total_payload_size += 4 * schedule.len();
    }
  }

  if total_payload_size > config.max_total_mutation_size_bytes {
    return Err(
      KvErrorKind::TotalMutationTooLarge(config.max_total_mutation_size_bytes)
        .into_box(),
    );
  }

  if total_key_size > config.max_total_key_size_bytes {
    return Err(
      KvErrorKind::TotalKeyTooLarge(config.max_total_key_size_bytes).into_box(),
    );
  }

  let atomic_write = AtomicWrite {
    checks,
    mutations,
    enqueues,
  };

  let result = db
    .atomic_write(atomic_write)
    .await
    .map_err(KvErrorKind::Kv)?;

  Ok(result.map(|res| faster_hex::hex_string(&res.versionstamp)))
}

// (prefix, start, end)
type EncodeCursorRangeSelector = (Option<KvKey>, Option<KvKey>, Option<KvKey>);

#[op2]
#[string]
fn op_kv_encode_cursor(
  #[serde] (prefix, start, end): EncodeCursorRangeSelector,
  #[serde] boundary_key: KvKey,
) -> Result<String, KvError> {
  let selector = RawSelector::from_tuple(prefix, start, end)?;
  let boundary_key = encode_v8_key(boundary_key)?;
  let cursor = encode_cursor(&selector, &boundary_key)?;
  Ok(cursor)
}

fn check_read_key_size(key: &[u8], config: &KvConfig) -> Result<(), KvError> {
  if key.len() > config.max_read_key_size_bytes {
    Err(
      KvErrorKind::KeyTooLargeToRead(config.max_read_key_size_bytes).into_box(),
    )
  } else {
    Ok(())
  }
}

fn check_write_key_size(
  key: &[u8],
  config: &KvConfig,
) -> Result<usize, KvError> {
  if key.len() > config.max_write_key_size_bytes {
    Err(
      KvErrorKind::KeyTooLargeToWrite(config.max_write_key_size_bytes)
        .into_box(),
    )
  } else {
    Ok(key.len())
  }
}

fn check_value_size(
  value: &KvValue,
  config: &KvConfig,
) -> Result<usize, KvError> {
  let payload = match value {
    KvValue::Bytes(x) => x,
    KvValue::V8(x) => x,
    KvValue::U64(_) => return Ok(8),
  };

  if payload.len() > config.max_value_size_bytes {
    Err(KvErrorKind::ValueTooLarge(config.max_value_size_bytes).into_box())
  } else {
    Ok(payload.len())
  }
}

fn check_enqueue_payload_size(
  payload: &[u8],
  config: &KvConfig,
) -> Result<usize, KvError> {
  if payload.len() > config.max_value_size_bytes {
    Err(
      KvErrorKind::EnqueuePayloadTooLarge(config.max_value_size_bytes)
        .into_box(),
    )
  } else {
    Ok(payload.len())
  }
}
