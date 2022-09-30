// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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
#[derive(Serialize, Deserialize)]
struct CjsAnalysisData {
  pub exports: Vec<String>,
  pub reexports: Vec<String>,
}

pub struct NodeAnalysisCache {
  db_file_path: Option<PathBuf>,
  version: String,
  conn: Arc<Mutex<Option<Connection>>>,
}

impl NodeAnalysisCache {
  pub fn new(db_file_path: Option<&Path>, version: &str) -> Self {
    Self {
      db_file_path: db_file_path.map(|p| p.to_owned()),
      version: version.to_string(),
      conn: Arc::new(Mutex::new(None)),
    }
  }

  fn lazy_create(&self) -> Result<(), AnyError> {
    if self.conn.lock().is_some() {
      return Ok(());
    }

    let conn = match self.db_file_path.as_ref() {
      Some(path) => Connection::open(path)?,
      None => Connection::open_in_memory()?,
    };
    run_sqlite_pragma(&conn)?;
    create_tables(&conn, &self.version)?;
    self.conn.lock().replace(conn);
    Ok(())
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
  ) -> Result<Option<CjsAnalysis>, AnyError> {
    self.lazy_create()?;
    let guard = self.conn.lock();
    let conn = guard.as_ref().unwrap();
    let query = "
      SELECT
        data
      FROM
        cjs_analysis_cache
      WHERE
        specifier=?1
        AND source_hash=?2
      LIMIT 1";
    let mut stmt = conn.prepare_cached(query)?;
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
    self.lazy_create()?;
    let guard = self.conn.lock();
    let conn = guard.as_ref().unwrap();
    let sql = "
      INSERT OR REPLACE INTO
      cjs_analysis_cache (specifier, source_hash, data)
      VALUES
        (?1, ?2, ?3)";
    let mut stmt = conn.prepare_cached(sql)?;
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
    self.lazy_create()?;
    let guard = self.conn.lock();
    let conn = guard.as_ref().unwrap();
    let query = "
      SELECT
        data
      FROM
        esm_globals_cache
      WHERE
        specifier=?1
        AND source_hash=?2
      LIMIT 1";
    let mut stmt = conn.prepare_cached(query)?;
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
    top_level_decls: Vec<String>,
  ) -> Result<(), AnyError> {
    self.lazy_create()?;
    let guard = self.conn.lock();
    let conn = guard.as_ref().unwrap();
    let sql = "
      INSERT OR REPLACE INTO
      esm_globals_cache (specifier, source_hash, data)
      VALUES
        (?1, ?2, ?3)";
    let mut stmt = conn.prepare_cached(sql)?;
    stmt.execute(params![
      specifier,
      &source_hash.to_string(),
      &serde_json::to_string(&top_level_decls)?,
    ])?;
    Ok(())
  }
}

fn create_tables(conn: &Connection, cli_version: &str) -> Result<(), AnyError> {
  // INT doesn't store up to u64, so use TEXT for source_hash
  conn.execute(
    "CREATE TABLE IF NOT EXISTS cjs_analysis_cache (
        specifier TEXT PRIMARY KEY,
        source_hash TEXT NOT NULL,
        data TEXT NOT NULL
      )",
    [],
  )?;
  conn.execute(
    "CREATE UNIQUE INDEX IF NOT EXISTS cjs_analysis_cache_idx
    ON cjsanalysiscache(specifier, source_hash)",
    [],
  )?;
  conn.execute(
    "CREATE TABLE IF NOT EXISTS esm_globals_cache (
        specifier TEXT PRIMARY KEY,
        source_hash TEXT NOT NULL,
        data TEXT NOT NULL
      )",
    [],
  )?;
  conn.execute(
    "CREATE UNIQUE INDEX IF NOT EXISTS esm_globals_cache_idx
    ON esmglobalscache(specifier, source_hash)",
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
    conn.execute("DELETE FROM cjs_analysis_cache", params![])?;
    conn.execute("DELETE FROM esm_globals_cache", params![])?;
    let mut stmt = conn
      .prepare("INSERT OR REPLACE INTO info (key, value) VALUES (?1, ?2)")?;
    stmt.execute(params!["CLI_VERSION", &cli_version])?;
  }

  Ok(())
}
