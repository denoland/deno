// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::proto::datapath as pb;
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
use anyhow::Context;
use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::TryFutureExt;
use deno_core::task::JoinHandle;
use deno_core::OpState;
use prost::Message;
use rand::Rng;
use serde::Deserialize;
use tokio::sync::watch;
use url::Url;
use uuid::Uuid;

pub trait RemoteDbHandlerPermissions {
  fn check_env(&mut self, var: &str) -> Result<(), AnyError>;
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError>;
}

pub struct RemoteDbHandler<P: RemoteDbHandlerPermissions + 'static> {
  _p: std::marker::PhantomData<P>,
}

impl<P: RemoteDbHandlerPermissions> RemoteDbHandler<P> {
  pub fn new() -> Self {
    Self { _p: PhantomData }
  }
}

impl<P: RemoteDbHandlerPermissions> Default for RemoteDbHandler<P> {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Deserialize)]
struct VersionInfo {
  version: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct DatabaseMetadata {
  version: u64,
  database_id: Uuid,
  endpoints: Vec<EndpointInfo>,
  token: String,
  expires_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointInfo {
  pub url: String,

  // Using `String` instead of an enum, so that parsing doesn't
  // break if more consistency levels are added.
  pub consistency: String,
}

#[async_trait(?Send)]
impl<P: RemoteDbHandlerPermissions> DatabaseHandler for RemoteDbHandler<P> {
  type DB = RemoteDb<P>;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, AnyError> {
    const ENV_VAR_NAME: &str = "DENO_ACCESS_TOKEN";

    let Some(url) = path else {
      return Err(type_error("Missing database url"));
    };

    let Ok(parsed_url) = Url::parse(&url) else {
      return Err(type_error(format!("Invalid database url: {}", url)));
    };

    {
      let mut state = state.borrow_mut();
      let permissions = state.borrow_mut::<P>();
      permissions.check_env(ENV_VAR_NAME)?;
      permissions.check_net_url(&parsed_url, "Deno.openKv")?;
    }

    let access_token = std::env::var(ENV_VAR_NAME)
      .map_err(anyhow::Error::from)
      .with_context(|| {
        "Missing DENO_ACCESS_TOKEN environment variable. Please set it to your access token from https://dash.deno.com/account."
      })?;

    let refresher = MetadataRefresher::new(url, access_token);

    let db = RemoteDb {
      client: reqwest::Client::new(),
      refresher,
      _p: PhantomData,
    };
    Ok(db)
  }
}

pub struct RemoteDb<P: RemoteDbHandlerPermissions + 'static> {
  client: reqwest::Client,
  refresher: MetadataRefresher,
  _p: std::marker::PhantomData<P>,
}

pub struct DummyQueueMessageHandle {}

#[async_trait(?Send)]
impl QueueMessageHandle for DummyQueueMessageHandle {
  async fn take_payload(&mut self) -> Result<Vec<u8>, AnyError> {
    unimplemented!()
  }

  async fn finish(&self, _success: bool) -> Result<(), AnyError> {
    unimplemented!()
  }
}

#[async_trait(?Send)]
impl<P: RemoteDbHandlerPermissions> Database for RemoteDb<P> {
  type QMH = DummyQueueMessageHandle;

  async fn snapshot_read(
    &self,
    state: Rc<RefCell<OpState>>,
    requests: Vec<ReadRange>,
    _options: SnapshotReadOptions,
  ) -> Result<Vec<ReadRangeOutput>, AnyError> {
    let req = pb::SnapshotRead {
      ranges: requests
        .into_iter()
        .map(|r| pb::ReadRange {
          start: r.start,
          end: r.end,
          limit: r.limit.get() as _,
          reverse: r.reverse,
        })
        .collect(),
    };

    let res: pb::SnapshotReadOutput = call_remote::<P, _, _>(
      &state,
      &self.refresher,
      &self.client,
      "snapshot_read",
      &req,
    )
    .await?;

    if res.read_disabled {
      return Err(type_error("Reads are disabled for this database."));
    }

    let out = res
      .ranges
      .into_iter()
      .map(|r| {
        Ok(ReadRangeOutput {
          entries: r
            .values
            .into_iter()
            .map(|e| {
              let encoding = e.encoding();
              Ok(KvEntry {
                key: e.key,
                value: decode_value(e.value, encoding)?,
                versionstamp: <[u8; 10]>::try_from(&e.versionstamp[..])?,
              })
            })
            .collect::<Result<_, AnyError>>()?,
        })
      })
      .collect::<Result<Vec<_>, AnyError>>()?;
    Ok(out)
  }

  async fn atomic_write(
    &self,
    state: Rc<RefCell<OpState>>,
    write: AtomicWrite,
  ) -> Result<Option<CommitResult>, AnyError> {
    if !write.enqueues.is_empty() {
      return Err(type_error("Enqueue operations are not supported yet."));
    }

    let req = pb::AtomicWrite {
      kv_checks: write
        .checks
        .into_iter()
        .map(|x| {
          Ok(pb::KvCheck {
            key: x.key,
            versionstamp: x.versionstamp.unwrap_or([0u8; 10]).to_vec(),
          })
        })
        .collect::<anyhow::Result<_>>()?,
      kv_mutations: write
        .mutations
        .into_iter()
        .map(|x| encode_mutation(x.key, x.kind))
        .collect(),
      enqueues: vec![],
    };

    let res: pb::AtomicWriteOutput = call_remote::<P, _, _>(
      &state,
      &self.refresher,
      &self.client,
      "atomic_write",
      &req,
    )
    .await?;
    match res.status() {
      pb::AtomicWriteStatus::AwSuccess => Ok(Some(CommitResult {
        versionstamp: if res.versionstamp.is_empty() {
          Default::default()
        } else {
          res.versionstamp[..].try_into()?
        },
      })),
      pb::AtomicWriteStatus::AwCheckFailure => Ok(None),
      pb::AtomicWriteStatus::AwUnsupportedWrite => {
        Err(type_error("Unsupported write"))
      }
      pb::AtomicWriteStatus::AwUsageLimitExceeded => {
        Err(type_error("The database usage limit has been exceeded."))
      }
      pb::AtomicWriteStatus::AwWriteDisabled => {
        // TODO: Auto retry
        Err(type_error("Writes are disabled for this database."))
      }
      pb::AtomicWriteStatus::AwUnspecified => {
        Err(type_error("Unspecified error"))
      }
      pb::AtomicWriteStatus::AwQueueBacklogLimitExceeded => {
        Err(type_error("Queue backlog limit exceeded"))
      }
    }
  }

  async fn dequeue_next_message(
    &self,
    _state: Rc<RefCell<OpState>>,
  ) -> Result<Self::QMH, AnyError> {
    deno_core::futures::future::pending().await
  }

  fn close(&self) {}
}

fn decode_value(
  value: Vec<u8>,
  encoding: pb::KvValueEncoding,
) -> anyhow::Result<crate::Value> {
  match encoding {
    pb::KvValueEncoding::VeV8 => Ok(crate::Value::V8(value)),
    pb::KvValueEncoding::VeBytes => Ok(crate::Value::Bytes(value)),
    pb::KvValueEncoding::VeLe64 => Ok(crate::Value::U64(u64::from_le_bytes(
      <[u8; 8]>::try_from(&value[..])?,
    ))),
    pb::KvValueEncoding::VeUnspecified => {
      Err(anyhow::anyhow!("Unspecified value encoding, cannot decode"))
    }
  }
}

fn encode_value(value: crate::Value) -> pb::KvValue {
  match value {
    crate::Value::V8(data) => pb::KvValue {
      data,
      encoding: pb::KvValueEncoding::VeV8 as _,
    },
    crate::Value::Bytes(data) => pb::KvValue {
      data,
      encoding: pb::KvValueEncoding::VeBytes as _,
    },
    crate::Value::U64(x) => pb::KvValue {
      data: x.to_le_bytes().to_vec(),
      encoding: pb::KvValueEncoding::VeLe64 as _,
    },
  }
}

fn encode_mutation(key: Vec<u8>, mutation: MutationKind) -> pb::KvMutation {
  match mutation {
    MutationKind::Set(x) => pb::KvMutation {
      key,
      value: Some(encode_value(x)),
      mutation_type: pb::KvMutationType::MSet as _,
    },
    MutationKind::Delete => pb::KvMutation {
      key,
      value: Some(encode_value(crate::Value::Bytes(vec![]))),
      mutation_type: pb::KvMutationType::MClear as _,
    },
    MutationKind::Max(x) => pb::KvMutation {
      key,
      value: Some(encode_value(x)),
      mutation_type: pb::KvMutationType::MMax as _,
    },
    MutationKind::Min(x) => pb::KvMutation {
      key,
      value: Some(encode_value(x)),
      mutation_type: pb::KvMutationType::MMin as _,
    },
    MutationKind::Sum(x) => pb::KvMutation {
      key,
      value: Some(encode_value(x)),
      mutation_type: pb::KvMutationType::MSum as _,
    },
  }
}

struct MetadataRefresher {
  metadata_rx: watch::Receiver<Option<Arc<DatabaseMetadata>>>,
  handle: JoinHandle<()>,
}

impl MetadataRefresher {
  pub fn new(url: String, access_token: String) -> Self {
    let (tx, rx) = watch::channel(None);
    let handle =
      deno_core::task::spawn(metadata_refresh_task(url, access_token, tx));
    Self {
      handle,
      metadata_rx: rx,
    }
  }
}

impl Drop for MetadataRefresher {
  fn drop(&mut self) {
    self.handle.abort();
  }
}

async fn metadata_refresh_task(
  metadata_url: String,
  access_token: String,
  tx: watch::Sender<Option<Arc<DatabaseMetadata>>>,
) {
  let client = reqwest::Client::new();
  loop {
    let mut attempt = 0u64;
    let metadata = loop {
      match fetch_metadata(&client, &metadata_url, &access_token).await {
        Ok(x) => break x,
        Err(e) => {
          log::error!("Failed to fetch database metadata: {}", e);
          randomized_exponential_backoff(Duration::from_secs(5), attempt).await;
          attempt += 1;
        }
      }
    };

    let ms_until_expire = u64::try_from(
      metadata
        .expires_at
        .timestamp_millis()
        .saturating_sub(Utc::now().timestamp_millis()),
    )
    .unwrap_or_default();

    // Refresh 10 minutes before expiry
    // In case of buggy clocks, don't refresh more than once per minute
    let interval = Duration::from_millis(ms_until_expire)
      .saturating_sub(Duration::from_secs(600))
      .max(Duration::from_secs(60));

    if tx.send(Some(Arc::new(metadata))).is_err() {
      return;
    }

    tokio::time::sleep(interval).await;
  }
}

async fn fetch_metadata(
  client: &reqwest::Client,
  metadata_url: &str,
  access_token: &str,
) -> anyhow::Result<DatabaseMetadata> {
  let res = client
    .post(metadata_url)
    .header("authorization", format!("Bearer {}", access_token))
    .send()
    .await?;

  if !res.status().is_success() {
    anyhow::bail!("remote returned error: {}", res.text().await?);
  }

  let res = res.bytes().await?;
  let version_info: VersionInfo = serde_json::from_slice(&res)?;
  if version_info.version > 1 {
    anyhow::bail!("Unsupported metadata version: {}", version_info.version);
  }

  Ok(serde_json::from_slice(&res)?)
}

async fn randomized_exponential_backoff(base: Duration, attempt: u64) {
  let attempt = attempt.min(12);
  let delay = base.as_millis() as u64 + (2 << attempt);
  let delay = delay + rand::thread_rng().gen_range(0..(delay / 2) + 1);
  tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
}

async fn call_remote<
  P: RemoteDbHandlerPermissions + 'static,
  T: Message,
  R: Message + Default,
>(
  state: &RefCell<OpState>,
  refresher: &MetadataRefresher,
  client: &reqwest::Client,
  method: &str,
  req: &T,
) -> anyhow::Result<R> {
  let mut attempt = 0u64;
  let res = loop {
    let mut metadata_rx = refresher.metadata_rx.clone();
    let metadata = loop {
      if let Some(x) = &*metadata_rx.borrow() {
        break x.clone();
      }
      // `unwrap()` never fails because `tx` is owned by the task held by `refresher`.
      metadata_rx.changed().await.unwrap();
    };
    let Some(sc_endpoint) = metadata.endpoints.iter().find(|x| x.consistency == "strong") else {
        return Err(type_error("No strong consistency endpoint is available for this database"));
      };

    let full_url = format!("{}/{}", sc_endpoint.url, method);
    {
      let parsed_url = Url::parse(&full_url)?;
      let mut state = state.borrow_mut();
      let permissions = state.borrow_mut::<P>();
      permissions.check_net_url(&parsed_url, "Deno.Kv")?;
    }

    let res = client
      .post(&full_url)
      .header("x-transaction-domain-id", metadata.database_id.to_string())
      .header("authorization", format!("Bearer {}", metadata.token))
      .body(req.encode_to_vec())
      .send()
      .map_err(anyhow::Error::from)
      .and_then(|x| async move {
        if x.status().is_success() {
          Ok(Ok(x.bytes().await?))
        } else if x.status().is_client_error() {
          Ok(Err((x.status(), x.text().await?)))
        } else {
          Err(anyhow::anyhow!(
            "server error ({:?}): {}",
            x.status(),
            x.text().await?
          ))
        }
      })
      .await;

    match res {
      Ok(x) => break x,
      Err(e) => {
        log::error!("retryable error in {}: {}", method, e);
        randomized_exponential_backoff(Duration::from_millis(0), attempt).await;
        attempt += 1;
      }
    }
  };

  let res = match res {
    Ok(x) => x,
    Err((status, message)) => {
      return Err(type_error(format!(
        "client error in {} (status {:?}): {}",
        method, status, message
      )))
    }
  };

  match R::decode(&*res) {
    Ok(x) => Ok(x),
    Err(e) => Err(type_error(format!(
      "failed to decode response from {}: {}",
      method, e
    ))),
  }
}
