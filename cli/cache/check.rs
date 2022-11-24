// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_runtime::deno_webstorage::rusqlite::params;
use deno_runtime::deno_webstorage::rusqlite::Connection;

use super::common::run_sqlite_pragma;

/// The cache used to tell whether type checking should occur again.
///
/// This simply stores a hash of the inputs of each successful type check
/// and only clears them out when changing CLI versions.
pub struct TypeCheckCache(Option<Connection>);

impl TypeCheckCache {
  pub fn new(db_file_path: &Path) -> Self {
    log::debug!("Loading type check cache.");
    match Self::try_new(db_file_path) {
      Ok(cache) => cache,
      Err(err) => {
        log::debug!(
          concat!(
            "Failed loading internal type checking cache. ",
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
                  "Unable to load internal cache for type checking. ",
                  "This will reduce the performance of type checking.\n\n",
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

  pub fn has_check_hash(&self, hash: u64) -> bool {
    match self.hash_check_hash_result(hash) {
      Ok(val) => val,
      Err(err) => {
        if cfg!(debug_assertions) {
          panic!("Error retrieving hash: {}", err);
        } else {
          log::debug!("Error retrieving hash: {}", err);
          // fail silently when not debugging
          false
        }
      }
    }
  }

  fn hash_check_hash_result(&self, hash: u64) -> Result<bool, AnyError> {
    let conn = match &self.0 {
      Some(conn) => conn,
      None => return Ok(false),
    };
    let query = "SELECT * FROM checkcache WHERE check_hash=?1 LIMIT 1";
    let mut stmt = conn.prepare_cached(query)?;
    Ok(stmt.exists(params![hash.to_string()])?)
  }

  pub fn add_check_hash(&self, check_hash: u64) {
    if let Err(err) = self.add_check_hash_result(check_hash) {
      if cfg!(debug_assertions) {
        panic!("Error saving check hash: {}", err);
      } else {
        log::debug!("Error saving check hash: {}", err);
      }
    }
  }

  fn add_check_hash_result(&self, check_hash: u64) -> Result<(), AnyError> {
    let conn = match &self.0 {
      Some(conn) => conn,
      None => return Ok(()),
    };
    let sql = "
    INSERT OR REPLACE INTO
      checkcache (check_hash)
    VALUES
      (?1)";
    let mut stmt = conn.prepare_cached(sql)?;
    stmt.execute(params![&check_hash.to_string(),])?;
    Ok(())
  }

  pub fn get_tsbuildinfo(&self, specifier: &ModuleSpecifier) -> Option<String> {
    let conn = match &self.0 {
      Some(conn) => conn,
      None => return None,
    };
    let mut stmt = conn
      .prepare_cached("SELECT text FROM tsbuildinfo WHERE specifier=?1 LIMIT 1")
      .ok()?;
    let mut rows = stmt.query(params![specifier.to_string()]).ok()?;
    let row = rows.next().ok().flatten()?;

    row.get(0).ok()
  }

  pub fn set_tsbuildinfo(&self, specifier: &ModuleSpecifier, text: &str) {
    if let Err(err) = self.set_tsbuildinfo_result(specifier, text) {
      // should never error here, but if it ever does don't fail
      if cfg!(debug_assertions) {
        panic!("Error saving tsbuildinfo: {}", err);
      } else {
        log::debug!("Error saving tsbuildinfo: {}", err);
      }
    }
  }

  fn set_tsbuildinfo_result(
    &self,
    specifier: &ModuleSpecifier,
    text: &str,
  ) -> Result<(), AnyError> {
    let conn = match &self.0 {
      Some(conn) => conn,
      None => return Ok(()),
    };
    let mut stmt = conn.prepare_cached(
      "INSERT OR REPLACE INTO tsbuildinfo (specifier, text) VALUES (?1, ?2)",
    )?;
    stmt.execute(params![specifier.to_string(), text])?;
    Ok(())
  }
}

fn create_tables(
  conn: &Connection,
  cli_version: String,
) -> Result<(), AnyError> {
  // INT doesn't store up to u64, so use TEXT
  conn.execute(
    "CREATE TABLE IF NOT EXISTS checkcache (
      check_hash TEXT PRIMARY KEY
    )",
    [],
  )?;
  conn.execute(
    "CREATE TABLE IF NOT EXISTS tsbuildinfo (
        specifier TEXT PRIMARY KEY,
        text TEXT NOT NULL
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
    conn.execute("DELETE FROM checkcache", params![])?;
    conn.execute("DELETE FROM tsbuildinfo", params![])?;
    let mut stmt = conn
      .prepare("INSERT OR REPLACE INTO info (key, value) VALUES (?1, ?2)")?;
    stmt.execute(params!["CLI_VERSION", &cli_version])?;
  }

  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  pub fn check_cache_general_use() {
    let conn = Connection::open_in_memory().unwrap();
    let cache =
      TypeCheckCache::from_connection(conn, "1.0.0".to_string()).unwrap();

    assert!(!cache.has_check_hash(1));
    cache.add_check_hash(1);
    assert!(cache.has_check_hash(1));
    assert!(!cache.has_check_hash(2));

    let specifier1 = ModuleSpecifier::parse("file:///test.json").unwrap();
    assert_eq!(cache.get_tsbuildinfo(&specifier1), None);
    cache.set_tsbuildinfo(&specifier1, "test");
    assert_eq!(cache.get_tsbuildinfo(&specifier1), Some("test".to_string()));

    // try changing the cli version (should clear)
    let conn = cache.0.unwrap();
    let cache =
      TypeCheckCache::from_connection(conn, "2.0.0".to_string()).unwrap();
    assert!(!cache.has_check_hash(1));
    cache.add_check_hash(1);
    assert!(cache.has_check_hash(1));
    assert_eq!(cache.get_tsbuildinfo(&specifier1), None);
    cache.set_tsbuildinfo(&specifier1, "test");
    assert_eq!(cache.get_tsbuildinfo(&specifier1), Some("test".to_string()));

    // recreating the cache should not remove the data because the CLI version is the same
    let conn = cache.0.unwrap();
    let cache =
      TypeCheckCache::from_connection(conn, "2.0.0".to_string()).unwrap();
    assert!(cache.has_check_hash(1));
    assert!(!cache.has_check_hash(2));
    assert_eq!(cache.get_tsbuildinfo(&specifier1), Some("test".to_string()));

    // adding when already exists should not cause issue
    cache.add_check_hash(1);
    assert!(cache.has_check_hash(1));
    cache.set_tsbuildinfo(&specifier1, "other");
    assert_eq!(
      cache.get_tsbuildinfo(&specifier1),
      Some("other".to_string())
    );
  }
}
