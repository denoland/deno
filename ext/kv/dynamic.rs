// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use deno_core::OpState;
use deno_error::JsErrorBox;
use denokv_proto::CommitResult;
use denokv_proto::ReadRangeOutput;
use denokv_proto::WatchStream;

use crate::remote::RemoteDbHandlerPermissions;
use crate::sqlite::SqliteDbHandler;
use crate::sqlite::SqliteDbHandlerPermissions;
use crate::AtomicWrite;
use crate::Database;
use crate::DatabaseHandler;
use crate::QueueMessageHandle;
use crate::ReadRange;
use crate::SnapshotReadOptions;

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
    versionstamp_rng_seed: Option<u64>,
    http_options: crate::remote::HttpOptions,
  ) -> Self {
    Self::new(vec![
      (
        &["https://", "http://"],
        Box::new(crate::remote::RemoteDbHandler::<P>::new(http_options)),
      ),
      (
        &[""],
        Box::new(SqliteDbHandler::<P>::new(
          default_storage_dir,
          versionstamp_rng_seed,
        )),
      ),
    ])
  }
}

#[async_trait(?Send)]
impl DatabaseHandler for MultiBackendDbHandler {
  type DB = RcDynamicDb;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, JsErrorBox> {
    for (prefixes, handler) in &self.backends {
      for &prefix in *prefixes {
        if prefix.is_empty() {
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
    Err(JsErrorBox::type_error(format!(
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
  ) -> Result<RcDynamicDb, JsErrorBox>;
}

#[async_trait(?Send)]
impl DatabaseHandler for Box<dyn DynamicDbHandler> {
  type DB = RcDynamicDb;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, JsErrorBox> {
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
  ) -> Result<RcDynamicDb, JsErrorBox> {
    Ok(RcDynamicDb(Rc::new(self.open(state, path).await?)))
  }
}

#[async_trait(?Send)]
pub trait DynamicDb {
  async fn dyn_snapshot_read(
    &self,
    requests: Vec<ReadRange>,
    options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, JsErrorBox>;

  async fn dyn_atomic_write(
    &self,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, JsErrorBox>;

  async fn dyn_dequeue_next_message(
    &self,
  ) -> Result<Option<Box<dyn QueueMessageHandle>>, JsErrorBox>;

  fn dyn_watch(&self, keys: Vec<Vec<u8>>) -> WatchStream;

  fn dyn_close(&self);
}

#[derive(Clone)]
pub struct RcDynamicDb(Rc<dyn DynamicDb>);

#[async_trait(?Send)]
impl Database for RcDynamicDb {
  type QMH = Box<dyn QueueMessageHandle>;

  async fn snapshot_read(
    &self,
    requests: Vec<ReadRange>,
    options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, JsErrorBox> {
    (*self.0).dyn_snapshot_read(requests, options).await
  }

  async fn atomic_write(
    &self,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, JsErrorBox> {
    (*self.0).dyn_atomic_write(write).await
  }

  async fn dequeue_next_message(
    &self,
  ) -> Result<Option<Box<dyn QueueMessageHandle>>, JsErrorBox> {
    (*self.0).dyn_dequeue_next_message().await
  }

  fn watch(&self, keys: Vec<Vec<u8>>) -> WatchStream {
    (*self.0).dyn_watch(keys)
  }

  fn close(&self) {
    (*self.0).dyn_close()
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
    requests: Vec<ReadRange>,
    options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, JsErrorBox> {
    Ok(self.snapshot_read(requests, options).await?)
  }

  async fn dyn_atomic_write(
    &self,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, JsErrorBox> {
    Ok(self.atomic_write(write).await?)
  }

  async fn dyn_dequeue_next_message(
    &self,
  ) -> Result<Option<Box<dyn QueueMessageHandle>>, JsErrorBox> {
    Ok(
      self
        .dequeue_next_message()
        .await?
        .map(|x| Box::new(x) as Box<dyn QueueMessageHandle>),
    )
  }

  fn dyn_watch(&self, keys: Vec<Vec<u8>>) -> WatchStream {
    self.watch(keys)
  }

  fn dyn_close(&self) {
    self.close()
  }
}
