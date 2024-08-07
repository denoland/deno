// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
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
  let exe_path = temp_dir_path.join(exe_name).with_extension(exe_ext);
  assert!(!exe_path.exists());

  let archive_ext = Path::new(archive_name)
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap();
  match archive_ext {
    "zip" => {
      let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(archive_data))?;
      archive.extract(temp_dir_path).with_context(|| {
        format!("failed to extract archive: {archive_name}")
      })?;
    }
    ext => bail!("Unsupported archive type: '{ext}'"),
  }

  assert!(exe_path.exists());
  Ok(exe_path)
}
