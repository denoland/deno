// Copyright 2018-2025 the Deno authors. MIT license.

use std::future::poll_fn;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::parking_lot::Mutex;
use deno_core::unsync::spawn_blocking;
use deno_core::BufMutView;
use deno_core::ByteString;
use deno_core::Resource;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

use crate::deserialize_headers;
use crate::get_header;
use crate::serialize_headers;
use crate::vary_header_matches;
use crate::CacheDeleteRequest;
use crate::CacheError;
use crate::CacheMatchRequest;
use crate::CacheMatchResponseMeta;
use crate::CachePutRequest;
use crate::CacheResponseResource;

#[derive(Clone)]
pub struct SqliteBackedCache {
  pub connection: Arc<Mutex<Connection>>,
  pub cache_storage_dir: PathBuf,
}

impl SqliteBackedCache {
  pub fn new(cache_storage_dir: PathBuf) -> Result<Self, CacheError> {
    {
      std::fs::create_dir_all(&cache_storage_dir).map_err(|source| {
        CacheError::CacheStorageDirectory {
          dir: cache_storage_dir.clone(),
          source,
        }
      })?;
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
      connection.execute_batch(initial_pragmas)?;
      connection.execute(
        "CREATE TABLE IF NOT EXISTS cache_storage (
                    id              INTEGER PRIMARY KEY,
                    cache_name      TEXT NOT NULL UNIQUE
                )",
        (),
      )?;
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
        )?;
      Ok(SqliteBackedCache {
        connection: Arc::new(Mutex::new(connection)),
        cache_storage_dir,
      })
    }
  }
}

impl SqliteBackedCache {
  /// Open a cache storage. Internally, this creates a row in the
  /// sqlite db if the cache doesn't exist and returns the internal id
  /// of the cache.
  pub async fn storage_open(
    &self,
    cache_name: String,
  ) -> Result<i64, CacheError> {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    spawn_blocking(move || {
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
      Ok::<i64, CacheError>(cache_id)
    })
    .await?
  }

  /// Check if a cache with the provided name exists.
  /// Note: this doesn't check the disk, it only checks the sqlite db.
  pub async fn storage_has(
    &self,
    cache_name: String,
  ) -> Result<bool, CacheError> {
    let db = self.connection.clone();
    spawn_blocking(move || {
      let db = db.lock();
      let cache_exists = db.query_row(
        "SELECT count(id) FROM cache_storage WHERE cache_name = ?1",
        params![cache_name],
        |row| {
          let count: i64 = row.get(0)?;
          Ok(count > 0)
        },
      )?;
      Ok::<bool, CacheError>(cache_exists)
    })
    .await?
  }

  /// Delete a cache storage. Internally, this deletes the row in the sqlite db.
  pub async fn storage_delete(
    &self,
    cache_name: String,
  ) -> Result<bool, CacheError> {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    spawn_blocking(move || {
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
      Ok::<bool, CacheError>(maybe_cache_id.is_some())
    })
    .await?
  }

  pub async fn put(
    &self,
    request_response: CachePutRequest,
    resource: Option<Rc<dyn Resource>>,
  ) -> Result<(), CacheError> {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("SystemTime is before unix epoch");

    if let Some(resource) = resource {
      let body_key = hash(&format!(
        "{}_{}",
        &request_response.request_url,
        now.as_nanos()
      ));
      let responses_dir =
        get_responses_dir(cache_storage_dir, request_response.cache_id);
      let response_path = responses_dir.join(&body_key);
      let mut file = tokio::fs::File::create(response_path).await?;
      let mut buf = BufMutView::new(64 * 1024);
      loop {
        let (size, buf2) = resource
          .clone()
          .read_byob(buf)
          .await
          .map_err(CacheError::Other)?;
        if size == 0 {
          break;
        }
        buf = buf2;

        // Use poll_write to avoid holding a slice across await points
        poll_fn(|cx| Pin::new(&mut file).poll_write(cx, &buf[..size])).await?;
      }

      file.flush().await?;
      file.sync_all().await?;

      assert_eq!(
        insert_cache_asset(db, request_response, Some(body_key.clone()),)
          .await?,
        Some(body_key)
      );
    } else {
      assert!(insert_cache_asset(db, request_response, None)
        .await?
        .is_none());
    }
    Ok(())
  }

  pub async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<
    Option<(CacheMatchResponseMeta, Option<CacheResponseResource>)>,
    CacheError,
  > {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    let (query_result, request) = spawn_blocking(move || {
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
          Ok((CacheMatchResponseMeta {
            request_headers,
            response_headers,
            response_status,
            response_status_text},
            response_body_key
          ))
        },
      );
      // Return ownership of request to the caller
      result.optional().map(|x| (x, request))
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
        let file = match tokio::fs::File::open(response_path).await {
          Ok(file) => file,
          Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // Best efforts to delete the old cache item
            _ = self
              .delete(CacheDeleteRequest {
                cache_id: request.cache_id,
                request_url: request.request_url,
              })
              .await;
            return Ok(None);
          }
          Err(err) => return Err(err.into()),
        };
        Ok(Some((
          cache_meta,
          Some(CacheResponseResource::sqlite(file)),
        )))
      }
      Some((cache_meta, None)) => Ok(Some((cache_meta, None))),
      None => Ok(None),
    }
  }

  pub async fn delete(
    &self,
    request: CacheDeleteRequest,
  ) -> Result<bool, CacheError> {
    let db = self.connection.clone();
    spawn_blocking(move || {
      // TODO(@satyarohith): remove the response body from disk if one exists
      let db = db.lock();
      let rows_effected = db.execute(
        "DELETE FROM request_response_list WHERE cache_id = ?1 AND request_url = ?2",
        (request.cache_id, &request.request_url),
      )?;
      Ok::<bool, CacheError>(rows_effected > 0)
    })
    .await?
  }
}

async fn insert_cache_asset(
  db: Arc<Mutex<Connection>>,
  put: CachePutRequest,
  response_body_key: Option<String>,
) -> Result<Option<String>, CacheError> {
  spawn_blocking(move || {
    let maybe_response_body = {
      let db = db.lock();
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
          SystemTime::now().duration_since(UNIX_EPOCH).expect("SystemTime is before unix epoch").as_secs(),
        ),
        |row| {
          let response_body_key: Option<String> = row.get(0)?;
          Ok(response_body_key)
        },
      )?
    };
    Ok::<Option<String>, CacheError>(maybe_response_body)
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

pub fn hash(token: &str) -> String {
  use sha2::Digest;
  format!("{:x}", sha2::Sha256::digest(token.as_bytes()))
}
