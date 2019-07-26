// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::disk_cache::DiskCache;
use deno::ErrBox;
use dirs;
use std;
use std::collections::HashMap;
use std::path::PathBuf;
use std::result::Result;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;

/// `DenoDir` serves as coordinator for multiple `DiskCache`s containing them
/// in single directory that can be controlled with `$DENO_DIR` env variable.
#[derive(Clone)]
pub struct DenoDir {
  // Example: /Users/rld/.deno/
  pub root: PathBuf,
  cache_map: Arc<Mutex<HashMap<String, DiskCache>>>,
}

impl DenoDir {
  // Must be called before using any function from this module.
  // https://github.com/denoland/deno/blob/golang/deno_dir.go#L99-L111
  pub fn new(custom_root: Option<PathBuf>) -> std::io::Result<Self> {
    // Only setup once.
    let home_dir = dirs::home_dir().expect("Could not get home directory.");
    let fallback = home_dir.join(".deno");
    // We use the OS cache dir because all files deno writes are cache files
    // Once that changes we need to start using different roots if DENO_DIR
    // is not set, and keep a single one if it is.
    let default = dirs::cache_dir()
      .map(|d| d.join("deno"))
      .unwrap_or(fallback);

    let root: PathBuf = custom_root.unwrap_or(default);

    let deno_dir = Self {
      root,
      cache_map: Arc::new(Mutex::new(HashMap::default())),
    };

    Ok(deno_dir)
  }

  pub fn register_cache(self: &Self, name: &str) -> Result<DiskCache, ErrBox> {
    let path = self.root.join(name);

    if self.cache_map.lock().unwrap().contains_key(name) {
      // TODO: change error type
      return Err(
        DenoError::new(
          ErrorKind::UnsupportedFetchScheme,
          format!("Cache with name \"{}\" was already registered", name),
        ).into(),
      );
    }

    let cache = DiskCache::new(&path);
    self
      .cache_map
      .lock()
      .unwrap()
      .insert(name.to_string(), cache.clone());

    Ok(cache)
  }
}
