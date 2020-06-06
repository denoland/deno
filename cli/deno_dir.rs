// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::disk_cache::DiskCache;
use std::path::PathBuf;

/// `DenoDir` serves as coordinator for multiple `DiskCache`s containing them
/// in single directory that can be controlled with `$DENO_DIR` env variable.
#[derive(Clone)]
pub struct DenoDir {
  // Example: /Users/rld/.deno/
  pub root: PathBuf,
  /// Used by TsCompiler to cache compiler output.
  pub gen_cache: DiskCache,
}

impl DenoDir {
  pub fn new(maybe_custom_root: Option<PathBuf>) -> std::io::Result<Self> {
    // Only setup once.
    let home_dir = dirs::home_dir().expect("Could not get home directory.");
    let fallback = home_dir.join(".deno");
    // We use the OS cache dir because all files deno writes are cache files
    // Once that changes we need to start using different roots if DENO_DIR
    // is not set, and keep a single one if it is.
    let default = dirs::cache_dir()
      .map(|d| d.join("deno"))
      .unwrap_or(fallback);

    let root: PathBuf = if let Some(root) = maybe_custom_root {
      if root.is_absolute() {
        root
      } else {
        std::env::current_dir()?.join(root)
      }
    } else {
      default
    };
    assert!(root.is_absolute());
    let gen_path = root.join("gen");

    let deno_dir = Self {
      root,
      gen_cache: DiskCache::new(&gen_path),
    };
    deno_dir.gen_cache.ensure_dir_exists(&gen_path)?;

    Ok(deno_dir)
  }
}
