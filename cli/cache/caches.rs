// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use once_cell::sync::OnceCell;

use super::cache_db::CacheDB;
use super::cache_db::CacheDBConfiguration;
use super::check::TYPE_CHECK_CACHE_DB;
use super::incremental::INCREMENTAL_CACHE_DB;
use super::node::NODE_ANALYSIS_CACHE_DB;
use super::parsed_source::PARSED_SOURCE_CACHE_DB;
use super::DenoDir;

#[derive(Clone, Default)]
pub struct Caches {
  fmt_incremental_cache_db: Arc<OnceCell<CacheDB>>,
  lint_incremental_cache_db: Arc<OnceCell<CacheDB>>,
  dep_analysis_db: Arc<OnceCell<CacheDB>>,
  node_analysis_db: Arc<OnceCell<CacheDB>>,
  type_checking_cache_db: Arc<OnceCell<CacheDB>>,
}

impl Caches {
  fn make_db(
    cell: &Arc<OnceCell<CacheDB>>,
    config: &'static CacheDBConfiguration,
    path: PathBuf,
  ) -> CacheDB {
    cell
      .get_or_init(|| CacheDB::from_path(config, path, crate::version::deno()))
      .clone()
  }

  pub fn fmt_incremental_cache_db(&self, dir: &DenoDir) -> CacheDB {
    Self::make_db(
      &self.fmt_incremental_cache_db,
      &INCREMENTAL_CACHE_DB,
      dir.fmt_incremental_cache_db_file_path(),
    )
  }

  pub fn lint_incremental_cache_db(&self, dir: &DenoDir) -> CacheDB {
    Self::make_db(
      &self.lint_incremental_cache_db,
      &INCREMENTAL_CACHE_DB,
      dir.lint_incremental_cache_db_file_path(),
    )
  }

  pub fn dep_analysis_db(&self, dir: &DenoDir) -> CacheDB {
    Self::make_db(
      &self.dep_analysis_db,
      &PARSED_SOURCE_CACHE_DB,
      dir.dep_analysis_db_file_path(),
    )
  }

  pub fn node_analysis_db(&self, dir: &DenoDir) -> CacheDB {
    Self::make_db(
      &self.node_analysis_db,
      &NODE_ANALYSIS_CACHE_DB,
      dir.node_analysis_db_file_path(),
    )
  }

  pub fn type_checking_cache_db(&self, dir: &DenoDir) -> CacheDB {
    Self::make_db(
      &self.type_checking_cache_db,
      &TYPE_CHECK_CACHE_DB,
      dir.type_checking_cache_db_file_path(),
    )
  }
}
