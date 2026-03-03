// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;

use sys_traits::EnvCacheDir;
use sys_traits::EnvCurrentDir;
use sys_traits::EnvHomeDir;
use sys_traits::EnvVar;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DenoDirResolutionError {
  #[error(
    "Could not resolve global Deno cache directory. Please make sure that either the DENO_DIR environment variable is set or the cache directory is available."
  )]
  NoCacheOrHomeDir,
  #[error(
    "Could not resolve global Deno cache directory because the current working directory could not be resolved. Please set the DENO_DIR environment variable and ensure it is pointing at an absolute path."
  )]
  FailedCwd {
    #[source]
    source: std::io::Error,
  },
}

#[sys_traits::auto_impl]
pub trait ResolveDenoDirSys:
  EnvCacheDir + EnvHomeDir + EnvVar + EnvCurrentDir
{
}

pub fn resolve_deno_dir<Sys: ResolveDenoDirSys>(
  sys: &Sys,
  maybe_custom_root: Option<PathBuf>,
) -> Result<PathBuf, DenoDirResolutionError> {
  let maybe_custom_root =
    maybe_custom_root.or_else(|| sys.env_var_path("DENO_DIR"));
  let root: PathBuf = if let Some(root) = maybe_custom_root {
    root
  } else if let Some(xdg_cache_dir) = sys.env_var_path("XDG_CACHE_HOME") {
    xdg_cache_dir.join("deno")
  } else if let Some(cache_dir) = sys.env_cache_dir() {
    // We use the OS cache dir because all files deno writes are cache files
    // Once that changes we need to start using different roots if DENO_DIR
    // is not set, and keep a single one if it is.
    cache_dir.join("deno")
  } else if let Some(home_dir) = sys.env_home_dir() {
    // fallback path
    home_dir.join(".deno")
  } else {
    return Err(DenoDirResolutionError::NoCacheOrHomeDir);
  };
  let root = if root.is_absolute() {
    root
  } else {
    sys
      .env_current_dir()
      .map_err(|source| DenoDirResolutionError::FailedCwd { source })?
      .join(root)
  };
  Ok(root)
}
