// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Weak;

use deno_core::FromV8;
use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8_static_strings;
use rusqlite::ffi;

use super::SqliteError;
use super::validators;

#[derive(Default)]
pub struct SessionOptions {
  pub table: Option<String>,
  pub db: Option<String>,
}

impl FromV8<'_> for SessionOptions {
  type Error = validators::Error;

  fn from_v8(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, validators::Error> {
    use validators::Error;

    if value.is_undefined() {
      return Ok(SessionOptions::default());
    }

    let obj = v8::Local::<v8::Object>::try_from(value).map_err(|_| {
      Error::InvalidArgType("The \"options\" argument must be an object.")
    })?;

    let mut options = SessionOptions::default();

    v8_static_strings! {
      TABLE_STRING = "table",
      DB_STRING = "db",
    }

    let table_string = TABLE_STRING.v8_string(scope).unwrap();
    if let Some(table_value) = obj.get(scope, table_string.into())
      && !table_value.is_undefined()
    {
      if !table_value.is_string() {
        return Err(Error::InvalidArgType(
          "The \"options.table\" argument must be a string.",
        ));
      }
      let table =
        v8::Local::<v8::String>::try_from(table_value).map_err(|_| {
          Error::InvalidArgType(
            "The \"options.table\" argument must be a string.",
          )
        })?;
      options.table = Some(table.to_rust_string_lossy(scope).to_string());
    }

    let db_string = DB_STRING.v8_string(scope).unwrap();
    if let Some(db_value) = obj.get(scope, db_string.into())
      && !db_value.is_undefined()
    {
      if !db_value.is_string() {
        return Err(Error::InvalidArgType(
          "The \"options.db\" argument must be a string.",
        ));
      }
      let db = v8::Local::<v8::String>::try_from(db_value).map_err(|_| {
        Error::InvalidArgType("The \"options.db\" argument must be a string.")
      })?;
      options.db = Some(db.to_rust_string_lossy(scope).to_string());
    }

    Ok(options)
  }
}

pub struct Session {
  pub(crate) inner: *mut ffi::sqlite3_session,
  pub(crate) freed: Cell<bool>,

  // Hold a weak reference to the database.
  pub(crate) db: Weak<RefCell<Option<rusqlite::Connection>>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for Session {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Session"
  }
}

impl Drop for Session {
  fn drop(&mut self) {
    let _ = self.delete();
  }
}

impl Session {
  fn delete(&self) -> Result<(), SqliteError> {
    if self.freed.get() {
      return Err(SqliteError::SessionClosed);
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
  #[constructor]
  #[cppgc]
  fn create(_: bool) -> Session {
    unreachable!()
  }

  // Closes the session.
  #[fast]
  #[undefined]
  fn close(&self) -> Result<(), SqliteError> {
    let db_rc = self
      .db
      .upgrade()
      .ok_or_else(|| SqliteError::AlreadyClosed)?;
    if db_rc.borrow().is_none() {
      return Err(SqliteError::AlreadyClosed);
    }

    self.delete()
  }

  // Retrieves a changeset containing all changes since the changeset
  // was created. Can be called multiple times.
  //
  // This method is a wrapper around `sqlite3session_changeset()`.
  #[buffer]
  fn changeset(&self) -> Result<Box<[u8]>, SqliteError> {
    let db_rc = self
      .db
      .upgrade()
      .ok_or_else(|| SqliteError::AlreadyClosed)?;
    if db_rc.borrow().is_none() {
      return Err(SqliteError::AlreadyClosed);
    }
    if self.freed.get() {
      return Err(SqliteError::SessionClosed);
    }

    session_buffer_op(self.inner, ffi::sqlite3session_changeset)
  }

  // Similar to the method above, but generates a more compact patchset.
  //
  // This method is a wrapper around `sqlite3session_patchset()`.
  #[buffer]
  fn patchset(&self) -> Result<Box<[u8]>, SqliteError> {
    let db_rc = self
      .db
      .upgrade()
      .ok_or_else(|| SqliteError::AlreadyClosed)?;
    if db_rc.borrow().is_none() {
      return Err(SqliteError::AlreadyClosed);
    }
    if self.freed.get() {
      return Err(SqliteError::SessionClosed);
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
    return Err(SqliteError::SessionChangesetFailed);
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
