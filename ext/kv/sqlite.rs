// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use async_trait::async_trait;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::OpState;
use rusqlite::params;
use rusqlite::OpenFlags;
use rusqlite::OptionalExtension;
use rusqlite::Transaction;

use crate::AtomicWrite;
use crate::CommitResult;
use crate::Database;
use crate::DatabaseHandler;
use crate::KvEntry;
use crate::MutationKind;
use crate::ReadRange;
use crate::ReadRangeOutput;
use crate::SnapshotReadOptions;
use crate::Value;

const STATEMENT_INC_AND_GET_DATA_VERSION: &str =
  "update data_version set version = version + 1 where k = 0 returning version";
const STATEMENT_KV_RANGE_SCAN: &str =
  "select k, v, v_encoding, version from kv where k >= ? and k < ? order by k asc limit ?";
const STATEMENT_KV_RANGE_SCAN_REVERSE: &str =
  "select k, v, v_encoding, version from kv where k >= ? and k < ? order by k desc limit ?";
const STATEMENT_KV_POINT_GET_VALUE_ONLY: &str =
  "select v, v_encoding from kv where k = ?";
const STATEMENT_KV_POINT_GET_VERSION_ONLY: &str =
  "select version from kv where k = ?";
const STATEMENT_KV_POINT_SET: &str =
  "insert into kv (k, v, v_encoding, version) values (:k, :v, :v_encoding, :version) on conflict(k) do update set v = :v, v_encoding = :v_encoding, version = :version";
const STATEMENT_KV_POINT_DELETE: &str = "delete from kv where k = ?";

const STATEMENT_CREATE_MIGRATION_TABLE: &str = "
create table if not exists migration_state(
  k integer not null primary key,
  version integer not null
)
";

const MIGRATIONS: [&str; 2] = [
  "
create table data_version (
  k integer primary key,
  version integer not null
);
insert into data_version (k, version) values (0, 0);
create table kv (
  k blob primary key,
  v blob not null,
  v_encoding integer not null,
  version integer not null
) without rowid;
",
  "
create table queue (
  ts integer not null,
  id text not null,
  data blob not null,
  backoff_schedule text not null,
  keys_if_undelivered blob not null,

  primary key (ts, id)
);
create table queue_running(
  deadline integer not null,
  id text not null,
  data blob not null,
  backoff_schedule text not null,
  keys_if_undelivered blob not null,

  primary key (deadline, id)
);
",
];

pub struct SqliteDbHandler<P: SqliteDbHandlerPermissions + 'static> {
  pub default_storage_dir: Option<PathBuf>,
  _permissions: PhantomData<P>,
}

pub trait SqliteDbHandlerPermissions {
  fn check_read(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
  fn check_write(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
}

impl<P: SqliteDbHandlerPermissions> SqliteDbHandler<P> {
  pub fn new(default_storage_dir: Option<PathBuf>) -> Self {
    Self {
      default_storage_dir,
      _permissions: PhantomData,
    }
  }
}

#[async_trait(?Send)]
impl<P: SqliteDbHandlerPermissions> DatabaseHandler for SqliteDbHandler<P> {
  type DB = SqliteDb;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, AnyError> {
    let conn = match (path.as_deref(), &self.default_storage_dir) {
      (Some(":memory:"), _) | (None, None) => {
        rusqlite::Connection::open_in_memory()?
      }
      (Some(path), _) => {
        if path.is_empty() {
          return Err(type_error("Filename cannot be empty"));
        }
        if path.starts_with(':') {
          return Err(type_error(
            "Filename cannot start with ':' unless prefixed with './'",
          ));
        }
        let path = Path::new(path);
        {
          let mut state = state.borrow_mut();
          let permissions = state.borrow_mut::<P>();
          permissions.check_read(path, "Deno.openKv")?;
          permissions.check_write(path, "Deno.openKv")?;
        }
        let flags = OpenFlags::default().difference(OpenFlags::SQLITE_OPEN_URI);
        rusqlite::Connection::open_with_flags(path, flags)?
      }
      (None, Some(path)) => {
        std::fs::create_dir_all(path)?;
        let path = path.join("kv.sqlite3");
        rusqlite::Connection::open(&path)?
      }
    };

    conn.pragma_update(None, "journal_mode", "wal")?;
    conn.execute(STATEMENT_CREATE_MIGRATION_TABLE, [])?;

    let current_version: usize = conn
      .query_row(
        "select version from migration_state where k = 0",
        [],
        |row| row.get(0),
      )
      .optional()?
      .unwrap_or(0);

    for (i, migration) in MIGRATIONS.iter().enumerate() {
      let version = i + 1;
      if version > current_version {
        conn.execute_batch(migration)?;
        conn.execute(
          "replace into migration_state (k, version) values(?, ?)",
          [&0, &version],
        )?;
      }
    }

    Ok(SqliteDb(RefCell::new(conn)))
  }
}

pub struct SqliteDb(RefCell<rusqlite::Connection>);

#[async_trait(?Send)]
impl Database for SqliteDb {
  async fn snapshot_read(
    &self,
    requests: Vec<ReadRange>,
    _options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, AnyError> {
    let mut responses = Vec::with_capacity(requests.len());
    let mut db = self.0.borrow_mut();
    let tx = db.transaction()?;

    for request in requests {
      let mut stmt = tx.prepare_cached(if request.reverse {
        STATEMENT_KV_RANGE_SCAN_REVERSE
      } else {
        STATEMENT_KV_RANGE_SCAN
      })?;
      let entries = stmt
        .query_map(
          (
            request.start.as_slice(),
            request.end.as_slice(),
            request.limit.get(),
          ),
          |row| {
            let key: Vec<u8> = row.get(0)?;
            let value: Vec<u8> = row.get(1)?;
            let encoding: i64 = row.get(2)?;

            let value = decode_value(value, encoding);

            let version: i64 = row.get(3)?;
            Ok(KvEntry {
              key,
              value,
              versionstamp: version_to_versionstamp(version),
            })
          },
        )?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;
      responses.push(ReadRangeOutput { entries });
    }

    Ok(responses)
  }

  async fn atomic_write(
    &self,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, AnyError> {
    let mut db = self.0.borrow_mut();

    let tx = db.transaction()?;

    for check in write.checks {
      let real_versionstamp = tx
        .prepare_cached(STATEMENT_KV_POINT_GET_VERSION_ONLY)?
        .query_row([check.key.as_slice()], |row| row.get(0))
        .optional()?
        .map(version_to_versionstamp);
      if real_versionstamp != check.versionstamp {
        return Ok(None);
      }
    }

    let version: i64 = tx
      .prepare_cached(STATEMENT_INC_AND_GET_DATA_VERSION)?
      .query_row([], |row| row.get(0))?;

    for mutation in write.mutations {
      match mutation.kind {
        MutationKind::Set(value) => {
          let (value, encoding) = encode_value(&value);
          let changed = tx
            .prepare_cached(STATEMENT_KV_POINT_SET)?
            .execute(params![mutation.key, &value, &encoding, &version])?;
          assert_eq!(changed, 1)
        }
        MutationKind::Delete => {
          let changed = tx
            .prepare_cached(STATEMENT_KV_POINT_DELETE)?
            .execute(params![mutation.key])?;
          assert!(changed == 0 || changed == 1)
        }
        MutationKind::Sum(operand) => {
          mutate_le64(&tx, &mutation.key, "sum", &operand, version, |a, b| {
            a.wrapping_add(b)
          })?;
        }
        MutationKind::Min(operand) => {
          mutate_le64(&tx, &mutation.key, "min", &operand, version, |a, b| {
            a.min(b)
          })?;
        }
        MutationKind::Max(operand) => {
          mutate_le64(&tx, &mutation.key, "max", &operand, version, |a, b| {
            a.max(b)
          })?;
        }
      }
    }

    // TODO(@losfair): enqueues

    tx.commit()?;

    let new_vesionstamp = version_to_versionstamp(version);

    Ok(Some(CommitResult {
      versionstamp: new_vesionstamp,
    }))
  }
}

/// Mutates a LE64 value in the database, defaulting to setting it to the
/// operand if it doesn't exist.
fn mutate_le64(
  tx: &Transaction,
  key: &[u8],
  op_name: &str,
  operand: &Value,
  new_version: i64,
  mutate: impl FnOnce(u64, u64) -> u64,
) -> Result<(), AnyError> {
  let Value::U64(operand) = *operand else {
    return Err(type_error(format!("Failed to perform '{op_name}' mutation on a non-U64 operand")));
  };

  let old_value = tx
    .prepare_cached(STATEMENT_KV_POINT_GET_VALUE_ONLY)?
    .query_row([key], |row| {
      let value: Vec<u8> = row.get(0)?;
      let encoding: i64 = row.get(1)?;

      let value = decode_value(value, encoding);
      Ok(value)
    })
    .optional()?;

  let new_value = match old_value {
    Some(Value::U64(old_value) ) => mutate(old_value, operand),
    Some(_) => return Err(type_error(format!("Failed to perform '{op_name}' mutation on a non-U64 value in the database"))),
    None => operand,
  };

  let new_value = Value::U64(new_value);
  let (new_value, encoding) = encode_value(&new_value);

  let changed = tx.prepare_cached(STATEMENT_KV_POINT_SET)?.execute(params![
    key,
    &new_value[..],
    encoding,
    new_version
  ])?;
  assert_eq!(changed, 1);

  Ok(())
}

fn version_to_versionstamp(version: i64) -> [u8; 10] {
  let mut versionstamp = [0; 10];
  versionstamp[..8].copy_from_slice(&version.to_be_bytes());
  versionstamp
}

const VALUE_ENCODING_V8: i64 = 1;
const VALUE_ENCODING_LE64: i64 = 2;
const VALUE_ENCODING_BYTES: i64 = 3;

fn decode_value(value: Vec<u8>, encoding: i64) -> crate::Value {
  match encoding {
    VALUE_ENCODING_V8 => crate::Value::V8(value),
    VALUE_ENCODING_BYTES => crate::Value::Bytes(value),
    VALUE_ENCODING_LE64 => {
      let mut buf = [0; 8];
      buf.copy_from_slice(&value);
      crate::Value::U64(u64::from_le_bytes(buf))
    }
    _ => todo!(),
  }
}

fn encode_value(value: &crate::Value) -> (Cow<'_, [u8]>, i64) {
  match value {
    crate::Value::V8(value) => (Cow::Borrowed(value), VALUE_ENCODING_V8),
    crate::Value::Bytes(value) => (Cow::Borrowed(value), VALUE_ENCODING_BYTES),
    crate::Value::U64(value) => {
      let mut buf = [0; 8];
      buf.copy_from_slice(&value.to_le_bytes());
      (Cow::Owned(buf.to_vec()), VALUE_ENCODING_LE64)
    }
  }
}
