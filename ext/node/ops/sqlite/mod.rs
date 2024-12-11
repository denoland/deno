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
  #[error("Unknown column type")]
  UnknownColumnType,
  #[error("Failed to get SQL")]
  GetSqlFailed,
  #[error(transparent)]
  Other(deno_core::error::AnyError),
}
