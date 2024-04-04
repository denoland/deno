// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;

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

  let content = std::fs::read_to_string(&filename)
    .with_context(|| format!("Failed reading '{}'.", filename.display()))?;
  let lockfile =
    Lockfile::with_lockfile_content(filename, &content, flags.lock_write)?;
  Ok(Some(lockfile))
}
