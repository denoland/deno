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
  /// The full file name of the file expected to be produced by unpacking the
  /// archive (e.g. `deno`, `deno.exe`, `libdenort.dylib`).
  pub exe_name: &'a str,
  pub archive_name: &'a str,
  pub archive_data: &'a [u8],
  pub dest_path: &'a Path,
}

pub fn unpack_into_dir(args: UnpackArgs) -> Result<PathBuf, AnyError> {
  let UnpackArgs {
    exe_name,
    archive_name,
    archive_data,
    dest_path,
  } = args;
  let archive_path = dest_path.join(exe_name).with_extension("zip");
  let exe_path = dest_path.join(exe_name);
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
  use std::io::Write;

  use super::*;

  fn make_zip(file_name: &str, contents: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
    writer
      .start_file(file_name, zip::write::SimpleFileOptions::default())
      .unwrap();
    writer.write_all(contents).unwrap();
    writer.finish().unwrap();
    buf
  }

  // The expected file name may carry a non-executable extension (e.g. a
  // `.dylib`/`.so`/`.dll` library). Regression test for a panic where the
  // extension was stripped and the unpacked file could not be found.
  #[test]
  fn unpacks_library_with_extension() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let data = make_zip("libdenort.dylib", b"fake dylib");
    let path = unpack_into_dir(UnpackArgs {
      exe_name: "libdenort.dylib",
      archive_name: "libdenort-aarch64-apple-darwin.zip",
      archive_data: &data,
      dest_path: temp_dir.path(),
    })
    .unwrap();
    assert_eq!(path, temp_dir.path().join("libdenort.dylib"));
    assert_eq!(std::fs::read(&path).unwrap(), b"fake dylib");
  }

  #[test]
  fn unpacks_executable_without_extension() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let data = make_zip("deno", b"fake exe");
    let path = unpack_into_dir(UnpackArgs {
      exe_name: "deno",
      archive_name: "deno-aarch64-apple-darwin.zip",
      archive_data: &data,
      dest_path: temp_dir.path(),
    })
    .unwrap();
    assert_eq!(path, temp_dir.path().join("deno"));
    assert_eq!(std::fs::read(&path).unwrap(), b"fake exe");
  }

  #[test]
  fn unpacks_executable_with_exe_extension() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let data = make_zip("deno.exe", b"fake exe");
    let path = unpack_into_dir(UnpackArgs {
      exe_name: "deno.exe",
      archive_name: "deno-x86_64-pc-windows-msvc.zip",
      archive_data: &data,
      dest_path: temp_dir.path(),
    })
    .unwrap();
    assert_eq!(path, temp_dir.path().join("deno.exe"));
  }
}
