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
  conn: Arc<Mutex<Connection>>,
}

impl NodeAnalysisCache {
  pub fn new(
    db_file_path: Option<&Path>,
    cli_version: String,
  ) -> Result<Self, AnyError> {
    let conn = match db_file_path {
      Some(path) => Connection::open(path)?,
      None => Connection::open_in_memory()?,
    };
    Self::from_connection(conn, cli_version)
  }

  fn from_connection(
    conn: Connection,
    cli_version: String,
  ) -> Result<Self, AnyError> {
    run_sqlite_pragma(&conn)?;
    create_tables(&conn, cli_version)?;

    Ok(Self {
      conn: Arc::new(Mutex::new(conn)),
    })
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
    let conn = self.conn.lock();
    let query = "
      SELECT
        data
      FROM
        cjsanalysiscache
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
    let conn = self.conn.lock();
    let sql = "
      INSERT OR REPLACE INTO
      cjsanalysiscache (specifier, source_hash, data)
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
    let conn = self.conn.lock();
    let query = "
      SELECT
        data
      FROM
        esmglobalscache
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
    let conn = self.conn.lock();
    let sql = "
      INSERT OR REPLACE INTO
      esmglobalscache (specifier, source_hash, data)
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

fn create_tables(
  conn: &Connection,
  cli_version: String,
) -> Result<(), AnyError> {
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
    "CREATE TABLE IF NOT EXISTS esmglobalscache (
        specifier TEXT PRIMARY KEY,
        source_hash TEXT NOT NULL,
        data TEXT NOT NULL
      )",
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
