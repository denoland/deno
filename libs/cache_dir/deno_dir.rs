// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;

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

pub struct ResolveDenoDirOptions<'a> {
  /// Optionally provide this as an optimization if
  /// you already have this value in order to potentially
  /// skip the cwd sys call.
  pub maybe_initial_cwd: Option<&'a Path>,
  pub maybe_custom_root: Option<&'a Path>,
}

pub fn resolve_deno_dir<'a, Sys: ResolveDenoDirSys>(
  sys: &Sys,
  options: ResolveDenoDirOptions<'a>,
) -> Result<Cow<'a, Path>, DenoDirResolutionError> {
  let maybe_custom_root = options
    .maybe_custom_root
    .map(Cow::Borrowed)
    .or_else(|| sys.env_var_path("DENO_DIR").map(Cow::Owned));
  let root = if let Some(root) = maybe_custom_root {
    root
  } else if let Some(xdg_cache_dir) = sys.env_var_path("XDG_CACHE_HOME") {
    Cow::Owned(xdg_cache_dir.join("deno"))
  } else if let Some(cache_dir) = sys.env_cache_dir() {
    // We use the OS cache dir because all files deno writes are cache files
    // Once that changes we need to start using different roots if DENO_DIR
    // is not set, and keep a single one if it is.
    Cow::Owned(cache_dir.join("deno"))
  } else if let Some(home_dir) = sys.env_home_dir() {
    // fallback path
    Cow::Owned(home_dir.join(".deno"))
  } else {
    return Err(DenoDirResolutionError::NoCacheOrHomeDir);
  };
  let root = if root.is_absolute() {
    root
  } else {
    let cwd = match options.maybe_initial_cwd {
      Some(cwd) => Cow::Borrowed(cwd),
      None => Cow::Owned(
        // ok because we allow people providing the initial cwd
        // as an optimization here
        #[allow(
          clippy::disallowed_methods,
          reason = "fallback when cwd not provided"
        )]
        sys
          .env_current_dir()
          .map_err(|source| DenoDirResolutionError::FailedCwd { source })?,
      ),
    };

    Cow::Owned(cwd.join(root))
  };
  Ok(root)
}
