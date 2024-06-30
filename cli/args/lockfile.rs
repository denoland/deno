// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_runtime::deno_node::PackageJson;

use crate::args::ConfigFile;
use crate::cache;
use crate::util::fs::atomic_write_file_with_retries;
use crate::Flags;

use super::DenoSubcommand;
use super::InstallFlags;
use super::InstallKind;

pub use deno_lockfile::Lockfile;

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

  let lockfile = if flags.lock_write {
    Lockfile::new_empty(filename, true)
  } else {
    read_lockfile_at_path(filename)?
  };
  Ok(Some(lockfile))
}

pub fn read_lockfile_at_path(filename: PathBuf) -> Result<Lockfile, AnyError> {
  match std::fs::read_to_string(&filename) {
    Ok(text) => Ok(Lockfile::with_lockfile_content(filename, &text, false)?),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      Ok(Lockfile::new_empty(filename, false))
    }
    Err(err) => Err(err).with_context(|| {
      format!("Failed reading lockfile '{}'", filename.display())
    }),
  }
}

pub fn write_lockfile_if_has_changes(
  lockfile: &mut Lockfile,
) -> Result<(), AnyError> {
  let Some(bytes) = lockfile.resolve_write_bytes() else {
    return Ok(()); // nothing to do
  };
  // do an atomic write to reduce the chance of multiple deno
  // processes corrupting the file
  atomic_write_file_with_retries(&lockfile.filename, bytes, cache::CACHE_PERM)
    .context("Failed writing lockfile.")?;
  lockfile.has_content_changed = false;
  Ok(())
}
