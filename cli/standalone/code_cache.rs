// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::unsync::sync::AtomicFlag;
use deno_runtime::code_cache::CodeCache;
use deno_runtime::code_cache::CodeCacheType;

use crate::cache::FastInsecureHasher;
use crate::util::path::get_atomic_file_path;

struct MutableData {
  cache: HashMap<String, DenoCompileCodeCacheEntry>,
  modified: bool,
  add_count: usize,
}

impl MutableData {
  fn take_from_cache(
    &mut self,
    specifier: &ModuleSpecifier,
    source_hash: u64,
  ) -> Option<Vec<u8>> {
    let entry = self.cache.remove(specifier.as_str())?;
    if entry.source_hash != source_hash {
      return None;
    }
    Some(entry.data)
  }

  fn take_cache_data(
    &mut self,
  ) -> Option<HashMap<String, DenoCompileCodeCacheEntry>> {
    // always purge this from memory
    let cache_data = std::mem::take(&mut self.cache);

    if !self.modified {
      return None;
    }
    Some(cache_data)
  }
}

#[derive(Debug, Clone)]
pub struct DenoCompileCodeCacheEntry {
  pub source_hash: u64,
  pub data: Vec<u8>,
}

pub struct DenoCompileCodeCache {
  cache_key: String,
  file_path: PathBuf,
  finished: AtomicFlag,
  data: Mutex<MutableData>,
}

impl DenoCompileCodeCache {
  pub fn new(file_path: PathBuf, cache_key: String) -> Self {
    // attempt to deserialize the cache data
    let cache = match deserialize(&file_path, &cache_key) {
      Ok(cache) => cache,
      Err(err) => {
        log::debug!("Failed to deserialize code cache: {}", err);
        HashMap::new()
      }
    };

    Self {
      cache_key,
      file_path,
      finished: AtomicFlag::lowered(),
      data: Mutex::new(MutableData {
        cache,
        modified: false,
        add_count: 0,
      }),
    }
  }

  fn write_cache_data(
    &self,
    cache_data: &HashMap<String, DenoCompileCodeCacheEntry>,
  ) {
    let temp_file = get_atomic_file_path(&self.file_path);
    match serialize(&temp_file, &self.cache_key, cache_data) {
      Ok(()) => {
        if let Err(err) = std::fs::rename(&temp_file, &self.file_path) {
          log::debug!("Failed to rename code cache: {}", err);
        }
      }
      Err(err) => {
        let _ = std::fs::remove_file(&temp_file);
        log::debug!("Failed to serialize code cache: {}", err);
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
    if self.finished.is_raised() {
      return None;
    }
    let mut data = self.data.lock();
    match data.take_from_cache(specifier, source_hash) {
      Some(data) => Some(data),
      None => {
        data.add_count += 1;
        None
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
    if self.finished.is_raised() {
      return;
    }
    let data_to_serialize = {
      let mut data = self.data.lock();
      data.cache.insert(
        specifier.to_string(),
        DenoCompileCodeCacheEntry {
          source_hash,
          data: bytes.to_vec(),
        },
      );
      data.modified = true;
      if data.add_count != 0 {
        data.add_count -= 1;
      }
      if data.add_count == 0 {
        // don't allow using the cache anymore
        self.finished.raise();
        data.take_cache_data()
      } else {
        None
      }
    };
    if let Some(cache_data) = &data_to_serialize {
      self.write_cache_data(&cache_data);
    }
  }

  fn enabled(&self) -> bool {
    !self.finished.is_raised()
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
///   - <u32: specifier length>
///   - <u64: source hash>
///   - <u64: entry data hash>
fn serialize(
  file_path: &Path,
  cache_key: &str,
  cache: &HashMap<String, DenoCompileCodeCacheEntry>,
) -> Result<(), AnyError> {
  let cache_file = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(file_path)?;
  let mut writer = BufWriter::new(cache_file);
  // header
  writer.write_all(cache_key.as_bytes())?;
  writer.write_all(&(cache.len() as u32).to_le_bytes())?;
  // lengths of each entry
  for (specifier, entry) in cache {
    let len: u64 = entry.data.len() as u64 + specifier.len() as u64 + 4 + 8 + 8;
    writer.write_all(&len.to_le_bytes())?;
  }
  // entries
  for (specifier, entry) in cache {
    writer.write_all(&entry.data)?;
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
  cache_key: &str,
) -> Result<HashMap<String, DenoCompileCodeCacheEntry>, AnyError> {
  let cache_file = std::fs::File::open(file_path)?;
  let mut reader = BufReader::new(cache_file);
  let mut header_bytes = vec![0; cache_key.len() + 4];
  reader.read_exact(&mut header_bytes)?;
  if &header_bytes[..cache_key.len()] != cache_key.as_bytes() {
    // cache bust
    bail!("Cache key mismatch");
  }
  let len =
    u32::from_le_bytes(header_bytes[cache_key.len()..].try_into()?) as usize;
  // read the lengths for each entry found in the file
  let entry_len_bytes_capacity = len * 8;
  let mut entry_len_bytes = Vec::new();
  entry_len_bytes.try_reserve(entry_len_bytes_capacity)?;
  entry_len_bytes.resize(entry_len_bytes_capacity, 0);
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
    let mut buffer = Vec::new();
    buffer.try_reserve(len)?;
    buffer.resize(len, 0);

    reader.read_exact(&mut buffer)?;
    let entry_data_hash_start_pos = buffer.len() - 8;
    let expected_entry_data_hash =
      u64::from_le_bytes(buffer[entry_data_hash_start_pos..].try_into()?);
    let source_hash_start_pos = entry_data_hash_start_pos - 8;
    let source_hash = u64::from_le_bytes(
      buffer[source_hash_start_pos..entry_data_hash_start_pos].try_into()?,
    );
    let specifier_end_pos = source_hash_start_pos - 4;
    let specifier_len = u32::from_le_bytes(
      buffer[specifier_end_pos..source_hash_start_pos].try_into()?,
    ) as usize;
    let specifier_start_pos = specifier_end_pos - specifier_len;
    let specifier = String::from_utf8(
      buffer[specifier_start_pos..specifier_end_pos].to_vec(),
    )?;
    buffer.truncate(specifier_start_pos);
    let actual_entry_data_hash: u64 =
      FastInsecureHasher::new_without_deno_version()
        .write(&buffer)
        .finish();
    if expected_entry_data_hash != actual_entry_data_hash {
      bail!("Hash mismatch.")
    }
    map.insert(
      specifier,
      DenoCompileCodeCacheEntry {
        source_hash,
        data: buffer,
      },
    );
  }

  Ok(map)
}
