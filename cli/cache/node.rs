// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use deno_ast::CjsAnalysis;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_runtime::deno_webstorage::rusqlite::params;
use deno_runtime::deno_webstorage::rusqlite::Connection;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

use super::common::run_sqlite_pragma;
use super::FastInsecureHasher;

// todo(dsherret): use deno_ast::CjsAnalysisData directly when upgrading deno_ast
// See https://github.com/denoland/deno_ast/pull/117
#[derive(Serialize, Deserialize)]
struct CjsAnalysisData {
  pub exports: Vec<String>,
  pub reexports: Vec<String>,
}

#[derive(Clone)]
pub struct NodeAnalysisCache {
  db_file_path: Option<PathBuf>,
  inner: Arc<Mutex<Option<Option<NodeAnalysisCacheInner>>>>,
}

impl NodeAnalysisCache {
  pub fn new(db_file_path: Option<PathBuf>) -> Self {
    Self {
      db_file_path,
      inner: Default::default(),
    }
  }

  pub fn compute_source_hash(text: &str) -> String {
    FastInsecureHasher::new()
      .write_str(text)
      .finish()
      .to_string()
  }

  pub fn get_cjs_analysis(
    &self,
    specifier: &str,
    expected_source_hash: &str,
  ) -> Option<CjsAnalysis> {
    self
      .with_inner(|inner| {
        inner.get_cjs_analysis(specifier, expected_source_hash)
      })
      .flatten()
  }

  pub fn set_cjs_analysis(
    &self,
    specifier: &str,
    source_hash: &str,
    cjs_analysis: &CjsAnalysis,
  ) {
    self.with_inner(|inner| {
      inner.set_cjs_analysis(specifier, source_hash, cjs_analysis)
    });
  }

  pub fn get_esm_analysis(
    &self,
    specifier: &str,
    expected_source_hash: &str,
  ) -> Option<Vec<String>> {
    self
      .with_inner(|inner| {
        inner.get_esm_analysis(specifier, expected_source_hash)
      })
      .flatten()
  }

  pub fn set_esm_analysis(
    &self,
    specifier: &str,
    source_hash: &str,
    top_level_decls: &Vec<String>,
  ) {
    self.with_inner(|inner| {
      inner.set_esm_analysis(specifier, source_hash, top_level_decls)
    });
  }

  fn with_inner<TResult>(
    &self,
    action: impl FnOnce(&NodeAnalysisCacheInner) -> Result<TResult, AnyError>,
  ) -> Option<TResult> {
    // lazily create the cache in order to not
    let mut maybe_created = self.inner.lock();
    let inner = match maybe_created.as_ref() {
      Some(maybe_inner) => maybe_inner.as_ref(),
      None => {
        let maybe_inner = match NodeAnalysisCacheInner::new(
          self.db_file_path.as_deref(),
          crate::version::deno(),
        ) {
          Ok(cache) => Some(cache),
          Err(err) => {
            // should never error here, but if it ever does don't fail
            if cfg!(debug_assertions) {
              panic!("Error creating node analysis cache: {:#}", err);
            } else {
              log::debug!("Error creating node analysis cache: {:#}", err);
              None
            }
          }
        };
        *maybe_created = Some(maybe_inner);
        maybe_created.as_ref().and_then(|p| p.as_ref())
      }
    }?;
    match action(inner) {
      Ok(result) => Some(result),
      Err(err) => {
        // should never error here, but if it ever does don't fail
        if cfg!(debug_assertions) {
          panic!("Error using esm analysis: {:#}", err);
        } else {
          log::debug!("Error using esm analysis: {:#}", err);
        }
        None
      }
    }
  }
}

struct NodeAnalysisCacheInner {
  conn: Connection,
}

impl NodeAnalysisCacheInner {
  pub fn new(
    db_file_path: Option<&Path>,
    version: String,
  ) -> Result<Self, AnyError> {
    log::debug!("Opening node analysis cache.");
    let conn = match db_file_path {
      Some(path) => Connection::open(path)?,
      None => Connection::open_in_memory()?,
    };
    Self::from_connection(conn, version)
  }

  fn from_connection(
    conn: Connection,
    version: String,
  ) -> Result<Self, AnyError> {
    run_sqlite_pragma(&conn)?;
    create_tables(&conn, &version)?;

    Ok(Self { conn })
  }

  pub fn get_cjs_analysis(
    &self,
    specifier: &str,
    expected_source_hash: &str,
  ) -> Result<Option<CjsAnalysis>, AnyError> {
    let query = "
      SELECT
        data
      FROM
        cjsanalysiscache
      WHERE
        specifier=?1
        AND source_hash=?2
      LIMIT 1";
    let mut stmt = self.conn.prepare_cached(query)?;
    let mut rows = stmt.query(params![specifier, &expected_source_hash])?;
    if let Some(row) = rows.next()? {
      let analysis_info: String = row.get(0)?;
      let analysis_info: CjsAnalysisData =
        serde_json::from_str(&analysis_info)?;
      Ok(Some(CjsAnalysis {
        exports: analysis_info.exports,
        reexports: analysis_info.reexports,
      }))
    } else {
      Ok(None)
    }
  }

  pub fn set_cjs_analysis(
    &self,
    specifier: &str,
    source_hash: &str,
    cjs_analysis: &CjsAnalysis,
  ) -> Result<(), AnyError> {
    let sql = "
      INSERT OR REPLACE INTO
        cjsanalysiscache (specifier, source_hash, data)
      VALUES
        (?1, ?2, ?3)";
    let mut stmt = self.conn.prepare_cached(sql)?;
    stmt.execute(params![
      specifier,
      &source_hash.to_string(),
      &serde_json::to_string(&CjsAnalysisData {
        // temporary clones until upgrading deno_ast
        exports: cjs_analysis.exports.clone(),
        reexports: cjs_analysis.reexports.clone(),
      })?,
    ])?;
    Ok(())
  }

  pub fn get_esm_analysis(
    &self,
    specifier: &str,
    expected_source_hash: &str,
  ) -> Result<Option<Vec<String>>, AnyError> {
    let query = "
      SELECT
        data
      FROM
        esmglobalscache
      WHERE
        specifier=?1
        AND source_hash=?2
      LIMIT 1";
    let mut stmt = self.conn.prepare_cached(query)?;
    let mut rows = stmt.query(params![specifier, &expected_source_hash])?;
    if let Some(row) = rows.next()? {
      let top_level_decls: String = row.get(0)?;
      let decls: Vec<String> = serde_json::from_str(&top_level_decls)?;
      Ok(Some(decls))
    } else {
      Ok(None)
    }
  }

  pub fn set_esm_analysis(
    &self,
    specifier: &str,
    source_hash: &str,
    top_level_decls: &Vec<String>,
  ) -> Result<(), AnyError> {
    let sql = "
      INSERT OR REPLACE INTO
        esmglobalscache (specifier, source_hash, data)
      VALUES
        (?1, ?2, ?3)";
    let mut stmt = self.conn.prepare_cached(sql)?;
    stmt.execute(params![
      specifier,
      &source_hash.to_string(),
      &serde_json::to_string(top_level_decls)?,
    ])?;
    Ok(())
  }
}

fn create_tables(conn: &Connection, cli_version: &str) -> Result<(), AnyError> {
  // INT doesn't store up to u64, so use TEXT for source_hash
  conn.execute(
    "CREATE TABLE IF NOT EXISTS cjsanalysiscache (
        specifier TEXT PRIMARY KEY,
        source_hash TEXT NOT NULL,
        data TEXT NOT NULL
      )",
    [],
  )?;
  conn.execute(
    "CREATE UNIQUE INDEX IF NOT EXISTS cjsanalysiscacheidx
    ON cjsanalysiscache(specifier)",
    [],
  )?;
  conn.execute(
    "CREATE TABLE IF NOT EXISTS esmglobalscache (
        specifier TEXT PRIMARY KEY,
        source_hash TEXT NOT NULL,
        data TEXT NOT NULL
      )",
    [],
  )?;
  conn.execute(
    "CREATE UNIQUE INDEX IF NOT EXISTS esmglobalscacheidx
      ON esmglobalscache(specifier)",
    [],
  )?;
  conn.execute(
    "CREATE TABLE IF NOT EXISTS info (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
      )",
    [],
  )?;

  // delete the cache when the CLI version changes
  let data_cli_version: Option<String> = conn
    .query_row(
      "SELECT value FROM info WHERE key='CLI_VERSION' LIMIT 1",
      [],
      |row| row.get(0),
    )
    .ok();
  if data_cli_version != Some(cli_version.to_string()) {
    conn.execute("DELETE FROM cjsanalysiscache", params![])?;
    conn.execute("DELETE FROM esmglobalscache", params![])?;
    let mut stmt = conn
      .prepare("INSERT OR REPLACE INTO info (key, value) VALUES (?1, ?2)")?;
    stmt.execute(params!["CLI_VERSION", &cli_version])?;
  }

  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  pub fn node_analysis_cache_general_use() {
    let conn = Connection::open_in_memory().unwrap();
    let cache =
      NodeAnalysisCacheInner::from_connection(conn, "1.0.0".to_string())
        .unwrap();

    assert!(cache.get_cjs_analysis("file.js", "2").unwrap().is_none());
    let cjs_analysis = CjsAnalysis {
      exports: vec!["export1".to_string()],
      reexports: vec!["re-export1".to_string()],
    };
    cache
      .set_cjs_analysis("file.js", "2", &cjs_analysis)
      .unwrap();
    assert!(cache.get_cjs_analysis("file.js", "3").unwrap().is_none()); // different hash
    let actual_cjs_analysis =
      cache.get_cjs_analysis("file.js", "2").unwrap().unwrap();
    assert_eq!(actual_cjs_analysis.exports, cjs_analysis.exports);
    assert_eq!(actual_cjs_analysis.reexports, cjs_analysis.reexports);

    assert!(cache.get_esm_analysis("file.js", "2").unwrap().is_none());
    let esm_analysis = vec!["esm1".to_string()];
    cache
      .set_esm_analysis("file.js", "2", &esm_analysis)
      .unwrap();
    assert!(cache.get_esm_analysis("file.js", "3").unwrap().is_none()); // different hash
    let actual_esm_analysis =
      cache.get_esm_analysis("file.js", "2").unwrap().unwrap();
    assert_eq!(actual_esm_analysis, esm_analysis);

    // adding when already exists should not cause issue
    cache
      .set_cjs_analysis("file.js", "2", &cjs_analysis)
      .unwrap();
    cache
      .set_esm_analysis("file.js", "2", &esm_analysis)
      .unwrap();

    // recreating with same cli version should still have it
    let conn = cache.conn;
    let cache =
      NodeAnalysisCacheInner::from_connection(conn, "1.0.0".to_string())
        .unwrap();
    let actual_analysis =
      cache.get_cjs_analysis("file.js", "2").unwrap().unwrap();
    assert_eq!(actual_analysis.exports, cjs_analysis.exports);
    assert_eq!(actual_analysis.reexports, cjs_analysis.reexports);
    let actual_esm_analysis =
      cache.get_esm_analysis("file.js", "2").unwrap().unwrap();
    assert_eq!(actual_esm_analysis, esm_analysis);

    // now changing the cli version should clear it
    let conn = cache.conn;
    let cache =
      NodeAnalysisCacheInner::from_connection(conn, "2.0.0".to_string())
        .unwrap();
    assert!(cache.get_cjs_analysis("file.js", "2").unwrap().is_none());
    assert!(cache.get_esm_analysis("file.js", "2").unwrap().is_none());
  }
}
