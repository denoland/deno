// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::unsync::sync::AtomicFlag;
use deno_runtime::code_cache::CodeCache;
use deno_runtime::code_cache::CodeCacheType;

use crate::cache::FastInsecureHasher;
use crate::util::path::get_atomic_file_path;
use crate::worker::CliCodeCache;

enum CodeCacheStrategy {
  FirstRun(FirstRunCodeCacheStrategy),
  SubsequentRun(SubsequentRunCodeCacheStrategy),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenoCompileCodeCacheEntry {
  pub source_hash: u64,
  pub data: Vec<u8>,
}

pub struct DenoCompileCodeCache {
  strategy: CodeCacheStrategy,
}

impl DenoCompileCodeCache {
  pub fn new(file_path: PathBuf, cache_key: u64) -> Self {
    // attempt to deserialize the cache data
    match deserialize(&file_path, cache_key) {
      Ok(data) => {
        log::debug!("Loaded {} code cache entries", data.len());
        Self {
          strategy: CodeCacheStrategy::SubsequentRun(
            SubsequentRunCodeCacheStrategy {
              is_finished: AtomicFlag::lowered(),
              data: Mutex::new(data),
            },
          ),
        }
      }
      Err(err) => {
        log::debug!("Failed to deserialize code cache: {:#}", err);
        Self {
          strategy: CodeCacheStrategy::FirstRun(FirstRunCodeCacheStrategy {
            cache_key,
            file_path,
            is_finished: AtomicFlag::lowered(),
            data: Mutex::new(FirstRunCodeCacheData {
              cache: HashMap::new(),
              add_count: 0,
            }),
          }),
        }
      }
    }
  }
}

impl CodeCache for DenoCompileCodeCache {
  fn get_sync(
    &self,
    specifier: &ModuleSpecifier,
    code_cache_type: CodeCacheType,
    source_hash: u64,
  ) -> Option<Vec<u8>> {
    match &self.strategy {
      CodeCacheStrategy::FirstRun(strategy) => {
        if !strategy.is_finished.is_raised() {
          // we keep track of how many times the cache is requested
          // then serialize the cache when we get that number of
          // "set" calls
          strategy.data.lock().add_count += 1;
        }
        None
      }
      CodeCacheStrategy::SubsequentRun(strategy) => {
        if strategy.is_finished.is_raised() {
          return None;
        }
        strategy.take_from_cache(specifier, code_cache_type, source_hash)
      }
    }
  }

  fn set_sync(
    &self,
    specifier: ModuleSpecifier,
    code_cache_type: CodeCacheType,
    source_hash: u64,
    bytes: &[u8],
  ) {
    match &self.strategy {
      CodeCacheStrategy::FirstRun(strategy) => {
        if strategy.is_finished.is_raised() {
          return;
        }

        let data_to_serialize = {
          let mut data = strategy.data.lock();
          data.cache.insert(
            (specifier.to_string(), code_cache_type),
            DenoCompileCodeCacheEntry {
              source_hash,
              data: bytes.to_vec(),
            },
          );
          if data.add_count != 0 {
            data.add_count -= 1;
          }
          if data.add_count == 0 {
            // don't allow using the cache anymore
            strategy.is_finished.raise();
            if data.cache.is_empty() {
              None
            } else {
              Some(std::mem::take(&mut data.cache))
            }
          } else {
            None
          }
        };
        if let Some(cache_data) = &data_to_serialize {
          strategy.write_cache_data(cache_data);
        }
      }
      CodeCacheStrategy::SubsequentRun(_) => {
        // do nothing
      }
    }
  }
}

impl CliCodeCache for DenoCompileCodeCache {
  fn enabled(&self) -> bool {
    match &self.strategy {
      CodeCacheStrategy::FirstRun(strategy) => {
        !strategy.is_finished.is_raised()
      }
      CodeCacheStrategy::SubsequentRun(strategy) => {
        !strategy.is_finished.is_raised()
      }
    }
  }

  fn as_code_cache(self: Arc<Self>) -> Arc<dyn CodeCache> {
    self
  }
}

type CodeCacheKey = (String, CodeCacheType);

struct FirstRunCodeCacheData {
  cache: HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>,
  add_count: usize,
}

struct FirstRunCodeCacheStrategy {
  cache_key: u64,
  file_path: PathBuf,
  is_finished: AtomicFlag,
  data: Mutex<FirstRunCodeCacheData>,
}

impl FirstRunCodeCacheStrategy {
  fn write_cache_data(
    &self,
    cache_data: &HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>,
  ) {
    let count = cache_data.len();
    let temp_file = get_atomic_file_path(&self.file_path);
    match serialize(&temp_file, self.cache_key, cache_data) {
      Ok(()) => {
        if let Err(err) = std::fs::rename(&temp_file, &self.file_path) {
          log::debug!("Failed to rename code cache: {}", err);
        } else {
          log::debug!("Serialized {} code cache entries", count);
        }
      }
      Err(err) => {
        let _ = std::fs::remove_file(&temp_file);
        log::debug!("Failed to serialize code cache: {}", err);
      }
    }
  }
}

struct SubsequentRunCodeCacheStrategy {
  is_finished: AtomicFlag,
  data: Mutex<HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>>,
}

impl SubsequentRunCodeCacheStrategy {
  fn take_from_cache(
    &self,
    specifier: &ModuleSpecifier,
    code_cache_type: CodeCacheType,
    source_hash: u64,
  ) -> Option<Vec<u8>> {
    let mut data = self.data.lock();
    // todo(dsherret): how to avoid the clone here?
    let entry = data.remove(&(specifier.to_string(), code_cache_type))?;
    if entry.source_hash != source_hash {
      return None;
    }
    if data.is_empty() {
      self.is_finished.raise();
    }
    Some(entry.data)
  }
}

/// File format:
/// - <header>
///   - <cache key>
///   - <u32: number of entries>
/// - <[entry length]> - u64 * number of entries
/// - <[entry]>
///   - <[u8]: entry data>
///   - <String: specifier>
///   - <u8>: code cache type
///   - <u32: specifier length>
///   - <u64: source hash>
///   - <u64: entry data hash>
fn serialize(
  file_path: &Path,
  cache_key: u64,
  cache: &HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>,
) -> Result<(), AnyError> {
  let cache_file = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(file_path)?;
  let mut writer = BufWriter::new(cache_file);
  serialize_with_writer(&mut writer, cache_key, cache)
}

fn serialize_with_writer<T: Write>(
  writer: &mut BufWriter<T>,
  cache_key: u64,
  cache: &HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>,
) -> Result<(), AnyError> {
  // header
  writer.write_all(&cache_key.to_le_bytes())?;
  writer.write_all(&(cache.len() as u32).to_le_bytes())?;
  // lengths of each entry
  for ((specifier, _), entry) in cache {
    let len: u64 =
      entry.data.len() as u64 + specifier.len() as u64 + 1 + 4 + 8 + 8;
    writer.write_all(&len.to_le_bytes())?;
  }
  // entries
  for ((specifier, code_cache_type), entry) in cache {
    writer.write_all(&entry.data)?;
    writer.write_all(&[match code_cache_type {
      CodeCacheType::EsModule => 0,
      CodeCacheType::Script => 1,
    }])?;
    writer.write_all(specifier.as_bytes())?;
    writer.write_all(&(specifier.len() as u32).to_le_bytes())?;
    writer.write_all(&entry.source_hash.to_le_bytes())?;
    let hash: u64 = FastInsecureHasher::new_without_deno_version()
      .write(&entry.data)
      .finish();
    writer.write_all(&hash.to_le_bytes())?;
  }

  writer.flush()?;

  Ok(())
}

fn deserialize(
  file_path: &Path,
  expected_cache_key: u64,
) -> Result<HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>, AnyError> {
  let cache_file = std::fs::File::open(file_path)?;
  let mut reader = BufReader::new(cache_file);
  deserialize_with_reader(&mut reader, expected_cache_key)
}

fn deserialize_with_reader<T: Read>(
  reader: &mut BufReader<T>,
  expected_cache_key: u64,
) -> Result<HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>, AnyError> {
  // it's very important to use this below so that a corrupt cache file
  // doesn't cause a memory allocation error
  fn new_vec_sized<T: Clone>(
    capacity: usize,
    default_value: T,
  ) -> Result<Vec<T>, AnyError> {
    let mut vec = Vec::new();
    vec.try_reserve(capacity)?;
    vec.resize(capacity, default_value);
    Ok(vec)
  }

  fn try_subtract(a: usize, b: usize) -> Result<usize, AnyError> {
    if a < b {
      bail!("Integer underflow");
    }
    Ok(a - b)
  }

  let mut header_bytes = vec![0; 8 + 4];
  reader.read_exact(&mut header_bytes)?;
  let actual_cache_key = u64::from_le_bytes(header_bytes[..8].try_into()?);
  if actual_cache_key != expected_cache_key {
    // cache bust
    bail!("Cache key mismatch");
  }
  let len = u32::from_le_bytes(header_bytes[8..].try_into()?) as usize;
  // read the lengths for each entry found in the file
  let entry_len_bytes_capacity = len * 8;
  let mut entry_len_bytes = new_vec_sized(entry_len_bytes_capacity, 0)?;
  reader.read_exact(&mut entry_len_bytes)?;
  let mut lengths = Vec::new();
  lengths.try_reserve(len)?;
  for i in 0..len {
    let pos = i * 8;
    lengths.push(
      u64::from_le_bytes(entry_len_bytes[pos..pos + 8].try_into()?) as usize,
    );
  }

  let mut map = HashMap::new();
  map.try_reserve(len)?;
  for len in lengths {
    let mut buffer = new_vec_sized(len, 0)?;
    reader.read_exact(&mut buffer)?;
    let entry_data_hash_start_pos = try_subtract(buffer.len(), 8)?;
    let expected_entry_data_hash =
      u64::from_le_bytes(buffer[entry_data_hash_start_pos..].try_into()?);
    let source_hash_start_pos = try_subtract(entry_data_hash_start_pos, 8)?;
    let source_hash = u64::from_le_bytes(
      buffer[source_hash_start_pos..entry_data_hash_start_pos].try_into()?,
    );
    let specifier_end_pos = try_subtract(source_hash_start_pos, 4)?;
    let specifier_len = u32::from_le_bytes(
      buffer[specifier_end_pos..source_hash_start_pos].try_into()?,
    ) as usize;
    let specifier_start_pos = try_subtract(specifier_end_pos, specifier_len)?;
    let specifier = String::from_utf8(
      buffer[specifier_start_pos..specifier_end_pos].to_vec(),
    )?;
    let code_cache_type_pos = try_subtract(specifier_start_pos, 1)?;
    let code_cache_type = match buffer[code_cache_type_pos] {
      0 => CodeCacheType::EsModule,
      1 => CodeCacheType::Script,
      _ => bail!("Invalid code cache type"),
    };
    buffer.truncate(code_cache_type_pos);
    let actual_entry_data_hash: u64 =
      FastInsecureHasher::new_without_deno_version()
        .write(&buffer)
        .finish();
    if expected_entry_data_hash != actual_entry_data_hash {
      bail!("Hash mismatch.")
    }
    map.insert(
      (specifier, code_cache_type),
      DenoCompileCodeCacheEntry {
        source_hash,
        data: buffer,
      },
    );
  }

  Ok(map)
}

#[cfg(test)]
mod test {
  use test_util::TempDir;

  use super::*;
  use std::fs::File;

  #[test]
  fn serialize_deserialize() {
    let cache_key = 123456;
    let cache = {
      let mut cache = HashMap::new();
      cache.insert(
        ("specifier1".to_string(), CodeCacheType::EsModule),
        DenoCompileCodeCacheEntry {
          source_hash: 1,
          data: vec![1, 2, 3],
        },
      );
      cache.insert(
        ("specifier2".to_string(), CodeCacheType::EsModule),
        DenoCompileCodeCacheEntry {
          source_hash: 2,
          data: vec![4, 5, 6],
        },
      );
      cache.insert(
        ("specifier2".to_string(), CodeCacheType::Script),
        DenoCompileCodeCacheEntry {
          source_hash: 2,
          data: vec![6, 5, 1],
        },
      );
      cache
    };
    let mut buffer = Vec::new();
    serialize_with_writer(&mut BufWriter::new(&mut buffer), cache_key, &cache)
      .unwrap();
    let deserialized =
      deserialize_with_reader(&mut BufReader::new(&buffer[..]), cache_key)
        .unwrap();
    assert_eq!(cache, deserialized);
  }

  #[test]
  fn serialize_deserialize_empty() {
    let cache_key = 1234;
    let cache = HashMap::new();
    let mut buffer = Vec::new();
    serialize_with_writer(&mut BufWriter::new(&mut buffer), cache_key, &cache)
      .unwrap();
    let deserialized =
      deserialize_with_reader(&mut BufReader::new(&buffer[..]), cache_key)
        .unwrap();
    assert_eq!(cache, deserialized);
  }

  #[test]
  fn serialize_deserialize_corrupt() {
    let buffer = "corrupttestingtestingtesting".as_bytes().to_vec();
    let err = deserialize_with_reader(&mut BufReader::new(&buffer[..]), 1234)
      .unwrap_err();
    assert_eq!(err.to_string(), "Cache key mismatch");
  }

  #[test]
  fn code_cache() {
    let temp_dir = TempDir::new();
    let file_path = temp_dir.path().join("cache.bin").to_path_buf();
    let url1 = ModuleSpecifier::parse("https://deno.land/example1.js").unwrap();
    let url2 = ModuleSpecifier::parse("https://deno.land/example2.js").unwrap();
    // first run
    {
      let code_cache = DenoCompileCodeCache::new(file_path.clone(), 1234);
      assert!(code_cache
        .get_sync(&url1, CodeCacheType::EsModule, 0)
        .is_none());
      assert!(code_cache
        .get_sync(&url2, CodeCacheType::EsModule, 1)
        .is_none());
      assert!(code_cache.enabled());
      code_cache.set_sync(url1.clone(), CodeCacheType::EsModule, 0, &[1, 2, 3]);
      assert!(code_cache.enabled());
      assert!(!file_path.exists());
      code_cache.set_sync(url2.clone(), CodeCacheType::EsModule, 1, &[2, 1, 3]);
      assert!(file_path.exists()); // now the new code cache exists
      assert!(!code_cache.enabled()); // no longer enabled
    }
    // second run
    {
      let code_cache = DenoCompileCodeCache::new(file_path.clone(), 1234);
      assert!(code_cache.enabled());
      let result1 = code_cache
        .get_sync(&url1, CodeCacheType::EsModule, 0)
        .unwrap();
      assert!(code_cache.enabled());
      let result2 = code_cache
        .get_sync(&url2, CodeCacheType::EsModule, 1)
        .unwrap();
      assert!(!code_cache.enabled()); // no longer enabled
      assert_eq!(result1, vec![1, 2, 3]);
      assert_eq!(result2, vec![2, 1, 3]);
    }

    // new cache key first run
    {
      let code_cache = DenoCompileCodeCache::new(file_path.clone(), 54321);
      assert!(code_cache
        .get_sync(&url1, CodeCacheType::EsModule, 0)
        .is_none());
      assert!(code_cache
        .get_sync(&url2, CodeCacheType::EsModule, 1)
        .is_none());
      code_cache.set_sync(url1.clone(), CodeCacheType::EsModule, 0, &[2, 2, 3]);
      code_cache.set_sync(url2.clone(), CodeCacheType::EsModule, 1, &[3, 2, 3]);
    }
    // new cache key second run
    {
      let code_cache = DenoCompileCodeCache::new(file_path.clone(), 54321);
      let result1 = code_cache
        .get_sync(&url1, CodeCacheType::EsModule, 0)
        .unwrap();
      assert_eq!(result1, vec![2, 2, 3]);
      assert!(code_cache
        .get_sync(&url2, CodeCacheType::EsModule, 5) // different hash will cause none
        .is_none());
    }
  }
}
