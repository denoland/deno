// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_runtime::deno_webstorage::rusqlite::params;
use deno_runtime::deno_webstorage::rusqlite::Connection;
use serde::Serialize;
use tokio::task::JoinHandle;

use super::common::run_sqlite_pragma;
use super::common::FastInsecureHasher;

/// Cache used to skip formatting/linting a file again when we
/// know it is already formatted or has no lint diagnostics.
pub struct IncrementalCache(Option<IncrementalCacheInner>);

impl IncrementalCache {
  pub fn new<TState: Serialize>(
    db_file_path: &Path,
    state: &TState,
    initial_file_paths: &[PathBuf],
  ) -> Self {
    // if creating the incremental cache fails, then we
    // treat it as not having a cache
    let result =
      IncrementalCacheInner::new(db_file_path, state, initial_file_paths);
    IncrementalCache(match result {
      Ok(inner) => Some(inner),
      Err(err) => {
        log::debug!("Creating the incremental cache failed.\n{:#}", err);
        // Maybe the cache file is corrupt. Attempt to remove
        // the cache file for next time
        let _ = std::fs::remove_file(db_file_path);
        None
      }
    })
  }

  pub fn is_file_same(&self, file_path: &Path, file_text: &str) -> bool {
    if let Some(inner) = &self.0 {
      inner.is_file_same(file_path, file_text)
    } else {
      false
    }
  }

  pub fn update_file(&self, file_path: &Path, file_text: &str) {
    if let Some(inner) = &self.0 {
      inner.update_file(file_path, file_text)
    }
  }

  pub async fn wait_completion(&self) {
    if let Some(inner) = &self.0 {
      inner.wait_completion().await;
    }
  }
}

enum ReceiverMessage {
  Update(PathBuf, u64),
  Exit,
}

struct IncrementalCacheInner {
  previous_hashes: HashMap<PathBuf, u64>,
  sender: tokio::sync::mpsc::UnboundedSender<ReceiverMessage>,
  handle: Mutex<Option<JoinHandle<()>>>,
}

impl IncrementalCacheInner {
  pub fn new<TState: Serialize>(
    db_file_path: &Path,
    state: &TState,
    initial_file_paths: &[PathBuf],
  ) -> Result<Self, AnyError> {
    let state_hash = FastInsecureHasher::new()
      .write_str(&serde_json::to_string(state).unwrap())
      .finish();
    let sql_cache = SqlIncrementalCache::new(db_file_path, state_hash)?;
    Ok(Self::from_sql_incremental_cache(
      sql_cache,
      initial_file_paths,
    ))
  }

  fn from_sql_incremental_cache(
    cache: SqlIncrementalCache,
    initial_file_paths: &[PathBuf],
  ) -> Self {
    let mut previous_hashes = HashMap::new();
    for path in initial_file_paths {
      if let Some(hash) = cache.get_source_hash(path) {
        previous_hashes.insert(path.to_path_buf(), hash);
      }
    }

    let (sender, mut receiver) =
      tokio::sync::mpsc::unbounded_channel::<ReceiverMessage>();

    // sqlite isn't `Sync`, so we do all the updating on a dedicated task
    let handle = tokio::task::spawn(async move {
      while let Some(message) = receiver.recv().await {
        match message {
          ReceiverMessage::Update(path, hash) => {
            let _ = cache.set_source_hash(&path, hash);
          }
          ReceiverMessage::Exit => break,
        }
      }
    });

    IncrementalCacheInner {
      previous_hashes,
      sender,
      handle: Mutex::new(Some(handle)),
    }
  }

  pub fn is_file_same(&self, file_path: &Path, file_text: &str) -> bool {
    match self.previous_hashes.get(file_path) {
      Some(hash) => {
        *hash == FastInsecureHasher::new().write_str(file_text).finish()
      }
      None => false,
    }
  }

  pub fn update_file(&self, file_path: &Path, file_text: &str) {
    let hash = FastInsecureHasher::new().write_str(file_text).finish();
    if let Some(previous_hash) = self.previous_hashes.get(file_path) {
      if *previous_hash == hash {
        return; // do not bother updating the db file because nothing has changed
      }
    }
    let _ = self
      .sender
      .send(ReceiverMessage::Update(file_path.to_path_buf(), hash));
  }

  pub async fn wait_completion(&self) {
    if self.sender.send(ReceiverMessage::Exit).is_err() {
      return;
    }
    let handle = self.handle.lock().take();
    if let Some(handle) = handle {
      handle.await.unwrap();
    }
  }
}

struct SqlIncrementalCache {
  conn: Connection,
  /// A hash of the state used to produce the formatting/linting other than
  /// the CLI version. This state is a hash of the configuration and ensures
  /// we format/lint a file when the configuration changes.
  state_hash: u64,
}

impl SqlIncrementalCache {
  pub fn new(db_file_path: &Path, state_hash: u64) -> Result<Self, AnyError> {
    log::debug!("Loading incremental cache.");
    let conn = Connection::open(db_file_path)?;
    Self::from_connection(conn, state_hash, crate::version::deno())
  }

  fn from_connection(
    conn: Connection,
    state_hash: u64,
    cli_version: String,
  ) -> Result<Self, AnyError> {
    run_sqlite_pragma(&conn)?;
    create_tables(&conn, cli_version)?;

    Ok(Self { conn, state_hash })
  }

  pub fn get_source_hash(&self, path: &Path) -> Option<u64> {
    match self.get_source_hash_result(path) {
      Ok(option) => option,
      Err(err) => {
        if cfg!(debug_assertions) {
          panic!("Error retrieving hash: {}", err);
        } else {
          // fail silently when not debugging
          None
        }
      }
    }
  }

  fn get_source_hash_result(
    &self,
    path: &Path,
  ) -> Result<Option<u64>, AnyError> {
    let query = "
      SELECT
        source_hash
      FROM
        incrementalcache
      WHERE
        file_path=?1
        AND state_hash=?2
      LIMIT 1";
    let mut stmt = self.conn.prepare_cached(query)?;
    let mut rows = stmt
      .query(params![path.to_string_lossy(), self.state_hash.to_string()])?;
    if let Some(row) = rows.next()? {
      let hash: String = row.get(0)?;
      Ok(Some(hash.parse::<u64>()?))
    } else {
      Ok(None)
    }
  }

  pub fn set_source_hash(
    &self,
    path: &Path,
    source_hash: u64,
  ) -> Result<(), AnyError> {
    let sql = "
      INSERT OR REPLACE INTO
        incrementalcache (file_path, state_hash, source_hash)
      VALUES
        (?1, ?2, ?3)";
    let mut stmt = self.conn.prepare_cached(sql)?;
    stmt.execute(params![
      path.to_string_lossy(),
      &self.state_hash.to_string(),
      &source_hash.to_string(),
    ])?;
    Ok(())
  }
}

fn create_tables(
  conn: &Connection,
  cli_version: String,
) -> Result<(), AnyError> {
  // INT doesn't store up to u64, so use TEXT
  conn.execute(
    "CREATE TABLE IF NOT EXISTS incrementalcache (
        file_path TEXT PRIMARY KEY,
        state_hash TEXT NOT NULL,
        source_hash TEXT NOT NULL
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
    conn.execute("DELETE FROM incrementalcache", params![])?;
    let mut stmt = conn
      .prepare("INSERT OR REPLACE INTO info (key, value) VALUES (?1, ?2)")?;
    stmt.execute(params!["CLI_VERSION", &cli_version])?;
  }

  Ok(())
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  use super::*;

  #[test]
  pub fn sql_cache_general_use() {
    let conn = Connection::open_in_memory().unwrap();
    let cache =
      SqlIncrementalCache::from_connection(conn, 1, "1.0.0".to_string())
        .unwrap();
    let path = PathBuf::from("/mod.ts");

    assert_eq!(cache.get_source_hash(&path), None);
    cache.set_source_hash(&path, 2).unwrap();
    assert_eq!(cache.get_source_hash(&path), Some(2));

    // try changing the cli version (should clear)
    let conn = cache.conn;
    let mut cache =
      SqlIncrementalCache::from_connection(conn, 1, "2.0.0".to_string())
        .unwrap();
    assert_eq!(cache.get_source_hash(&path), None);

    // add back the file to the cache
    cache.set_source_hash(&path, 2).unwrap();
    assert_eq!(cache.get_source_hash(&path), Some(2));

    // try changing the state hash
    cache.state_hash = 2;
    assert_eq!(cache.get_source_hash(&path), None);
    cache.state_hash = 1;

    // should return now that everything is back
    assert_eq!(cache.get_source_hash(&path), Some(2));

    // recreating the cache should not remove the data because the CLI version and state hash is the same
    let conn = cache.conn;
    let cache =
      SqlIncrementalCache::from_connection(conn, 1, "2.0.0".to_string())
        .unwrap();
    assert_eq!(cache.get_source_hash(&path), Some(2));

    // now try replacing and using another path
    cache.set_source_hash(&path, 3).unwrap();
    cache.set_source_hash(&path, 4).unwrap();
    let path2 = PathBuf::from("/mod2.ts");
    cache.set_source_hash(&path2, 5).unwrap();
    assert_eq!(cache.get_source_hash(&path), Some(4));
    assert_eq!(cache.get_source_hash(&path2), Some(5));
  }

  #[tokio::test]
  pub async fn incremental_cache_general_use() {
    let conn = Connection::open_in_memory().unwrap();
    let sql_cache =
      SqlIncrementalCache::from_connection(conn, 1, "1.0.0".to_string())
        .unwrap();
    let file_path = PathBuf::from("/mod.ts");
    let file_text = "test";
    let file_hash = FastInsecureHasher::new().write_str(file_text).finish();
    sql_cache.set_source_hash(&file_path, file_hash).unwrap();
    let cache = IncrementalCacheInner::from_sql_incremental_cache(
      sql_cache,
      &[file_path.clone()],
    );

    assert!(cache.is_file_same(&file_path, "test"));
    assert!(!cache.is_file_same(&file_path, "other"));

    // just ensure this doesn't panic
    cache.update_file(&file_path, "other");
  }
}
