// Copyright 2018-2025 the Deno authors. MIT license.

mod database;
mod session;
mod statement;

pub use database::DatabaseSync;
use rusqlite::ffi as libsqlite3_sys;
pub use session::Session;
pub use statement::StatementSync;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SqliteError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(generic)]
  #[error(transparent)]
  SqliteError(#[from] rusqlite::Error),
  #[class(generic)]
  #[error("{0}")]
  SqliteSysError(String),
  #[class(generic)]
  #[error("Database is already in use")]
  InUse,
  #[class(generic)]
  #[error("Failed to bind parameter. {0}")]
  FailedBind(&'static str),
  #[class(generic)]
  #[error("Unknown column type")]
  UnknownColumnType,
  #[class(generic)]
  #[error("Failed to get SQL")]
  GetSqlFailed,
  #[class(generic)]
  #[error("Database is already closed")]
  AlreadyClosed,
  #[class(generic)]
  #[error("Database is already open")]
  AlreadyOpen,
  #[class(generic)]
  #[error("Failed to prepare statement")]
  PrepareFailed,
  #[class(generic)]
  #[error("Failed to create session")]
  SessionCreateFailed,
  #[class(generic)]
  #[error("Failed to retrieve changeset")]
  SessionChangesetFailed,
  #[class(generic)]
  #[error("Session is already closed")]
  SessionClosed,
  #[class(generic)]
  #[error("Invalid constructor")]
  InvalidConstructor,
  #[class(generic)]
  #[error("Expanded SQL text would exceed configured limits")]
  InvalidExpandedSql,
  #[class(range)]
  #[error("The value of column {0} is too large to be represented as a JavaScript number: {1}")]
  NumberTooLarge(i32, i64),
  #[class(generic)]
  #[error("Failed to apply changeset")]
  ChangesetApplyFailed,
  #[class(type)]
  #[error("Invalid callback: {0}")]
  InvalidCallback(&'static str),
}

pub trait SqliteResultExt<T> {
  fn with_enhanced_errors(
    self,
    db: &rusqlite::Connection,
  ) -> Result<T, SqliteError>;
}

impl<T> SqliteResultExt<T> for Result<T, rusqlite::Error> {
  fn with_enhanced_errors(
    self,
    db: &rusqlite::Connection,
  ) -> Result<T, SqliteError> {
    match self {
      Ok(value) => Ok(value),
      Err(error) => {
        // SAFETY: lifetime of the connection is guaranteed by the rusqlite API.
        let handle = unsafe { db.handle() };
        // SAFETY: error conversion does not perform additional dereferencing beyond what is documented.
        Err(unsafe { SqliteError::from_rusqlite_with_details(error, handle) })
      }
    }
  }
}

impl SqliteError {
  pub const ERROR_CODE_GENERIC: i32 = 1;

  pub const ERROR_STR_UNKNOWN: &str = "unknown error";

  pub fn create_enhanced_error<T>(
    extended_code: i32,
    message: &str,
    db_handle: Option<*mut libsqlite3_sys::sqlite3>,
  ) -> Result<T, Self> {
    let rusqlite_error = rusqlite::Error::SqliteFailure(
      rusqlite::ffi::Error {
        code: rusqlite::ErrorCode::Unknown,
        extended_code,
      },
      Some(message.to_string()),
    );

    let handle = db_handle.unwrap_or(std::ptr::null_mut());
    // SAFETY: error conversion does not perform additional dereferencing beyond what is documented.
    Err(unsafe {
      SqliteError::from_rusqlite_with_details(rusqlite_error, handle)
    })
  }

  /// Creates a `SqliteError` from a rusqlite error and a raw SQLite handle.
  ///
  /// # Safety
  ///
  /// Caller must ensure `handle` is non-null and points to a valid, initialized sqlite3 instance.
  pub unsafe fn from_rusqlite_with_details(
    error: rusqlite::Error,
    handle: *mut libsqlite3_sys::sqlite3,
  ) -> Self {
    let message = error.to_string();

    let err_code = match &error {
      rusqlite::Error::SqliteFailure(ffi_error, _) => ffi_error.code as i32,
      _ => {
        if !handle.is_null() {
          // SAFETY: We've verified that handle is not null in the previous condition.
          unsafe { libsqlite3_sys::sqlite3_errcode(handle) }
        } else {
          Self::ERROR_CODE_GENERIC
        }
      }
    };

    // SAFETY: We're using sqlite3_errstr which returns a static string.
    let err_str = unsafe {
      let ptr = libsqlite3_sys::sqlite3_errstr(err_code);
      if !ptr.is_null() {
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
      } else {
        Self::ERROR_STR_UNKNOWN.to_string()
      }
    };

    let encoded_message = format!(
      "{}\n  {{\n  code: 'ERR_SQLITE_ERROR',\n  errcode: {},\n  errstr: '{}'\n}}",
      message,
      err_code,
      err_str
    );

    let custom_error = rusqlite::Error::SqliteFailure(
      rusqlite::ffi::Error {
        code: rusqlite::ErrorCode::Unknown,
        extended_code: err_code,
      },
      Some(encoded_message),
    );

    SqliteError::SqliteError(custom_error)
  }
}
