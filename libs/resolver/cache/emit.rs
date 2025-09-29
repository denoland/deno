// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::PathBuf;

use anyhow::Error as AnyError;
use anyhow::anyhow;
use deno_unsync::sync::AtomicFlag;
use url::Url;

use super::DiskCache;
use super::DiskCacheSys;

#[allow(clippy::disallowed_types)]
pub type EmitCacheRc<TSys> = deno_maybe_sync::MaybeArc<EmitCache<TSys>>;

#[sys_traits::auto_impl]
pub trait EmitCacheSys: DiskCacheSys + sys_traits::EnvVar {}

/// The cache that stores previously emitted files.
#[derive(Debug)]
pub struct EmitCache<TSys: EmitCacheSys> {
  disk_cache: DiskCache<TSys>,
  emit_failed_flag: AtomicFlag,
  file_serializer: EmitFileSerializer,
  mode: Mode,
}

#[derive(Debug)]
enum Mode {
  Normal,
  Disable,
}

impl<TSys: EmitCacheSys> EmitCache<TSys> {
  pub fn new(
    sys: &TSys,
    disk_cache: DiskCache<TSys>,
    cache_version: Cow<'static, str>,
  ) -> Self {
    let emit_cache_mode = sys.env_var_os("DENO_EMIT_CACHE_MODE");
    let mode = match emit_cache_mode.as_ref().and_then(|s| s.to_str()) {
      Some("normal") | Some("") | None => Mode::Normal,
      Some("disable") => Mode::Disable,
      _ => {
        log::warn!("Unknown DENO_EMIT_CACHE_MODE value, defaulting to normal");
        Mode::Normal
      }
    };

    Self {
      disk_cache,
      emit_failed_flag: Default::default(),
      file_serializer: EmitFileSerializer { cache_version },
      mode,
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
    specifier: &Url,
    expected_source_hash: u64,
  ) -> Option<String> {
    if matches!(self.mode, Mode::Disable) {
      return None;
    }

    let emit_filename = self.get_emit_filename(specifier)?;
    let bytes = self.disk_cache.get(&emit_filename).ok()?;
    self
      .file_serializer
      .deserialize(bytes, expected_source_hash)
  }

  /// Sets the emit code in the cache.
  pub fn set_emit_code(&self, specifier: &Url, source_hash: u64, code: &[u8]) {
    if let Err(err) = self.set_emit_code_result(specifier, source_hash, code) {
      // might error in cases such as a readonly file system
      log::debug!("Error saving emit data ({}): {}", specifier, err);
      // assume the cache can't be written to and disable caching to it
      self.emit_failed_flag.raise();
    }
  }

  fn set_emit_code_result(
    &self,
    specifier: &Url,
    source_hash: u64,
    code: &[u8],
  ) -> Result<(), AnyError> {
    if matches!(self.mode, Mode::Disable) || self.emit_failed_flag.is_raised() {
      log::debug!("Skipped emit cache save of {}", specifier);
      return Ok(());
    }

    let emit_filename = self
      .get_emit_filename(specifier)
      .ok_or_else(|| anyhow!("Could not get emit filename."))?;
    let cache_data = self.file_serializer.serialize(code, source_hash);
    self.disk_cache.set(&emit_filename, &cache_data)?;

    Ok(())
  }

  fn get_emit_filename(&self, specifier: &Url) -> Option<PathBuf> {
    self
      .disk_cache
      .get_cache_filename_with_extension(specifier, "js")
  }
}

const LAST_LINE_PREFIX: &str = "\n// denoCacheMetadata=";

#[derive(Debug)]
struct EmitFileSerializer {
  cache_version: Cow<'static, str>,
}

impl EmitFileSerializer {
  pub fn deserialize(
    &self,
    mut bytes: Vec<u8>,
    expected_source_hash: u64,
  ) -> Option<String> {
    let last_newline_index = bytes.iter().rposition(|&b| b == b'\n')?;
    let (content, last_line) = bytes.split_at(last_newline_index);
    let hashes = last_line.strip_prefix(LAST_LINE_PREFIX.as_bytes())?;
    let hashes = String::from_utf8_lossy(hashes);
    let (source_hash, emit_hash) = hashes.split_once(',')?;

    // verify the meta data file is for this source and CLI version
    let source_hash = source_hash.parse::<u64>().ok()?;
    if source_hash != expected_source_hash {
      return None;
    }
    let emit_hash = emit_hash.parse::<u64>().ok()?;
    // prevent using an emit from a different cli version or emits that were tampered with
    if emit_hash != self.compute_emit_hash(content) {
      return None;
    }

    // everything looks good, truncate and return it
    bytes.truncate(content.len());
    String::from_utf8(bytes).ok()
  }

  pub fn serialize(&self, code: &[u8], source_hash: u64) -> Vec<u8> {
    let source_hash = source_hash.to_string();
    let emit_hash = self.compute_emit_hash(code).to_string();
    let capacity = code.len()
      + LAST_LINE_PREFIX.len()
      + source_hash.len()
      + 1
      + emit_hash.len();
    let mut cache_data = Vec::with_capacity(capacity);
    cache_data.extend(code);
    cache_data.extend(LAST_LINE_PREFIX.as_bytes());
    cache_data.extend(source_hash.as_bytes());
    cache_data.push(b',');
    cache_data.extend(emit_hash.as_bytes());
    debug_assert_eq!(cache_data.len(), capacity);
    cache_data
  }

  fn compute_emit_hash(&self, bytes: &[u8]) -> u64 {
    // it's ok to use an insecure hash here because
    // if someone can change the emit source then they
    // can also change the version hash
    let mut hasher = twox_hash::XxHash64::default();
    bytes.hash(&mut hasher);
    // emit should not be re-used between cli versions
    self.cache_version.hash(&mut hasher);
    hasher.finish()
  }
}

#[cfg(test)]
mod test {
  use test_util::TempDir;

  use super::*;

  #[test]
  pub fn emit_cache_general_use() {
    let temp_dir = TempDir::new();
    let disk_cache =
      DiskCache::new(sys_traits::impls::RealSys, temp_dir.path().to_path_buf());
    let cache = EmitCache {
      disk_cache: disk_cache.clone(),
      file_serializer: EmitFileSerializer {
        cache_version: "1.0.0".into(),
      },
      emit_failed_flag: Default::default(),
      mode: Mode::Normal,
    };

    let specifier1 = deno_path_util::url_from_file_path(
      temp_dir.path().join("file1.ts").as_path(),
    )
    .unwrap();
    let specifier2 = deno_path_util::url_from_file_path(
      temp_dir.path().join("file2.ts").as_path(),
    )
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
      cache.get_emit_code(&specifier1, 10),
      Some(emit_code1.clone()),
    );
    assert_eq!(cache.get_emit_code(&specifier2, 2), Some(emit_code2));

    // try changing the cli version (should not load previous ones)
    let cache = EmitCache {
      disk_cache: disk_cache.clone(),
      file_serializer: EmitFileSerializer {
        cache_version: "2.0.0".into(),
      },
      emit_failed_flag: Default::default(),
      mode: Mode::Normal,
    };
    assert_eq!(cache.get_emit_code(&specifier1, 10), None);
    cache.set_emit_code(&specifier1, 5, emit_code1.as_bytes());

    // recreating the cache should still load the data because the CLI version is the same
    let cache = EmitCache {
      disk_cache,
      file_serializer: EmitFileSerializer {
        cache_version: "2.0.0".into(),
      },
      emit_failed_flag: Default::default(),
      mode: Mode::Normal,
    };
    assert_eq!(cache.get_emit_code(&specifier1, 5), Some(emit_code1));

    // adding when already exists should not cause issue
    let emit_code3 = "asdf".to_string();
    cache.set_emit_code(&specifier1, 20, emit_code3.as_bytes());
    assert_eq!(cache.get_emit_code(&specifier1, 5), None);
    assert_eq!(cache.get_emit_code(&specifier1, 20), Some(emit_code3));
  }
}
