// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod database;
mod statement;

pub use database::DatabaseSync;
pub use statement::StatementSync;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SqliteError {
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
  #[error("Invalid constructor")]
  InvalidConstructor,
}
