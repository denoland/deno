// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_runtime::code_cache;
use deno_runtime::deno_webstorage::rusqlite::params;

use super::cache_db::CacheDB;
use super::cache_db::CacheDBConfiguration;
use super::cache_db::CacheFailure;

pub static CODE_CACHE_DB: CacheDBConfiguration = CacheDBConfiguration {
  table_initializer: "CREATE TABLE IF NOT EXISTS codecache (
      specifier TEXT NOT NULL,
      type TEXT NOT NULL,
      source_hash TEXT NOT NULL,
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
    source_hash: &str,
  ) -> Option<Vec<u8>> {
    Self::ensure_ok(self.inner.get_sync(
      specifier,
      code_cache_type,
      source_hash,
    ))
  }

  pub fn set_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: &str,
    data: &[u8],
  ) {
    Self::ensure_ok(self.inner.set_sync(
      specifier,
      code_cache_type,
      source_hash,
      data,
    ));
  }
}

impl code_cache::CodeCache for CodeCache {
  fn get_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: &str,
  ) -> Option<Vec<u8>> {
    self.get_sync(specifier, code_cache_type, source_hash)
  }

  fn set_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: &str,
    data: &[u8],
  ) {
    self.set_sync(specifier, code_cache_type, source_hash, data);
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
    source_hash: &str,
  ) -> Result<Option<Vec<u8>>, AnyError> {
    let query = "
      SELECT
        data
      FROM
        codecache
      WHERE
        specifier=?1 AND type=?2 AND source_hash=?3
      LIMIT 1";
    let params = params![specifier, code_cache_type.as_str(), source_hash,];
    self.conn.query_row(query, params, |row| {
      let value: Vec<u8> = row.get(0)?;
      Ok(value)
    })
  }

  pub fn set_sync(
    &self,
    specifier: &str,
    code_cache_type: code_cache::CodeCacheType,
    source_hash: &str,
    data: &[u8],
  ) -> Result<(), AnyError> {
    let sql = "
      INSERT OR REPLACE INTO
        codecache (specifier, type, source_hash, data)
      VALUES
        (?1, ?2, ?3, ?4)";
    let params =
      params![specifier, code_cache_type.as_str(), source_hash, data];
    self.conn.execute(sql, params)?;
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
        "hash",
      )
      .unwrap()
      .is_none());
    let data_esm = vec![1, 2, 3];
    cache
      .set_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::EsModule,
        "hash",
        &data_esm,
      )
      .unwrap();
    assert_eq!(
      cache
        .get_sync(
          "file:///foo/bar.js",
          code_cache::CodeCacheType::EsModule,
          "hash",
        )
        .unwrap()
        .unwrap(),
      data_esm
    );

    assert!(cache
      .get_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::Script,
        "hash",
      )
      .unwrap()
      .is_none());
    let data_script = vec![4, 5, 6];
    cache
      .set_sync(
        "file:///foo/bar.js",
        code_cache::CodeCacheType::Script,
        "hash",
        &data_script,
      )
      .unwrap();
    assert_eq!(
      cache
        .get_sync(
          "file:///foo/bar.js",
          code_cache::CodeCacheType::Script,
          "hash",
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
          "hash",
        )
        .unwrap()
        .unwrap(),
      data_esm
    );
  }
}
