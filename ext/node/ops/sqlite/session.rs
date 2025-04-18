// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use deno_core::op2;
use deno_core::GarbageCollected;
use rusqlite::ffi;
use serde::Deserialize;

use super::SqliteError;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOptions {
  pub table: Option<String>,
  pub db: Option<String>,
}

pub struct Session {
  pub(crate) inner: *mut ffi::sqlite3_session,
  pub(crate) freed: Cell<bool>,

  // Hold a strong reference to the database.
  pub(crate) db: Rc<RefCell<Option<rusqlite::Connection>>>,
}

impl GarbageCollected for Session {}

impl Drop for Session {
  fn drop(&mut self) {
    let _ = self.delete();
  }
}

impl Session {
  fn delete(&self) -> Result<(), SqliteError> {
    if self.freed.get() {
      return SqliteError::create_enhanced_error(
        ffi::SQLITE_MISUSE,
        &SqliteError::SessionClosed.to_string(),
        None,
      );
    }

    self.freed.set(true);
    // Safety: `self.inner` is a valid session. double free is
    // prevented by `freed` flag.
    unsafe {
      ffi::sqlite3session_delete(self.inner);
    }

    Ok(())
  }
}

#[op2]
impl Session {
  // Closes the session.
  #[fast]
  fn close(&self) -> Result<(), SqliteError> {
    if self.db.borrow().is_none() {
      return SqliteError::create_enhanced_error(
        ffi::SQLITE_MISUSE,
        &SqliteError::AlreadyClosed.to_string(),
        None,
      );
    }

    self.delete()
  }

  // Retrieves a changeset containing all changes since the changeset
  // was created. Can be called multiple times.
  //
  // This method is a wrapper around `sqlite3session_changeset()`.
  #[buffer]
  fn changeset(&self) -> Result<Box<[u8]>, SqliteError> {
    if self.db.borrow().is_none() {
      return SqliteError::create_enhanced_error(
        ffi::SQLITE_MISUSE,
        &SqliteError::AlreadyClosed.to_string(),
        None,
      );
    }
    if self.freed.get() {
      return SqliteError::create_enhanced_error(
        ffi::SQLITE_MISUSE,
        &SqliteError::SessionClosed.to_string(),
        None,
      );
    }

    session_buffer_op(self.inner, ffi::sqlite3session_changeset)
  }

  // Similar to the method above, but generates a more compact patchset.
  //
  // This method is a wrapper around `sqlite3session_patchset()`.
  #[buffer]
  fn patchset(&self) -> Result<Box<[u8]>, SqliteError> {
    if self.db.borrow().is_none() {
      return SqliteError::create_enhanced_error(
        ffi::SQLITE_MISUSE,
        &SqliteError::AlreadyClosed.to_string(),
        None,
      );
    }
    if self.freed.get() {
      return SqliteError::create_enhanced_error(
        ffi::SQLITE_MISUSE,
        &SqliteError::SessionClosed.to_string(),
        None,
      );
    }

    session_buffer_op(self.inner, ffi::sqlite3session_patchset)
  }
}

fn session_buffer_op(
  s: *mut ffi::sqlite3_session,
  f: unsafe extern "C" fn(
    *mut ffi::sqlite3_session,
    *mut i32,
    *mut *mut c_void,
  ) -> i32,
) -> Result<Box<[u8]>, SqliteError> {
  let mut n_buffer = 0;
  let mut p_buffer = std::ptr::null_mut();

  // Safety: `s` is a valid session and the buffer is allocated
  // by sqlite3 and will be freed later.
  let r = unsafe { f(s, &mut n_buffer, &mut p_buffer) };
  if r != ffi::SQLITE_OK {
    return SqliteError::create_enhanced_error(
      r,
      &SqliteError::SessionChangesetFailed.to_string(),
      None,
    );
  }

  if n_buffer == 0 {
    return Ok(Default::default());
  }

  // Safety: n_buffer is the size of the buffer.
  let buffer = unsafe {
    std::slice::from_raw_parts(p_buffer as *const u8, n_buffer as usize)
  }
  .to_vec()
  .into_boxed_slice();

  // Safety: free sqlite allocated buffer, we copied it into the JS buffer.
  unsafe {
    ffi::sqlite3_free(p_buffer);
  }

  Ok(buffer)
}
