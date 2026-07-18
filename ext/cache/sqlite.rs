// Copyright 2018-2026 the Deno authors. MIT license.

use std::future::poll_fn;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::BufMutView;
use deno_core::Resource;
use deno_core::convert::ByteString;
use deno_core::parking_lot::Mutex;
use deno_core::unsync::spawn_blocking;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use rusqlite::params;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

use crate::CacheDeleteRequest;
use crate::CacheError;
use crate::CacheKeyEntry;
use crate::CacheMatchRequest;
use crate::CacheMatchResponseMeta;
use crate::CachePutRequest;
use crate::CacheResponseResource;
use crate::deserialize_headers;
use crate::get_header;
use crate::serialize_headers;
use crate::vary_header_matches;

#[derive(Clone)]
pub struct SqliteBackedCache {
  pub connection: Arc<Mutex<Connection>>,
  pub cache_storage_dir: PathBuf,
}

#[derive(Debug)]
enum Mode {
  Disk,
  InMemory,
}

impl SqliteBackedCache {
  pub fn new(cache_storage_dir: PathBuf) -> Result<Self, CacheError> {
    let mode = match std::env::var("DENO_CACHE_DB_MODE")
      .unwrap_or_default()
      .as_str()
    {
      "disk" | "" => Mode::Disk,
      "memory" => Mode::InMemory,
      _ => {
        log::warn!("Unknown DENO_CACHE_DB_MODE value, defaulting to disk");
        Mode::Disk
      }
    };

    let connection = if matches!(mode, Mode::InMemory) {
      rusqlite::Connection::open_in_memory()
        .unwrap_or_else(|_| panic!("failed to open in-memory cache db"))
    } else {
      #[allow(
        clippy::disallowed_methods,
        reason = "cache storage manages its own directory"
      )]
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
      connection
    };

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
      #[allow(
        clippy::disallowed_methods,
        reason = "cache storage manages its own directory"
      )]
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
        #[allow(
          clippy::disallowed_methods,
          reason = "cache storage manages its own directory"
        )]
        if cache_dir.exists() {
          #[allow(
            clippy::disallowed_methods,
            reason = "cache storage manages its own directory"
          )]
          std::fs::remove_dir_all(cache_dir)?;
        }
      }
      Ok::<bool, CacheError>(maybe_cache_id.is_some())
    })
    .await?
  }

  /// List all cache names.
  pub async fn storage_keys(&self) -> Result<Vec<String>, CacheError> {
    let db = self.connection.clone();
    spawn_blocking(move || {
      let db = db.lock();
      let mut stmt =
        db.prepare("SELECT cache_name FROM cache_storage ORDER BY id")?;
      let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
      let mut names = Vec::new();
      for row in rows {
        names.push(row?);
      }
      Ok::<Vec<String>, CacheError>(names)
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
    let now = now_unix();

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
      assert!(
        insert_cache_asset(db, request_response, None)
          .await?
          .is_none()
      );
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
        "SELECT response_body_key, response_headers, response_status, response_status_text, request_headers, last_inserted_at
             FROM request_response_list
             WHERE cache_id = ?1 AND request_url = ?2",
        (request.cache_id, &request.request_url),
        |row| {
          let response_body_key: Option<String> = row.get(0)?;
          let response_headers: Vec<u8> = row.get(1)?;
          let response_status: u16 = row.get(2)?;
          let response_status_text: String = row.get(3)?;
          let request_headers: Vec<u8> = row.get(4)?;
          let last_inserted_at: u64 = row.get(5)?;
          let response_headers: Vec<(ByteString, ByteString)> = deserialize_headers(&response_headers);
          let request_headers: Vec<(ByteString, ByteString)> = deserialize_headers(&request_headers);
          Ok((CacheMatchResponseMeta {
            request_headers,
            response_headers,
            response_status,
            response_status_text},
            response_body_key,
            last_inserted_at
          ))
        },
      );
      // Return ownership of request to the caller
      result.optional().map(|x| (x, request))
    })
    .await??;

    match query_result {
      Some((cache_meta, response_body_key, last_inserted_at)) => {
        // From https://w3c.github.io/ServiceWorker/#request-matches-cached-item-algorithm
        // If there's Vary header in the response, ensure all the
        // headers of the cached request match the query request.
        if let Some(vary_header) =
          get_header("vary", &cache_meta.response_headers)
          && !vary_header_matches(
            &vary_header,
            &request.request_headers,
            &cache_meta.request_headers,
          )
        {
          return Ok(None);
        }
        let now = now_unix().as_secs();
        if let Some(expires_at) =
          response_expires_at(&cache_meta.response_headers, last_inserted_at)
          && now >= expires_at
        {
          // Best efforts to delete the expired cache item
          _ = self
            .delete(CacheDeleteRequest {
              cache_id: request.cache_id,
              request_url: request.request_url,
            })
            .await;
          return Ok(None);
        }
        match response_body_key {
          Some(response_body_key) => {
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
          None => Ok(Some((cache_meta, None))),
        }
      }
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

  pub async fn keys(
    &self,
    cache_id: i64,
    request_url: Option<String>,
  ) -> Result<Vec<CacheKeyEntry>, CacheError> {
    let db = self.connection.clone();
    spawn_blocking(move || {
      let db = db.lock();
      // When a request URL is provided, filter in SQL rather than
      // materializing every entry in the cache just to return a single key.
      let mut sql = String::from(
        "SELECT request_url, request_headers FROM request_response_list
             WHERE cache_id = ?1",
      );
      let mut sql_params: Vec<rusqlite::types::Value> =
        vec![rusqlite::types::Value::Integer(cache_id)];
      if let Some(request_url) = request_url {
        sql.push_str(" AND request_url = ?2");
        sql_params.push(rusqlite::types::Value::Text(request_url));
      }
      sql.push_str(" ORDER BY id");
      let mut stmt = db.prepare(&sql)?;
      let rows =
        stmt.query_map(rusqlite::params_from_iter(sql_params), |row| {
          let request_url: String = row.get(0)?;
          let request_headers: Vec<u8> = row.get(1)?;
          Ok((request_url, request_headers))
        })?;
      let mut entries = Vec::new();
      for row in rows {
        let (request_url, request_headers) = row?;
        entries.push(CacheKeyEntry {
          request_url,
          request_headers: deserialize_headers(&request_headers),
        });
      }
      Ok::<Vec<CacheKeyEntry>, CacheError>(entries)
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
          now_unix().as_secs(),
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

fn now_unix() -> std::time::Duration {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("SystemTime should be after unix epoch")
}

fn response_expires_at(
  response_headers: &[(ByteString, ByteString)],
  last_inserted_at: u64,
) -> Option<u64> {
  if let Some(cache_control) = get_header("cache-control", response_headers) {
    let cache_control =
      String::from_utf8_lossy(&cache_control).to_ascii_lowercase();
    let mut max_age = None;
    let mut s_maxage = None;
    for directive in cache_control.split(',') {
      let directive = directive.trim();
      if let Some(value) = directive.strip_prefix("s-maxage=") {
        s_maxage = value.trim_matches('"').parse::<u64>().ok();
      } else if let Some(value) = directive.strip_prefix("max-age=") {
        max_age = value.trim_matches('"').parse::<u64>().ok();
      }
    }
    if let Some(max_age) = s_maxage.or(max_age) {
      return Some(last_inserted_at.saturating_add(max_age));
    }
  }
  if let Some(expires) = get_header("expires", response_headers) {
    let expires_at = chrono::DateTime::parse_from_rfc2822(
      String::from_utf8_lossy(&expires).trim(),
    )
    .map(|date| date.timestamp().max(0) as u64)
    // https://www.rfc-editor.org/rfc/rfc9111#section-5.3: a cache
    // recipient must interpret invalid date formats as representing
    // a time in the past
    .unwrap_or(0);
    return Some(expires_at);
  }
  None
}

#[inline]
fn get_responses_dir(cache_storage_dir: PathBuf, cache_id: i64) -> PathBuf {
  cache_storage_dir
    .join(cache_id.to_string())
    .join("responses")
}

impl deno_core::Resource for SqliteBackedCache {
  fn name(&self) -> std::borrow::Cow<'_, str> {
    "SqliteBackedCache".into()
  }
}

pub fn hash(token: &str) -> String {
  use sha2::Digest;
  format!("{:x}", sha2::Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_response_expires_at() {
    fn h(name: &str, value: &str) -> (ByteString, ByteString) {
      (ByteString::from(name), ByteString::from(value))
    }
    assert_eq!(response_expires_at(&[], 1000), None);
    assert_eq!(
      response_expires_at(&[h("content-type", "text/plain")], 1000),
      None
    );
    assert_eq!(
      response_expires_at(&[h("cache-control", "no-store")], 1000),
      None
    );
    assert_eq!(
      response_expires_at(&[h("cache-control", "max-age=600")], 1000),
      Some(1600)
    );
    assert_eq!(
      response_expires_at(&[h("Cache-Control", "public, Max-Age=600")], 1000),
      Some(1600)
    );
    assert_eq!(
      response_expires_at(&[h("cache-control", "max-age=\"600\"")], 1000),
      Some(1600)
    );
    assert_eq!(
      response_expires_at(
        &[h("cache-control", "max-age=600, s-maxage=30")],
        1000
      ),
      Some(1030)
    );
    assert_eq!(
      response_expires_at(
        &[h("cache-control", "max-age=18446744073709551615")],
        1000
      ),
      Some(u64::MAX)
    );
    assert_eq!(
      response_expires_at(&[h("expires", "Thu, 01 Jan 1970 00:16:40 GMT")], 0),
      Some(1000)
    );
    assert_eq!(
      response_expires_at(&[h("expires", "Thu, 01 Jan 1900 00:00:00 GMT")], 0),
      Some(0)
    );
    assert_eq!(response_expires_at(&[h("expires", "0")], 1000), Some(0));
    assert_eq!(
      response_expires_at(
        &[
          h("cache-control", "max-age=600"),
          h("expires", "Thu, 01 Jan 1970 00:16:40 GMT")
        ],
        1000
      ),
      Some(1600)
    );
    assert_eq!(
      response_expires_at(
        &[
          h("cache-control", "public"),
          h("expires", "Thu, 01 Jan 1970 00:16:40 GMT")
        ],
        1000
      ),
      Some(1000)
    );
  }
}
