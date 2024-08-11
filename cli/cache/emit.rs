// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::unsync::sync::AtomicFlag;

use super::DiskCache;
use super::FastInsecureHasher;

/// The cache that stores previously emitted files.
pub struct EmitCache {
  disk_cache: DiskCache,
  cli_version: &'static str,
  emit_failed_flag: AtomicFlag,
}

impl EmitCache {
  pub fn new(disk_cache: DiskCache) -> Self {
    Self {
      disk_cache,
      cli_version: crate::version::deno(),
      emit_failed_flag: Default::default(),
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
    expected_source_hash: u64,
  ) -> Option<Vec<u8>> {
    let emit_filename = self.get_emit_filename(specifier)?;

    // load and verify the meta data file is for this source and CLI version
    let mut bytes = self.disk_cache.get(&emit_filename).ok()?;
    if bytes.len() < 16 {
      return None;
    }
    let mut u64_buf: [u8; 8] = [0; 8];
    u64_buf.copy_from_slice(&bytes[bytes.len() - 16..bytes.len() - 8]);
    let source_hash = u64::from_le_bytes(u64_buf);
    u64_buf.copy_from_slice(&bytes[bytes.len() - 8..]);
    let emit_hash = u64::from_le_bytes(u64_buf);
    if source_hash != expected_source_hash {
      return None;
    }
    bytes.truncate(bytes.len() - 16);
    let emit_bytes = bytes;
    // prevent using an emit from a different cli version or emits that were tampered with
    if emit_hash != compute_emit_hash(&emit_bytes, self.cli_version) {
      return None;
    }

    // everything looks good, return it
    Some(emit_bytes)
  }

  /// Sets the emit code in the cache.
  pub fn set_emit_code(
    &self,
    specifier: &ModuleSpecifier,
    source_hash: u64,
    code: &[u8],
  ) {
    if let Err(err) = self.set_emit_code_result(specifier, source_hash, code) {
      // might error in cases such as a readonly file system
      log::debug!("Error saving emit data ({}): {}", specifier, err);
      // assume the cache can't be written to and disable caching to it
      self.emit_failed_flag.raise();
    }
  }

  fn set_emit_code_result(
    &self,
    specifier: &ModuleSpecifier,
    source_hash: u64,
    code: &[u8],
  ) -> Result<(), AnyError> {
    if self.emit_failed_flag.is_raised() {
      log::debug!("Skipped emit cache save of {}", specifier);
      return Ok(());
    }

    let emit_filename = self
      .get_emit_filename(specifier)
      .ok_or_else(|| anyhow!("Could not get emit filename."))?;

    // we store the hashes at the end of the file so we can truncate them off when loading
    let capacity = code.len() + 8 * 2;
    let mut cache_data = Vec::with_capacity(capacity);
    cache_data.extend(code);
    cache_data.extend(source_hash.to_le_bytes());
    cache_data.extend(compute_emit_hash(code, self.cli_version).to_le_bytes());
    debug_assert_eq!(cache_data.len(), capacity);
    self.disk_cache.set(&emit_filename, &cache_data)?;

    Ok(())
  }

  fn get_emit_filename(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "bin")
  }
}

fn compute_emit_hash(bytes: &[u8], cli_version: &str) -> u64 {
  // it's ok to use an insecure hash here because
  // if someone can change the emit source then they
  // can also change the version hash
  FastInsecureHasher::new_without_deno_version() // use cli_version param instead
    .write(bytes)
    // emit should not be re-used between cli versions
    .write_str(cli_version)
    .finish()
}

#[cfg(test)]
mod test {
  use test_util::TempDir;

  use super::*;

  #[test]
  pub fn emit_cache_general_use() {
    let temp_dir = TempDir::new();
    let disk_cache = DiskCache::new(temp_dir.path().as_path());
    let cache = EmitCache {
      disk_cache: disk_cache.clone(),
      cli_version: "1.0.0",
      emit_failed_flag: Default::default(),
    };
    let to_string =
      |bytes: Vec<u8>| -> String { String::from_utf8(bytes).unwrap() };

    let specifier1 =
      ModuleSpecifier::from_file_path(temp_dir.path().join("file1.ts"))
        .unwrap();
    let specifier2 =
      ModuleSpecifier::from_file_path(temp_dir.path().join("file2.ts"))
        .unwrap();
    assert_eq!(cache.get_emit_code(&specifier1, 1), None);
    let emit_code1 = "text1".to_string();
    let emit_code2 = "text2".to_string();
    cache.set_emit_code(&specifier1, 10, emit_code1.as_bytes());
    cache.set_emit_code(&specifier2, 2, emit_code2.as_bytes());
    // providing the incorrect source hash
    assert_eq!(cache.get_emit_code(&specifier1, 5), None);
    // providing the correct source hash
    assert_eq!(
      cache.get_emit_code(&specifier1, 10).map(to_string),
      Some(emit_code1.clone()),
    );
    assert_eq!(
      cache.get_emit_code(&specifier2, 2).map(to_string),
      Some(emit_code2)
    );

    // try changing the cli version (should not load previous ones)
    let cache = EmitCache {
      disk_cache: disk_cache.clone(),
      cli_version: "2.0.0",
      emit_failed_flag: Default::default(),
    };
    assert_eq!(cache.get_emit_code(&specifier1, 10), None);
    cache.set_emit_code(&specifier1, 5, emit_code1.as_bytes());

    // recreating the cache should still load the data because the CLI version is the same
    let cache = EmitCache {
      disk_cache,
      cli_version: "2.0.0",
      emit_failed_flag: Default::default(),
    };
    assert_eq!(
      cache.get_emit_code(&specifier1, 5).map(to_string),
      Some(emit_code1)
    );

    // adding when already exists should not cause issue
    let emit_code3 = "asdf".to_string();
    cache.set_emit_code(&specifier1, 20, emit_code3.as_bytes());
    assert_eq!(cache.get_emit_code(&specifier1, 5), None);
    assert_eq!(
      cache.get_emit_code(&specifier1, 20).map(to_string),
      Some(emit_code3)
    );
  }
}
