// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;

fn unzip_with_shell(
  archive_path: &Path,
  archive_data: Vec<u8>,
  temp_dir_path: &Path,
) -> Result<(), AnyError> {
  let unpack_status = if cfg!(windows) {
    fs::write(archive_path, &archive_data)?;
    Command::new("tar.exe")
      .arg("xf")
      .arg(archive_path)
      .arg("-C")
      .arg(temp_dir_path)
      .spawn()
      .map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
          std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "`tar.exe` was not found in your PATH",
          )
        } else {
          err
        }
      })?
      .wait()?
  } else {
    fs::write(archive_path, &archive_data)?;
    Command::new("unzip")
      .current_dir(temp_dir_path)
      .arg(archive_path)
      .spawn()
      .map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
          std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "`unzip` was not found in your PATH, please install `unzip`",
          )
        } else {
          err
        }
      })?
      .wait()?
  };

  if !unpack_status.success() {
    bail!("Failed to unpack archive.");
  }

  Ok(())
}

fn unzip(
  archive_name: &str,
  archive_data: &[u8],
  temp_dir_path: &Path,
) -> Result<(), AnyError> {
  let mut archive = zip::ZipArchive::new(std::io::Cursor::new(archive_data))?;
  archive
    .extract(temp_dir_path)
    .with_context(|| format!("failed to extract archive: {archive_name}"))?;

  Ok(())
}

pub fn unpack_into_dir(
  exe_name: &str,
  archive_name: &str,
  archive_data: Vec<u8>,
  is_windows: bool,
  temp_dir: &tempfile::TempDir,
) -> Result<PathBuf, AnyError> {
  let temp_dir_path = temp_dir.path();
  let exe_ext = if is_windows { "exe" } else { "" };
  let archive_path = temp_dir_path.join(exe_name).with_extension("zip");
  let exe_path = temp_dir_path.join(exe_name).with_extension(exe_ext);
  assert!(!exe_path.exists());

  let archive_ext = Path::new(archive_name)
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap();
  match archive_ext {
    "zip" => match unzip(archive_name, &archive_data, temp_dir_path) {
      Ok(()) if !exe_path.exists() => {
        log::warn!("unpacking via the zip crate didn't produce the executable");
        // No error but didn't produce exe, fallback to shelling out
        unzip_with_shell(&archive_path, archive_data, temp_dir_path)?;
      }
      Ok(_) => {}
      Err(e) => {
        log::warn!("unpacking via zip crate failed: {e}");
        // Fallback to shelling out
        unzip_with_shell(&archive_path, archive_data, temp_dir_path)?;
      }
    },
    ext => bail!("Unsupported archive type: '{ext}'"),
  }

  assert!(exe_path.exists());
  Ok(exe_path)
}
