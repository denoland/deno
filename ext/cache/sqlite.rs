// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::ByteString;
use deno_core::Resource;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::deserialize_headers;
use crate::get_header;
use crate::serialize_headers;
use crate::vary_header_matches;
use crate::Cache;
use crate::CacheDeleteRequest;
use crate::CacheMatchRequest;
use crate::CacheMatchResponseMeta;
use crate::CachePutRequest;

#[derive(Clone)]
pub struct SqliteBackedCache {
  pub connection: Arc<Mutex<Connection>>,
  pub cache_storage_dir: PathBuf,
}

impl SqliteBackedCache {
  pub fn new(cache_storage_dir: PathBuf) -> Self {
    {
      std::fs::create_dir_all(&cache_storage_dir)
        .expect("failed to create cache dir");
      let path = cache_storage_dir.join("cache_metadata.db");
      let connection = rusqlite::Connection::open(&path).unwrap_or_else(|_| {
        panic!("failed to open cache db at {}", path.display())
      });
      // Enable write-ahead-logging mode.
      let initial_pragmas = "
        -- enable write-ahead-logging mode
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA optimize;
      ";
      connection
        .execute_batch(initial_pragmas)
        .expect("failed to execute pragmas");
      connection
        .execute(
          "CREATE TABLE IF NOT EXISTS cache_storage (
                    id              INTEGER PRIMARY KEY,
                    cache_name      TEXT NOT NULL UNIQUE
                )",
          (),
        )
        .expect("failed to create cache_storage table");
      connection
        .execute(
          "CREATE TABLE IF NOT EXISTS request_response_list (
                    id                     INTEGER PRIMARY KEY,
                    cache_id               INTEGER NOT NULL,
                    request_url            TEXT NOT NULL,
                    request_headers        BLOB NOT NULL,
                    response_headers       BLOB NOT NULL,
                    response_status        INTEGER NOT NULL,
                    response_status_text   TEXT,
                    response_body_key      TEXT,
                    last_inserted_at       INTEGER UNSIGNED NOT NULL,
                    FOREIGN KEY (cache_id) REFERENCES cache_storage(id) ON DELETE CASCADE,

                    UNIQUE (cache_id, request_url)
                )",
          (),
        )
        .expect("failed to create request_response_list table");
      SqliteBackedCache {
        connection: Arc::new(Mutex::new(connection)),
        cache_storage_dir,
      }
    }
  }
}

#[async_trait]
impl Cache for SqliteBackedCache {
  /// Open a cache storage. Internally, this creates a row in the
  /// sqlite db if the cache doesn't exist and returns the internal id
  /// of the cache.
  async fn storage_open(&self, cache_name: String) -> Result<i64, AnyError> {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    tokio::task::spawn_blocking(move || {
      let db = db.lock();
      db.execute(
        "INSERT OR IGNORE INTO cache_storage (cache_name) VALUES (?1)",
        params![cache_name],
      )?;
      let cache_id = db.query_row(
        "SELECT id FROM cache_storage WHERE cache_name = ?1",
        params![cache_name],
        |row| {
          let id: i64 = row.get(0)?;
          Ok(id)
        },
      )?;
      let responses_dir = get_responses_dir(cache_storage_dir, cache_id);
      std::fs::create_dir_all(responses_dir)?;
      Ok::<i64, AnyError>(cache_id)
    })
    .await?
  }

  /// Check if a cache with the provided name exists.
  /// Note: this doesn't check the disk, it only checks the sqlite db.
  async fn storage_has(&self, cache_name: String) -> Result<bool, AnyError> {
    let db = self.connection.clone();
    tokio::task::spawn_blocking(move || {
      let db = db.lock();
      let cache_exists = db.query_row(
        "SELECT count(id) FROM cache_storage WHERE cache_name = ?1",
        params![cache_name],
        |row| {
          let count: i64 = row.get(0)?;
          Ok(count > 0)
        },
      )?;
      Ok::<bool, AnyError>(cache_exists)
    })
    .await?
  }

  /// Delete a cache storage. Internally, this deletes the row in the sqlite db.
  async fn storage_delete(&self, cache_name: String) -> Result<bool, AnyError> {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    tokio::task::spawn_blocking(move || {
      let db = db.lock();
      let maybe_cache_id = db
        .query_row(
          "DELETE FROM cache_storage WHERE cache_name = ?1 RETURNING id",
          params![cache_name],
          |row| {
            let id: i64 = row.get(0)?;
            Ok(id)
          },
        )
        .optional()?;
      if let Some(cache_id) = maybe_cache_id {
        let cache_dir = cache_storage_dir.join(cache_id.to_string());
        if cache_dir.exists() {
          std::fs::remove_dir_all(cache_dir)?;
        }
      }
      Ok::<bool, AnyError>(maybe_cache_id.is_some())
    })
    .await?
  }

  async fn put(
    &self,
    request_response: CachePutRequest,
  ) -> Result<Option<Rc<dyn Resource>>, AnyError> {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let response_body_key = if request_response.response_has_body {
      Some(hash(&format!(
        "{}_{}",
        &request_response.request_url,
        now.as_nanos()
      )))
    } else {
      None
    };

    if let Some(body_key) = response_body_key {
      let responses_dir =
        get_responses_dir(cache_storage_dir, request_response.cache_id);
      let response_path = responses_dir.join(&body_key);
      let file = tokio::fs::File::create(response_path).await?;
      Ok(Some(Rc::new(CachePutResource {
        file: AsyncRefCell::new(file),
        db,
        put_request: request_response,
        response_body_key: body_key,
        start_time: now.as_secs(),
      })))
    } else {
      insert_cache_asset(db, request_response, None).await?;
      Ok(None)
    }
  }

  async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<
    Option<(CacheMatchResponseMeta, Option<Rc<dyn Resource>>)>,
    AnyError,
  > {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    let query_result = tokio::task::spawn_blocking(move || {
      let db = db.lock();
      let result = db.query_row(
        "SELECT response_body_key, response_headers, response_status, response_status_text, request_headers
             FROM request_response_list
             WHERE cache_id = ?1 AND request_url = ?2",
        (request.cache_id, &request.request_url),
        |row| {
          let response_body_key: Option<String> = row.get(0)?;
          let response_headers: Vec<u8> = row.get(1)?;
          let response_status: u16 = row.get(2)?;
          let response_status_text: String = row.get(3)?;
          let request_headers: Vec<u8> = row.get(4)?;
          let response_headers: Vec<(ByteString, ByteString)> = deserialize_headers(&response_headers);
          let request_headers: Vec<(ByteString, ByteString)> = deserialize_headers(&request_headers);
          Ok((CacheMatchResponseMeta {request_headers, response_headers,response_status,response_status_text}, response_body_key))
        },
      );
      result.optional()
    })
    .await??;

    match query_result {
      Some((cache_meta, Some(response_body_key))) => {
        // From https://w3c.github.io/ServiceWorker/#request-matches-cached-item-algorithm
        // If there's Vary header in the response, ensure all the
        // headers of the cached request match the query request.
        if let Some(vary_header) =
          get_header("vary", &cache_meta.response_headers)
        {
          if !vary_header_matches(
            &vary_header,
            &request.request_headers,
            &cache_meta.request_headers,
          ) {
            return Ok(None);
          }
        }
        let response_path =
          get_responses_dir(cache_storage_dir, request.cache_id)
            .join(response_body_key);
        let file = tokio::fs::File::open(response_path).await?;
        return Ok(Some((
          cache_meta,
          Some(Rc::new(CacheResponseResource::new(file))),
        )));
      }
      Some((cache_meta, None)) => {
        return Ok(Some((cache_meta, None)));
      }
      None => return Ok(None),
    }
  }

  async fn delete(
    &self,
    request: CacheDeleteRequest,
  ) -> Result<bool, AnyError> {
    let db = self.connection.clone();
    tokio::task::spawn_blocking(move || {
      // TODO(@satyarohith): remove the response body from disk if one exists
      let db = db.lock();
      let rows_effected = db.execute(
        "DELETE FROM request_response_list WHERE cache_id = ?1 AND request_url = ?2",
        (request.cache_id, &request.request_url),
      )?;
      Ok::<bool, AnyError>(rows_effected > 0)
    })
    .await?
  }
}

async fn insert_cache_asset(
  db: Arc<Mutex<rusqlite::Connection>>,
  put: CachePutRequest,
  body_key_start_time: Option<(String, u64)>,
) -> Result<Option<String>, deno_core::anyhow::Error> {
  tokio::task::spawn_blocking(move || {
    let maybe_response_body = {
      let db = db.lock();
      let mut response_body_key = None;
      if let Some((body_key, start_time)) = body_key_start_time {
        response_body_key = Some(body_key);
          let last_inserted_at = db.query_row("
          SELECT last_inserted_at FROM request_response_list
          WHERE cache_id = ?1 AND request_url = ?2",
          (put.cache_id, &put.request_url), |row| {
            let last_inserted_at: i64 = row.get(0)?;
            Ok(last_inserted_at)
          }).optional()?;
          if let Some(last_inserted) = last_inserted_at {
            // Some other worker has already inserted this resource into the cache.
            // Note: okay to unwrap() as it is always present when response_body_key is present.
            if start_time > (last_inserted as u64) {
              return Ok(None);
            }
          }
      }
      db.query_row(
        "INSERT OR REPLACE INTO request_response_list
             (cache_id, request_url, request_headers, response_headers,
              response_body_key, response_status, response_status_text, last_inserted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             RETURNING response_body_key",
        (
          put.cache_id,
          put.request_url,
          serialize_headers(&put.request_headers),
          serialize_headers(&put.response_headers),
          response_body_key,
          put.response_status,
          put.response_status_text,
          SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        ),
        |row| {
          let response_body_key: Option<String> = row.get(0)?;
          Ok(response_body_key)
        },
      )?
    };
      Ok::<Option<String>, AnyError>(maybe_response_body)
  }).await?
}

#[inline]
fn get_responses_dir(cache_storage_dir: PathBuf, cache_id: i64) -> PathBuf {
  cache_storage_dir
    .join(cache_id.to_string())
    .join("responses")
}

impl deno_core::Resource for SqliteBackedCache {
  fn name(&self) -> std::borrow::Cow<str> {
    "SqliteBackedCache".into()
  }
}

pub struct CachePutResource {
  pub db: Arc<Mutex<rusqlite::Connection>>,
  pub put_request: CachePutRequest,
  pub response_body_key: String,
  pub file: AsyncRefCell<tokio::fs::File>,
  pub start_time: u64,
}

impl CachePutResource {
  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, AnyError> {
    let resource = deno_core::RcRef::map(&self, |r| &r.file);
    let mut file = resource.borrow_mut().await;
    file.write_all(data).await?;
    Ok(data.len())
  }

  async fn shutdown(self: Rc<Self>) -> Result<(), AnyError> {
    let resource = deno_core::RcRef::map(&self, |r| &r.file);
    let mut file = resource.borrow_mut().await;
    file.flush().await?;
    file.sync_all().await?;
    insert_cache_asset(
      self.db.clone(),
      self.put_request.clone(),
      Some((self.response_body_key.clone(), self.start_time)),
    )
    .await?;
    Ok(())
  }
}

impl Resource for CachePutResource {
  fn name(&self) -> Cow<str> {
    "CachePutResource".into()
  }

  deno_core::impl_writable!();

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown())
  }
}

pub struct CacheResponseResource {
  file: AsyncRefCell<tokio::fs::File>,
}

impl CacheResponseResource {
  fn new(file: tokio::fs::File) -> Self {
    Self {
      file: AsyncRefCell::new(file),
    }
  }

  async fn read(self: Rc<Self>, data: &mut [u8]) -> Result<usize, AnyError> {
    let resource = deno_core::RcRef::map(&self, |r| &r.file);
    let mut file = resource.borrow_mut().await;
    let nread = file.read(data).await?;
    Ok(nread)
  }
}

impl Resource for CacheResponseResource {
  deno_core::impl_readable_byob!();

  fn name(&self) -> Cow<str> {
    "CacheResponseResource".into()
  }
}

pub fn hash(token: &str) -> String {
  use sha2::Digest;
  format!("{:x}", sha2::Sha256::digest(token.as_bytes()))
}
