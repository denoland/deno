// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use serde::Deserialize;
use serde::Serialize;

use super::DiskCache;
use super::FastInsecureHasher;

#[derive(Debug, Deserialize, Serialize)]
struct EmitMetadata {
  pub source_hash: String,
  pub emit_hash: String,
}

/// The cache that stores previously emitted files.
#[derive(Clone)]
pub struct EmitCache {
  disk_cache: DiskCache,
  cli_version: String,
}

impl EmitCache {
  pub fn new(disk_cache: DiskCache) -> Self {
    Self {
      disk_cache,
      cli_version: crate::version::deno(),
    }
  }

  /// Gets the emitted code with embedded sourcemap from the cache.
  ///
  /// The expected source hash is used in order to verify
  /// that you're getting a value from the cache that is
  /// for the provided source.
  ///
  /// Cached emits from previous CLI releases will not be returned
  /// or emits that do not match the source.
  pub fn get_emit_code(
    &self,
    specifier: &ModuleSpecifier,
    expected_source_hash: Option<u64>,
  ) -> Option<String> {
    let meta_filename = self.get_meta_filename(specifier)?;
    let emit_filename = self.get_emit_filename(specifier)?;

    // load and verify the meta data file is for this source and CLI version
    let bytes = self.disk_cache.get(&meta_filename).ok()?;
    let meta: EmitMetadata = serde_json::from_slice(&bytes).ok()?;
    if let Some(expected_source_hash) = expected_source_hash {
      if meta.source_hash != expected_source_hash.to_string() {
        return None;
      }
    }

    // load and verify the emit is for the meta data
    let emit_bytes = self.disk_cache.get(&emit_filename).ok()?;
    if meta.emit_hash != compute_emit_hash(&emit_bytes, &self.cli_version) {
      return None;
    }

    // everything looks good, return it
    let emit_text = String::from_utf8(emit_bytes).ok()?;
    Some(emit_text)
  }

  /// Gets the filepath which stores the emit.
  pub fn get_emit_filepath(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<PathBuf> {
    Some(
      self
        .disk_cache
        .location
        .join(self.get_emit_filename(specifier)?),
    )
  }

  /// Sets the emit code in the cache.
  pub fn set_emit_code(
    &self,
    specifier: &ModuleSpecifier,
    source_hash: u64,
    code: &str,
  ) {
    if let Err(err) = self.set_emit_code_result(specifier, source_hash, code) {
      // should never error here, but if it ever does don't fail
      if cfg!(debug_assertions) {
        panic!("Error saving emit data ({}): {}", specifier, err);
      } else {
        log::debug!("Error saving emit data({}): {}", specifier, err);
      }
    }
  }

  fn set_emit_code_result(
    &self,
    specifier: &ModuleSpecifier,
    source_hash: u64,
    code: &str,
  ) -> Result<(), AnyError> {
    let meta_filename = self
      .get_meta_filename(specifier)
      .ok_or_else(|| anyhow!("Could not get meta filename."))?;
    let emit_filename = self
      .get_emit_filename(specifier)
      .ok_or_else(|| anyhow!("Could not get emit filename."))?;

    // save the metadata
    let metadata = EmitMetadata {
      source_hash: source_hash.to_string(),
      emit_hash: compute_emit_hash(code.as_bytes(), &self.cli_version),
    };
    self
      .disk_cache
      .set(&meta_filename, &serde_json::to_vec(&metadata)?)?;

    // save the emit source
    self.disk_cache.set(&emit_filename, code.as_bytes())?;

    Ok(())
  }

  fn get_meta_filename(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "meta")
  }

  fn get_emit_filename(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "js")
  }
}

fn compute_emit_hash(bytes: &[u8], cli_version: &str) -> String {
  // it's ok to use an insecure hash here because
  // if someone can change the emit source then they
  // can also change the version hash
  FastInsecureHasher::new()
    .write(bytes)
    // emit should not be re-used between cli versions
    .write(cli_version.as_bytes())
    .finish()
    .to_string()
}

#[cfg(test)]
mod test {
  use test_util::TempDir;

  use super::*;

  #[test]
  pub fn emit_cache_general_use() {
    let temp_dir = TempDir::new();
    let disk_cache = DiskCache::new(temp_dir.path());
    let cache = EmitCache {
      disk_cache: disk_cache.clone(),
      cli_version: "1.0.0".to_string(),
    };

    let specifier1 =
      ModuleSpecifier::from_file_path(temp_dir.path().join("file1.ts"))
        .unwrap();
    let specifier2 =
      ModuleSpecifier::from_file_path(temp_dir.path().join("file2.ts"))
        .unwrap();
    assert_eq!(cache.get_emit_code(&specifier1, Some(1)), None);
    let emit_code1 = "text1".to_string();
    let emit_code2 = "text2".to_string();
    cache.set_emit_code(&specifier1, 10, &emit_code1);
    cache.set_emit_code(&specifier2, 2, &emit_code2);
    // providing the incorrect source hash
    assert_eq!(cache.get_emit_code(&specifier1, Some(5)), None);
    // providing the correct source hash
    assert_eq!(
      cache.get_emit_code(&specifier1, Some(10)),
      Some(emit_code1.clone()),
    );
    assert_eq!(cache.get_emit_code(&specifier2, Some(2)), Some(emit_code2));
    // providing no hash
    assert_eq!(
      cache.get_emit_code(&specifier1, None),
      Some(emit_code1.clone()),
    );

    // try changing the cli version (should not load previous ones)
    let cache = EmitCache {
      disk_cache: disk_cache.clone(),
      cli_version: "2.0.0".to_string(),
    };
    assert_eq!(cache.get_emit_code(&specifier1, Some(10)), None);
    cache.set_emit_code(&specifier1, 5, &emit_code1);

    // recreating the cache should still load the data because the CLI version is the same
    let cache = EmitCache {
      disk_cache,
      cli_version: "2.0.0".to_string(),
    };
    assert_eq!(cache.get_emit_code(&specifier1, Some(5)), Some(emit_code1));

    // adding when already exists should not cause issue
    let emit_code3 = "asdf".to_string();
    cache.set_emit_code(&specifier1, 20, &emit_code3);
    assert_eq!(cache.get_emit_code(&specifier1, Some(5)), None);
    assert_eq!(cache.get_emit_code(&specifier1, Some(20)), Some(emit_code3));
  }
}
