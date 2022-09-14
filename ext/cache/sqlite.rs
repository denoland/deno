// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use async_trait::async_trait;
use deno_core::parking_lot::Mutex;
use deno_core::{error::AnyError, ZeroCopyBuf};
use deno_core::{serde_json, AsyncRefCell, AsyncResult, Resource};
use rusqlite::params;
use std::borrow::{BorrowMut, Cow};
use std::rc::Rc;
use tokio::io::AsyncWriteExt;

use rusqlite::Connection;
use std::{path::PathBuf, sync::Arc};

use crate::{
  Cache, CacheDeleteRequest, CacheMatchRequest, CacheMatchResponseMeta,
  CachePutRequest,
};

#[derive(Clone)]
pub struct SqliteBackedCache(Arc<Mutex<Connection>>);

impl SqliteBackedCache {
  pub fn new(db_dir: PathBuf) -> Self {
    std::fs::create_dir_all(&db_dir).expect("failed to create cache dir");
    let path = db_dir.join("cache.db");
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
                  FOREIGN KEY (cache_id) REFERENCES cache_storage(id) ON DELETE CASCADE
              )",
        (),
      )
      .expect("failed to create request_response_list table");
    SqliteBackedCache(Arc::new(Mutex::new(connection)))
  }
}

#[async_trait]
impl Cache for SqliteBackedCache {
  async fn storage_open(&self, cache_name: String) -> Result<i64, AnyError> {
    let db = self.0.lock();
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
    Ok(cache_id)
  }

  async fn storage_has(&self, cache_name: String) -> Result<bool, AnyError> {
    let db = self.0.lock();
    let cache_exists = db.query_row(
      "SELECT count(cache_name) FROM cache_storage WHERE cache_name = ?1",
      params![cache_name],
      |row| {
        let count: i64 = row.get(0)?;
        Ok(count > 0)
      },
    )?;
    Ok(cache_exists)
  }

  async fn storage_delete(&self, cache_name: String) -> Result<bool, AnyError> {
    let db = self.0.lock();
    let mut stmt =
      db.prepare("DELETE FROM cache_storage WHERE cache_name = ?1")?;
    let rows_effected = stmt.execute([cache_name])?;
    let deleted = rows_effected > 0;
    Ok(deleted)
  }

  async fn put(
    &self,
    request_response: CachePutRequest,
  ) -> Result<Option<Rc<dyn Resource>>, AnyError> {
    let response_body_key = if request_response.response_has_body {
      Some(format!("responses/{}", hash(&request_response.request_url)))
    } else {
      None
    };
    let maybe_response_body = {
      let db = self.0.lock();
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
      let path = std::env::current_dir().unwrap().join(body_key);
      let file = tokio::fs::File::open(path).await?;
      // TODO(@satyarohith): store the body somewhere.
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
    let db = self.0.lock();
    let (cache_meta, _response_body_key) = db.query_row(
      "SELECT response_body_key, response_headers, response_status, response_status_text
           FROM request_response_list
           WHERE cache_id = ?1 AND request_url = ?2",
      (request.cache_id, &request.request_url),
      |row| {
        let response_body_key: Option<String> = row.get(0)?;
        let response_headers: String = row.get(1)?;
        let response_status: u16 = row.get(2)?;
        let response_status_text: String = row.get(3)?;
        let response_headers: Vec<(String, String)> = serde_json::from_str(&response_headers).expect("malformed response headers from db");

        Ok((
          CacheMatchResponseMeta {
            response_headers,
            response_status,
            response_status_text,
          },
          response_body_key,
        ))
      },
    )?;

    // TODO(@satyarohith): read the body from somewhere.
    Ok(Some((cache_meta, Some(Rc::new(CacheResponseResource {})))))
  }

  async fn delete(
    &self,
    request: CacheDeleteRequest,
  ) -> Result<bool, AnyError> {
    // TODO(@satyarohith): remove the response body from disk if one exists
    let db = self.0.lock();
    let rows_effected = db.execute(
      "DELETE FROM request_response_list
         WHERE cache_id = ?1 AND request_url = ?2",
      (request.cache_id, &request.request_url),
    )?;
    let deleted = rows_effected > 0;
    Ok(deleted)
  }
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

  async fn write(
    self: Rc<Self>,
    data: ZeroCopyBuf,
    end_of_stream: bool,
  ) -> Result<usize, AnyError> {
    println!("write() called (len: {} bytes)", data.len());
    let resource = deno_core::RcRef::map(&self, |r| &r.file);
    let mut file = resource.borrow_mut().await;
    file.write_all(&data).await?;
    Ok(data.len())
  }

  async fn end_of_stream(self: Rc<Self>) -> Result<(), AnyError> {
    println!("end of strema called!");
    Ok(())
  }
}

impl Resource for CachePutResource {
  fn name(&self) -> Cow<str> {
    "CachePutResource".into()
  }

  fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(self.write(buf, false))
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.end_of_stream())
  }
}

pub struct CacheResponseResource {}

impl CacheResponseResource {
  async fn read(
    self: Rc<Self>,
    mut buf: ZeroCopyBuf,
  ) -> Result<(usize, ZeroCopyBuf), AnyError> {
    println!("read():");
    buf.fill_with(|| b'a');
    Ok((0, buf))
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
