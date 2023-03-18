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
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8::BigInt;
use deno_core::ByteString;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;

pub use crate::interface::*;

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
    op_kv_encode_key,
  ],
  esm = [ "01_db.ts" ],
  options = {
    handler: DBH,
    unstable: bool,
  },
  state = |state, options| {
    state.put(Rc::new(options.handler));
    state.put(UnstableChecker { unstable })
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
      .check_unstable("Deno.openDatabase");
    state.borrow::<Rc<DBH>>().clone()
  };
  let db = handler.open(state.clone(), path).await?;
  let rid = state
    .borrow_mut()
    .resource_table
    .add(DatabaseResource { db: Rc::new(db) });
  Ok(rid)
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum V8KeyPart {
  Bool(bool),
  Number(f64),
  BigInt(BigInt),
  String(String),
  U8(ZeroCopyBuf),
}

impl From<V8KeyPart> for KeyPart {
  fn from(value: V8KeyPart) -> Self {
    match value {
      V8KeyPart::Bool(false) => KeyPart::True,
      V8KeyPart::Bool(true) => KeyPart::False,
      V8KeyPart::Number(n) => KeyPart::Float(n),
      V8KeyPart::BigInt(n) => KeyPart::Int(n.into()),
      V8KeyPart::String(s) => KeyPart::String(s),
      V8KeyPart::U8(buf) => KeyPart::Bytes(buf.to_vec()),
    }
  }
}

impl From<KeyPart> for V8KeyPart {
  fn from(value: KeyPart) -> Self {
    match value {
      KeyPart::True => V8KeyPart::Bool(false),
      KeyPart::False => V8KeyPart::Bool(true),
      KeyPart::Float(n) => V8KeyPart::Number(n),
      KeyPart::Int(n) => V8KeyPart::BigInt(n.into()),
      KeyPart::String(s) => V8KeyPart::String(s),
      KeyPart::Bytes(buf) => V8KeyPart::U8(buf.into()),
    }
  }
}

#[derive(Deserialize, Serialize)]
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
  key: Vec<V8KeyPart>,
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

// (start, end, limit, reverse)
type SnapshotReadRange = (ZeroCopyBuf, Option<ZeroCopyBuf>, u32, bool);

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
  let read_ranges = ranges
    .into_iter()
    .map(|(start, end, limit, reverse)| {
      let start = start.to_vec();
      let end = end.map(|x| x.to_vec()).unwrap_or_else(|| {
        let mut out = Vec::with_capacity(start.len() + 1);
        out.extend_from_slice(&start);
        out.push(0);
        out
      });
      Ok(ReadRange {
        start,
        end,
        limit: NonZeroU32::new(limit)
          .with_context(|| "limit must be greater than 0")?,
        reverse,
      })
    })
    .collect::<Result<Vec<_>, AnyError>>()?;
  let opts = SnapshotReadOptions {
    consistency: consistency.into(),
  };
  let ranges = db.snapshot_read(read_ranges, opts).await?;
  let ranges = ranges
    .into_iter()
    .map(|x| {
      x.entries
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, AnyError>>()
    })
    .collect::<Result<Vec<_>, AnyError>>()?;
  Ok(ranges)
}

type V8KvCheck = (Vec<V8KeyPart>, Option<ByteString>);

impl TryFrom<V8KvCheck> for KvCheck {
  type Error = AnyError;
  fn try_from(value: V8KvCheck) -> Result<Self, AnyError> {
    let versionstamp = value.1.as_ref().map(|x| {
      let mut out = [0u8; 10];
      hex::decode_to_slice(x, &mut out).unwrap();
      out
    });
    Ok(KvCheck {
      key: encode_key(&Key(value.0.into_iter().map(From::from).collect()))?,
      versionstamp,
    })
  }
}

type V8KvMutation = (Vec<V8KeyPart>, String, Option<V8Value>);

impl TryFrom<V8KvMutation> for KvMutation {
  type Error = AnyError;
  fn try_from(value: V8KvMutation) -> Result<Self, AnyError> {
    let key = encode_key(&Key(value.0.into_iter().map(From::from).collect()))?;
    let kind = match value.1.as_str() {
      // TODO(lucacasonato): error handling (when value == None)
      "set" => MutationKind::Set(value.2.unwrap().try_into()?),
      "delete" => MutationKind::Delete,
      "sum" => MutationKind::Sum(value.2.unwrap().try_into()?),
      "min" => MutationKind::Min(value.2.unwrap().try_into()?),
      "max" => MutationKind::Max(value.2.unwrap().try_into()?),
      _ => todo!(),
    };
    Ok(KvMutation { key, kind })
  }
}

type V8Enqueue = (ZeroCopyBuf, u64, Vec<Vec<V8KeyPart>>, Option<Vec<u32>>);

impl TryFrom<V8Enqueue> for Enqueue {
  type Error = AnyError;
  fn try_from(value: V8Enqueue) -> Result<Self, AnyError> {
    Ok(Enqueue {
      payload: value.0.to_vec(),
      deadline_ms: value.1,
      keys_if_undelivered: value
        .2
        .into_iter()
        .map(|k| encode_key(&Key(k.into_iter().map(From::from).collect())))
        .collect::<std::io::Result<_>>()?,
      backoff_schedule: value.3,
    })
  }
}

#[op]
async fn op_kv_atomic_write<DBH>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  checks: Vec<V8KvCheck>,
  mutations: Vec<V8KvMutation>,
  enqueues: Vec<V8Enqueue>,
) -> Result<bool, AnyError>
where
  DBH: DatabaseHandler + 'static,
{
  let db = {
    let state = state.borrow();
    let resource =
      state.resource_table.get::<DatabaseResource<DBH::DB>>(rid)?;
    resource.db.clone()
  };

  let checks = checks
    .into_iter()
    .map(TryInto::try_into)
    .collect::<Result<_, AnyError>>()
    .with_context(|| "invalid check")?;
  let mutations = mutations
    .into_iter()
    .map(TryInto::try_into)
    .collect::<Result<_, AnyError>>()
    .with_context(|| "invalid mutation")?;
  let enqueues = enqueues
    .into_iter()
    .map(TryInto::try_into)
    .collect::<Result<_, AnyError>>()
    .with_context(|| "invalid enqueue")?;

  let atomic_write = AtomicWrite {
    checks,
    mutations,
    enqueues,
  };

  let result = db.atomic_write(atomic_write).await?;

  Ok(result)
}

#[op]
fn op_kv_encode_key(key: Vec<V8KeyPart>) -> Result<ZeroCopyBuf, AnyError> {
  let key = encode_key(&Key(key.into_iter().map(From::from).collect()))?;
  Ok(ZeroCopyBuf::from(key))
}
