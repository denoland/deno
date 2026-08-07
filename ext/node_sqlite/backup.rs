// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::CStr;
use std::ffi::CString;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use rusqlite::Connection;
use rusqlite::ffi as libsqlite3_sys;

use super::DatabaseSync;
use super::SqliteError;
use crate::database::DEFAULT_OPEN_FLAGS;

// Native backup state owned by the resource.
struct BackupInner {
  backup: *mut libsqlite3_sys::sqlite3_backup,
  // Destination connection.
  dest: Connection,
  // Keep the source connection alive while the backup runs.
  _src_conn: Rc<RefCell<Option<Connection>>>,
  active_backups: Rc<Cell<usize>>,
  deferred_close_conn: Rc<RefCell<Option<Connection>>>,
}

impl BackupInner {
  // Skip finishing the handle while a blocking step may still be running.
  // Dropping the connections without finishing leaves the native handle alive
  // until the backup is done.
  fn finalize(self, finish_handle: bool) {
    if finish_handle {
      // SAFETY: `backup` is a valid handle and no step is in flight.
      unsafe {
        libsqlite3_sys::sqlite3_backup_finish(self.backup);
      }
    }
    drop(self.dest);
    let remaining = self.active_backups.get().saturating_sub(1);
    self.active_backups.set(remaining);
    if remaining == 0 {
      // The source DB connection is parked until the backup is gone.
      self.deferred_close_conn.borrow_mut().take();
    }
  }
}

pub struct BackupJob {
  inner: RefCell<Option<BackupInner>>,
  step_in_flight: Cell<bool>,
}

impl Resource for BackupJob {
  fn name(&self) -> Cow<'_, str> {
    "nodeSqliteBackup".into()
  }
}

impl Drop for BackupJob {
  fn drop(&mut self) {
    if let Some(inner) = self.inner.borrow_mut().take() {
      inner.finalize(!self.step_in_flight.get());
    }
  }
}

struct BackupHandle(*mut libsqlite3_sys::sqlite3_backup);
// SAFETY: only one step runs at a time, and both connections use serialized
// SQLite mode.
unsafe impl Send for BackupHandle {}

fn sqlite_sys_error(errcode: i32, message: String) -> SqliteError {
  // SAFETY: sqlite3_errstr always returns a valid static string.
  let errstr =
    unsafe { CStr::from_ptr(libsqlite3_sys::sqlite3_errstr(errcode)) }
      .to_string_lossy()
      .into_owned();
  SqliteError::SqliteSysError {
    message,
    errstr,
    errcode: errcode as f64,
  }
}

#[op2(fast, stack_trace)]
#[smi]
pub fn op_node_database_backup_init(
  state: &mut OpState,
  #[cppgc] source_db: &DatabaseSync,
  #[string] path: &str,
  #[string] source_name: &str,
  #[string] target_name: &str,
) -> Result<ResourceId, SqliteError> {
  let source_name = CString::new(source_name)?;
  let target_name = CString::new(target_name)?;

  let src_handle = {
    let conn = source_db.conn.borrow();
    let conn = conn.as_ref().ok_or(SqliteError::AlreadyClosed)?;
    // SAFETY: the raw handle stays valid for the lifetime of the backup:
    // the Rc clone stored in BackupInner keeps the connection alive even if
    // the wrapper is GC'd, and DatabaseSync::close defers the real close
    // while `active_backups` is non-zero.
    unsafe { conn.handle() }
  };

  let checked_path = {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_open(
      Cow::Borrowed(std::path::Path::new(path)),
      OpenAccessKind::Write,
      Some("node:sqlite.backup"),
    )?
  };

  let dest = Connection::open_with_flags(checked_path, DEFAULT_OPEN_FLAGS)
    .map_err(|e| match e {
      rusqlite::Error::SqliteFailure(err, Some(msg)) => {
        let message = if err.extended_code == libsqlite3_sys::SQLITE_CANTOPEN {
          "unable to open database file".to_string()
        } else {
          msg
        };
        SqliteError::SqliteSysError {
          message: message.clone(),
          errstr: message,
          errcode: err.extended_code as _,
        }
      }
      other_err => SqliteError::from(other_err),
    })?;

  // SAFETY: both connections are open and the names are valid C strings.
  let backup = unsafe {
    libsqlite3_sys::sqlite3_backup_init(
      dest.handle(),
      target_name.as_ptr(),
      src_handle,
      source_name.as_ptr(),
    )
  };
  if backup.is_null() {
    // Match Node and report the destination connection error.
    // SAFETY: `dest` is open; sqlite3_errmsg returns a valid string.
    let (errcode, message) = unsafe {
      let handle = dest.handle();
      (
        libsqlite3_sys::sqlite3_extended_errcode(handle),
        CStr::from_ptr(libsqlite3_sys::sqlite3_errmsg(handle))
          .to_string_lossy()
          .into_owned(),
      )
    };
    return Err(sqlite_sys_error(errcode, message));
  }

  source_db
    .active_backups
    .set(source_db.active_backups.get() + 1);

  Ok(state.resource_table.add(BackupJob {
    inner: RefCell::new(Some(BackupInner {
      backup,
      dest,
      _src_conn: Rc::clone(&source_db.conn),
      active_backups: Rc::clone(&source_db.active_backups),
      deferred_close_conn: Rc::clone(&source_db.deferred_close_conn),
    })),
    step_in_flight: Cell::new(false),
  }))
}

#[derive(deno_core::ToV8)]
pub struct BackupStepResult {
  done: bool,
  total_pages: i32,
  remaining_pages: i32,
}

// Run one backup step on the blocking pool and report the SQLite status.
async fn run_backup_step(
  job: &Rc<BackupJob>,
  pages: i32,
) -> Result<i32, SqliteError> {
  let handle = {
    let inner = job.inner.borrow();
    let inner = inner.as_ref().ok_or(SqliteError::AlreadyClosed)?;
    BackupHandle(inner.backup)
  };

  job.step_in_flight.set(true);
  let status = spawn_blocking(move || {
    let handle = handle;
    // SAFETY: the handle is valid — the BackupJob resource owns it and only
    // finalizes it when no step is in flight.
    unsafe { libsqlite3_sys::sqlite3_backup_step(handle.0, pages) }
  })
  .await;
  job.step_in_flight.set(false);
  Ok(status?)
}

// Turns an unexpected step status code into an error, mirroring Node which
// reports `sqlite3_errstr` for step failures (e.g. "attempt to write a
// readonly database").
fn backup_step_error(errcode: i32) -> SqliteError {
  // SAFETY: sqlite3_errstr always returns a valid static string.
  let message =
    unsafe { CStr::from_ptr(libsqlite3_sys::sqlite3_errstr(errcode)) }
      .to_string_lossy()
      .into_owned();
  sqlite_sys_error(errcode, message)
}

#[op2]
pub async fn op_node_database_backup_step(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[smi] pages: i32,
) -> Result<BackupStepResult, SqliteError> {
  let job = state.borrow().resource_table.get::<BackupJob>(rid)?;

  let status = run_backup_step(&job, pages).await?;

  let inner = job.inner.borrow();
  let inner = inner.as_ref().ok_or(SqliteError::AlreadyClosed)?;
  // SAFETY: no step is in flight; these only read the backup object.
  let (total_pages, remaining_pages) = unsafe {
    (
      libsqlite3_sys::sqlite3_backup_pagecount(inner.backup),
      libsqlite3_sys::sqlite3_backup_remaining(inner.backup),
    )
  };

  match status {
    libsqlite3_sys::SQLITE_DONE => Ok(BackupStepResult {
      done: true,
      total_pages,
      remaining_pages,
    }),
    // Retry on BUSY/LOCKED, like Node.
    libsqlite3_sys::SQLITE_OK
    | libsqlite3_sys::SQLITE_BUSY
    | libsqlite3_sys::SQLITE_LOCKED => Ok(BackupStepResult {
      done: false,
      total_pages,
      remaining_pages,
    }),
    errcode => Err(backup_step_error(errcode)),
  }
}

// Fast path for `backup()` without a `progress` callback: runs the whole copy
// loop server-side instead of returning to JS between steps. Each step still
// runs via `spawn_blocking` and yields at the `.await`, so `rate` batching and
// event-loop responsiveness match the stepped path; only the per-step JS
// round-trip is removed. Resolves with the final total page count.
#[op2]
pub async fn op_node_database_backup_run(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[smi] pages: i32,
) -> Result<i32, SqliteError> {
  let job = state.borrow().resource_table.get::<BackupJob>(rid)?;

  loop {
    let status = run_backup_step(&job, pages).await?;
    match status {
      libsqlite3_sys::SQLITE_DONE => {
        let inner = job.inner.borrow();
        let inner = inner.as_ref().ok_or(SqliteError::AlreadyClosed)?;
        // SAFETY: no step is in flight; this only reads the backup object.
        return Ok(unsafe {
          libsqlite3_sys::sqlite3_backup_pagecount(inner.backup)
        });
      }
      // Retry on BUSY/LOCKED, continue on OK, like the stepped path.
      libsqlite3_sys::SQLITE_OK
      | libsqlite3_sys::SQLITE_BUSY
      | libsqlite3_sys::SQLITE_LOCKED => continue,
      errcode => return Err(backup_step_error(errcode)),
    }
  }
}

#[op2(fast)]
pub fn op_node_database_backup_finish(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), SqliteError> {
  let job = state.resource_table.take::<BackupJob>(rid)?;
  drop(job);
  Ok(())
}
