use std::borrow::{Cow};
use std::cell::RefCell;
use super::DomExceptionNotSupportedError;
use super::OriginStorageDir;
use crate::{DomExceptionConstraintError, DomExceptionInvalidStateError};
use deno_core::error::AnyError;
use deno_core::{op, Resource};
use deno_core::serde_json;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use fallible_iterator::FallibleIterator;
use rusqlite::params;
use rusqlite::types::FromSqlResult;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone)]
struct Database {
  name: String,
  version: u64,
}

pub struct IndexedDb(Rc<RefCell<Connection>>, Option<rusqlite::Transaction<'static>>);

impl Resource for IndexedDb {
  fn name(&self) -> Cow<str> {
    "indexedDb".into()
  }
}

#[op]
pub fn op_indexeddb_open(state: &mut OpState) -> Result<(), AnyError> {
  let path = state.try_borrow::<OriginStorageDir>().ok_or_else(|| {
    DomExceptionNotSupportedError::new(
      "IndexedDB is not supported in this context.",
    )
  })?;
  std::fs::create_dir_all(&path.0)?;
  let conn = Connection::open(path.0.join("indexeddb"))?;
  let initial_pragmas = "
    -- enable write-ahead-logging mode
    PRAGMA recursive_triggers = ON;
    PRAGMA secure_delete = OFF;
    PRAGMA foreign_keys = ON;
    ";
  conn.execute_batch(initial_pragmas)?;

  let create_statements = r#"
    CREATE TABLE IF NOT EXISTS database (
      name TEXT PRIMARY KEY,
      version INTEGER NOT NULL DEFAULT 0
    ) WITHOUT ROWID;

    CREATE TABLE IF NOT EXISTS object_store (
      id INTEGER PRIMARY KEY,
      name TEXT NOT NULL,
      key_path TEXT,
      unique_index INTEGER NOT NULL,
      database_name TEXT NOT NULL,
      FOREIGN KEY (database_name)
        REFERENCES database(name)
    );

    CREATE TABLE IF NOT EXISTS record (
      object_store_id INTEGER NOT NULL,
      key BLOB NOT NULL,
      index_data_values BLOB DEFAULT NULL,
      value BLOB NOT NULL,
      PRIMARY KEY (object_store_id, key),
      FOREIGN KEY (object_store_id)
        REFERENCES object_store(id)
    ) WITHOUT ROWID;

    CREATE TABLE index (
      id INTEGER PRIMARY KEY,
      object_store_id INTEGER NOT NULL,
      name TEXT NOT NULL,
      key_path TEXT NOT NULL,
      unique INTEGER NOT NULL,
      multientry INTEGER NOT NULL,
      FOREIGN KEY (object_store_id)
        REFERENCES object_store(id)
    );

    CREATE TABLE IF NOT EXISTS index_data (
      index_id INTEGER NOT NULL,
      value BLOB NOT NULL,
      record_key BLOB NOT NULL,
      object_store_id INTEGER NOT NULL,
      PRIMARY KEY (index_id, value, record_key),
      FOREIGN KEY (index_id)
        REFERENCES index(id),
      FOREIGN KEY (object_store_id, record_key)
        REFERENCES record(object_store_id, key)
    ) WITHOUT ROWID;

    CREATE TABLE IF NOT EXISTS unique_index_data (
      index_id INTEGER NOT NULL,
      value BLOB NOT NULL,
      record_key BLOB NOT NULL,
      object_store_id INTEGER NOT NULL,
      PRIMARY KEY (index_id, value),
      FOREIGN KEY (index_id)
        REFERENCES index(id),
      FOREIGN KEY (object_store_id, record_key)
        REFERENCES record(object_store_id, key)
    ) WITHOUT ROWID;
    "#;
  conn.execute_batch(create_statements)?;

  conn.set_prepared_statement_cache_capacity(128);
  state.resource_table.add(IndexedDb(Rc::new(RefCell::new(conn)), None));

  Ok(())
}

#[op]
pub fn op_indexeddb_transaction_create(
  state: &mut OpState,
) -> Result<ResourceId, AnyError> {
  let idbmanager = state.borrow::<IndexedDbManager>();
  let mut conn = idbmanager.0.borrow_mut();
  let transaction = conn.transaction()?;
  let rid = state.resource_table.add(IndexedDb(idbmanager.0.clone(), transaction));
  Ok(rid)
}

#[op]
pub fn op_indexeddb_transaction_commit(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let idb = Rc::try_unwrap(state.resource_table.take::<IndexedDb>(rid)?).unwrap();
  idb.1.commit()?;
  Ok(())
}

#[op]
pub fn op_indexeddb_transaction_abort(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let idb = Rc::try_unwrap(state.resource_table.take::<IndexedDb>(rid)?).unwrap();
  idb.1.rollback()?;
  Ok(())
}

// Ref: https://w3c.github.io/IndexedDB/#open-a-database
#[op]
pub fn op_indexeddb_open_database(
  state: &mut OpState,
  name: String,
  version: Option<u64>,
) -> Result<(u64, u64), AnyError> {
  let idbmanager = state.borrow::<IndexedDbManager>();
  let conn = &idbmanager.0;
  let mut stmt =
    conn.prepare_cached("SELECT * FROM database WHERE name = ?")?;
  let db = stmt
    .query_row(params![name], |row| {
      Ok(Database {
        name: row.get(0)?,
        version: row.get(1)?,
      })
    })
    .optional()?;
  let version = version
    .or_else(|| db.clone().map(|db| db.version))
    .unwrap_or(1);

  let db = if let Some(db) = db {
    db
  } else {
    let mut stmt =
      conn.prepare_cached("INSERT INTO database (name) VALUES (?)")?;
    stmt.execute(params![name])?; // TODO: 6. DOMException
    Database { name, version: 0 }
  };

  Ok((version, db.version))
}

// Ref: https://w3c.github.io/IndexedDB/#dom-idbfactory-databases
#[op]
pub fn op_indexeddb_list_databases(
  state: &mut OpState,
) -> Result<Vec<String>, AnyError> {
  let idbmanager = &state.borrow::<IndexedDbManager>().0;
  let mut stmt = idbmanager.prepare_cached("SELECT name FROM database")?;
  let names = stmt
    .query(params![])?
    .map(|row| row.get(0))
    .collect::<Vec<String>>()?;
  Ok(names)
}

// Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-createobjectstore
#[op]
pub fn op_indexeddb_database_create_object_store(
  state: &mut OpState,
  database_name: String,
  name: String,
  key_path: serde_json::Value,
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT * FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  if stmt.exists(params![name, database_name])? {
    return Err(
      DomExceptionConstraintError::new(&format!(
        "ObjectStore with name '{name}' already exists"
      ))
      .into(),
    );
  }

  // TODO: 8.

  let mut stmt = conn.prepare_cached(
    "INSERT INTO object_store (name, key_path, unique_index, database_name) VALUES (?, ?, ?, ?)",
  )?;
  stmt.execute(params![
    name,
    key_path,
    // TODO: unique_index
    database_name,
  ])?;

  Ok(())
}

// Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-deleteobjectstore
#[op]
pub fn op_indexeddb_database_delete_object_store(
  state: &mut OpState,
  database_name: String,
  name: String,
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt: rusqlite::CachedStatement = conn.prepare_cached(
    "DELETE FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  stmt.execute(params![name, database_name])?;

  // TODO: delete indexes & records. maybe use ON DELETE CASCADE?

  Ok(())
}

#[op]
pub fn op_indexeddb_object_store_exists(
  state: &mut OpState,
  database_name: String,
  name: String,
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT * FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  if !stmt.exists(params![name, database_name])? {
    return Err(
      DomExceptionInvalidStateError::new(&format!(
        "ObjectStore with name '{name}' does not exists"
      ))
      .into(),
    );
  }

  Ok(())
}

// Ref: https://w3c.github.io/IndexedDB/#ref-for-dom-idbobjectstore-name%E2%91%A2
#[op]
pub fn op_indexeddb_object_store_rename(
  state: &mut OpState,
  database_name: String,
  prev_name: String,
  new_name: String,
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT * FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  if stmt.exists(params![new_name, database_name])? {
    return Err(
      DomExceptionConstraintError::new(&format!(
        "ObjectStore with name '{new_name}' already exists"
      ))
      .into(),
    );
  }

  let mut stmt = conn.prepare_cached(
    "UPDATE object_store SET name = ? WHERE name = ? AND database_name = ?",
  )?;
  stmt.execute(params![new_name, prev_name, database_name])?;

  Ok(())
}

// Ref: https://w3c.github.io/IndexedDB/#key-construct
#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", content = "value")]
pub enum Key {
  Number(u64),
  Date(u64),
  String(String),
  Binary(ZeroCopyBuf),
  Array(Box<Vec<Key>>),
}

impl Key {
  // Ref: https://w3c.github.io/IndexedDB/#compare-two-keys
  fn cmp(&self, other: &Self) -> Ordering {
    if std::mem::discriminant(self) != std::mem::discriminant(other) {
      if let Key::Array(_) = self {
        Ordering::Greater
      } else if let Key::Array(_) = other {
        Ordering::Less
      } else if let Key::Binary(_) = self {
        Ordering::Greater
      } else if let Key::Binary(_) = other {
        Ordering::Less
      } else if let Key::String(_) = self {
        Ordering::Greater
      } else if let Key::String(_) = other {
        Ordering::Less
      } else if let Key::Number(_) = self {
        Ordering::Greater
      } else if let Key::Number(_) = other {
        Ordering::Less
      } else if let Key::Date(_) = self {
        Ordering::Greater
      } else if let Key::Date(_) = other {
        Ordering::Less
      } else {
        unreachable!()
      }
    } else {
      match (self, other) {
        (Key::Number(va), Key::Number(vb)) | (Key::Date(va), Key::Date(vb)) => {
          va.cmp(vb)
        }
        (Key::String(va), Key::String(vb)) => va.cmp(vb),
        (Key::Binary(va), Key::Binary(vb)) => va.cmp(vb),
        (Key::Array(va), Key::Array(vb)) => {
          for x in va.iter().zip(vb.iter()) {
            match x.0.cmp(x.1) {
              Ordering::Greater => {}
              res => return res,
            }
          }
          va.len().cmp(&vb.len())
        }
        _ => unreachable!(),
      }
    }
  }
}

impl rusqlite::types::ToSql for Key {
  fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
    Ok(rusqlite::types::ToSqlOutput::Owned(
      rusqlite::types::Value::Blob(
        serde_json::to_vec(self)
          .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?,
      ),
    ))
  }
}

impl rusqlite::types::FromSql for Key {
  fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
    value.as_blob().and_then(|blob| {
      Ok(
        serde_json::from_slice(blob)
          .map_err(|e| rusqlite::types::FromSqlError::Other(e.into()))?,
      )
    })
  }
}

// Ref: https://w3c.github.io/IndexedDB/#range-construct
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Range {
  lower: Option<Key>,
  upper: Option<Key>,
  lower_open: bool,
  upper_open: bool,
}

impl Range {
  // Ref: https://w3c.github.io/IndexedDB/#in
  fn contains(&self, key: &Key) -> bool {
    let lower = match &self.lower {
      Some(lower_key) => match lower_key.cmp(key) {
        Ordering::Less => true,
        Ordering::Equal if !self.lower_open => true,
        _ => false,
      },
      None => true,
    };
    let upper = match &self.upper {
      Some(upper_key) => match upper_key.cmp(key) {
        Ordering::Greater => true,
        Ordering::Equal if !self.upper_open => true,
        _ => false,
      },
      None => true,
    };
    lower && upper
  }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Index {
  id: u64,
  object_store_id: u64,
  database_name: String,
  name: String,
  key_path: serde_json::Value,
  unique: bool,
  multi_entry: bool,
}

// Ref: https://w3c.github.io/IndexedDB/#delete-records-from-an-object-store
#[op]
pub fn op_indexeddb_object_store_add_or_put_records(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  value: ZeroCopyBuf,
  key: Key,
  no_overwrite: bool,
) -> Result<Vec<Index>, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let query = if no_overwrite {
    "INSERT INTO record (object_store_id, key, value) VALUES (?, ?, ?)"
  } else {
    "INSERT OR REPLACE INTO record (object_store_id, key, value) VALUES (?, ?, ?)"
  };

  let mut stmt = conn.prepare_cached(query)?;
  stmt.execute(params![object_store_id, key, value.to_vec()])?;
  // TODO: keys are to be sorted (4.)

  let mut stmt =
    conn.prepare_cached("SELECT * FROM index WHERE object_store_id = ?")?;
  let indexes = stmt
    .query_map(params![object_store_id], |row| {
      Ok(Index {
        id: row.get(0)?,
        object_store_id: row.get(1)?,
        database_name: row.get(2)?,
        name: row.get(3)?,
        key_path: row.get(4)?,
        unique: row.get(5)?,
        multi_entry: row.get(6)?,
      })
    })?
    .collect::<Result<_, rusqlite::Error>>()?;

  Ok(indexes)
}

// For: https://w3c.github.io/IndexedDB/#store-a-record-into-an-object-store
#[op]
pub fn op_indexeddb_object_store_add_or_put_records_handle_index(
  state: &mut OpState,
  index: Index,
  index_key: Key,
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  if !index.multi_entry
    || matches!(
      index_key,
      Key::String(_) | Key::Date(_) | Key::Binary(_) | Key::Number(_)
    )
  {
    let mut stmt = conn.prepare_cached(
      "SELECT record_Key FROM unique_index_data WHERE index_id = ?",
    )?;
    let key = stmt
      .query_map(params![index.id], |row| row.get::<usize, Key>(0))?
      .find_map(|key| {
        key
          .map(|key| {
            if matches!(key.cmp(&index_key), Ordering::Equal) {
              Some(key)
            } else {
              None
            }
          })
          .transpose()
      });
    if key.is_some() {
      return Err(DomExceptionConstraintError::new("").into()); // TODO
    }
  }
  if index.multi_entry {
    if let Key::Array(keys) = &index_key {
      let mut stmt = conn.prepare_cached(
        "SELECT record_Key FROM unique_index_data WHERE index_id = ?",
      )?;
      let key = stmt
        .query_map(params![index.id], |row| row.get::<usize, Key>(0))?
        .find_map(|key| {
          key
            .map(|key| {
              if keys
                .iter()
                .any(|subkey| matches!(key.cmp(subkey), Ordering::Equal))
              {
                Some(key)
              } else {
                None
              }
            })
            .transpose()
        });
      if key.is_some() {
        return Err(DomExceptionConstraintError::new("").into()); // TODO
      }
    }
  }
  if !index.multi_entry
    || matches!(
      index_key,
      Key::String(_) | Key::Date(_) | Key::Binary(_) | Key::Number(_)
    )
  {
    // TODO: 5.
  }
  if index.multi_entry {
    if let Key::Array(keys) = index_key {
      for key in *keys {
        // TODO: 6.
      }
    }
  }

  Ok(())
}

// Ref: https://w3c.github.io/IndexedDB/#delete-records-from-an-object-store
#[op]
pub fn op_indexeddb_object_store_delete_records(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  range: Range,
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt = conn
    .prepare_cached("SELECT id, key FROM record WHERE object_store_id = ?")?;
  let mut delete_stmt =
    conn.prepare_cached("DELETE FROM record WHERE id = ?")?;
  for row in stmt.query_map(params![object_store_id], |row| {
    Ok((row.get::<usize, u64>(0)?, row.get::<usize, Key>(1)?))
  })? {
    let (id, key) = row?;
    if range.contains(&key) {
      delete_stmt.execute(params![id])?;
    }
  }

  let mut stmt = conn.prepare_cached(
    "SELECT index_id, value FROM index_data WHERE object_store_id = ?",
  )?;
  let mut delete_stmt =
    conn.prepare_cached("DELETE FROM index_data WHERE index_id = ?")?;
  for row in stmt.query_map(params![object_store_id], |row| {
    Ok((row.get::<usize, u64>(0)?, row.get::<usize, Key>(1)?))
  })? {
    let (id, key) = row?;
    if range.contains(&key) {
      delete_stmt.execute(params![id])?;
    }
  }

  Ok(())
}

// Ref: https://w3c.github.io/IndexedDB/#clear-an-object-store
#[op]
pub fn op_indexeddb_object_store_clear(
  state: &mut OpState,
  database_name: String,
  store_name: String,
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt =
    conn.prepare_cached("DELETE FROM record WHERE object_store_id = ?")?;
  stmt.execute(params![object_store_id])?;

  let mut stmt =
    conn.prepare_cached("DELETE FROM index_data WHERE object_store_id = ?")?;
  stmt.execute(params![object_store_id])?;

  Ok(())
}

// Ref: https://w3c.github.io/IndexedDB/#retrieve-a-value-from-an-object-store
#[op]
pub fn op_indexeddb_object_store_retrieve_value(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  range: Range,
) -> Result<Option<ZeroCopyBuf>, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt = conn.prepare_cached(
    "SELECT key, value FROM record WHERE object_store_id = ?",
  )?;
  for row in stmt.query_map(params![object_store_id], |row| {
    Ok((row.get::<usize, Key>(0)?, row.get::<usize, Vec<u8>>(1)?))
  })? {
    let (key, value) = row?;
    if range.contains(&key) {
      return Ok(Some(value.into()));
    }
  }

  Ok(None)
}

// Ref: https://w3c.github.io/IndexedDB/#retrieve-multiple-values-from-an-object-store
#[op]
pub fn op_indexeddb_object_store_retrieve_multiple_values(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  range: Range,
  count: Option<u64>,
) -> Result<Vec<ZeroCopyBuf>, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt = conn.prepare_cached(
    "SELECT key, value FROM record WHERE object_store_id = ?",
  )?;
  let res = stmt
    .query_map(params![object_store_id], |row| {
      Ok((row.get::<usize, Key>(0)?, row.get::<usize, Vec<u8>>(1)?))
    })?
    .filter_map(|row| {
      row
        .map(|(key, val)| {
          if range.contains(&key) {
            Some(val.into())
          } else {
            None
          }
        })
        .transpose()
    });

  Ok(if let Some(count) = count {
    res
      .take(count as usize)
      .collect::<Result<_, rusqlite::Error>>()?
  } else {
    res.collect::<Result<_, rusqlite::Error>>()?
  })
}

// Ref: https://w3c.github.io/IndexedDB/#retrieve-a-key-from-an-object-store
#[op]
pub fn op_indexeddb_object_store_retrieve_key(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  range: Range,
) -> Result<Option<Key>, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt =
    conn.prepare_cached("SELECT key FROM record WHERE object_store_id = ?")?;
  for row in
    stmt.query_map(params![object_store_id], |row| row.get::<usize, Key>(0))?
  {
    let key = row?;
    if range.contains(&key) {
      return Ok(Some(key));
    }
  }

  Ok(None)
}

// Ref: https://w3c.github.io/IndexedDB/#retrieve-multiple-keys-from-an-object-store
#[op]
pub fn op_indexeddb_object_store_retrieve_multiple_keys(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  range: Range,
  count: Option<u64>,
) -> Result<Vec<Key>, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt =
    conn.prepare_cached("SELECT key FROM record WHERE object_store_id = ?")?;
  let res = stmt
    .query_map(params![object_store_id], |row| row.get::<usize, Key>(0))?
    .filter_map(|row| {
      row
        .map(|key| {
          if range.contains(&key) {
            Some(key)
          } else {
            None
          }
        })
        .transpose()
    });

  Ok(if let Some(count) = count {
    res
      .take(count as usize)
      .collect::<Result<_, rusqlite::Error>>()?
  } else {
    res.collect::<Result<_, rusqlite::Error>>()?
  })
}

// Ref: https://w3c.github.io/IndexedDB/#count-the-records-in-a-range
#[op]
pub fn op_indexeddb_object_store_count_records(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  range: Range,
) -> Result<u64, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt = conn.prepare_cached(
    "SELECT key, value FROM record WHERE object_store_id = ?",
  )?;
  let res = stmt
    .query_map(params![object_store_id], |row| {
      Ok((row.get::<usize, Key>(0)?, row.get::<usize, Vec<u8>>(1)?))
    })?
    .filter_map(|row| {
      row
        .map(|(key, val)| {
          if range.contains(&key) {
            Some(())
          } else {
            None
          }
        })
        .transpose()
    });

  Ok(res.count() as u64)
}

#[op]
pub fn op_indexeddb_index_exists(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  index_name: String,
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt = conn.prepare_cached(
    "SELECT * FROM index WHERE name = ? AND object_store_id = ?",
  )?;
  if !stmt.exists(params![index_name, object_store_id])? {
    return Err(
      DomExceptionInvalidStateError::new(&format!(
        "Index with name '{index_name}' does not exists"
      ))
      .into(),
    );
  }

  Ok(())
}

// Ref: https://w3c.github.io/IndexedDB/#count-the-records-in-a-range
#[op]
pub fn op_indexeddb_index_count_records(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  index_name: String,
  range: Range,
) -> Result<u64, AnyError> {
  todo!()
}

// Ref: https://w3c.github.io/IndexedDB/#retrieve-a-referenced-value-from-an-index
// Ref: https://w3c.github.io/IndexedDB/#retrieve-a-value-from-an-index
#[op]
pub fn op_indexeddb_index_retrieve_value(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  index_name: String,
  range: Range,
) -> Result<Option<ZeroCopyBuf>, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM index WHERE name = ? AND object_store_id = ?",
  )?;
  let index_id: u64 =
    stmt.query_row(params![index_name, object_store_id], |row| row.get(0))?;

  todo!();

  Ok(None)
}

// Ref: https://w3c.github.io/IndexedDB/#retrieve-multiple-referenced-values-from-an-index
// Ref: https://w3c.github.io/IndexedDB/#retrieve-a-value-from-an-index
#[op]
pub fn op_indexeddb_index_retrieve_multiple_values(
  state: &mut OpState,
  database_name: String,
  store_name: String,
  index_name: String,
  range: Range,
  count: Option<u64>,
) -> Result<Vec<ZeroCopyBuf>, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM index WHERE name = ? AND object_store_id = ?",
  )?;
  let index_id: u64 =
    stmt.query_row(params![index_name, object_store_id], |row| row.get(0))?;

  todo!();

  Ok(Default::default())
}

#[derive(Serialize, Clone)]
enum Direction {
  Next,
  Nextunique,
  Prev,
  Prevunique,
}

// Ref: https://w3c.github.io/IndexedDB/#iterate-a-cursor
#[op]
pub fn op_indexeddb_object_store_get_records(
  state: &mut OpState,
  database_name: String,
  store_name: String,
) -> Result<Vec<(Key, Vec<u8>)>, AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  let mut stmt = conn.prepare_cached(
    "SELECT id FROM object_store WHERE name = ? AND database_name = ?",
  )?;
  let object_store_id: u64 =
    stmt.query_row(params![store_name, database_name], |row| row.get(0))?;

  let mut stmt = conn.prepare_cached(
    "SELECT key, value FROM record WHERE object_store_id = ?",
  )?;
  let res = stmt
    .query_map(params![object_store_id], |row| {
      Ok((row.get::<usize, Key>(0)?, row.get::<usize, Vec<u8>>(1)?))
    })?
    .collect::<Result<_, rusqlite::Error>>()?;

  Ok(res)
}
