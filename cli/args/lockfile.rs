// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_runtime::deno_node::PackageJson;

use crate::args::ConfigFile;
use crate::Flags;

use super::DenoSubcommand;
use super::InstallFlags;
use super::InstallKind;

pub use deno_lockfile::Lockfile;
pub use deno_lockfile::LockfileError;

pub fn discover(
  flags: &Flags,
  maybe_config_file: Option<&ConfigFile>,
  maybe_package_json: Option<&PackageJson>,
) -> Result<Option<Lockfile>, AnyError> {
  if flags.no_lock
    || matches!(
      flags.subcommand,
      DenoSubcommand::Install(InstallFlags {
        kind: InstallKind::Global(..),
        ..
      }) | DenoSubcommand::Uninstall(_)
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
      None => match maybe_package_json {
        Some(package_json) => {
          package_json.path.parent().unwrap().join("deno.lock")
        }
        None => return Ok(None),
      },
    },
  };

  let lockfile = Lockfile::new(filename, flags.lock_write)?;
  Ok(Some(lockfile))
}
