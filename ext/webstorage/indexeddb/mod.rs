mod idbtrait;
mod sqlite_idb;

use crate::{
  DomExceptionNotSupportedError, DomExceptionVersionError, OriginStorageDir,
};
use deno_core::error::AnyError;
use deno_core::{OpState, Resource, ResourceId};
use rusqlite::{params, Connection, OptionalExtension};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

fn create_file_table(conn: &Connection) -> Result<(), AnyError> {
  let statements = r#"
      CREATE TABLE IF NOT EXISTS file (
        id INTEGER PRIMARY KEY,
        refcount INTEGER NOT NULL
      );

      CREATE TRIGGER object_data_insert_trigger
        AFTER INSERT ON object_data
        FOR EACH ROW
        WHEN NEW.file_ids IS NOT NULL
        BEGIN
        SELECT update_refcount(NULL, NEW.file_ids);

      CREATE TRIGGER object_data_update_trigger
        AFTER UPDATE OF file_ids ON object_data
        FOR EACH ROW
        WHEN OLD.file_ids IS NOT NULL OR NEW.file_ids IS NOT NULL
        BEGIN
        SELECT update_refcount(OLD.file_ids, NEW.file_ids);

      CREATE TRIGGER object_data_delete_trigger
        AFTER DELETE ON object_data
        FOR EACH ROW WHEN OLD.file_ids IS NOT NULL
        BEGIN
        SELECT update_refcount(OLD.file_ids, NULL);

      CREATE TRIGGER file_update_trigger
        AFTER UPDATE ON file
        FOR EACH ROW WHEN NEW.refcount = 0
        BEGIN
        DELETE FROM file WHERE id = OLD.id;
      "#;
  conn.execute_batch(statements)?;
  Ok(())
}

fn create_table(conn: &Connection) -> Result<(), AnyError> {
  let statements = r#"
      CREATE TABLE database (
        name TEXT PRIMARY KEY,
        version INTEGER NOT NULL DEFAULT 0
      ) WITHOUT ROWID;

      CREATE TABLE object_store (
        id INTEGER PRIMARY KEY,
        auto_increment INTEGER NOT NULL DEFAULT 0,
        name TEXT NOT NULL,
        key_path TEXT
      );

      CREATE TABLE object_store_index (
        id INTEGER PRIMARY KEY,
        object_store_id INTEGER NOT NULL,
        database_name TEXT NOT NULL,
        name TEXT NOT NULL,
        key_path TEXT NOT NULL,
        unique_index INTEGER NOT NULL,
        multientry INTEGER NOT NULL,
        FOREIGN KEY (object_store_id)
          REFERENCES object_store(id)
        FOREIGN KEY (database_name)
          REFERENCES database(name)
      );

      CREATE TABLE object_data (
        object_store_id INTEGER NOT NULL,
        key BLOB NOT NULL,
        index_data_values BLOB DEFAULT NULL,
        file_ids TEXT,
        data BLOB NOT NULL,
        PRIMARY KEY (object_store_id, key),
        FOREIGN KEY (object_store_id)
          REFERENCES object_store(id)
      ) WITHOUT ROWID;

      CREATE TABLE index_data (
        index_id INTEGER NOT NULL,
        value BLOB NOT NULL,
        object_data_key BLOB NOT NULL,
        object_store_id INTEGER NOT NULL,
        value_locale BLOB,
        PRIMARY KEY (index_id, value, object_data_key),
        FOREIGN KEY (index_id)
          REFERENCES object_store_index(id),
        FOREIGN KEY (object_store_id, object_data_key)
          REFERENCES object_data(object_store_id, key)
      ) WITHOUT ROWID;

      CREATE INDEX index_data_value_locale_index
      ON index_data (index_id, value_locale, object_data_key, value)
      WHERE value_locale IS NOT NULL;

      CREATE TABLE unique_index_data (
        index_id INTEGER NOT NULL,
        value BLOB NOT NULL,
        object_store_id INTEGER NOT NULL,
        object_data_key BLOB NOT NULL,
        value_locale BLOB,
        PRIMARY KEY (index_id, value),
        FOREIGN KEY (index_id)
          REFERENCES object_store_index(id),
        FOREIGN KEY (object_store_id, object_data_key)
          REFERENCES object_data(object_store_id, key)
      ) WITHOUT ROWID;

      CREATE INDEX unique_index_data_value_locale_index
      ON unique_index_data (index_id, value_locale, object_data_key, value)
      WHERE value_locale IS NOT NULL
      "#;
  conn.execute_batch(statements)?;
  Ok(())
}

struct Database {
  name: String,
  version: u64,
}

struct IndexedDbConnection {
  conn: Connection,
  close_pending: bool,
}

pub struct IndexedDbManager(Connection);
pub struct IndexedDbResource(Connection);

impl Resource for IndexedDbResource {
  fn name(&self) -> Cow<str> {
    "indexedDb".into()
  }
}

// Ref: https://w3c.github.io/IndexedDB/#open-a-database
pub fn op_indexeddb_open(
  state: &mut OpState,
  name: String,
  version: Option<u64>,
) -> Result<(u64, u64), AnyError> {
  if state.try_borrow::<IndexedDbManager>().is_none() {
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
    create_table(&conn)?;

    conn.set_prepared_statement_cache_capacity(128);
    state.put(IndexedDbManager(conn));
  }

  let idbmanager = state.borrow::<IndexedDbManager>();
  let conn = &idbmanager.0;
  let mut stmt = conn.prepare_cached("SELECT * FROM database WHERE name = ?")?;
  let db = stmt
    .query_row(params![name], |row| {
      Ok(Database {
        name: row.get(0)?,
        version: row.get(1)?,
      })
    })
    .optional()?;
  let version = version.or_else(|| db.map(|db| db.version)).unwrap_or(1);

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

// Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-createobjectstore
pub fn op_indexeddb_list_databases(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<Vec<String>, AnyError> {
  let idbmanager = &state.borrow::<IndexedDbManager>().0;
  let mut stmt = idbmanager.prepare_cached("SELECT name FROM database")?;
  let names = stmt.query(params![])?.map(|row| row.get(0).unwrap()).collect::<Vec<String>>()?;
  Ok(names)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateObjectStoreArgs {
  database_name: String,
  name: String,
  key_path: Option<String>,
  auto_increment: bool,
}

// Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-createobjectstore
pub fn op_indexeddb_database_create_object_store(
  state: &mut OpState,
  args: CreateObjectStoreArgs,
  _: (),
) -> Result<(), AnyError> {
  let conn = &state.borrow::<IndexedDbManager>().0;

  // TODO: this might be doable on the JS side
  let mut stmt = conn.prepare_cached(
    "SELECT * FROM object_store_index WHERE name = ? AND database_name = ?",
  )?;
  if stmt.exists(params![args.name, args.database_name])? {
    return Err();
  }

  // TODO: 8.

  let mut stmt = conn.prepare_cached(
    "INSERT INTO object_store (name, keyPath) VALUES (?, ?) RETURNING id",
  )?;
  let store_id: u64 =
    stmt.query_row(params![args.name, args.key_path], |row| row.get(0))?;

  let mut stmt = conn.prepare_cached("INSERT INTO object_store_index (object_store_id, database_name, name, key_path) VALUES (?, ?, ?, ?)")?;
  stmt.execute(params![
    store_id,
    args.database_name,
    args.name,
    args.key_path
  ])?; // TODO: more args needed

  Ok(())
}
