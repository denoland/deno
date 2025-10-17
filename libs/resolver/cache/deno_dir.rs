// Copyright 2018-2025 the Deno authors. MIT license.

use std::env;
use std::path::PathBuf;

use deno_cache_dir::DenoDirResolutionError;
use deno_cache_dir::ResolveDenoDirSys;

use super::DiskCache;
use super::DiskCacheSys;

#[derive(Debug, Clone)]
pub struct DenoDirOptions {
  pub maybe_custom_root: Option<PathBuf>,
}

#[sys_traits::auto_impl]
pub trait DenoDirSys: DiskCacheSys + ResolveDenoDirSys + Clone {}

#[allow(clippy::disallowed_types)]
pub type DenoDirProviderRc<TSys> =
  deno_maybe_sync::MaybeArc<DenoDirProvider<TSys>>;

/// Lazily creates the deno dir which might be useful in scenarios
/// where functionality wants to continue if the DENO_DIR can't be created.
pub struct DenoDirProvider<TSys: DenoDirSys> {
  options: DenoDirOptions,
  sys: TSys,
  deno_dir_cell: once_cell::sync::OnceCell<DenoDir<TSys>>,
}

impl<TSys: DenoDirSys> DenoDirProvider<TSys> {
  pub fn new(sys: TSys, options: DenoDirOptions) -> Self {
    Self {
      options,
      sys,
      deno_dir_cell: Default::default(),
    }
  }

  pub fn get_or_create(
    &self,
  ) -> Result<&DenoDir<TSys>, DenoDirResolutionError> {
    self.deno_dir_cell.get_or_try_init(|| {
      let path = deno_cache_dir::resolve_deno_dir(
        &self.sys,
        self.options.maybe_custom_root.clone(),
      )?;
      Ok(DenoDir::new(self.sys.clone(), path))
    })
  }
}

/// `DenoDir` serves as coordinator for multiple `DiskCache`s containing them
/// in single directory that can be controlled with `$DENO_DIR` env variable.
#[derive(Debug, Clone)]
pub struct DenoDir<TSys: DiskCacheSys> {
  /// Example: /Users/rld/.deno/
  pub root: PathBuf,
  /// Used by TsCompiler to cache compiler output.
  pub gen_cache: DiskCache<TSys>,
}

impl<TSys: DiskCacheSys> DenoDir<TSys> {
  pub fn new(sys: TSys, root: PathBuf) -> Self {
    #[cfg(not(target_arch = "wasm32"))]
    assert!(root.is_absolute());

    let gen_path = root.join("gen");
    Self {
      root,
      gen_cache: DiskCache::new(sys, gen_path),
    }
  }

  /// The root directory of the DENO_DIR for display purposes only.
  pub fn root_path_for_display(&self) -> std::path::Display<'_> {
    self.root.display()
  }

  /// Path for the V8 code cache.
  pub fn code_cache_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("v8_code_cache_v2")
  }

  /// Path for the incremental cache used for formatting.
  pub fn fmt_incremental_cache_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("fmt_incremental_cache_v2")
  }

  /// Path for the incremental cache used for linting.
  pub fn lint_incremental_cache_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("lint_incremental_cache_v2")
  }

  /// Path for caching swc dependency analysis.
  pub fn dep_analysis_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("dep_analysis_cache_v2")
  }

  /// Path for the cache used for fast check.
  pub fn fast_check_cache_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("fast_check_cache_v2")
  }

  /// Path for caching node analysis.
  pub fn node_analysis_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("node_analysis_cache_v2")
  }

  /// Path for the cache used for type checking.
  pub fn type_checking_cache_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("check_cache_v2")
  }

  /// Path to the registries cache, used for the lps.
  pub fn registries_folder_path(&self) -> PathBuf {
    self.root.join("registries")
  }

  /// Path to the remote cache folder.
  pub fn remote_folder_path(&self) -> PathBuf {
    self.root.join("remote")
  }

  /// Path to the origin data cache folder.
  pub fn origin_data_folder_path(&self) -> PathBuf {
    // TODO(@crowlKats): change to origin_data for 2.0
    self.root.join("location_data")
  }

  /// File used for the upgrade checker.
  pub fn upgrade_check_file_path(&self) -> PathBuf {
    self.root.join("latest.txt")
  }

  /// Folder used for the npm cache.
  pub fn npm_folder_path(&self) -> PathBuf {
    self.root.join("npm")
  }

  /// Path used for the REPL history file.
  /// Can be overridden or disabled by setting `DENO_REPL_HISTORY` environment variable.
  pub fn repl_history_file_path(&self) -> Option<PathBuf> {
    if let Some(deno_repl_history) = env::var_os("DENO_REPL_HISTORY") {
      if deno_repl_history.is_empty() {
        None
      } else {
        Some(PathBuf::from(deno_repl_history))
      }
    } else {
      Some(self.root.join("deno_history.txt"))
    }
  }

  /// Folder path used for downloading new versions of deno.
  pub fn dl_folder_path(&self) -> PathBuf {
    self.root.join("dl")
  }
}
