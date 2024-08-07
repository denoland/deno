use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;

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
  let unpack_status = match archive_ext {
    "zip" if cfg!(windows) => {
      fs::write(&archive_path, &archive_data)?;
      Command::new("tar.exe")
        .arg("xf")
        .arg(&archive_path)
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
    }
    "zip" => {
      fs::write(&archive_path, &archive_data)?;
      Command::new("unzip")
        .current_dir(temp_dir_path)
        .arg(&archive_path)
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
    }
    ext => bail!("Unsupported archive type: '{ext}'"),
  };
  if !unpack_status.success() {
    bail!("Failed to unpack archive.");
  }
  assert!(exe_path.exists());
  fs::remove_file(&archive_path)?;
  Ok(exe_path)
}
