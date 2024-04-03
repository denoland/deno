// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_runtime::code_cache;
use deno_runtime::deno_webstorage::rusqlite;
use deno_runtime::deno_webstorage::rusqlite::params;

use super::cache_db::CacheDB;
use super::cache_db::CacheDBConfiguration;
use super::cache_db::CacheFailure;

pub static CODE_CACHE_DB: CacheDBConfiguration = CacheDBConfiguration {
  table_initializer: "CREATE TABLE IF NOT EXISTS codecache (
      specifier TEXT NOT NULL,
      type TEXT NOT NULL,
      source_hash TEXT,
      source_timestamp INTEGER,
      data BLOB NOT NULL,
      PRIMARY KEY (specifier, type)
    );",
  on_version_change: "DELETE FROM codecache;",
  preheat_queries: &[],
  on_failure: CacheFailure::Blackhole,
};

#[derive(Clone)]
pub struct CodeCache {
  inner: CodeCacheInner,
}

impl CodeCache {
  pub fn new(db: CacheDB) -> Self {
    Self {
      inner: CodeCacheInner::new(db),
    }
  }

  fn ensure_ok<T: Default>(res: Result<T, AnyError>) -> T {
    match res {
      Ok(x) => x,
      Err(err) => {
        // TODO(mmastrac): This behavior was inherited from before the refactoring but it probably makes sense to move it into the cache
        // at some point.
        // should never error here, but if it ever does don't fail
        if cfg!(debug_assertions) {
          panic!("Error using code cache: {err:#}");
        } else {
          log::debug!("Error using code cache: {:#}", err);
        }
        T::default()
      }
    }
  }

  pub fn get_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: Option<&str>,
    source_timestamp: Option<u64>,
  ) -> Option<Vec<u8>> {
    Self::ensure_ok(self.inner.get_sync(
      specifier,
      code_cache_type,
      source_hash,
      source_timestamp,
    ))
  }

  pub fn set_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: Option<&str>,
    source_timestamp: Option<u64>,
    data: &[u8],
  ) {
    Self::ensure_ok(self.inner.set_sync(
      specifier,
      code_cache_type,
      source_hash,
      source_timestamp,
      data,
    ));
  }
}

impl code_cache::CodeCache for CodeCache {
  fn get_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: Option<&str>,
    source_timestamp: Option<u64>,
  ) -> Option<Vec<u8>> {
    self.get_sync(specifier, code_cache_type, source_hash, source_timestamp)
  }

  fn set_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: Option<&str>,
    source_timestamp: Option<u64>,
    data: &[u8],
  ) {
    self.set_sync(
      specifier,
      code_cache_type,
      source_hash,
      source_timestamp,
      data,
    );
  }
}

#[derive(Clone)]
struct CodeCacheInner {
  conn: CacheDB,
}

impl CodeCacheInner {
  pub fn new(conn: CacheDB) -> Self {
    Self { conn }
  }

  pub fn get_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: Option<&str>,
    source_timestamp: Option<u64>,
  ) -> Result<Option<Vec<u8>>, AnyError> {
    let mut query = "
      SELECT
        data
      FROM
        codecache
      WHERE
        specifier=?1 AND type=?2"
      .to_string();
    let mut params: Vec<rusqlite::types::Value> = vec![
      specifier.to_string().into(),
      code_cache_type.as_str().to_string().into(),
    ];
    let mut param_index = 3;
    if let Some(source_hash) = source_hash {
      query += &format!(" AND source_hash=?{}", param_index);
      param_index += 1;
      params.push(source_hash.to_string().into());
    }
    if let Some(source_timestamp) = source_timestamp {
      query += &format!(" AND source_timestamp=?{}", param_index);
      params.push(source_timestamp.to_string().into());
    }
    self
      .conn
      .query_row(&query, rusqlite::params_from_iter(params), |row| {
        let value: Vec<u8> = row.get(0)?;
        Ok(value)
      })
  }

  pub fn set_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: Option<&str>,
    source_timestamp: Option<u64>,
    data: &[u8],
  ) -> Result<(), AnyError> {
    let sql = "
      INSERT OR REPLACE INTO
        codecache (specifier, type, source_hash, source_timestamp, data)
      VALUES
        (?1, ?2, ?3, ?4, ?5)";
    self.conn.execute(
      sql,
      params![
        specifier,
        code_cache_type.as_str(),
        source_hash,
        source_timestamp,
        data
      ],
    )?;
    Ok(())
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  pub fn end_to_end() {
    let conn = CacheDB::in_memory(&CODE_CACHE_DB, "1.0.0");
    let cache = CodeCacheInner::new(conn);

    assert!(cache
      .get_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::EsModule,
        Some("hash"),
        Some(10),
      )
      .unwrap()
      .is_none());
    let data_esm = vec![1, 2, 3];
    cache
      .set_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::EsModule,
        Some("hash"),
        Some(10),
        &data_esm,
      )
      .unwrap();
    assert_eq!(
      cache
        .get_sync(
          "file:///foo/bar.js",
          code_cache::CodeCacheType::EsModule,
          Some("hash"),
          Some(10),
        )
        .unwrap()
        .unwrap(),
      data_esm
    );
    assert!(cache
      .get_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::EsModule,
        Some("hash"),
        Some(20),
      )
      .unwrap()
      .is_none());
    assert!(cache
      .get_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::EsModule,
        Some("hash"),
        None,
      )
      .unwrap()
      .is_none());
    assert!(cache
      .get_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::EsModule,
        None,
        Some(10),
      )
      .unwrap()
      .is_none());

    assert!(cache
      .get_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::Script,
        Some("hash"),
        Some(10),
      )
      .unwrap()
      .is_none());
    let data_script = vec![1, 2, 3];
    cache
      .set_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::Script,
        Some("hash"),
        Some(10),
        &data_script,
      )
      .unwrap();
    assert_eq!(
      cache
        .get_sync(
          "file:///foo/bar.js",
          code_cache::CodeCacheType::Script,
          Some("hash"),
          Some(10),
        )
        .unwrap()
        .unwrap(),
      data_script
    );
    assert_eq!(
      cache
        .get_sync(
          "file:///foo/bar.js",
          code_cache::CodeCacheType::EsModule,
          Some("hash"),
          Some(10),
        )
        .unwrap()
        .unwrap(),
      data_esm
    );
  }

  #[test]
  pub fn time_stamp_only() {
    let conn = CacheDB::in_memory(&CODE_CACHE_DB, "1.0.0");
    let cache = CodeCacheInner::new(conn);

    assert!(cache
      .get_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::Script,
        None,
        Some(10),
      )
      .unwrap()
      .is_none());
    let data_esm = vec![1, 2, 3];
    cache
      .set_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::Script,
        None,
        Some(10),
        &data_esm,
      )
      .unwrap();
    assert_eq!(
      cache
        .get_sync(
          "file:///foo/bar.js",
          code_cache::CodeCacheType::Script,
          None,
          Some(10),
        )
        .unwrap()
        .unwrap(),
      data_esm
    );
  }
}
