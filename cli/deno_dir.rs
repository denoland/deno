// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::disk_cache::DiskCache;
use std::path::PathBuf;

/// `DenoDir` serves as coordinator for multiple `DiskCache`s containing them
/// in single directory that can be controlled with `$DENO_DIR` env variable.
#[derive(Clone)]
pub struct DenoDir {
  /// Example: /Users/rld/.deno/
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

/// To avoid the poorly managed dirs crate
#[cfg(not(windows))]
mod dirs {
  use std::path::PathBuf;

  pub fn cache_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
      home_dir().map(|h| h.join("Library/Caches"))
    } else {
      std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".cache")))
    }
  }

  pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
      .and_then(|h| if h.is_empty() { None } else { Some(h) })
      .map(PathBuf::from)
  }
}

/// To avoid the poorly managed dirs crate
// Copied from
// https://github.com/dirs-dev/dirs-sys-rs/blob/ec7cee0b3e8685573d847f0a0f60aae3d9e07fa2/src/lib.rs#L140-L164
// MIT license. Copyright (c) 2018-2019 dirs-rs contributors
#[cfg(windows)]
mod dirs {
  use std::ffi::OsString;
  use std::os::windows::ffi::OsStringExt;
  use std::path::PathBuf;
  use winapi::shared::winerror;
  use winapi::um::{combaseapi, knownfolders, shlobj, shtypes, winbase, winnt};

  fn known_folder(folder_id: shtypes::REFKNOWNFOLDERID) -> Option<PathBuf> {
    unsafe {
      let mut path_ptr: winnt::PWSTR = std::ptr::null_mut();
      let result = shlobj::SHGetKnownFolderPath(
        folder_id,
        0,
        std::ptr::null_mut(),
        &mut path_ptr,
      );
      if result == winerror::S_OK {
        let len = winbase::lstrlenW(path_ptr) as usize;
        let path = std::slice::from_raw_parts(path_ptr, len);
        let ostr: OsString = OsStringExt::from_wide(path);
        combaseapi::CoTaskMemFree(path_ptr as *mut winapi::ctypes::c_void);
        Some(PathBuf::from(ostr))
      } else {
        None
      }
    }
  }

  pub fn cache_dir() -> Option<PathBuf> {
    known_folder(&knownfolders::FOLDERID_LocalAppData)
  }

  pub fn home_dir() -> Option<PathBuf> {
    known_folder(&knownfolders::FOLDERID_Profile)
  }
}
