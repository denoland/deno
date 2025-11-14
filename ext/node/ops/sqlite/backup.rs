// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::ffi::c_int;
use std::time;

use deno_core::OpState;
use deno_core::op2;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use rusqlite::Connection;
use rusqlite::backup;
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
}

#[derive(Serialize, Deserialize)]
struct BackupResult {
  #[serde(rename = "totalPages")]
  total_pages: c_int,
}

#[op2(stack_trace)]
#[serde]
pub fn op_node_database_backup(
  state: &mut OpState,
  #[cppgc] source_db: &DatabaseSync,
  #[string] path: &str,
  #[serde] options: Option<BackupOptions>,
) -> Result<BackupResult, SqliteError> {
  let src_conn_ref = source_db.conn.borrow();
  let src_conn = src_conn_ref.as_ref().ok_or(SqliteError::SessionClosed)?;
  let path = std::path::Path::new(path);
  let checked_path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(path),
    OpenAccessKind::Write,
    Some("node:sqlite.backup"),
  )?;
  let mut dst_conn = Connection::open(checked_path)?;
  let rate = options
    .and_then(|opts| opts.rate)
    .unwrap_or(DEFAULT_BACKUP_RATE);
  let backup = backup::Backup::new(src_conn, &mut dst_conn)?;
  backup.run_to_completion(rate, time::Duration::from_millis(250), None)?;
  Ok(BackupResult {
    total_pages: backup.progress().pagecount,
  })
}
