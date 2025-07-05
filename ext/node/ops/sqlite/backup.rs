// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::c_int;
use std::time;

use deno_core::op2;
use rusqlite::backup;
use rusqlite::Connection;
use serde::Deserialize;
use serde::Serialize;

use super::DatabaseSync;
use super::SqliteError;

const DEFAULT_BACKUP_RATE: c_int = 5;

#[derive(Serialize, Deserialize)]
struct BackupOptions {
  source: Option<String>,
  target: Option<String>,
  rate: Option<c_int>,
  // progress: fn(backup::Progress),
}

#[op2]
#[serde]
pub fn op_node_database_backup(
  #[cppgc] source_db: &DatabaseSync,
  #[string] path: String,
  #[serde] options: Option<BackupOptions>,
) -> std::result::Result<(), SqliteError> {
  let src_conn_ref = source_db.conn.borrow();
  let src_conn = src_conn_ref.as_ref().ok_or(SqliteError::SessionClosed)?;
  let path = std::path::Path::new(&path);
  let mut dst_conn = Connection::open(path)?;
  let rate = options
    .and_then(|opts| opts.rate)
    .unwrap_or(DEFAULT_BACKUP_RATE);
  let backup = backup::Backup::new(src_conn, &mut dst_conn)?;
  Ok(backup.run_to_completion(rate, time::Duration::from_millis(250), None)?)
}
