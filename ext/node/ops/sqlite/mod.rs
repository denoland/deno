// Copyright 2018-2025 the Deno authors. MIT license.

mod backup;
mod database;
mod session;
mod statement;
mod validators;

pub use backup::op_node_database_backup;
pub use database::DatabaseSync;
pub use session::Session;
pub use statement::StatementSync;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SqliteError {
  #[class(inherit)]
  #[error(transparent)]
  #[property("code" = self.code())]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(generic)]
  #[error(transparent)]
  #[property("code" = self.code())]
  SqliteError(#[from] rusqlite::Error),
  #[class(generic)]
  #[error("{message}")]
  #[property("code" = self.code())]
  #[property("errstr" = self.errstr())]
  SqliteSysError {
    message: String,
    errstr: String,
    #[property]
    errcode: f64,
  },
  #[class(generic)]
  #[error("Database is already in use")]
  #[property("code" = self.code())]
  InUse,
  #[class(generic)]
  #[error("Failed to load SQLite extension: {0}")]
  #[property("code" = self.code())]
  LoadExensionFailed(String),
  #[class(generic)]
  #[error("Failed to bind parameter. {0}")]
  #[property("code" = self.code())]
  FailedBind(&'static str),
  #[class(type)]
  #[error("Provided value cannot be bound to SQLite parameter {0}.")]
  #[property("code" = self.code())]
  InvalidBindType(i32),
  #[class(type)]
  #[error("{0}")]
  #[property("code" = self.code())]
  InvalidBindValue(&'static str),
  #[class(generic)]
  #[error(
    "Cannot create bare named parameter '{0}' because of conflicting names '{1}' and '{2}'."
  )]
  #[property("code" = self.code())]
  DuplicateNamedParameter(String, String, String),
  #[class(generic)]
  #[error("Unknown named parameter '{0}'")]
  #[property("code" = self.code())]
  UnknownNamedParameter(String),
  #[class(generic)]
  #[error("unknown column type")]
  #[property("code" = self.code())]
  UnknownColumnType,
  #[class(generic)]
  #[error("failed to get SQL")]
  #[property("code" = self.code())]
  GetSqlFailed,
  #[class(generic)]
  #[error("database is not open")]
  #[property("code" = self.code())]
  AlreadyClosed,
  #[class(generic)]
  #[error("database is already open")]
  #[property("code" = self.code())]
  AlreadyOpen,
  #[class(generic)]
  #[error("failed to create session")]
  #[property("code" = self.code())]
  SessionCreateFailed,
  #[class(generic)]
  #[error("failed to retrieve changeset")]
  #[property("code" = self.code())]
  SessionChangesetFailed,
  #[class(generic)]
  #[error("session is not open")]
  #[property("code" = self.code())]
  SessionClosed,
  #[class(type)]
  #[error("Illegal constructor")]
  #[property("code" = self.code())]
  InvalidConstructor,
  #[class(generic)]
  #[error("Expanded SQL text would exceed configured limits")]
  #[property("code" = self.code())]
  InvalidExpandedSql,
  #[class(range)]
  #[error(
    "The value of column {0} is too large to be represented as a JavaScript number: {1}"
  )]
  #[property("code" = self.code())]
  NumberTooLarge(i32, i64),
  #[class(type)]
  #[error("Invalid callback: {0}")]
  #[property("code" = self.code())]
  InvalidCallback(&'static str),
  #[class(type)]
  #[error("FromUtf8Error: {0}")]
  #[property("code" = self.code())]
  FromNullError(#[from] std::ffi::NulError),
  #[class(type)]
  #[error("FromUtf8Error: {0}")]
  #[property("code" = self.code())]
  FromUtf8Error(#[from] std::str::Utf8Error),
  #[class(inherit)]
  #[error(transparent)]
  #[property("code" = self.code())]
  Validation(#[from] validators::Error),
  #[class(generic)]
  #[error("statement has been finalized")]
  #[property("code" = self.code())]
  StatementFinalized,
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
  fn errstr(&self) -> String {
    match self {
      Self::SqliteSysError { errstr, .. } => errstr.clone(),
      _ => unreachable!(),
    }
  }

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
      | Self::AlreadyOpen
      | Self::StatementFinalized => ErrorCode::ERR_INVALID_STATE,
      Self::NumberTooLarge(_, _) => ErrorCode::ERR_OUT_OF_RANGE,
      Self::LoadExensionFailed(_) => ErrorCode::ERR_LOAD_SQLITE_EXTENSION,
      _ => ErrorCode::ERR_SQLITE_ERROR,
    }
  }
}
