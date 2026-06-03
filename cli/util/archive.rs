// Copyright 2018-2026 the Deno authors. MIT license.

use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Component;
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

struct ZipEntry {
  path: PathBuf,
  compression_method: u16,
  crc32: u32,
  compressed_size: u32,
  uncompressed_size: u32,
  local_header_offset: u32,
  external_attrs: u32,
  made_by: u16,
  is_dir: bool,
}

fn read_zip_u16(data: &[u8], offset: usize) -> Result<u16, AnyError> {
  let end = offset.checked_add(2).context("invalid zip archive")?;
  let bytes = data.get(offset..end).context("invalid zip archive")?;
  Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_zip_u32(data: &[u8], offset: usize) -> Result<u32, AnyError> {
  let end = offset.checked_add(4).context("invalid zip archive")?;
  let bytes = data.get(offset..end).context("invalid zip archive")?;
  Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn checked_zip_range(
  data: &[u8],
  offset: usize,
  len: usize,
) -> Result<&[u8], AnyError> {
  let end = offset.checked_add(len).context("invalid zip archive")?;
  data.get(offset..end).context("invalid zip archive")
}

fn find_end_of_central_directory(data: &[u8]) -> Result<usize, AnyError> {
  const END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x06054b50;

  if data.len() < 22 {
    bail!("invalid zip archive");
  }

  let earliest_offset = data.len().saturating_sub(22 + u16::MAX as usize);
  for offset in (earliest_offset..=data.len() - 22).rev() {
    if read_zip_u32(data, offset)? == END_OF_CENTRAL_DIRECTORY_SIGNATURE {
      return Ok(offset);
    }
  }

  bail!("invalid zip archive");
}

fn zip_entry_path(file_name: &[u8]) -> Result<Option<PathBuf>, AnyError> {
  let file_name = std::str::from_utf8(file_name)
    .context("zip entry path is not valid utf-8")?;
  let normalized_file_name = file_name.replace('\\', "/");
  let mut path = PathBuf::new();
  for component in Path::new(&normalized_file_name).components() {
    match component {
      Component::Normal(component) => path.push(component),
      Component::CurDir => {}
      Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
        bail!("zip entry path is outside the destination directory");
      }
    }
  }

  if path.as_os_str().is_empty() {
    Ok(None)
  } else {
    Ok(Some(path))
  }
}

fn parse_zip_entries(data: &[u8]) -> Result<Vec<ZipEntry>, AnyError> {
  const CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x02014b50;

  let eocd_offset = find_end_of_central_directory(data)?;
  let disk_number = read_zip_u16(data, eocd_offset + 4)?;
  let central_directory_disk = read_zip_u16(data, eocd_offset + 6)?;
  let entries_on_disk = read_zip_u16(data, eocd_offset + 8)?;
  let entries_total = read_zip_u16(data, eocd_offset + 10)?;
  let central_directory_size = read_zip_u32(data, eocd_offset + 12)?;
  let central_directory_offset = read_zip_u32(data, eocd_offset + 16)?;

  if disk_number != 0
    || central_directory_disk != 0
    || entries_on_disk != entries_total
  {
    bail!("multi-disk zip archives are not supported");
  }
  if entries_total == u16::MAX
    || central_directory_size == u32::MAX
    || central_directory_offset == u32::MAX
  {
    bail!("zip64 archives are not supported");
  }

  let central_directory_offset = central_directory_offset as usize;
  let central_directory_size = central_directory_size as usize;
  checked_zip_range(data, central_directory_offset, central_directory_size)?;

  let mut entries = Vec::with_capacity(entries_total as usize);
  let mut offset = central_directory_offset;
  let central_directory_end = central_directory_offset + central_directory_size;
  for _ in 0..entries_total {
    if read_zip_u32(data, offset)? != CENTRAL_DIRECTORY_SIGNATURE {
      bail!("invalid zip archive");
    }

    let made_by = read_zip_u16(data, offset + 4)?;
    let flags = read_zip_u16(data, offset + 8)?;
    let compression_method = read_zip_u16(data, offset + 10)?;
    let crc32 = read_zip_u32(data, offset + 16)?;
    let compressed_size = read_zip_u32(data, offset + 20)?;
    let uncompressed_size = read_zip_u32(data, offset + 24)?;
    let file_name_len = read_zip_u16(data, offset + 28)? as usize;
    let extra_field_len = read_zip_u16(data, offset + 30)? as usize;
    let file_comment_len = read_zip_u16(data, offset + 32)? as usize;
    let local_header_offset = read_zip_u32(data, offset + 42)?;
    let external_attrs = read_zip_u32(data, offset + 38)?;

    if flags & 1 != 0 {
      bail!("encrypted zip entries are not supported");
    }
    if compressed_size == u32::MAX
      || uncompressed_size == u32::MAX
      || local_header_offset == u32::MAX
    {
      bail!("zip64 archives are not supported");
    }

    let file_name = checked_zip_range(data, offset + 46, file_name_len)?;
    let Some(path) = zip_entry_path(file_name)? else {
      offset += 46 + file_name_len + extra_field_len + file_comment_len;
      continue;
    };
    let file_name = std::str::from_utf8(file_name)
      .context("zip entry path is not valid utf-8")?;
    let unix_mode = if made_by >> 8 == 3 {
      Some(external_attrs >> 16)
    } else {
      None
    };
    if let Some(unix_mode) = unix_mode {
      match unix_mode & 0o170000 {
        0 | 0o040000 | 0o100000 => {}
        _ => bail!("unsupported zip entry type"),
      }
    }

    entries.push(ZipEntry {
      path,
      compression_method,
      crc32,
      compressed_size,
      uncompressed_size,
      local_header_offset,
      external_attrs,
      made_by,
      is_dir: file_name.ends_with('/')
        || unix_mode
          .map(|mode| mode & 0o170000 == 0o040000)
          .unwrap_or(false),
    });

    offset += 46 + file_name_len + extra_field_len + file_comment_len;
    if offset > central_directory_end {
      bail!("invalid zip archive");
    }
  }

  if offset != central_directory_end {
    bail!("invalid zip archive");
  }

  Ok(entries)
}

#[cfg(unix)]
fn apply_unix_zip_permissions(
  path: &Path,
  made_by: u16,
  external_attrs: u32,
) -> Result<(), AnyError> {
  use std::os::unix::fs::PermissionsExt;

  if made_by >> 8 == 3 {
    let mode = (external_attrs >> 16) & 0o7777;
    if mode != 0 {
      fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
  }

  Ok(())
}

#[cfg(not(unix))]
fn apply_unix_zip_permissions(
  _path: &Path,
  _made_by: u16,
  _external_attrs: u32,
) -> Result<(), AnyError> {
  Ok(())
}

fn extract_zip_entry(
  data: &[u8],
  entry: &ZipEntry,
  dest_path: &Path,
) -> Result<(), AnyError> {
  const LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x04034b50;

  let output_path = dest_path.join(&entry.path);
  if entry.is_dir {
    fs::create_dir_all(&output_path)?;
    apply_unix_zip_permissions(
      &output_path,
      entry.made_by,
      entry.external_attrs,
    )?;
    return Ok(());
  }

  let local_header_offset = entry.local_header_offset as usize;
  if read_zip_u32(data, local_header_offset)? != LOCAL_FILE_HEADER_SIGNATURE {
    bail!("invalid zip archive");
  }

  let file_name_len = read_zip_u16(data, local_header_offset + 26)? as usize;
  let extra_field_len = read_zip_u16(data, local_header_offset + 28)? as usize;
  let compressed_data_offset = local_header_offset
    .checked_add(30)
    .and_then(|offset| offset.checked_add(file_name_len))
    .and_then(|offset| offset.checked_add(extra_field_len))
    .context("invalid zip archive")?;
  let compressed_data = checked_zip_range(
    data,
    compressed_data_offset,
    entry.compressed_size as usize,
  )?;

  if let Some(parent) = output_path.parent() {
    fs::create_dir_all(parent)?;
  }

  let mut output_file = fs::File::create(&output_path)?;
  let mut hasher = crc32fast::Hasher::new();
  let mut extracted_size = 0u64;

  match entry.compression_method {
    0 => {
      hasher.update(compressed_data);
      output_file.write_all(compressed_data)?;
      extracted_size = compressed_data.len() as u64;
    }
    8 => {
      let mut decoder = flate2::read::DeflateDecoder::new(compressed_data);
      let mut buffer = [0; 32 * 1024];
      loop {
        let read_count = decoder.read(&mut buffer)?;
        if read_count == 0 {
          break;
        }
        hasher.update(&buffer[..read_count]);
        output_file.write_all(&buffer[..read_count])?;
        extracted_size += read_count as u64;
      }
    }
    _ => bail!(
      "unsupported zip compression method: {}",
      entry.compression_method
    ),
  }

  if extracted_size != entry.uncompressed_size as u64 {
    bail!("zip entry has an invalid uncompressed size");
  }
  if hasher.finalize() != entry.crc32 {
    bail!("zip entry has an invalid checksum");
  }

  apply_unix_zip_permissions(
    &output_path,
    entry.made_by,
    entry.external_attrs,
  )?;

  Ok(())
}

fn unzip(
  archive_name: &str,
  archive_data: &[u8],
  dest_path: &Path,
) -> Result<(), AnyError> {
  let entries = parse_zip_entries(archive_data)
    .with_context(|| format!("failed to read archive: {archive_name}"))?;
  for entry in entries {
    extract_zip_entry(archive_data, &entry, dest_path)
      .with_context(|| format!("failed to extract archive: {archive_name}"))?;
  }

  Ok(())
}

pub struct UnpackArgs<'a> {
  pub exe_name: &'a str,
  pub archive_name: &'a str,
  pub archive_data: &'a [u8],
  pub is_windows: bool,
  pub dest_path: &'a Path,
}

pub fn unpack_into_dir(args: UnpackArgs) -> Result<PathBuf, AnyError> {
  let UnpackArgs {
    exe_name,
    archive_name,
    archive_data,
    is_windows,
    dest_path,
  } = args;
  let exe_ext = if is_windows { "exe" } else { "" };
  let archive_path = dest_path.join(exe_name).with_extension("zip");
  let exe_path = dest_path.join(exe_name).with_extension(exe_ext);
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

  struct TestZipEntry<'a> {
    name: &'a str,
    contents: &'a [u8],
    compression_method: u16,
    unix_mode: u32,
  }

  fn write_u16(data: &mut Vec<u8>, value: u16) {
    data.extend_from_slice(&value.to_le_bytes());
  }

  fn write_u32(data: &mut Vec<u8>, value: u32) {
    data.extend_from_slice(&value.to_le_bytes());
  }

  fn test_zip(entries: &[TestZipEntry<'_>]) -> Vec<u8> {
    let mut zip_data = Vec::new();
    let mut central_directory = Vec::new();

    for entry in entries {
      let local_header_offset = zip_data.len() as u32;
      let compressed_contents = match entry.compression_method {
        0 => entry.contents.to_vec(),
        8 => {
          let mut encoder = flate2::write::DeflateEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
          );
          encoder.write_all(entry.contents).unwrap();
          encoder.finish().unwrap()
        }
        _ => unreachable!(),
      };
      let crc32 = crc32fast::hash(entry.contents);

      write_u32(&mut zip_data, 0x04034b50);
      write_u16(&mut zip_data, 20);
      write_u16(&mut zip_data, 0);
      write_u16(&mut zip_data, entry.compression_method);
      write_u16(&mut zip_data, 0);
      write_u16(&mut zip_data, 0);
      write_u32(&mut zip_data, crc32);
      write_u32(&mut zip_data, compressed_contents.len() as u32);
      write_u32(&mut zip_data, entry.contents.len() as u32);
      write_u16(&mut zip_data, entry.name.len() as u16);
      write_u16(&mut zip_data, 0);
      zip_data.extend_from_slice(entry.name.as_bytes());
      zip_data.extend_from_slice(&compressed_contents);

      write_u32(&mut central_directory, 0x02014b50);
      write_u16(&mut central_directory, (3 << 8) | 20);
      write_u16(&mut central_directory, 20);
      write_u16(&mut central_directory, 0);
      write_u16(&mut central_directory, entry.compression_method);
      write_u16(&mut central_directory, 0);
      write_u16(&mut central_directory, 0);
      write_u32(&mut central_directory, crc32);
      write_u32(&mut central_directory, compressed_contents.len() as u32);
      write_u32(&mut central_directory, entry.contents.len() as u32);
      write_u16(&mut central_directory, entry.name.len() as u16);
      write_u16(&mut central_directory, 0);
      write_u16(&mut central_directory, 0);
      write_u16(&mut central_directory, 0);
      write_u16(&mut central_directory, 0);
      write_u32(&mut central_directory, entry.unix_mode << 16);
      write_u32(&mut central_directory, local_header_offset);
      central_directory.extend_from_slice(entry.name.as_bytes());
    }

    let central_directory_offset = zip_data.len() as u32;
    let central_directory_size = central_directory.len() as u32;
    zip_data.extend_from_slice(&central_directory);
    write_u32(&mut zip_data, 0x06054b50);
    write_u16(&mut zip_data, 0);
    write_u16(&mut zip_data, 0);
    write_u16(&mut zip_data, entries.len() as u16);
    write_u16(&mut zip_data, entries.len() as u16);
    write_u32(&mut zip_data, central_directory_size);
    write_u32(&mut zip_data, central_directory_offset);
    write_u16(&mut zip_data, 0);

    zip_data
  }

  #[test]
  fn unzip_extracts_stored_file() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let archive_data = test_zip(&[TestZipEntry {
      name: "deno",
      contents: b"stored binary",
      compression_method: 0,
      unix_mode: 0o100755,
    }]);

    unzip("deno.zip", &archive_data, temp_dir.path()).unwrap();

    let output_path = temp_dir.path().join("deno");
    assert_eq!(fs::read(&output_path).unwrap(), b"stored binary");
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      assert_eq!(
        fs::metadata(output_path).unwrap().permissions().mode() & 0o777,
        0o755
      );
    }
  }

  #[test]
  fn unzip_extracts_deflated_nested_file() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let archive_data = test_zip(&[TestZipEntry {
      name: "bin/deno",
      contents: b"deflated binary",
      compression_method: 8,
      unix_mode: 0o100755,
    }]);

    unzip("deno.zip", &archive_data, temp_dir.path()).unwrap();

    assert_eq!(
      fs::read(temp_dir.path().join("bin/deno")).unwrap(),
      b"deflated binary"
    );
  }

  #[test]
  fn unzip_rejects_path_traversal() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let archive_data = test_zip(&[TestZipEntry {
      name: "../deno",
      contents: b"bad",
      compression_method: 0,
      unix_mode: 0o100755,
    }]);

    let err = unzip("deno.zip", &archive_data, temp_dir.path()).unwrap_err();

    assert!(err.to_string().contains("failed to read archive"));
    assert!(!temp_dir.path().join("deno").exists());
  }
}
