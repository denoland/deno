// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_graph::ModuleInfo;
use deno_graph::ParserModuleAnalyzer;
use deno_runtime::deno_webstorage::rusqlite::params;

use super::cache_db::CacheDB;
use super::cache_db::CacheDBConfiguration;
use super::cache_db::CacheDBHash;
use super::cache_db::CacheFailure;
use super::ParsedSourceCache;

const SELECT_MODULE_INFO: &str = "
SELECT
  module_info
FROM
  moduleinfocache
WHERE
  specifier=?1
  AND media_type=?2
  AND source_hash=?3
LIMIT 1";

pub static MODULE_INFO_CACHE_DB: CacheDBConfiguration = CacheDBConfiguration {
  table_initializer: concat!(
    "CREATE TABLE IF NOT EXISTS moduleinfocache (",
    "specifier TEXT PRIMARY KEY,",
    "media_type INTEGER NOT NULL,",
    "source_hash INTEGER NOT NULL,",
    "module_info TEXT NOT NULL",
    ");"
  ),
  on_version_change: "DELETE FROM moduleinfocache;",
  preheat_queries: &[SELECT_MODULE_INFO],
  on_failure: CacheFailure::InMemory,
};

/// A cache of `deno_graph::ModuleInfo` objects. Using this leads to a considerable
/// performance improvement because when it exists we can skip parsing a module for
/// deno_graph.
#[derive(Debug)]
pub struct ModuleInfoCache {
  conn: CacheDB,
  parsed_source_cache: Arc<ParsedSourceCache>,
}

impl ModuleInfoCache {
  #[cfg(test)]
  pub fn new_in_memory(
    version: &'static str,
    parsed_source_cache: Arc<ParsedSourceCache>,
  ) -> Self {
    Self::new(
      CacheDB::in_memory(&MODULE_INFO_CACHE_DB, version),
      parsed_source_cache,
    )
  }

  pub fn new(
    conn: CacheDB,
    parsed_source_cache: Arc<ParsedSourceCache>,
  ) -> Self {
    Self {
      conn,
      parsed_source_cache,
    }
  }

  /// Useful for testing: re-create this cache DB with a different current version.
  #[cfg(test)]
  pub(crate) fn recreate_with_version(self, version: &'static str) -> Self {
    Self {
      conn: self.conn.recreate_with_version(version),
      parsed_source_cache: self.parsed_source_cache,
    }
  }

  pub fn get_module_info(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    expected_source_hash: CacheDBHash,
  ) -> Result<Option<ModuleInfo>, AnyError> {
    let query = SELECT_MODULE_INFO;
    let res = self.conn.query_row(
      query,
      params![
        &specifier.as_str(),
        serialize_media_type(media_type),
        expected_source_hash,
      ],
      |row| {
        let module_info: String = row.get(0)?;
        let module_info = serde_json::from_str(&module_info)?;
        Ok(module_info)
      },
    )?;
    Ok(res)
  }

  pub fn set_module_info(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source_hash: CacheDBHash,
    module_info: &ModuleInfo,
  ) -> Result<(), AnyError> {
    let sql = "
      INSERT OR REPLACE INTO
        moduleinfocache (specifier, media_type, source_hash, module_info)
      VALUES
        (?1, ?2, ?3, ?4)";
    self.conn.execute(
      sql,
      params![
        specifier.as_str(),
        serialize_media_type(media_type),
        source_hash,
        &serde_json::to_string(&module_info)?,
      ],
    )?;
    Ok(())
  }

  pub fn as_module_analyzer(&self) -> ModuleInfoCacheModuleAnalyzer {
    ModuleInfoCacheModuleAnalyzer {
      module_info_cache: self,
      parsed_source_cache: &self.parsed_source_cache,
    }
  }
}

pub struct ModuleInfoCacheModuleAnalyzer<'a> {
  module_info_cache: &'a ModuleInfoCache,
  parsed_source_cache: &'a Arc<ParsedSourceCache>,
}

impl ModuleInfoCacheModuleAnalyzer<'_> {
  fn load_cached_module_info(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source_hash: CacheDBHash,
  ) -> Option<ModuleInfo> {
    match self.module_info_cache.get_module_info(
      specifier,
      media_type,
      source_hash,
    ) {
      Ok(Some(info)) => Some(info),
      Ok(None) => None,
      Err(err) => {
        log::debug!(
          "Error loading module cache info for {}. {:#}",
          specifier,
          err
        );
        None
      }
    }
  }

  fn save_module_info_to_cache(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source_hash: CacheDBHash,
    module_info: &ModuleInfo,
  ) {
    if let Err(err) = self.module_info_cache.set_module_info(
      specifier,
      media_type,
      source_hash,
      module_info,
    ) {
      log::debug!(
        "Error saving module cache info for {}. {:#}",
        specifier,
        err
      );
    }
  }

  pub fn analyze_sync(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source: &Arc<str>,
  ) -> Result<ModuleInfo, deno_ast::ParseDiagnostic> {
    // attempt to load from the cache
    let source_hash = CacheDBHash::from_hashable(source);
    if let Some(info) =
      self.load_cached_module_info(specifier, media_type, source_hash)
    {
      return Ok(info);
    }

    // otherwise, get the module info from the parsed source cache
    let parser = self.parsed_source_cache.as_capturing_parser();
    let analyzer = ParserModuleAnalyzer::new(&parser);
    let module_info =
      analyzer.analyze_sync(specifier, source.clone(), media_type)?;

    // then attempt to cache it
    self.save_module_info_to_cache(
      specifier,
      media_type,
      source_hash,
      &module_info,
    );

    Ok(module_info)
  }
}

#[async_trait::async_trait(?Send)]
impl deno_graph::ModuleAnalyzer for ModuleInfoCacheModuleAnalyzer<'_> {
  async fn analyze(
    &self,
    specifier: &ModuleSpecifier,
    source: Arc<str>,
    media_type: MediaType,
  ) -> Result<ModuleInfo, deno_ast::ParseDiagnostic> {
    // attempt to load from the cache
    let source_hash = CacheDBHash::from_hashable(&source);
    if let Some(info) =
      self.load_cached_module_info(specifier, media_type, source_hash)
    {
      return Ok(info);
    }

    // otherwise, get the module info from the parsed source cache
    let module_info = deno_core::unsync::spawn_blocking({
      let cache = self.parsed_source_cache.clone();
      let specifier = specifier.clone();
      move || {
        let parser = cache.as_capturing_parser();
        let analyzer = ParserModuleAnalyzer::new(&parser);
        analyzer.analyze_sync(&specifier, source, media_type)
      }
    })
    .await
    .unwrap()?;

    // then attempt to cache it
    self.save_module_info_to_cache(
      specifier,
      media_type,
      source_hash,
      &module_info,
    );

    Ok(module_info)
  }
}

// note: there is no deserialize for this because this is only ever
// saved in the db and then used for comparisons
fn serialize_media_type(media_type: MediaType) -> i64 {
  use MediaType::*;
  match media_type {
    JavaScript => 1,
    Jsx => 2,
    Mjs => 3,
    Cjs => 4,
    TypeScript => 5,
    Mts => 6,
    Cts => 7,
    Dts => 8,
    Dmts => 9,
    Dcts => 10,
    Tsx => 11,
    Json => 12,
    Wasm => 13,
    Css => 14,
    Html => 15,
    SourceMap => 16,
    Sql => 17,
    Unknown => 18,
  }
}

#[cfg(test)]
mod test {
  use deno_graph::JsDocImportInfo;
  use deno_graph::PositionRange;
  use deno_graph::SpecifierWithRange;

  use super::*;

  #[test]
  pub fn module_info_cache_general_use() {
    let cache = ModuleInfoCache::new_in_memory("1.0.0", Default::default());
    let specifier1 =
      ModuleSpecifier::parse("https://localhost/mod.ts").unwrap();
    let specifier2 =
      ModuleSpecifier::parse("https://localhost/mod2.ts").unwrap();
    assert_eq!(
      cache
        .get_module_info(
          &specifier1,
          MediaType::JavaScript,
          CacheDBHash::new(1)
        )
        .unwrap(),
      None
    );

    let mut module_info = ModuleInfo::default();
    module_info.jsdoc_imports.push(JsDocImportInfo {
      specifier: SpecifierWithRange {
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
      },
      resolution_mode: None,
    });
    cache
      .set_module_info(
        &specifier1,
        MediaType::JavaScript,
        CacheDBHash::new(1),
        &module_info,
      )
      .unwrap();
    assert_eq!(
      cache
        .get_module_info(
          &specifier1,
          MediaType::JavaScript,
          CacheDBHash::new(1)
        )
        .unwrap(),
      Some(module_info.clone())
    );
    assert_eq!(
      cache
        .get_module_info(
          &specifier2,
          MediaType::JavaScript,
          CacheDBHash::new(1)
        )
        .unwrap(),
      None,
    );
    // different media type
    assert_eq!(
      cache
        .get_module_info(
          &specifier1,
          MediaType::TypeScript,
          CacheDBHash::new(1)
        )
        .unwrap(),
      None,
    );
    // different source hash
    assert_eq!(
      cache
        .get_module_info(
          &specifier1,
          MediaType::JavaScript,
          CacheDBHash::new(2)
        )
        .unwrap(),
      None,
    );

    // try recreating with the same version
    let cache = cache.recreate_with_version("1.0.0");

    // should get it
    assert_eq!(
      cache
        .get_module_info(
          &specifier1,
          MediaType::JavaScript,
          CacheDBHash::new(1)
        )
        .unwrap(),
      Some(module_info)
    );

    // try recreating with a different version
    let cache = cache.recreate_with_version("1.0.1");

    // should no longer exist
    assert_eq!(
      cache
        .get_module_info(
          &specifier1,
          MediaType::JavaScript,
          CacheDBHash::new(1)
        )
        .unwrap(),
      None,
    );
  }
}
