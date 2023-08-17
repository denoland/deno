// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use crate::remote::RemoteDbHandlerPermissions;
use crate::sqlite::SqliteDbHandler;
use crate::sqlite::SqliteDbHandlerPermissions;
use crate::AtomicWrite;
use crate::CommitResult;
use crate::Database;
use crate::DatabaseHandler;
use crate::QueueMessageHandle;
use crate::ReadRange;
use crate::ReadRangeOutput;
use crate::SnapshotReadOptions;
use async_trait::async_trait;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::OpState;

pub struct MultiBackendDbHandler {
  backends: Vec<(&'static [&'static str], Box<dyn DynamicDbHandler>)>,
}

impl MultiBackendDbHandler {
  pub fn new(
    backends: Vec<(&'static [&'static str], Box<dyn DynamicDbHandler>)>,
  ) -> Self {
    Self { backends }
  }

  pub fn remote_or_sqlite<
    P: SqliteDbHandlerPermissions + RemoteDbHandlerPermissions + 'static,
  >(
    default_storage_dir: Option<std::path::PathBuf>,
  ) -> Self {
    Self::new(vec![
      (
        &["https://", "http://"],
        Box::new(crate::remote::RemoteDbHandler::<P>::new()),
      ),
      (
        &[""],
        Box::new(SqliteDbHandler::<P>::new(default_storage_dir)),
      ),
    ])
  }
}

#[async_trait(?Send)]
impl DatabaseHandler for MultiBackendDbHandler {
  type DB = Box<dyn DynamicDb>;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, AnyError> {
    for (prefixes, handler) in &self.backends {
      for &prefix in *prefixes {
        if prefix == "" {
          return handler.dyn_open(state.clone(), path.clone()).await;
        }
        let Some(path) = &path else {
          continue;
        };
        if path.starts_with(prefix) {
          return handler.dyn_open(state.clone(), Some(path.clone())).await;
        }
      }
    }
    Err(type_error(format!(
      "No backend supports the given path: {:?}",
      path
    )))
  }
}

#[async_trait(?Send)]
pub trait DynamicDbHandler {
  async fn dyn_open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Box<dyn DynamicDb>, AnyError>;
}

#[async_trait(?Send)]
impl DatabaseHandler for Box<dyn DynamicDbHandler> {
  type DB = Box<dyn DynamicDb>;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, AnyError> {
    (**self).dyn_open(state, path).await
  }
}

#[async_trait(?Send)]
impl<T, DB> DynamicDbHandler for T
where
  T: DatabaseHandler<DB = DB>,
  DB: Database + 'static,
{
  async fn dyn_open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Box<dyn DynamicDb>, AnyError> {
    Ok(Box::new(self.open(state, path).await?))
  }
}

#[async_trait(?Send)]
pub trait DynamicDb {
  async fn dyn_snapshot_read(
    &self,
    state: Rc<RefCell<OpState>>,
    requests: Vec<ReadRange>,
    options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, AnyError>;

  async fn dyn_atomic_write(
    &self,
    state: Rc<RefCell<OpState>>,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, AnyError>;

  async fn dyn_dequeue_next_message(
    &self,
    state: Rc<RefCell<OpState>>,
  ) -> Result<Box<dyn QueueMessageHandle>, AnyError>;

  fn dyn_close(&self);
}

#[async_trait(?Send)]
impl Database for Box<dyn DynamicDb> {
  type QMH = Box<dyn QueueMessageHandle>;

  async fn snapshot_read(
    &self,
    state: Rc<RefCell<OpState>>,
    requests: Vec<ReadRange>,
    options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, AnyError> {
    (**self).dyn_snapshot_read(state, requests, options).await
  }

  async fn atomic_write(
    &self,
    state: Rc<RefCell<OpState>>,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, AnyError> {
    (**self).dyn_atomic_write(state, write).await
  }

  async fn dequeue_next_message(
    &self,
    state: Rc<RefCell<OpState>>,
  ) -> Result<Box<dyn QueueMessageHandle>, AnyError> {
    (**self).dyn_dequeue_next_message(state).await
  }

  fn close(&self) {
    (**self).dyn_close()
  }
}

#[async_trait(?Send)]
impl<T, QMH> DynamicDb for T
where
  T: Database<QMH = QMH>,
  QMH: QueueMessageHandle + 'static,
{
  async fn dyn_snapshot_read(
    &self,
    state: Rc<RefCell<OpState>>,
    requests: Vec<ReadRange>,
    options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, AnyError> {
    Ok(self.snapshot_read(state, requests, options).await?)
  }

  async fn dyn_atomic_write(
    &self,
    state: Rc<RefCell<OpState>>,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, AnyError> {
    Ok(self.atomic_write(state, write).await?)
  }

  async fn dyn_dequeue_next_message(
    &self,
    state: Rc<RefCell<OpState>>,
  ) -> Result<Box<dyn QueueMessageHandle>, AnyError> {
    Ok(Box::new(self.dequeue_next_message(state).await?))
  }

  fn dyn_close(&self) {
    self.close()
  }
}

#[async_trait(?Send)]
impl QueueMessageHandle for Box<dyn QueueMessageHandle> {
  async fn take_payload(&mut self) -> Result<Vec<u8>, AnyError> {
    (**self).take_payload().await
  }
  async fn finish(&self, success: bool) -> Result<(), AnyError> {
    (**self).finish(success).await
  }
}
