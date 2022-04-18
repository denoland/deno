use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use rusqlite::params;
use rusqlite::Connection;
use tokio::task::JoinHandle;

use crate::config_file::FmtOptionsConfig;

enum ReceiverMessage {
  Update(PathBuf, u64),
  Exit,
}

/// Cache used to not bother formatting a file again when we
/// know it is already formatted.
pub struct IncrementalCache {
  previous_hashes: HashMap<PathBuf, u64>,
  sender: tokio::sync::mpsc::UnboundedSender<ReceiverMessage>,
  handle: Mutex<Option<JoinHandle<()>>>,
}

impl IncrementalCache {
  pub fn new(
    db_file_path: &Path,
    fmt_config: &FmtOptionsConfig,
    initial_file_paths: &[PathBuf],
  ) -> Result<Self, AnyError> {
    let state_hash =
      fast_insecure_hash(serde_json::to_string(fmt_config).unwrap().as_bytes());
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

    // sqlite isn't `Sync`, so we do all the updating on a dedicated thread
    let handle = tokio::task::spawn(async move {
      while let Some(message) = receiver.recv().await {
        match message {
          ReceiverMessage::Update(path, hash) => {
            let _ = cache.set_source_hash(&path, hash);
          }
          ReceiverMessage::Exit => break,
        }
      }
      let _ = cache.cleanup();
    });

    IncrementalCache {
      previous_hashes,
      sender,
      handle: Mutex::new(Some(handle)),
    }
  }

  pub fn is_file_same(&self, file_path: &Path, file_text: &str) -> bool {
    match self.previous_hashes.get(file_path) {
      Some(hash) => *hash == fast_insecure_hash(file_text.as_bytes()),
      None => false,
    }
  }

  pub fn update_file(&self, path: &Path, formatted_text: &str) {
    let hash = fast_insecure_hash(formatted_text.as_bytes());
    if let Some(previous_hash) = self.previous_hashes.get(path) {
      if *previous_hash == hash {
        return; // do not bother updating the db file because nothing has changed
      }
    }
    let _ = self
      .sender
      .send(ReceiverMessage::Update(path.to_path_buf(), hash));
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
  /// The CLI version, which is used to clean up the cache of entries that
  /// don't match the current CLI version.
  cli_version: String,
  /// A hash of the state used to produce the formatting other than the CLI version.
  /// This state is a hash of the configuration and ensures we don't not format
  /// a file when the configuration changes.
  state_hash: u64,
}

impl SqlIncrementalCache {
  pub fn new(db_file_path: &Path, state_hash: u64) -> Result<Self, AnyError> {
    let conn = Connection::open(db_file_path)?;
    Self::from_connection(conn, state_hash)
  }

  fn from_connection(
    conn: Connection,
    state_hash: u64,
  ) -> Result<Self, AnyError> {
    run_pragma(&conn)?;
    create_tables(&conn)?;

    Ok(Self {
      conn,
      cli_version: crate::version::deno(),
      state_hash,
    })
  }

  pub fn get_source_hash(&self, path: &Path) -> Option<u64> {
    let mut stmt = self.conn.prepare_cached("SELECT source_hash FROM incrementalcache WHERE file_path=?1 AND cli_version=?2 AND state_hash=?3 LIMIT 1").ok()?;
    let mut rows = stmt
      .query(params![
        path.to_string_lossy(),
        self.cli_version,
        self.state_hash
      ])
      .ok()?;
    let row = rows.next().ok().flatten()?;
    let hash: String = row.get(0).ok()?;
    hash.parse::<u64>().ok()
  }

  pub fn set_source_hash(
    &self,
    path: &Path,
    source_hash: u64,
  ) -> Result<(), AnyError> {
    let mut stmt = self.conn.prepare_cached("INSERT OR REPLACE INTO incrementalcache (file_path, cli_version, state_hash, source_hash) VALUES (?1, ?2, ?3, ?4)")?;
    stmt.execute(params![
      path.to_string_lossy(),
      &self.cli_version,
      &self.state_hash.to_string(),
      &source_hash.to_string(),
    ])?;
    Ok(())
  }

  /// Only keep around items in the cache for the current CLI version
  pub fn cleanup(&self) -> Result<(), AnyError> {
    self.conn.execute(
      "DELETE FROM incrementalcache WHERE cli_version!=?1",
      params![self.cli_version],
    )?;
    Ok(())
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
  // INT doesn't store up to u64, so use TEXT
  conn.execute(
    "CREATE TABLE IF NOT EXISTS incrementalcache (
        file_path TEXT PRIMARY KEY,
        cli_version TEXT NOT NULL,
        state_hash TEXT NOT NULL,
        source_hash TEXT NOT NULL
      )",
    [],
  )?;
  Ok(())
}

/// Very fast non-cryptographically secure hash.
fn fast_insecure_hash(bytes: &[u8]) -> u64 {
  use std::hash::Hasher;
  use twox_hash::XxHash64;

  let mut hasher = XxHash64::default();
  hasher.write(bytes);
  hasher.finish()
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  use super::*;

  #[test]
  pub fn sql_cache_general_use() {
    let conn = Connection::open_in_memory().unwrap();
    let mut cache = SqlIncrementalCache::from_connection(conn, 1).unwrap();
    let path = PathBuf::from("/mod.ts");

    assert_eq!(cache.get_source_hash(&path), None);
    cache.set_source_hash(&path, 2).unwrap();
    assert_eq!(cache.get_source_hash(&path), Some(2));

    // try changing the cli version (should not return)
    let old_version = cache.cli_version.to_string();
    cache.cli_version = "1.0.0".to_string();
    assert_eq!(cache.get_source_hash(&path), None);
    cache.cli_version = old_version.clone();

    // try changing the state hash
    cache.state_hash = 2;
    assert_eq!(cache.get_source_hash(&path), None);
    cache.state_hash = 1;

    // should return now that everything is back
    assert_eq!(cache.get_source_hash(&path), Some(2));

    // cleanup should not remove because the CLI version is the same
    cache.cleanup().unwrap();
    assert_eq!(cache.get_source_hash(&path), Some(2));

    // now change the version and cleanup
    cache.cli_version = "1.0.0".to_string();
    cache.cleanup().unwrap();
    cache.cli_version = old_version;
    assert_eq!(cache.get_source_hash(&path), None);

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
    let sql_cache = SqlIncrementalCache::from_connection(conn, 1).unwrap();
    let file_path = PathBuf::from("/mod.ts");
    let file_text = "test";
    let file_hash = fast_insecure_hash(file_text.as_bytes());
    sql_cache.set_source_hash(&file_path, file_hash).unwrap();
    let cache = IncrementalCache::from_sql_incremental_cache(
      sql_cache,
      &[file_path.clone()],
    );

    assert!(cache.is_file_same(&file_path, "test"));
    assert!(!cache.is_file_same(&file_path, "other"));

    // just ensure this doesn't panic
    cache.update_file(&file_path, "other");
  }
}
