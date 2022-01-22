use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use rusqlite::params;
use rusqlite::Connection;
use serde::Deserialize;
use serde::Serialize;

// Bump these to invalidate the caches on a row by row basis
const EMIT_DATA_VERSION: usize = 1;
const TS_BUILD_INFO_VERSION: usize = 1;

// todo: remove serialize, deserialize BEFORE PR
/// Emit cache for a single file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmitCacheData {
  pub source_hash: String,
  pub text: String,
  pub map: Option<String>,
  pub declaration: Option<String>,
}

pub trait EmitCache {
  /// Gets the emit data from the cache.
  fn get_emit_data(&self, specifier: &ModuleSpecifier)
    -> Option<EmitCacheData>;
  /// Sets the emit data in the cache.
  fn set_emit_data(
    &self,
    specifier: &ModuleSpecifier,
    data: &EmitCacheData,
  ) -> Result<(), AnyError>;

  /// Gets the .tsbuildinfo file from the cache.
  fn get_tsbuildinfo(&self, specifier: &ModuleSpecifier) -> Option<String>;
  /// Sets the .tsbuildinfo file in the cache.
  fn set_tsbuildinfo(
    &self,
    specifier: &ModuleSpecifier,
    text: &str,
  ) -> Result<(), AnyError>;
}

#[derive(Default)]
pub struct MemoryEmitCache {
  build_infos: RefCell<HashMap<ModuleSpecifier, String>>,
  emits: RefCell<HashMap<ModuleSpecifier, EmitCacheData>>,
}

impl EmitCache for MemoryEmitCache {
  fn get_emit_data(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<EmitCacheData> {
    self.emits.borrow().get(specifier).cloned()
  }

  fn set_emit_data(
    &self,
    specifier: &ModuleSpecifier,
    data: &EmitCacheData,
  ) -> Result<(), AnyError> {
    self
      .emits
      .borrow_mut()
      .insert(specifier.clone(), data.clone());
    Ok(())
  }

  fn get_tsbuildinfo(&self, specifier: &ModuleSpecifier) -> Option<String> {
    self.build_infos.borrow().get(specifier).cloned()
  }

  fn set_tsbuildinfo(
    &self,
    specifier: &ModuleSpecifier,
    text: &str,
  ) -> Result<(), AnyError> {
    self
      .build_infos
      .borrow_mut()
      .insert(specifier.clone(), text.to_string());
    Ok(())
  }
}

pub struct SqliteEmitCache {
  conn: Connection,
  /// these are stored here for mutating during testing
  emit_data_version: usize,
  ts_build_info_version: usize,
}

impl SqliteEmitCache {
  pub fn new(db_file_path: &Path) -> Result<Self, AnyError> {
    let conn = Connection::open(db_file_path)?;
    Self::from_connection(conn)
  }

  pub(super) fn from_connection(conn: Connection) -> Result<Self, AnyError> {
    run_pragma(&conn)?;
    create_tables(&conn)?;

    Ok(Self {
      conn,
      emit_data_version: EMIT_DATA_VERSION,
      ts_build_info_version: TS_BUILD_INFO_VERSION,
    })
  }
}

fn run_pragma(conn: &Connection) -> Result<(), AnyError> {
  // Enable write-ahead-logging and tweak some other stuff
  let initial_pragmas = "
    -- enable write-ahead-logging mode
    PRAGMA journal_mode=WAL;
    PRAGMA synchronous=NORMAL;
    PRAGMA temp_store=memory;
    PRAGMA page_size=4096;
    PRAGMA mmap_size=6000000;
    PRAGMA optimize;
  ";

  conn.execute_batch(initial_pragmas)?;
  Ok(())
}

fn create_tables(conn: &Connection) -> Result<(), AnyError> {
  conn.execute(
    "CREATE TABLE IF NOT EXISTS emitdata (
        specifier TEXT PRIMARY KEY,
        version INTEGER NOT NULL,
        source_hash TEXT NOT NULL,
        text TEXT NOT NULL,
        source_map TEXT,
        declaration TEXT
      )",
    [],
  )?;
  conn.execute(
    "CREATE TABLE IF NOT EXISTS tsbuildinfo (
        specifier TEXT PRIMARY KEY,
        version INTEGER NOT NULL,
        text TEXT NOT NULL
      )",
    [],
  )?;
  Ok(())
}

impl EmitCache for SqliteEmitCache {
  fn get_emit_data(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<EmitCacheData> {
    let mut stmt = self.conn.prepare_cached("SELECT source_hash, text, source_map, declaration FROM emitdata WHERE specifier=?1 AND version=?2 LIMIT 1").ok()?;
    let mut rows = stmt
      .query(params![specifier.to_string(), self.emit_data_version])
      .ok()?;
    let row = rows.next().ok().flatten()?;

    Some(EmitCacheData {
      source_hash: row.get(0).ok()?,
      text: row.get(1).ok()?,
      map: row.get(2).ok()?,
      declaration: row.get(3).ok()?,
    })
  }

  fn set_emit_data(
    &self,
    specifier: &ModuleSpecifier,
    data: &EmitCacheData,
  ) -> Result<(), AnyError> {
    let mut stmt = self.conn.prepare_cached("INSERT OR REPLACE INTO emitdata (specifier, version, source_hash, text, source_map, declaration) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    stmt.execute(params![
      specifier.to_string(),
      self.emit_data_version,
      &data.source_hash,
      &data.text,
      &data.map,
      &data.declaration
    ])?;
    Ok(())
  }

  fn get_tsbuildinfo(&self, specifier: &ModuleSpecifier) -> Option<String> {
    let mut stmt = self.conn.prepare_cached("SELECT text FROM tsbuildinfo WHERE specifier=?1 AND version=?2 LIMIT 1").ok()?;
    let mut rows = stmt
      .query(params![specifier.to_string(), self.ts_build_info_version])
      .ok()?;
    let row = rows.next().ok().flatten()?;

    row.get(0).ok()
  }

  fn set_tsbuildinfo(
    &self,
    specifier: &ModuleSpecifier,
    text: &str,
  ) -> Result<(), AnyError> {
    let mut stmt = self.conn.prepare_cached("INSERT OR REPLACE INTO tsbuildinfo (specifier, version, text) VALUES (?1, ?2, ?3)")?;
    stmt.execute(params![
      specifier.to_string(),
      self.ts_build_info_version,
      text
    ])?;
    Ok(())
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  pub fn emit_data_inserts_and_updates() {
    let conn = Connection::open_in_memory().unwrap();
    let mut cache = SqliteEmitCache::from_connection(conn).unwrap();
    let specifier = ModuleSpecifier::parse("file:///mod.ts").unwrap();

    // insert
    let data = EmitCacheData {
      source_hash: "source_hash".to_string(),
      text: "text".to_string(),
      declaration: Some("declaration".to_string()),
      map: Some("map".to_string()),
    };
    cache.set_emit_data(&specifier, &data).unwrap();
    let retrieved_data = cache.get_emit_data(&specifier).unwrap();
    assert_eq!(retrieved_data, data);

    // update
    let data = EmitCacheData {
      source_hash: "source_hash2".to_string(),
      text: "text2".to_string(),
      declaration: Some("declaration2".to_string()),
      map: Some("map2".to_string()),
    };
    cache.set_emit_data(&specifier, &data).unwrap();
    let retrieved_data = cache.get_emit_data(&specifier).unwrap();
    assert_eq!(retrieved_data, data);

    // update empty decl and map
    let data = EmitCacheData {
      source_hash: "source_hash3".to_string(),
      text: "text3".to_string(),
      declaration: None,
      map: None,
    };
    cache.set_emit_data(&specifier, &data).unwrap();
    let retrieved_data = cache.get_emit_data(&specifier).unwrap();
    assert_eq!(retrieved_data, data);

    // insert another record
    let other_specifier = ModuleSpecifier::parse("file:///mod2.ts").unwrap();
    let other_data = EmitCacheData {
      source_hash: "other_source_hash".to_string(),
      text: "other_text".to_string(),
      declaration: None,
      map: None,
    };
    cache.set_emit_data(&other_specifier, &other_data).unwrap();
    let retrieved_data = cache.get_emit_data(&other_specifier).unwrap();
    assert_eq!(retrieved_data, other_data);

    // ensure the previous record still exists
    let retrieved_data = cache.get_emit_data(&specifier).unwrap();
    assert_eq!(retrieved_data, data);

    // should return None for record that doesn't exist
    let non_existing_specifier =
      ModuleSpecifier::parse("file:///does-not-exist.ts").unwrap();
    let retrieved_data = cache.get_emit_data(&non_existing_specifier);
    assert_eq!(retrieved_data, None);

    // now modify the emit data version
    cache.emit_data_version = EMIT_DATA_VERSION + 1;

    // everything should return None
    assert_eq!(cache.get_emit_data(&specifier), None);
    assert_eq!(cache.get_emit_data(&other_specifier), None);

    // ...until it's updated again
    let data = EmitCacheData {
      source_hash: "source_hash".to_string(),
      text: "text".to_string(),
      declaration: Some("declaration".to_string()),
      map: Some("map".to_string()),
    };
    cache.set_emit_data(&specifier, &data).unwrap();

    assert_eq!(cache.get_emit_data(&specifier), Some(data));
    assert_eq!(cache.get_emit_data(&other_specifier), None);
  }

  #[test]
  pub fn tsbuildinfo_inserts_and_updates() {
    let conn = Connection::open_in_memory().unwrap();
    let mut cache = SqliteEmitCache::from_connection(conn).unwrap();
    let specifier = ModuleSpecifier::parse("file:///mod.ts").unwrap();
    // insert
    cache.set_tsbuildinfo(&specifier, "1").unwrap();
    assert_eq!(cache.get_tsbuildinfo(&specifier), Some("1".to_string()));

    // update
    cache.set_tsbuildinfo(&specifier, "2").unwrap();
    assert_eq!(cache.get_tsbuildinfo(&specifier), Some("2".to_string()));

    // other record
    let other_specifier = ModuleSpecifier::parse("file:///other.ts").unwrap();
    cache.set_tsbuildinfo(&other_specifier, "other").unwrap();
    assert_eq!(
      cache.get_tsbuildinfo(&other_specifier),
      Some("other".to_string())
    );

    // ensure original record is still the same
    assert_eq!(cache.get_tsbuildinfo(&specifier), Some("2".to_string()));

    // return None for specifier that doesn't exist
    let non_existing_specifier =
      ModuleSpecifier::parse("file:///does-not-exist.ts").unwrap();
    assert_eq!(cache.get_tsbuildinfo(&non_existing_specifier), None);

    // now modify the tsbuildinfo_version
    cache.ts_build_info_version = TS_BUILD_INFO_VERSION + 1;

    // everything should return None
    assert_eq!(cache.get_tsbuildinfo(&specifier), None);
    assert_eq!(cache.get_tsbuildinfo(&other_specifier), None);

    // ...until it's updated again
    cache.set_tsbuildinfo(&specifier, "new").unwrap();
    assert_eq!(cache.get_tsbuildinfo(&specifier), Some("new".to_string()));

    assert_eq!(cache.get_tsbuildinfo(&other_specifier), None);
    cache.set_tsbuildinfo(&other_specifier, "other").unwrap();
    assert_eq!(
      cache.get_tsbuildinfo(&other_specifier),
      Some("other".to_string())
    );
  }
}
