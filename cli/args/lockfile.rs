// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::error::AnyError;
use std::path::PathBuf;

use crate::args::config_file::LockConfig;
use crate::args::ConfigFile;
use crate::npm::NpmResolutionPackage;
use crate::Flags;

use super::DenoSubcommand;

pub use deno_lockfile::Lockfile;
pub use deno_lockfile::LockfileError;
use deno_lockfile::NpmPackageDependencyLockfileInfo;
use deno_lockfile::NpmPackageLockfileInfo;

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
          match config_file.to_lock_config()? {
            Some(LockConfig::Bool(lock)) if !lock => {
              return Ok(None);
            }
            Some(LockConfig::PathBuf(lock)) => config_file
              .specifier
              .to_file_path()
              .unwrap()
              .parent()
              .unwrap()
              .join(lock),
            _ => {
              let mut path = config_file.specifier.to_file_path().unwrap();
              path.set_file_name("deno.lock");
              path
            }
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

// NOTE(bartlomieju): we don't want a reverse mapping to be possible.
#[allow(clippy::from_over_into)]
impl Into<NpmPackageLockfileInfo> for NpmResolutionPackage {
  fn into(self) -> NpmPackageLockfileInfo {
    let dependencies = self
      .dependencies
      .into_iter()
      .map(|(name, id)| NpmPackageDependencyLockfileInfo {
        name,
        id: id.as_serialized(),
      })
      .collect();

    NpmPackageLockfileInfo {
      display_id: self.id.display(),
      serialized_id: self.id.as_serialized(),
      integrity: self.dist.integrity().to_string(),
      dependencies,
    }
  }
}
