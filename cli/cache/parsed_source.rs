use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_graph::CapturingModuleParser;
use deno_graph::DefaultModuleAnalyzer;
use deno_graph::ModuleInfo;
use deno_graph::ModuleParser;
use deno_graph::ParsedSourceStore;
use deno_runtime::deno_webstorage::rusqlite::params;
use deno_runtime::deno_webstorage::rusqlite::Connection;

use super::common::run_sqlite_pragma;
use super::FastInsecureHasher;

#[derive(Clone, Default)]
struct ParsedSourceCacheSources(
  Arc<Mutex<HashMap<ModuleSpecifier, ParsedSource>>>,
);

/// It's ok that this is racy since in non-LSP situations
/// this will only ever store one form of a parsed source
/// and in LSP settings the concurrency will be enforced
/// at a higher level to ensure this will have the latest
/// parsed source.
impl deno_graph::ParsedSourceStore for ParsedSourceCacheSources {
  fn set_parsed_source(
    &self,
    specifier: deno_graph::ModuleSpecifier,
    parsed_source: ParsedSource,
  ) -> Option<ParsedSource> {
    self.0.lock().insert(specifier, parsed_source)
  }

  fn get_parsed_source(
    &self,
    specifier: &deno_graph::ModuleSpecifier,
  ) -> Option<ParsedSource> {
    self.0.lock().get(specifier).cloned()
  }
}

/// A cache of `ParsedSource`s, which may be used with `deno_graph`
/// for cached dependency analysis.
#[derive(Clone)]
pub struct ParsedSourceCache {
  db_cache_path: Option<PathBuf>,
  cli_version: String,
  sources: ParsedSourceCacheSources,
}

impl ParsedSourceCache {
  pub fn new(sql_cache_path: Option<PathBuf>) -> Self {
    Self {
      db_cache_path: sql_cache_path,
      cli_version: crate::version::deno(),
      sources: Default::default(),
    }
  }

  pub fn get_parsed_source_from_module(
    &self,
    module: &deno_graph::Module,
  ) -> Result<Option<ParsedSource>, AnyError> {
    if let Some(source) = &module.maybe_source {
      Ok(Some(self.get_or_parse_module(
        &module.specifier,
        source.clone(),
        module.media_type,
      )?))
    } else {
      Ok(None)
    }
  }

  /// Gets the matching `ParsedSource` from the cache
  /// or parses a new one and stores that in the cache.
  pub fn get_or_parse_module(
    &self,
    specifier: &deno_graph::ModuleSpecifier,
    source: Arc<str>,
    media_type: MediaType,
  ) -> deno_core::anyhow::Result<ParsedSource, deno_ast::Diagnostic> {
    let parser = self.as_capturing_parser();
    // this will conditionally parse because it's using a CapturingModuleParser
    parser.parse_module(specifier, source, media_type)
  }

  /// Frees the parsed source from memory.
  pub fn free(&self, specifier: &ModuleSpecifier) {
    self.sources.0.lock().remove(specifier);
  }

  /// Gets this cache as a `deno_graph::ParsedSourceStore`.
  pub fn as_store(&self) -> Box<dyn ParsedSourceStore> {
    // This trait is not implemented directly on ParsedSourceCache
    // in order to prevent its methods from being accidentally used.
    // Generally, people should prefer the methods found that will
    // lazily parse if necessary.
    Box::new(self.sources.clone())
  }

  pub fn as_analyzer(&self) -> Box<dyn deno_graph::ModuleAnalyzer> {
    match ParsedSourceCacheModuleAnalyzer::new(
      self.db_cache_path.as_deref(),
      self.cli_version.clone(),
      self.sources.clone(),
    ) {
      Ok(analyzer) => Box::new(analyzer),
      Err(err) => {
        log::debug!("Could not create cached module analyzer. {:#}", err);
        // fallback to not caching if it can't be created
        Box::new(deno_graph::CapturingModuleAnalyzer::new(
          None,
          Some(self.as_store()),
        ))
      }
    }
  }

  /// Creates a parser that will reuse a ParsedSource from the store
  /// if it exists, or else parse.
  pub fn as_capturing_parser(&self) -> CapturingModuleParser {
    CapturingModuleParser::new(None, &self.sources)
  }
}

struct ParsedSourceCacheModuleAnalyzer {
  conn: Connection,
  sources: ParsedSourceCacheSources,
}

impl ParsedSourceCacheModuleAnalyzer {
  pub fn new(
    db_file_path: Option<&Path>,
    cli_version: String,
    sources: ParsedSourceCacheSources,
  ) -> Result<Self, AnyError> {
    log::debug!("Loading cached module analyzer.");
    let conn = match db_file_path {
      Some(path) => Connection::open(path)?,
      None => Connection::open_in_memory()?,
    };
    Self::from_connection(conn, cli_version, sources)
  }

  fn from_connection(
    conn: Connection,
    cli_version: String,
    sources: ParsedSourceCacheSources,
  ) -> Result<Self, AnyError> {
    run_sqlite_pragma(&conn)?;
    create_tables(&conn, cli_version)?;

    Ok(Self { conn, sources })
  }

  pub fn get_module_info(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    expected_source_hash: &str,
  ) -> Result<Option<ModuleInfo>, AnyError> {
    let query = "
      SELECT
        module_info
      FROM
        moduleinfocache
      WHERE
        specifier=?1
        AND media_type=?2
        AND source_hash=?3
      LIMIT 1";
    let mut stmt = self.conn.prepare_cached(query)?;
    let mut rows = stmt.query(params![
      &specifier.as_str(),
      &media_type.to_string(),
      &expected_source_hash,
    ])?;
    if let Some(row) = rows.next()? {
      let module_info: String = row.get(0)?;
      let module_info = serde_json::from_str(&module_info)?;
      Ok(Some(module_info))
    } else {
      Ok(None)
    }
  }

  pub fn set_module_info(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source_hash: &str,
    module_info: &ModuleInfo,
  ) -> Result<(), AnyError> {
    let sql = "
      INSERT OR REPLACE INTO
        moduleinfocache (specifier, media_type, source_hash, module_info)
      VALUES
        (?1, ?2, ?3, ?4)";
    let mut stmt = self.conn.prepare_cached(sql)?;
    stmt.execute(params![
      specifier.as_str(),
      &media_type.to_string(),
      &source_hash.to_string(),
      &serde_json::to_string(&module_info)?,
    ])?;
    Ok(())
  }
}

impl deno_graph::ModuleAnalyzer for ParsedSourceCacheModuleAnalyzer {
  fn analyze(
    &self,
    specifier: &ModuleSpecifier,
    source: Arc<str>,
    media_type: MediaType,
  ) -> Result<ModuleInfo, deno_ast::Diagnostic> {
    // attempt to load from the cache
    let source_hash = compute_source_hash(source.as_bytes());
    match self.get_module_info(specifier, media_type, &source_hash) {
      Ok(Some(info)) => return Ok(info),
      Ok(None) => {}
      Err(err) => {
        log::debug!(
          "Error loading module cache info for {}. {:#}",
          specifier,
          err
        );
      }
    }

    // otherwise, get the module info from the parsed source cache
    let parser = CapturingModuleParser::new(None, &self.sources);
    let analyzer = DefaultModuleAnalyzer::new(&parser);

    let module_info = analyzer.analyze(specifier, source, media_type)?;

    // then attempt to cache it
    if let Err(err) =
      self.set_module_info(specifier, media_type, &source_hash, &module_info)
    {
      log::debug!(
        "Error saving module cache info for {}. {:#}",
        specifier,
        err
      );
    }

    Ok(module_info)
  }
}

fn create_tables(
  conn: &Connection,
  cli_version: String,
) -> Result<(), AnyError> {
  // INT doesn't store up to u64, so use TEXT for source_hash
  conn.execute(
    "CREATE TABLE IF NOT EXISTS moduleinfocache (
        specifier TEXT PRIMARY KEY,
        media_type TEXT NOT NULL,
        source_hash TEXT NOT NULL,
        module_info TEXT NOT NULL
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
    conn.execute("DELETE FROM moduleinfocache", params![])?;
    let mut stmt = conn
      .prepare("INSERT OR REPLACE INTO info (key, value) VALUES (?1, ?2)")?;
    stmt.execute(params!["CLI_VERSION", &cli_version])?;
  }

  Ok(())
}

fn compute_source_hash(bytes: &[u8]) -> String {
  FastInsecureHasher::new().write(bytes).finish().to_string()
}

#[cfg(test)]
mod test {
  use deno_graph::PositionRange;
  use deno_graph::SpecifierWithRange;

  use super::*;

  #[test]
  pub fn parsed_source_cache_module_analyzer_general_use() {
    let conn = Connection::open_in_memory().unwrap();
    let cache = ParsedSourceCacheModuleAnalyzer::from_connection(
      conn,
      "1.0.0".to_string(),
      Default::default(),
    )
    .unwrap();
    let specifier1 =
      ModuleSpecifier::parse("https://localhost/mod.ts").unwrap();
    let specifier2 =
      ModuleSpecifier::parse("https://localhost/mod2.ts").unwrap();
    assert_eq!(
      cache
        .get_module_info(&specifier1, MediaType::JavaScript, "1")
        .unwrap(),
      None
    );

    let mut module_info = ModuleInfo::default();
    module_info.jsdoc_imports.push(SpecifierWithRange {
      range: PositionRange {
        start: deno_graph::Position {
          line: 0,
          character: 3,
        },
        end: deno_graph::Position {
          line: 1,
          character: 2,
        },
      },
      text: "test".to_string(),
    });
    cache
      .set_module_info(&specifier1, MediaType::JavaScript, "1", &module_info)
      .unwrap();
    assert_eq!(
      cache
        .get_module_info(&specifier1, MediaType::JavaScript, "1")
        .unwrap(),
      Some(module_info.clone())
    );
    assert_eq!(
      cache
        .get_module_info(&specifier2, MediaType::JavaScript, "1")
        .unwrap(),
      None,
    );
    // different media type
    assert_eq!(
      cache
        .get_module_info(&specifier1, MediaType::TypeScript, "1")
        .unwrap(),
      None,
    );
    // different source hash
    assert_eq!(
      cache
        .get_module_info(&specifier1, MediaType::JavaScript, "2")
        .unwrap(),
      None,
    );

    // try recreating with the same version
    let conn = cache.conn;
    let cache = ParsedSourceCacheModuleAnalyzer::from_connection(
      conn,
      "1.0.0".to_string(),
      Default::default(),
    )
    .unwrap();

    // should get it
    assert_eq!(
      cache
        .get_module_info(&specifier1, MediaType::JavaScript, "1")
        .unwrap(),
      Some(module_info)
    );

    // try recreating with a different version
    let conn = cache.conn;
    let cache = ParsedSourceCacheModuleAnalyzer::from_connection(
      conn,
      "1.0.1".to_string(),
      Default::default(),
    )
    .unwrap();

    // should no longer exist
    assert_eq!(
      cache
        .get_module_info(&specifier1, MediaType::JavaScript, "1")
        .unwrap(),
      None,
    );
  }
}
