// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::MutexGuard;
use deno_runtime::deno_node::PackageJson;

use crate::args::ConfigFile;
use crate::cache;
use crate::util::fs::atomic_write_file_with_retries;
use crate::Flags;

use crate::args::DenoSubcommand;
use crate::args::InstallFlags;
use crate::args::InstallKind;

use deno_lockfile::Lockfile;

#[derive(Debug)]
pub struct CliLockfile {
  lockfile: Mutex<Lockfile>,
  pub filename: PathBuf,
}

pub struct Guard<'a, T> {
  guard: MutexGuard<'a, T>,
}

impl<'a, T> std::ops::Deref for Guard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<'a, T> std::ops::DerefMut for Guard<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

impl CliLockfile {
  pub fn new(lockfile: Lockfile) -> Self {
    let filename = lockfile.filename.clone();
    Self {
      lockfile: Mutex::new(lockfile),
      filename,
    }
  }

  /// Get the inner deno_lockfile::Lockfile.
  pub fn lock(&self) -> Guard<Lockfile> {
    Guard {
      guard: self.lockfile.lock(),
    }
  }

  pub fn set_workspace_config(
    &self,
    options: deno_lockfile::SetWorkspaceConfigOptions,
  ) {
    self.lockfile.lock().set_workspace_config(options);
  }

  pub fn overwrite(&self) -> bool {
    self.lockfile.lock().overwrite
  }

  pub fn write_if_changed(&self) -> Result<(), AnyError> {
    let mut lockfile = self.lockfile.lock();
    let Some(bytes) = lockfile.resolve_write_bytes() else {
      return Ok(()); // nothing to do
    };
    // do an atomic write to reduce the chance of multiple deno
    // processes corrupting the file
    atomic_write_file_with_retries(
      &lockfile.filename,
      bytes,
      cache::CACHE_PERM,
    )
    .context("Failed writing lockfile.")?;
    lockfile.has_content_changed = false;
    Ok(())
  }

  pub fn discover(
    flags: &Flags,
    maybe_config_file: Option<&ConfigFile>,
    maybe_package_json: Option<&PackageJson>,
  ) -> Result<Option<CliLockfile>, AnyError> {
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
      CliLockfile::new(Lockfile::new_empty(filename, true))
    } else {
      Self::read_from_path(filename)?
    };
    Ok(Some(lockfile))
  }
  pub fn read_from_path(filename: PathBuf) -> Result<CliLockfile, AnyError> {
    match std::fs::read_to_string(&filename) {
      Ok(text) => Ok(CliLockfile::new(Lockfile::with_lockfile_content(
        filename, &text, false,
      )?)),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        Ok(CliLockfile::new(Lockfile::new_empty(filename, false)))
      }
      Err(err) => Err(err).with_context(|| {
        format!("Failed reading lockfile '{}'", filename.display())
      }),
    }
  }
}
