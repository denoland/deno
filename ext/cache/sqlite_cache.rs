// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use async_trait::async_trait;
use deno_core::parking_lot::Mutex;
use deno_core::{error::AnyError, ResourceId};
use rusqlite::params;
use rusqlite::Connection;
use std::{path::PathBuf, sync::Arc};

use crate::{
  Cache, CacheDeleteRequest, CacheMatchRequest, CacheMatchResponse,
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
                  FOREIGN KEY (cache_id) REFERENCES cache_storage(id)
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
  ) -> Result<Option<ResourceId>, AnyError> {
    println!("put: {:#?}", request_response);
    Ok(None)
  }

  async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<Option<CacheMatchResponse>, AnyError> {
    println!("match: {:#?}", request);
    Ok(None)
  }

  async fn delete(
    &self,
    request: CacheDeleteRequest,
  ) -> Result<bool, AnyError> {
    println!("delete: {:#?}", request);
    Ok(false)
  }
}

impl deno_core::Resource for SqliteBackedCache {
  fn name(&self) -> std::borrow::Cow<str> {
    "SqliteBackedCache".into()
  }
}
