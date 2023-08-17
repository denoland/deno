// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use async_trait::async_trait;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::task::spawn;
use deno_core::task::spawn_blocking;
use deno_core::AsyncRefCell;
use deno_core::OpState;
use rand::Rng;
use rusqlite::params;
use rusqlite::OpenFlags;
use rusqlite::OptionalExtension;
use rusqlite::Transaction;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::sync::OnceCell;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::AtomicWrite;
use crate::CommitResult;
use crate::Database;
use crate::DatabaseHandler;
use crate::KvEntry;
use crate::MutationKind;
use crate::QueueMessageHandle;
use crate::ReadRange;
use crate::ReadRangeOutput;
use crate::SnapshotReadOptions;
use crate::Value;

const STATEMENT_INC_AND_GET_DATA_VERSION: &str =
  "update data_version set version = version + 1 where k = 0 returning version";
const STATEMENT_KV_RANGE_SCAN: &str =
  "select k, v, v_encoding, version from kv where k >= ? and k < ? order by k asc limit ?";
const STATEMENT_KV_RANGE_SCAN_REVERSE: &str =
  "select k, v, v_encoding, version from kv where k >= ? and k < ? order by k desc limit ?";
const STATEMENT_KV_POINT_GET_VALUE_ONLY: &str =
  "select v, v_encoding from kv where k = ?";
const STATEMENT_KV_POINT_GET_VERSION_ONLY: &str =
  "select version from kv where k = ?";
const STATEMENT_KV_POINT_SET: &str =
  "insert into kv (k, v, v_encoding, version) values (:k, :v, :v_encoding, :version) on conflict(k) do update set v = :v, v_encoding = :v_encoding, version = :version";
const STATEMENT_KV_POINT_DELETE: &str = "delete from kv where k = ?";

const STATEMENT_QUEUE_ADD_READY: &str = "insert into queue (ts, id, data, backoff_schedule, keys_if_undelivered) values(?, ?, ?, ?, ?)";
const STATEMENT_QUEUE_GET_NEXT_READY: &str = "select ts, id, data, backoff_schedule, keys_if_undelivered from queue where ts <= ? order by ts limit 100";
const STATEMENT_QUEUE_GET_EARLIEST_READY: &str =
  "select ts from queue order by ts limit 1";
const STATEMENT_QUEUE_REMOVE_READY: &str = "delete from queue where id = ?";
const STATEMENT_QUEUE_ADD_RUNNING: &str = "insert into queue_running (deadline, id, data, backoff_schedule, keys_if_undelivered) values(?, ?, ?, ?, ?)";
const STATEMENT_QUEUE_REMOVE_RUNNING: &str =
  "delete from queue_running where id = ?";
const STATEMENT_QUEUE_GET_RUNNING_BY_ID: &str = "select deadline, id, data, backoff_schedule, keys_if_undelivered from queue_running where id = ?";
const STATEMENT_QUEUE_GET_RUNNING: &str =
  "select id from queue_running order by deadline limit 100";

const STATEMENT_CREATE_MIGRATION_TABLE: &str = "
create table if not exists migration_state(
  k integer not null primary key,
  version integer not null
)
";

const MIGRATIONS: [&str; 2] = [
  "
create table data_version (
  k integer primary key,
  version integer not null
);
insert into data_version (k, version) values (0, 0);
create table kv (
  k blob primary key,
  v blob not null,
  v_encoding integer not null,
  version integer not null
) without rowid;
",
  "
create table queue (
  ts integer not null,
  id text not null,
  data blob not null,
  backoff_schedule text not null,
  keys_if_undelivered blob not null,

  primary key (ts, id)
);
create table queue_running(
  deadline integer not null,
  id text not null,
  data blob not null,
  backoff_schedule text not null,
  keys_if_undelivered blob not null,

  primary key (deadline, id)
);
",
];

const DISPATCH_CONCURRENCY_LIMIT: usize = 100;
const DEFAULT_BACKOFF_SCHEDULE: [u32; 5] = [100, 1000, 5000, 30000, 60000];

pub struct SqliteDbHandler<P: SqliteDbHandlerPermissions + 'static> {
  pub default_storage_dir: Option<PathBuf>,
  _permissions: PhantomData<P>,
}

pub trait SqliteDbHandlerPermissions {
  fn check_read(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
  fn check_write(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
}

impl<P: SqliteDbHandlerPermissions> SqliteDbHandler<P> {
  pub fn new(default_storage_dir: Option<PathBuf>) -> Self {
    Self {
      default_storage_dir,
      _permissions: PhantomData,
    }
  }
}

#[async_trait(?Send)]
impl<P: SqliteDbHandlerPermissions> DatabaseHandler for SqliteDbHandler<P> {
  type DB = SqliteDb;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, AnyError> {
    // Validate path
    if let Some(path) = &path {
      if path != ":memory:" {
        if path.is_empty() {
          return Err(type_error("Filename cannot be empty"));
        }
        if path.starts_with(':') {
          return Err(type_error(
            "Filename cannot start with ':' unless prefixed with './'",
          ));
        }
        let path = Path::new(path);
        {
          let mut state = state.borrow_mut();
          let permissions = state.borrow_mut::<P>();
          permissions.check_read(path, "Deno.openKv")?;
          permissions.check_write(path, "Deno.openKv")?;
        }
      }
    }

    let conn = sqlite_retry_loop(|| {
      let path = path.clone();
      let default_storage_dir = self.default_storage_dir.clone();
      async move {
        spawn_blocking(move || {
          let conn = match (path.as_deref(), &default_storage_dir) {
            (Some(":memory:"), _) | (None, None) => {
              rusqlite::Connection::open_in_memory()?
            }
            (Some(path), _) => {
              let flags =
                OpenFlags::default().difference(OpenFlags::SQLITE_OPEN_URI);
              rusqlite::Connection::open_with_flags(path, flags)?
            }
            (None, Some(path)) => {
              std::fs::create_dir_all(path)?;
              let path = path.join("kv.sqlite3");
              rusqlite::Connection::open(path)?
            }
          };

          conn.pragma_update(None, "journal_mode", "wal")?;

          Ok::<_, AnyError>(conn)
        })
        .await
        .unwrap()
      }
    })
    .await?;
    let conn = Rc::new(AsyncRefCell::new(Cell::new(Some(conn))));
    SqliteDb::run_tx(conn.clone(), |tx| {
      tx.execute(STATEMENT_CREATE_MIGRATION_TABLE, [])?;

      let current_version: usize = tx
        .query_row(
          "select version from migration_state where k = 0",
          [],
          |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);

      for (i, migration) in MIGRATIONS.iter().enumerate() {
        let version = i + 1;
        if version > current_version {
          tx.execute_batch(migration)?;
          tx.execute(
            "replace into migration_state (k, version) values(?, ?)",
            [&0, &version],
          )?;
        }
      }

      tx.commit()?;

      Ok(())
    })
    .await?;

    Ok(SqliteDb {
      conn,
      queue: OnceCell::new(),
    })
  }
}

pub struct SqliteDb {
  conn: Rc<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>,
  queue: OnceCell<SqliteQueue>,
}

async fn sqlite_retry_loop<R, Fut: Future<Output = Result<R, AnyError>>>(
  mut f: impl FnMut() -> Fut,
) -> Result<R, AnyError> {
  loop {
    match f().await {
      Ok(x) => return Ok(x),
      Err(e) => {
        if let Some(x) = e.downcast_ref::<rusqlite::Error>() {
          if x.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy) {
            log::debug!("kv: Database is busy, retrying");
            tokio::time::sleep(Duration::from_millis(
              rand::thread_rng().gen_range(5..20),
            ))
            .await;
            continue;
          }
        }
        return Err(e);
      }
    }
  }
}

impl SqliteDb {
  async fn run_tx<F, R>(
    conn: Rc<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: (FnOnce(rusqlite::Transaction<'_>) -> Result<R, AnyError>)
      + Clone
      + Send
      + 'static,
    R: Send + 'static,
  {
    sqlite_retry_loop(|| Self::run_tx_inner(conn.clone(), f.clone())).await
  }

  async fn run_tx_inner<F, R>(
    conn: Rc<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: (FnOnce(rusqlite::Transaction<'_>) -> Result<R, AnyError>)
      + Send
      + 'static,
    R: Send + 'static,
  {
    // Transactions need exclusive access to the connection. Wait until
    // we can borrow_mut the connection.
    let cell = conn.borrow_mut().await;

    // Take the db out of the cell and run the transaction via spawn_blocking.
    let mut db = cell.take().unwrap();
    let (result, db) = spawn_blocking(move || {
      let result = {
        match db.transaction() {
          Ok(tx) => f(tx),
          Err(e) => Err(e.into()),
        }
      };
      (result, db)
    })
    .await
    .unwrap();

    // Put the db back into the cell.
    cell.set(Some(db));
    result
  }
}

pub struct DequeuedMessage {
  conn: Weak<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>,
  id: String,
  payload: Option<Vec<u8>>,
  waker_tx: mpsc::Sender<()>,
  _permit: OwnedSemaphorePermit,
}

#[async_trait(?Send)]
impl QueueMessageHandle for DequeuedMessage {
  async fn finish(&self, success: bool) -> Result<(), AnyError> {
    let Some(conn) = self.conn.upgrade() else {
      return Ok(());
    };
    let id = self.id.clone();
    let requeued = SqliteDb::run_tx(conn, move |tx| {
      let requeued = {
        if success {
          let changed = tx
            .prepare_cached(STATEMENT_QUEUE_REMOVE_RUNNING)?
            .execute([&id])?;
          assert!(changed <= 1);
          false
        } else {
          SqliteQueue::requeue_message(&id, &tx)?
        }
      };
      tx.commit()?;
      Ok(requeued)
    })
    .await?;
    if requeued {
      // If the message was requeued, wake up the dequeue loop.
      self.waker_tx.send(()).await?;
    }
    Ok(())
  }

  async fn take_payload(&mut self) -> Result<Vec<u8>, AnyError> {
    self
      .payload
      .take()
      .ok_or_else(|| type_error("Payload already consumed"))
  }
}

type DequeueReceiver = mpsc::Receiver<(Vec<u8>, String)>;

struct SqliteQueue {
  conn: Rc<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>,
  dequeue_rx: Rc<AsyncRefCell<DequeueReceiver>>,
  concurrency_limiter: Arc<Semaphore>,
  waker_tx: mpsc::Sender<()>,
  shutdown_tx: watch::Sender<()>,
}

impl SqliteQueue {
  fn new(conn: Rc<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>) -> Self {
    let conn_clone = conn.clone();
    let (shutdown_tx, shutdown_rx) = watch::channel::<()>(());
    let (waker_tx, waker_rx) = mpsc::channel::<()>(1);
    let (dequeue_tx, dequeue_rx) = mpsc::channel::<(Vec<u8>, String)>(64);

    spawn(async move {
      // Oneshot requeue of all inflight messages.
      Self::requeue_inflight_messages(conn.clone()).await.unwrap();

      // Continuous dequeue loop.
      Self::dequeue_loop(conn.clone(), dequeue_tx, shutdown_rx, waker_rx)
        .await
        .unwrap();
    });

    Self {
      conn: conn_clone,
      dequeue_rx: Rc::new(AsyncRefCell::new(dequeue_rx)),
      waker_tx,
      shutdown_tx,
      concurrency_limiter: Arc::new(Semaphore::new(DISPATCH_CONCURRENCY_LIMIT)),
    }
  }

  async fn dequeue(&self) -> Result<DequeuedMessage, AnyError> {
    // Wait for the next message to be available from dequeue_rx.
    let (payload, id) = {
      let mut queue_rx = self.dequeue_rx.borrow_mut().await;
      let Some(msg) = queue_rx.recv().await else {
        return Err(type_error("Database closed"));
      };
      msg
    };

    let permit = self.concurrency_limiter.clone().acquire_owned().await?;

    Ok(DequeuedMessage {
      conn: Rc::downgrade(&self.conn),
      id,
      payload: Some(payload),
      waker_tx: self.waker_tx.clone(),
      _permit: permit,
    })
  }

  async fn wake(&self) -> Result<(), AnyError> {
    self.waker_tx.send(()).await?;
    Ok(())
  }

  fn shutdown(&self) {
    self.shutdown_tx.send(()).unwrap();
  }

  async fn dequeue_loop(
    conn: Rc<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>,
    dequeue_tx: mpsc::Sender<(Vec<u8>, String)>,
    mut shutdown_rx: watch::Receiver<()>,
    mut waker_rx: mpsc::Receiver<()>,
  ) -> Result<(), AnyError> {
    loop {
      let messages = SqliteDb::run_tx(conn.clone(), move |tx| {
        let now = SystemTime::now()
          .duration_since(SystemTime::UNIX_EPOCH)
          .unwrap()
          .as_millis() as u64;

        let messages = tx
          .prepare_cached(STATEMENT_QUEUE_GET_NEXT_READY)?
          .query_map([now], |row| {
            let ts: u64 = row.get(0)?;
            let id: String = row.get(1)?;
            let data: Vec<u8> = row.get(2)?;
            let backoff_schedule: String = row.get(3)?;
            let keys_if_undelivered: String = row.get(4)?;
            Ok((ts, id, data, backoff_schedule, keys_if_undelivered))
          })?
          .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        for (ts, id, data, backoff_schedule, keys_if_undelivered) in &messages {
          let changed = tx
            .prepare_cached(STATEMENT_QUEUE_REMOVE_READY)?
            .execute(params![id])?;
          assert_eq!(changed, 1);

          let changed =
            tx.prepare_cached(STATEMENT_QUEUE_ADD_RUNNING)?.execute(
              params![ts, id, &data, &backoff_schedule, &keys_if_undelivered],
            )?;
          assert_eq!(changed, 1);
        }
        tx.commit()?;

        Ok(
          messages
            .into_iter()
            .map(|(_, id, data, _, _)| (id, data))
            .collect::<Vec<_>>(),
        )
      })
      .await?;

      let busy = !messages.is_empty();

      for (id, data) in messages {
        if dequeue_tx.send((data, id)).await.is_err() {
          // Queue receiver was dropped. Stop the dequeue loop.
          return Ok(());
        }
      }

      if !busy {
        // There's nothing to dequeue right now; sleep until one of the
        // following happens:
        // - It's time to dequeue the next message based on its timestamp
        // - A new message is added to the queue
        // - The database is closed
        let sleep_fut = {
          match Self::get_earliest_ready_ts(conn.clone()).await? {
            Some(ts) => {
              let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
              if ts <= now {
                continue;
              }
              tokio::time::sleep(Duration::from_millis(ts - now)).boxed()
            }
            None => futures::future::pending().boxed(),
          }
        };
        tokio::select! {
          _ = sleep_fut => {}
          _ = waker_rx.recv() => {}
          _ = shutdown_rx.changed() => return Ok(())
        }
      }
    }
  }

  async fn get_earliest_ready_ts(
    conn: Rc<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>,
  ) -> Result<Option<u64>, AnyError> {
    SqliteDb::run_tx(conn.clone(), move |tx| {
      let ts = tx
        .prepare_cached(STATEMENT_QUEUE_GET_EARLIEST_READY)?
        .query_row([], |row| {
          let ts: u64 = row.get(0)?;
          Ok(ts)
        })
        .optional()?;
      Ok(ts)
    })
    .await
  }

  async fn requeue_inflight_messages(
    conn: Rc<AsyncRefCell<Cell<Option<rusqlite::Connection>>>>,
  ) -> Result<(), AnyError> {
    loop {
      let done = SqliteDb::run_tx(conn.clone(), move |tx| {
        let entries = tx
          .prepare_cached(STATEMENT_QUEUE_GET_RUNNING)?
          .query_map([], |row| {
            let id: String = row.get(0)?;
            Ok(id)
          })?
          .collect::<Result<Vec<_>, rusqlite::Error>>()?;
        for id in &entries {
          Self::requeue_message(id, &tx)?;
        }
        tx.commit()?;
        Ok(entries.is_empty())
      })
      .await?;
      if done {
        return Ok(());
      }
    }
  }

  fn requeue_message(
    id: &str,
    tx: &rusqlite::Transaction<'_>,
  ) -> Result<bool, AnyError> {
    let Some((_, id, data, backoff_schedule, keys_if_undelivered)) = tx
    .prepare_cached(STATEMENT_QUEUE_GET_RUNNING_BY_ID)?
    .query_row([id], |row| {
      let deadline: u64 = row.get(0)?;
      let id: String = row.get(1)?;
      let data: Vec<u8> = row.get(2)?;
      let backoff_schedule: String = row.get(3)?;
      let keys_if_undelivered: String = row.get(4)?;
      Ok((deadline, id, data, backoff_schedule, keys_if_undelivered))
    })
    .optional()? else {
      return Ok(false);
    };

    let backoff_schedule = {
      let backoff_schedule =
        serde_json::from_str::<Option<Vec<u64>>>(&backoff_schedule)?;
      backoff_schedule.unwrap_or_default()
    };

    let mut requeued = false;
    if !backoff_schedule.is_empty() {
      // Requeue based on backoff schedule
      let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
      let new_ts = now + backoff_schedule[0];
      let new_backoff_schedule = serde_json::to_string(&backoff_schedule[1..])?;
      let changed = tx
        .prepare_cached(STATEMENT_QUEUE_ADD_READY)?
        .execute(params![
          new_ts,
          id,
          &data,
          &new_backoff_schedule,
          &keys_if_undelivered
        ])
        .unwrap();
      assert_eq!(changed, 1);
      requeued = true;
    } else if !keys_if_undelivered.is_empty() {
      // No more requeues. Insert the message into the undelivered queue.
      let keys_if_undelivered =
        serde_json::from_str::<Vec<Vec<u8>>>(&keys_if_undelivered)?;

      let version: i64 = tx
        .prepare_cached(STATEMENT_INC_AND_GET_DATA_VERSION)?
        .query_row([], |row| row.get(0))?;

      for key in keys_if_undelivered {
        let changed = tx
          .prepare_cached(STATEMENT_KV_POINT_SET)?
          .execute(params![key, &data, &VALUE_ENCODING_V8, &version])?;
        assert_eq!(changed, 1);
      }
    }

    // Remove from running
    let changed = tx
      .prepare_cached(STATEMENT_QUEUE_REMOVE_RUNNING)?
      .execute(params![id])?;
    assert_eq!(changed, 1);

    Ok(requeued)
  }
}

#[async_trait(?Send)]
impl Database for SqliteDb {
  type QMH = DequeuedMessage;

  async fn snapshot_read(
    &self,
    requests: Vec<ReadRange>,
    _options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, AnyError> {
    let requests = Arc::new(requests);
    Self::run_tx(self.conn.clone(), move |tx| {
      let mut responses = Vec::with_capacity(requests.len());
      for request in &*requests {
        let mut stmt = tx.prepare_cached(if request.reverse {
          STATEMENT_KV_RANGE_SCAN_REVERSE
        } else {
          STATEMENT_KV_RANGE_SCAN
        })?;
        let entries = stmt
          .query_map(
            (
              request.start.as_slice(),
              request.end.as_slice(),
              request.limit.get(),
            ),
            |row| {
              let key: Vec<u8> = row.get(0)?;
              let value: Vec<u8> = row.get(1)?;
              let encoding: i64 = row.get(2)?;

              let value = decode_value(value, encoding);

              let version: i64 = row.get(3)?;
              Ok(KvEntry {
                key,
                value,
                versionstamp: version_to_versionstamp(version),
              })
            },
          )?
          .collect::<Result<Vec<_>, rusqlite::Error>>()?;
        responses.push(ReadRangeOutput { entries });
      }

      Ok(responses)
    })
    .await
  }

  async fn atomic_write(
    &self,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, AnyError> {
    let write = Arc::new(write);
    let (has_enqueues, commit_result) =
      Self::run_tx(self.conn.clone(), move |tx| {
        for check in &write.checks {
          let real_versionstamp = tx
            .prepare_cached(STATEMENT_KV_POINT_GET_VERSION_ONLY)?
            .query_row([check.key.as_slice()], |row| row.get(0))
            .optional()?
            .map(version_to_versionstamp);
          if real_versionstamp != check.versionstamp {
            return Ok((false, None));
          }
        }

        let version: i64 = tx
          .prepare_cached(STATEMENT_INC_AND_GET_DATA_VERSION)?
          .query_row([], |row| row.get(0))?;

        for mutation in &write.mutations {
          match &mutation.kind {
            MutationKind::Set(value) => {
              let (value, encoding) = encode_value(value);
              let changed = tx
                .prepare_cached(STATEMENT_KV_POINT_SET)?
                .execute(params![mutation.key, &value, &encoding, &version])?;
              assert_eq!(changed, 1)
            }
            MutationKind::Delete => {
              let changed = tx
                .prepare_cached(STATEMENT_KV_POINT_DELETE)?
                .execute(params![mutation.key])?;
              assert!(changed == 0 || changed == 1)
            }
            MutationKind::Sum(operand) => {
              mutate_le64(
                &tx,
                &mutation.key,
                "sum",
                operand,
                version,
                |a, b| a.wrapping_add(b),
              )?;
            }
            MutationKind::Min(operand) => {
              mutate_le64(
                &tx,
                &mutation.key,
                "min",
                operand,
                version,
                |a, b| a.min(b),
              )?;
            }
            MutationKind::Max(operand) => {
              mutate_le64(
                &tx,
                &mutation.key,
                "max",
                operand,
                version,
                |a, b| a.max(b),
              )?;
            }
          }
        }

        let now = SystemTime::now()
          .duration_since(SystemTime::UNIX_EPOCH)
          .unwrap()
          .as_millis() as u64;

        let has_enqueues = !write.enqueues.is_empty();
        for enqueue in &write.enqueues {
          let id = Uuid::new_v4().to_string();
          let backoff_schedule = serde_json::to_string(
            &enqueue
              .backoff_schedule
              .as_deref()
              .or_else(|| Some(&DEFAULT_BACKOFF_SCHEDULE[..])),
          )?;
          let keys_if_undelivered =
            serde_json::to_string(&enqueue.keys_if_undelivered)?;

          let changed =
            tx.prepare_cached(STATEMENT_QUEUE_ADD_READY)?
              .execute(params![
                now + enqueue.delay_ms,
                id,
                &enqueue.payload,
                &backoff_schedule,
                &keys_if_undelivered
              ])?;
          assert_eq!(changed, 1)
        }

        tx.commit()?;
        let new_versionstamp = version_to_versionstamp(version);

        Ok((
          has_enqueues,
          Some(CommitResult {
            versionstamp: new_versionstamp,
          }),
        ))
      })
      .await?;

    if has_enqueues {
      if let Some(queue) = self.queue.get() {
        queue.wake().await?;
      }
    }
    Ok(commit_result)
  }

  async fn dequeue_next_message(&self) -> Result<Self::QMH, AnyError> {
    let queue = self
      .queue
      .get_or_init(|| async move { SqliteQueue::new(self.conn.clone()) })
      .await;
    let handle = queue.dequeue().await?;
    Ok(handle)
  }

  fn close(&self) {
    if let Some(queue) = self.queue.get() {
      queue.shutdown();
    }
  }
}

/// Mutates a LE64 value in the database, defaulting to setting it to the
/// operand if it doesn't exist.
fn mutate_le64(
  tx: &Transaction,
  key: &[u8],
  op_name: &str,
  operand: &Value,
  new_version: i64,
  mutate: impl FnOnce(u64, u64) -> u64,
) -> Result<(), AnyError> {
  let Value::U64(operand) = *operand else {
    return Err(type_error(format!("Failed to perform '{op_name}' mutation on a non-U64 operand")));
  };

  let old_value = tx
    .prepare_cached(STATEMENT_KV_POINT_GET_VALUE_ONLY)?
    .query_row([key], |row| {
      let value: Vec<u8> = row.get(0)?;
      let encoding: i64 = row.get(1)?;

      let value = decode_value(value, encoding);
      Ok(value)
    })
    .optional()?;

  let new_value = match old_value {
    Some(Value::U64(old_value) ) => mutate(old_value, operand),
    Some(_) => return Err(type_error(format!("Failed to perform '{op_name}' mutation on a non-U64 value in the database"))),
    None => operand,
  };

  let new_value = Value::U64(new_value);
  let (new_value, encoding) = encode_value(&new_value);

  let changed = tx.prepare_cached(STATEMENT_KV_POINT_SET)?.execute(params![
    key,
    &new_value[..],
    encoding,
    new_version
  ])?;
  assert_eq!(changed, 1);

  Ok(())
}

fn version_to_versionstamp(version: i64) -> [u8; 10] {
  let mut versionstamp = [0; 10];
  versionstamp[..8].copy_from_slice(&version.to_be_bytes());
  versionstamp
}

const VALUE_ENCODING_V8: i64 = 1;
const VALUE_ENCODING_LE64: i64 = 2;
const VALUE_ENCODING_BYTES: i64 = 3;

fn decode_value(value: Vec<u8>, encoding: i64) -> crate::Value {
  match encoding {
    VALUE_ENCODING_V8 => crate::Value::V8(value),
    VALUE_ENCODING_BYTES => crate::Value::Bytes(value),
    VALUE_ENCODING_LE64 => {
      let mut buf = [0; 8];
      buf.copy_from_slice(&value);
      crate::Value::U64(u64::from_le_bytes(buf))
    }
    _ => todo!(),
  }
}

fn encode_value(value: &crate::Value) -> (Cow<'_, [u8]>, i64) {
  match value {
    crate::Value::V8(value) => (Cow::Borrowed(value), VALUE_ENCODING_V8),
    crate::Value::Bytes(value) => (Cow::Borrowed(value), VALUE_ENCODING_BYTES),
    crate::Value::U64(value) => {
      let mut buf = [0; 8];
      buf.copy_from_slice(&value.to_le_bytes());
      (Cow::Owned(buf.to_vec()), VALUE_ENCODING_LE64)
    }
  }
}
