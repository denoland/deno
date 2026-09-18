// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::unsync::sync::AtomicFlag;
use deno_lib::util::hash::FastInsecureHasher;
use deno_path_util::get_atomic_path;
use deno_runtime::code_cache::CodeCache;
use deno_runtime::code_cache::CodeCacheType;
use url::Url;

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
  specifier_base: Option<String>,
}

impl DenoCompileCodeCache {
  pub fn new(
    file_path: PathBuf,
    cache_key: u64,
    embedded_data: Option<&[u8]>,
    specifier_base: Option<&Url>,
  ) -> Self {
    if let Some(embedded_data) = embedded_data {
      match deserialize_bytes(embedded_data, cache_key) {
        Ok(data) => {
          log::debug!("Loaded {} embedded code cache entries", data.len());
          return Self::subsequent_run(data, specifier_base);
        }
        Err(err) => {
          log::debug!("Failed to deserialize embedded code cache: {:#}", err);
        }
      }
    }

    // attempt to deserialize the cache data
    match deserialize(&file_path, cache_key) {
      Ok(data) => {
        log::debug!(
          "Loaded {} code cache entries from {}",
          data.len(),
          file_path.display()
        );
        Self::subsequent_run(data, specifier_base)
      }
      Err(err) => {
        log::debug!(
          "Failed to deserialize code cache from {}: {:#}",
          file_path.display(),
          err
        );
        Self::first_run(file_path, cache_key, specifier_base)
      }
    }
  }

  fn first_run(
    file_path: PathBuf,
    cache_key: u64,
    specifier_base: Option<&Url>,
  ) -> Self {
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
      specifier_base: specifier_base.map(|url| url.as_str().to_string()),
    }
  }

  fn subsequent_run(
    data: HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>,
    specifier_base: Option<&Url>,
  ) -> Self {
    Self {
      strategy: CodeCacheStrategy::SubsequentRun(
        SubsequentRunCodeCacheStrategy {
          is_finished: AtomicFlag::lowered(),
          data: Mutex::new(data),
        },
      ),
      specifier_base: specifier_base.map(|url| url.as_str().to_string()),
    }
  }

  fn specifier_key(&self, specifier: &Url) -> String {
    self
      .specifier_base
      .as_deref()
      .and_then(|base| specifier.as_str().strip_prefix(base))
      .map(|relative| format!("deno-compile-internal:///{}", relative))
      .unwrap_or_else(|| specifier.to_string())
  }

  pub fn for_deno_core(self: Arc<Self>) -> Arc<dyn CodeCache> {
    self.clone()
  }

  pub fn enabled(&self) -> bool {
    match &self.strategy {
      CodeCacheStrategy::FirstRun(strategy) => {
        !strategy.is_finished.is_raised()
      }
      CodeCacheStrategy::SubsequentRun(strategy) => {
        !strategy.is_finished.is_raised()
      }
    }
  }
}

impl CodeCache for DenoCompileCodeCache {
  fn get_sync(
    &self,
    specifier: &Url,
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
        strategy.take_from_cache(
          &self.specifier_key(specifier),
          code_cache_type,
          source_hash,
        )
      }
    }
  }

  fn set_sync(
    &self,
    specifier: Url,
    code_cache_type: CodeCacheType,
    source_hash: u64,
    bytes: &[u8],
  ) {
    match &self.strategy {
      CodeCacheStrategy::FirstRun(strategy) => {
        if strategy.is_finished.is_raised() {
          return;
        }

        let specifier = self.specifier_key(&specifier);
        let data_to_serialize = {
          let mut data = strategy.data.lock();
          data.cache.insert(
            (specifier, code_cache_type),
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
    let temp_file =
      get_atomic_path(&sys_traits::impls::RealSys, &self.file_path);
    match serialize(&temp_file, self.cache_key, cache_data) {
      Ok(()) => {
        if let Err(err) = std::fs::rename(&temp_file, &self.file_path) {
          log::debug!("Failed to rename code cache: {}", err);
          let _ = std::fs::remove_file(&temp_file);
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
    specifier: &str,
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
  // The external cache did not need stable ordering, but embedded cache bytes
  // become part of the compiled executable. Sort them so identical inputs do
  // not produce different executables because of HashMap randomization.
  let mut entries = cache.iter().collect::<Vec<_>>();
  entries.sort_unstable_by(
    |((specifier_a, type_a), _), ((specifier_b, type_b), _)| {
      specifier_a.cmp(specifier_b).then_with(|| {
        code_cache_type_byte(type_a).cmp(&code_cache_type_byte(type_b))
      })
    },
  );

  // header
  writer.write_all(&cache_key.to_le_bytes())?;
  writer.write_all(&(cache.len() as u32).to_le_bytes())?;
  // lengths of each entry
  for ((specifier, _), entry) in &entries {
    let len: u64 =
      entry.data.len() as u64 + specifier.len() as u64 + 1 + 4 + 8 + 8;
    writer.write_all(&len.to_le_bytes())?;
  }
  // entries
  for ((specifier, code_cache_type), entry) in entries {
    writer.write_all(&entry.data)?;
    writer.write_all(&[code_cache_type_byte(code_cache_type)])?;
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

fn code_cache_type_byte(code_cache_type: &CodeCacheType) -> u8 {
  match code_cache_type {
    CodeCacheType::EsModule => 0,
    CodeCacheType::Script => 1,
  }
}

fn deserialize(
  file_path: &Path,
  expected_cache_key: u64,
) -> Result<HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>, AnyError> {
  let cache_file = std::fs::File::open(file_path)?;
  let mut reader = BufReader::new(cache_file);
  deserialize_with_reader(&mut reader, expected_cache_key)
}

fn deserialize_bytes(
  bytes: &[u8],
  expected_cache_key: u64,
) -> Result<HashMap<CodeCacheKey, DenoCompileCodeCacheEntry>, AnyError> {
  deserialize_with_reader(&mut BufReader::new(bytes), expected_cache_key)
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
  fn embedded_code_cache() {
    let cache_key = 1234;
    let url = Url::parse("https://deno.land/embedded.js").unwrap();
    let cache = HashMap::from([(
      (url.to_string(), CodeCacheType::EsModule),
      DenoCompileCodeCacheEntry {
        source_hash: 42,
        data: vec![1, 2, 3],
      },
    )]);
    let mut buffer = Vec::new();
    serialize_with_writer(&mut BufWriter::new(&mut buffer), cache_key, &cache)
      .unwrap();

    let temp_dir = TempDir::new();
    let file_path = temp_dir.path().join("unused-cache.bin").to_path_buf();
    let code_cache = DenoCompileCodeCache::new(
      file_path.clone(),
      cache_key,
      Some(&buffer),
      None,
    );

    assert_eq!(
      code_cache.get_sync(&url, CodeCacheType::EsModule, 42),
      Some(vec![1, 2, 3])
    );
    assert!(!file_path.exists());
  }

  #[test]
  fn embedded_code_cache_is_independent_of_executable_name() {
    let cache_key = 1234;
    let generation_base =
      Url::parse("file:///tmp/deno-compile-code-cache-generator/").unwrap();
    let final_base =
      Url::parse("file:///tmp/deno-compile-production-binary/").unwrap();
    let generation_url = generation_base.join("main.js").unwrap();
    let final_url = final_base.join("main.js").unwrap();
    let temp_dir = TempDir::new();
    let generated_path =
      temp_dir.path().join("generated-cache.bin").to_path_buf();

    let generator = DenoCompileCodeCache::new(
      generated_path.clone(),
      cache_key,
      None,
      Some(&generation_base),
    );
    assert_eq!(
      generator.get_sync(&generation_url, CodeCacheType::EsModule, 42),
      None
    );
    generator.set_sync(generation_url, CodeCacheType::EsModule, 42, &[1, 2, 3]);

    let bytes = std::fs::read(generated_path).unwrap();
    let embedded = DenoCompileCodeCache::new(
      temp_dir.path().join("unused-cache.bin").to_path_buf(),
      cache_key,
      Some(&bytes),
      Some(&final_base),
    );
    assert_eq!(
      embedded.get_sync(&final_url, CodeCacheType::EsModule, 42),
      Some(vec![1, 2, 3])
    );
  }

  #[test]
  fn serialization_is_deterministic() {
    let entry = |index: u8| {
      (
        (format!("file:///{}.js", index), CodeCacheType::EsModule),
        DenoCompileCodeCacheEntry {
          source_hash: index as u64,
          data: vec![index],
        },
      )
    };
    let forward = (0..8).map(entry).collect::<HashMap<_, _>>();
    let reverse = (0..8).rev().map(entry).collect::<HashMap<_, _>>();
    let mut forward_bytes = Vec::new();
    let mut reverse_bytes = Vec::new();

    serialize_with_writer(
      &mut BufWriter::new(&mut forward_bytes),
      1234,
      &forward,
    )
    .unwrap();
    serialize_with_writer(
      &mut BufWriter::new(&mut reverse_bytes),
      1234,
      &reverse,
    )
    .unwrap();

    assert_eq!(forward_bytes, reverse_bytes);
    let positions = (0..8)
      .map(|index| {
        let specifier = format!("file:///{}.js", index);
        forward_bytes
          .windows(specifier.len())
          .position(|window| window == specifier.as_bytes())
          .unwrap()
      })
      .collect::<Vec<_>>();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
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
    let url1 = Url::parse("https://deno.land/example1.js").unwrap();
    let url2 = Url::parse("https://deno.land/example2.js").unwrap();
    // first run
    {
      let code_cache =
        DenoCompileCodeCache::new(file_path.clone(), 1234, None, None);
      assert!(
        code_cache
          .get_sync(&url1, CodeCacheType::EsModule, 0)
          .is_none()
      );
      assert!(
        code_cache
          .get_sync(&url2, CodeCacheType::EsModule, 1)
          .is_none()
      );
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
      let code_cache =
        DenoCompileCodeCache::new(file_path.clone(), 1234, None, None);
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
      let code_cache =
        DenoCompileCodeCache::new(file_path.clone(), 54321, None, None);
      assert!(
        code_cache
          .get_sync(&url1, CodeCacheType::EsModule, 0)
          .is_none()
      );
      assert!(
        code_cache
          .get_sync(&url2, CodeCacheType::EsModule, 1)
          .is_none()
      );
      code_cache.set_sync(url1.clone(), CodeCacheType::EsModule, 0, &[2, 2, 3]);
      code_cache.set_sync(url2.clone(), CodeCacheType::EsModule, 1, &[3, 2, 3]);
    }
    // new cache key second run
    {
      let code_cache =
        DenoCompileCodeCache::new(file_path.clone(), 54321, None, None);
      let result1 = code_cache
        .get_sync(&url1, CodeCacheType::EsModule, 0)
        .unwrap();
      assert_eq!(result1, vec![2, 2, 3]);
      assert!(
        code_cache
          .get_sync(&url2, CodeCacheType::EsModule, 5) // different hash will cause none
          .is_none()
      );
    }
  }
}
