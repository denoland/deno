// Copyright 2018-2026 the Deno authors. MIT license.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;

fn unzip_with_shell(
  archive_path: &Path,
  archive_data: &[u8],
  dest_path: &Path,
) -> Result<(), AnyError> {
  fs::write(archive_path, archive_data)?;
  let unpack_status = if cfg!(windows) {
    Command::new("tar.exe")
      .arg("xf")
      .arg(archive_path)
      .arg("-C")
      .arg(dest_path)
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
    Command::new("unzip")
      .current_dir(dest_path)
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
  dest_path: &Path,
) -> Result<(), AnyError> {
  let mut archive = zip::ZipArchive::new(std::io::Cursor::new(archive_data))?;
  archive
    .extract(dest_path)
    .with_context(|| format!("failed to extract archive: {archive_name}"))?;

  Ok(())
}

pub struct UnpackArgs<'a> {
  pub exe_name: &'a str,
  pub archive_name: &'a str,
  pub archive_data: &'a [u8],
  pub is_windows: bool,
  pub dest_path: &'a Path,
}

/// Resolve the file name of the executable (or dynamic library) that unpacking
/// the archive is expected to produce.
///
/// `exe_name` may already include an extension — for example `libdenort.dylib`
/// for the desktop runtime — in which case it is used verbatim. A bare name
/// like `deno` or `denort` gets the platform executable extension applied
/// (`.exe` on Windows, none elsewhere).
fn unpacked_exe_name(exe_name: &str, is_windows: bool) -> PathBuf {
  if Path::new(exe_name).extension().is_some() {
    PathBuf::from(exe_name)
  } else if is_windows {
    Path::new(exe_name).with_extension("exe")
  } else {
    PathBuf::from(exe_name)
  }
}

pub fn unpack_into_dir(args: UnpackArgs) -> Result<PathBuf, AnyError> {
  let UnpackArgs {
    exe_name,
    archive_name,
    archive_data,
    is_windows,
    dest_path,
  } = args;
  let archive_path = dest_path.join(exe_name).with_extension("zip");
  let exe_path = dest_path.join(unpacked_exe_name(exe_name, is_windows));
  assert!(!exe_path.exists());

  let archive_ext = Path::new(archive_name)
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap();
  match archive_ext {
    "zip" => match unzip(archive_name, archive_data, dest_path) {
      Ok(()) if !exe_path.exists() => {
        log::warn!("unpacking via the zip crate didn't produce the executable");
        // No error but didn't produce exe, fallback to shelling out
        unzip_with_shell(&archive_path, archive_data, dest_path)?;
      }
      Ok(_) => {}
      Err(e) => {
        log::warn!("unpacking via zip crate failed: {e}");
        // Fallback to shelling out
        unzip_with_shell(&archive_path, archive_data, dest_path)?;
      }
    },
    ext => bail!("Unsupported archive type: '{ext}'"),
  }

  assert!(exe_path.exists());
  Ok(exe_path)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn unpacked_exe_name_bare() {
    // Bare names get the platform executable extension.
    assert_eq!(unpacked_exe_name("deno", false), PathBuf::from("deno"));
    assert_eq!(unpacked_exe_name("deno", true), PathBuf::from("deno.exe"));
    assert_eq!(unpacked_exe_name("denort", false), PathBuf::from("denort"));
    assert_eq!(unpacked_exe_name("denort", true), PathBuf::from("denort.exe"));
  }

  #[test]
  fn unpacked_exe_name_with_extension() {
    // Names that already carry an extension (the desktop runtime dylib) are
    // used verbatim — the extension must not be stripped or replaced.
    assert_eq!(
      unpacked_exe_name("libdenort.dylib", false),
      PathBuf::from("libdenort.dylib"),
    );
    assert_eq!(
      unpacked_exe_name("libdenort.so", false),
      PathBuf::from("libdenort.so"),
    );
    assert_eq!(
      unpacked_exe_name("denort.dll", true),
      PathBuf::from("denort.dll"),
    );
  }
}
