mod database;
mod statement;

pub use database::DatabaseSync;
pub use statement::StatementSync;

#[derive(Debug, thiserror::Error)]
pub enum SqliteError {
  #[error(transparent)]
  SqliteError(#[from] rusqlite::Error),
  #[error("Database is already in use")]
  InUse,
  #[error("Failed to step statement")]
  FailedStep,
  #[error("Failed to bind parameter. {0}")]
  FailedBind(&'static str),
  #[error("Unknown column type")]
  UnknownColumnType,
  #[error("Failed to get SQL")]
  GetSqlFailed,
  #[error("Database is already closed")]
  AlreadyClosed,
  #[error("Database is already open")]
  AlreadyOpen,
  #[error("Failed to prepare statement")]
  PrepareFailed,
  #[error("Invalid constructor")]
  InvalidConstructor,
  #[error(transparent)]
  Other(deno_core::error::AnyError),
}
