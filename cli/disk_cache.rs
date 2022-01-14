// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cache::Cacher;
use crate::fs_util;

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::str;

#[derive(Clone)]
pub struct DiskCache {
  pub location: PathBuf,
}

fn with_io_context<T: AsRef<str>>(
  e: &std::io::Error,
  context: T,
) -> std::io::Error {
  std::io::Error::new(e.kind(), format!("{} (for '{}')", e, context.as_ref()))
}

impl DiskCache {
  /// `location` must be an absolute path.
  pub fn new(location: &Path) -> Self {
    assert!(location.is_absolute());
    Self {
      location: location.to_owned(),
    }
  }

  /// Ensures the location of the cache.
  pub fn ensure_dir_exists(&self, path: &Path) -> io::Result<()> {
    if path.is_dir() {
      return Ok(());
    }
    fs::create_dir_all(&path).map_err(|e| {
      io::Error::new(e.kind(), format!(
        "Could not create TypeScript compiler cache location: {:?}\nCheck the permission of the directory.",
        path
      ))
    })
  }
}

impl Cacher for DiskCache {
  fn get(&self, filename: &Path) -> std::io::Result<Vec<u8>> {
    let path = self.location.join(filename);
    fs::read(&path)
  }

  fn set(&mut self, filename: &Path, data: &[u8]) -> std::io::Result<()> {
    let path = self.location.join(filename);
    match path.parent() {
      Some(parent) => self.ensure_dir_exists(parent),
      None => Ok(()),
    }?;
    fs_util::atomic_write_file(&path, data, crate::http_cache::CACHE_PERM)
      .map_err(|e| with_io_context(&e, format!("{:#?}", &path)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn test_create_cache_if_dir_exits() {
    let cache_location = TempDir::new().unwrap();
    let mut cache_path = cache_location.path().to_owned();
    cache_path.push("foo");
    let cache = DiskCache::new(&cache_path);
    cache
      .ensure_dir_exists(&cache.location)
      .expect("Testing expect:");
    assert!(cache_path.is_dir());
  }

  #[test]
  fn test_create_cache_if_dir_not_exits() {
    let temp_dir = TempDir::new().unwrap();
    let mut cache_location = temp_dir.path().to_owned();
    assert!(fs::remove_dir(&cache_location).is_ok());
    cache_location.push("foo");
    assert!(!cache_location.is_dir());
    let cache = DiskCache::new(&cache_location);
    cache
      .ensure_dir_exists(&cache.location)
      .expect("Testing expect:");
    assert!(cache_location.is_dir());
  }
}
