// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_runtime::deno_webstorage::rusqlite::params;
use deno_runtime::deno_webstorage::rusqlite::Connection;

use super::common::run_sqlite_pragma;

/// Emit cache for a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecifierEmitCacheData {
  pub source_hash: u64,
  pub text: String,
  pub map: Option<String>,
}

/// The cache that stores previously emitted files.
pub struct EmitCache(Option<Connection>);

impl EmitCache {
  pub fn new(db_file_path: &Path) -> Self {
    match Self::try_new(db_file_path) {
      Ok(cache) => cache,
      Err(err) => {
        log::debug!(
          concat!(
            "Failed creating internal emit cache. ",
            "Recreating...\n\nError details:\n{:#}",
          ),
          err
        );
        // Maybe the cache file is corrupt. Attempt to remove the cache file
        // then attempt to recreate again. Otherwise, use null object pattern.
        match std::fs::remove_file(db_file_path) {
          Ok(_) => match Self::try_new(db_file_path) {
            Ok(cache) => cache,
            Err(err) => {
              log::debug!(
                concat!(
                  "Unable to create internal emit cache. ",
                  "This will reduce the performance of emitting.\n\n",
                  "Error details:\n{:#}",
                ),
                err
              );
              Self(None)
            }
          },
          Err(_) => Self(None),
        }
      }
    }
  }

  fn try_new(db_file_path: &Path) -> Result<Self, AnyError> {
    let conn = Connection::open(db_file_path)?;
    Self::from_connection(conn, crate::version::deno())
  }

  fn from_connection(
    conn: Connection,
    cli_version: String,
  ) -> Result<Self, AnyError> {
    run_sqlite_pragma(&conn)?;
    create_tables(&conn, cli_version)?;

    Ok(Self(Some(conn)))
  }

  /// Gets the emit data from the cache.
  pub fn get_emit_data(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<SpecifierEmitCacheData> {
    let conn = match &self.0 {
      Some(conn) => conn,
      None => return None,
    };
    let mut stmt = conn
      .prepare_cached("SELECT source_hash, emit_text, source_map FROM emitcache WHERE specifier=?1 LIMIT 1")
      .ok()?;
    let mut rows = stmt.query(params![specifier.to_string()]).ok()?;
    let row = rows.next().ok().flatten()?;

    Some(SpecifierEmitCacheData {
      source_hash: row.get::<usize, String>(0).ok()?.parse::<u64>().ok()?,
      text: row.get(1).ok()?,
      map: row.get(2).ok()?,
    })
  }

  /// Sets the emit data in the cache.
  pub fn set_emit_data(
    &self,
    specifier: &ModuleSpecifier,
    data: &SpecifierEmitCacheData,
  ) {
    if let Err(err) = self.set_emit_data_result(specifier, data) {
      // should never error here, but if it ever does don't fail
      if cfg!(debug_assertions) {
        panic!("Error saving emit data: {}", err);
      } else {
        log::debug!("Error saving emit data: {}", err);
      }
    }
  }

  fn set_emit_data_result(
    &self,
    specifier: &ModuleSpecifier,
    data: &SpecifierEmitCacheData,
  ) -> Result<(), AnyError> {
    let conn = match &self.0 {
      Some(conn) => conn,
      None => return Ok(()),
    };
    let mut stmt = conn.prepare_cached(
      "INSERT OR REPLACE INTO emitcache (specifier, source_hash, emit_text, source_map) VALUES (?1, ?2, ?3, ?4)",
    )?;
    stmt.execute(params![
      specifier.to_string(),
      &data.source_hash.to_string(),
      &data.text,
      &data.map,
    ])?;
    Ok(())
  }
}

fn create_tables(
  conn: &Connection,
  cli_version: String,
) -> Result<(), AnyError> {
  // INT doesn't store up to u64, so use TEXT for source_hash
  conn.execute(
    "CREATE TABLE IF NOT EXISTS emitcache (
      specifier TEXT PRIMARY KEY,
      source_hash TEXT NOT NULL,
      emit_text TEXT NOT NULL,
      source_map TEXT
    )",
    [],
  )?;
  conn.execute(
    "CREATE TABLE IF NOT EXISTS info (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    )",
    [],
  )?;

  // delete the cache when the CLI version changes
  let data_cli_version: Option<String> = conn
    .query_row(
      "SELECT value FROM info WHERE key='CLI_VERSION' LIMIT 1",
      [],
      |row| row.get(0),
    )
    .ok();
  if data_cli_version != Some(cli_version.to_string()) {
    conn.execute("DELETE FROM emitcache", params![])?;
    let mut stmt = conn
      .prepare("INSERT OR REPLACE INTO info (key, value) VALUES (?1, ?2)")?;
    stmt.execute(params!["CLI_VERSION", &cli_version])?;
  }

  Ok(())
}

// #[cfg(test)]
// mod test {
//   use super::*;

//   #[test]
//   pub fn check_cache_general_use() {
//     let conn = Connection::open_in_memory().unwrap();
//     let cache = EmitCache::from_connection(conn, "1.0.0".to_string()).unwrap();

//     assert!(!cache.has_check_hash(1));
//     cache.add_check_hash(1);
//     assert!(cache.has_check_hash(1));
//     assert!(!cache.has_check_hash(2));

//     let specifier1 = ModuleSpecifier::parse("file:///test.json").unwrap();
//     assert_eq!(cache.get_tsbuildinfo(&specifier1), None);
//     cache.set_tsbuildinfo(&specifier1, "test");
//     assert_eq!(cache.get_tsbuildinfo(&specifier1), Some("test".to_string()));

//     // try changing the cli version (should clear)
//     let conn = cache.0.unwrap();
//     let cache =
//       TypeCheckCache::from_connection(conn, "2.0.0".to_string()).unwrap();
//     assert!(!cache.has_check_hash(1));
//     cache.add_check_hash(1);
//     assert!(cache.has_check_hash(1));
//     assert_eq!(cache.get_tsbuildinfo(&specifier1), None);
//     cache.set_tsbuildinfo(&specifier1, "test");
//     assert_eq!(cache.get_tsbuildinfo(&specifier1), Some("test".to_string()));

//     // recreating the cache should not remove the data because the CLI version and state hash is the same
//     let conn = cache.0.unwrap();
//     let cache =
//       TypeCheckCache::from_connection(conn, "2.0.0".to_string()).unwrap();
//     assert!(cache.has_check_hash(1));
//     assert!(!cache.has_check_hash(2));
//     assert_eq!(cache.get_tsbuildinfo(&specifier1), Some("test".to_string()));

//     // adding when already exists should not cause issue
//     cache.add_check_hash(1);
//     assert!(cache.has_check_hash(1));
//     cache.set_tsbuildinfo(&specifier1, "other");
//     assert_eq!(
//       cache.get_tsbuildinfo(&specifier1),
//       Some("other".to_string())
//     );
//   }
// }
