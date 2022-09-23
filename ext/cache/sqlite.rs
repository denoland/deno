// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

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

#[async_trait]
impl Cache for SqliteBackedCache {
  fn new(cache_storage_dir: PathBuf) -> Self {
    {
      std::fs::create_dir_all(&cache_storage_dir)
        .expect("failed to create cache dir");
      let path = cache_storage_dir.join("cache_metadata.db");
      let connection = rusqlite::Connection::open(&path).unwrap_or_else(|_| {
        panic!("failed to open cache db at {}", path.display())
      });
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
                    request_headers        TEXT NOT NULL,
                    response_headers       TEXT NOT NULL,
                    response_status        INTEGER NOT NULL,
                    response_status_text   TEXT,
                    response_body_key      TEXT,
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

  /// Open a cache storage. Internally, this creates a row in the
  /// sqlite db if the cache doesn't exist and returns the internal id
  /// of the cache.
  async fn storage_open(&self, cache_name: String) -> Result<i64, AnyError> {
    let db = self.connection.clone();
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
        "SELECT count(cache_name) FROM cache_storage WHERE cache_name = ?1",
        params![cache_name],
        |row| {
          let count: i64 = row.get(0)?;
          Ok(count > 0)
        },
      )?;
      // TODO(@satyarohith): check if cache exists on disk.
      Ok::<bool, AnyError>(cache_exists)
    })
    .await?
  }

  /// Delete a cache storage. Internally, this deletes the row in the sqlite db.
  async fn storage_delete(&self, cache_name: String) -> Result<bool, AnyError> {
    let db = self.connection.clone();
    tokio::task::spawn_blocking(move || {
      let db = db.lock();
      let rows_effected = db.execute(
        "DELETE FROM cache_storage WHERE cache_name = ?1",
        params![cache_name],
      )?;
      // TODO(@satyarohith): delete assets related to cache from disk.
      Ok::<bool, AnyError>(rows_effected > 0)
    })
    .await?
  }

  async fn put(
    &self,
    request_response: CachePutRequest,
  ) -> Result<Option<Rc<dyn Resource>>, AnyError> {
    let db = self.connection.clone();
    let cache_storage_dir = self.cache_storage_dir.clone();
    let maybe_body_path = tokio::task::spawn_blocking(move || {
      let response_body_key = if request_response.response_has_body {
        Some(hash(&request_response.request_url))
      } else {
        None
      };
      let maybe_response_body = {
        let db = db.lock();
        db.query_row(
          "INSERT OR REPLACE INTO request_response_list
               (cache_id, request_url, request_headers, response_headers, response_body_key, response_status, response_status_text)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               RETURNING response_body_key",
          (
            request_response.cache_id,
            &request_response.request_url,
            serde_json::to_string(&request_response.request_headers)?,
            serde_json::to_string(&request_response.response_headers)?,
            response_body_key,
            &request_response.response_status,
            &request_response.response_status_text,
          ),
          |row| {
            let response_body_key: Option<String> = row.get(0)?;
            Ok(response_body_key)
          },
        )?
      };
      if let Some(body_key) = maybe_response_body {
        let responses_dir  = cache_storage_dir.join("responses");
          std::fs::create_dir_all(&responses_dir)?;
        let response_path = responses_dir.join(body_key);
        Ok::<Option<PathBuf>, AnyError>(Some(response_path))
      } else {
        Ok::<Option<PathBuf>, AnyError>(None)
      }
    }).await??;
    if let Some(path) = maybe_body_path {
      // TODO(@satyarohith): can we create the file in the blocking task above?
      let file = tokio::fs::File::create(path).await?;
      Ok(Some(Rc::new(CachePutResource::new(file))))
    } else {
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
          let response_headers: String = row.get(1)?;
          let response_status: u16 = row.get(2)?;
          let response_status_text: String = row.get(3)?;
          let request_headers: String = row.get(4)?;
          let response_headers: Vec<(String, String)> = serde_json::from_str(&response_headers).expect("malformed response headers from db");
          let request_headers: Vec<(String, String)> = serde_json::from_str(&request_headers).expect("malformed request headers from db");
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
          cache_storage_dir.join("responses").join(response_body_key);
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

/// Check if the headers provided in the vary_header match
/// the query request headers and the cached request headers.
fn vary_header_matches(
  vary_header: &str,
  query_request_headers: &[(String, String)],
  cached_request_headers: &[(String, String)],
) -> bool {
  let headers = get_headers_from_vary_header(vary_header);
  for header in headers {
    let query_header = get_header(&header, query_request_headers);
    let cached_header = get_header(&header, cached_request_headers);
    if query_header != cached_header {
      return false;
    }
  }
  true
}

fn get_headers_from_vary_header(vary_header: &str) -> Vec<String> {
  vary_header
    .split(',')
    .map(|s| s.trim().to_lowercase())
    .collect()
}

fn get_header(name: &str, headers: &[(String, String)]) -> Option<String> {
  headers
    .iter()
    .find(|(k, _)| k.to_lowercase() == name.to_lowercase())
    .map(|(_, v)| v.to_owned())
}

impl deno_core::Resource for SqliteBackedCache {
  fn name(&self) -> std::borrow::Cow<str> {
    "SqliteBackedCache".into()
  }
}

pub struct CachePutResource {
  file: AsyncRefCell<tokio::fs::File>,
}

impl CachePutResource {
  fn new(file: tokio::fs::File) -> Self {
    Self {
      file: AsyncRefCell::new(file),
    }
  }

  async fn write(self: Rc<Self>, data: ZeroCopyBuf) -> Result<usize, AnyError> {
    let resource = deno_core::RcRef::map(&self, |r| &r.file);
    let mut file = resource.borrow_mut().await;
    file.write_all(&data).await?;
    Ok(data.len())
  }

  async fn shutdown(self: Rc<Self>) -> Result<(), AnyError> {
    let resource = deno_core::RcRef::map(&self, |r| &r.file);
    let mut file = resource.borrow_mut().await;
    file.flush().await?;
    file.sync_all().await?;
    Ok(())
  }
}

impl Resource for CachePutResource {
  fn name(&self) -> Cow<str> {
    "CachePutResource".into()
  }

  fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(self.write(buf))
  }

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

  async fn read(
    self: Rc<Self>,
    mut buf: ZeroCopyBuf,
  ) -> Result<(usize, ZeroCopyBuf), AnyError> {
    let resource = deno_core::RcRef::map(&self, |r| &r.file);
    let mut file = resource.borrow_mut().await;
    let nread = file.read(&mut buf).await?;
    Ok((nread, buf))
  }
}

impl Resource for CacheResponseResource {
  fn name(&self) -> Cow<str> {
    "CacheResponseResource".into()
  }

  fn read_return(
    self: Rc<Self>,
    buf: ZeroCopyBuf,
  ) -> AsyncResult<(usize, ZeroCopyBuf)> {
    Box::pin(self.read(buf))
  }
}

pub fn hash(token: &str) -> String {
  use sha2::Digest;
  format!("{:x}", sha2::Sha256::digest(token.as_bytes()))
}
