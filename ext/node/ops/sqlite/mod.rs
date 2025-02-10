// Copyright 2018-2025 the Deno authors. MIT license.

mod database;
mod session;
mod statement;

pub use database::DatabaseSync;
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
  #[error("Database is already in use")]
  InUse,
  #[class(generic)]
  #[error("Failed to step statement")]
  FailedStep,
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
