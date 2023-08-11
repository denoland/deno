// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;

use crate::args::ConfigFile;
use crate::Flags;

use super::DenoSubcommand;

pub use deno_lockfile::Lockfile;
pub use deno_lockfile::LockfileError;

pub fn discover(
  flags: &Flags,
  maybe_config_file: Option<&ConfigFile>,
) -> Result<Option<Lockfile>, AnyError> {
  if flags.no_lock
    || matches!(
      flags.subcommand,
      DenoSubcommand::Install(_) | DenoSubcommand::Uninstall(_)
    )
  {
    return Ok(None);
  }

  let filename = match flags.lock {
    Some(ref lock) => PathBuf::from(lock),
    None => match maybe_config_file {
      Some(config_file) => {
        if config_file.specifier.scheme() == "file" {
          match config_file.resolve_lockfile_path()? {
            Some(path) => path,
            None => return Ok(None),
          }
        } else {
          return Ok(None);
        }
      }
      None => return Ok(None),
    },
  };

  let lockfile = Lockfile::new(filename, flags.lock_write)?;
  Ok(Some(lockfile))
}

pub async fn snapshot_from_lockfile(
  lockfile: Arc<Mutex<Lockfile>>,
  api: &dyn NpmRegistryApi,
) -> Result<ValidSerializedNpmResolutionSnapshot, AnyError> {
  let incomplete_snapshot = {
    let lock = lockfile.lock();
    deno_npm::resolution::incomplete_snapshot_from_lockfile(&lock)?
  };
  let snapshot =
    deno_npm::resolution::snapshot_from_lockfile(incomplete_snapshot, api)
      .await?;
  Ok(snapshot)
}
