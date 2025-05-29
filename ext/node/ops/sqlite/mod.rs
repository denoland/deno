// Copyright 2018-2025 the Deno authors. MIT license.

mod database;
mod session;
mod statement;
mod validators;

pub use database::DatabaseSync;
pub use session::Session;
pub use statement::StatementSync;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[property("code" = self.code())]
pub enum SqliteError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(generic)]
  #[error(transparent)]
  SqliteError(#[from] rusqlite::Error),
  #[class(generic)]
  #[error("{message}")]
  SqliteSysError {
    message: String,
    #[property]
    errcode: i32,
  },
  #[class(generic)]
  #[error("Database is already in use")]
  InUse,
  #[class(generic)]
  #[error("Failed to load SQLite extension: {0}")]
  LoadExensionFailed(String),
  #[class(generic)]
  #[error("Failed to bind parameter. {0}")]
  FailedBind(&'static str),
  #[class(type)]
  #[error("Provided value cannot be bound to SQLite parameter {0}.")]
  InvalidBindType(i32),
  #[class(type)]
  #[error("{0}")]
  InvalidBindValue(&'static str),
  #[class(generic)]
  #[error("Cannot create bare named parameter '{0}' because of conflicting names '{1}' and '{2}'.")]
  DuplicateNamedParameter(String, String, String),
  #[class(generic)]
  #[error("Unknown named parameter '{0}'")]
  UnknownNamedParameter(String),
  #[class(generic)]
  #[error("unknown column type")]
  UnknownColumnType,
  #[class(generic)]
  #[error("failed to get SQL")]
  GetSqlFailed,
  #[class(generic)]
  #[error("database is not open")]
  AlreadyClosed,
  #[class(generic)]
  #[error("database is already open")]
  AlreadyOpen,
  #[class(generic)]
  #[error("failed to prepare statement")]
  PrepareFailed,
  #[class(generic)]
  #[error("failed to create session")]
  SessionCreateFailed,
  #[class(generic)]
  #[error("failed to retrieve changeset")]
  SessionChangesetFailed,
  #[class(generic)]
  #[error("session is already closed")]
  SessionClosed,
  #[class(generic)]
  #[error("Illegal constructor")]
  InvalidConstructor,
  #[class(generic)]
  #[error("Expanded SQL text would exceed configured limits")]
  InvalidExpandedSql,
  #[class(range)]
  #[error("The value of column {0} is too large to be represented as a JavaScript number: {1}")]
  NumberTooLarge(i32, i64),
  #[class(range)]
  #[class(generic)]
  #[error("Failed to apply changeset")]
  ChangesetApplyFailed,
  #[class(type)]
  #[error("Invalid callback: {0}")]
  InvalidCallback(&'static str),
  #[class(type)]
  #[error("FromUtf8Error: {0}")]
  FromUtf8Error(#[from] std::ffi::NulError),
  #[class(inherit)]
  #[error(transparent)]
  #[property("code" = self.code())]
  Validation(#[from] validators::Error),
}

#[derive(Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
enum ErrorCode {
  ERR_SQLITE_ERROR,
  ERR_ILLEGAL_CONSTRUCTOR,
  ERR_INVALID_STATE,
  ERR_OUT_OF_RANGE,
  ERR_LOAD_SQLITE_EXTENSION,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
}

impl std::fmt::Display for ErrorCode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

impl ErrorCode {
  pub fn as_str(&self) -> &str {
    match self {
      Self::ERR_SQLITE_ERROR => "ERR_SQLITE_ERROR",
      Self::ERR_ILLEGAL_CONSTRUCTOR => "ERR_ILLEGAL_CONSTRUCTOR",
      Self::ERR_INVALID_ARG_TYPE => "ERR_INVALID_ARG_TYPE",
      Self::ERR_INVALID_ARG_VALUE => "ERR_INVALID_ARG_VALUE",
      Self::ERR_INVALID_STATE => "ERR_INVALID_STATE",
      Self::ERR_OUT_OF_RANGE => "ERR_OUT_OF_RANGE",
      Self::ERR_LOAD_SQLITE_EXTENSION => "ERR_LOAD_SQLITE_EXTENSION",
    }
  }
}

impl From<ErrorCode> for deno_error::PropertyValue {
  fn from(code: ErrorCode) -> Self {
    deno_error::PropertyValue::from(code.as_str().to_string())
  }
}

impl SqliteError {
  fn code(&self) -> ErrorCode {
    match self {
      Self::InvalidConstructor => ErrorCode::ERR_ILLEGAL_CONSTRUCTOR,
      Self::InvalidBindType(_) => ErrorCode::ERR_INVALID_ARG_TYPE,
      Self::InvalidBindValue(_) => ErrorCode::ERR_INVALID_ARG_VALUE,
      Self::FailedBind(_)
      | Self::UnknownNamedParameter(_)
      | Self::DuplicateNamedParameter(..)
      | Self::AlreadyClosed
      | Self::InUse
      | Self::AlreadyOpen => ErrorCode::ERR_INVALID_STATE,
      Self::NumberTooLarge(_, _) => ErrorCode::ERR_OUT_OF_RANGE,
      Self::LoadExensionFailed(_) => ErrorCode::ERR_LOAD_SQLITE_EXTENSION,
      _ => ErrorCode::ERR_SQLITE_ERROR,
    }
  }
}
