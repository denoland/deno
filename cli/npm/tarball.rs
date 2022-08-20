// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use flate2::read::GzDecoder;
use tar::Archive;
use tar::EntryType;

use super::cache::NPM_PACKAGE_SYNC_LOCK_FILENAME;
use super::registry::NpmPackageVersionDistInfo;
use super::NpmPackageId;

pub fn verify_and_extract_tarball(
  package: &NpmPackageId,
  data: &[u8],
  dist_info: &NpmPackageVersionDistInfo,
  output_folder: &Path,
) -> Result<(), AnyError> {
  if let Some(integrity) = &dist_info.integrity {
    verify_tarball_integrity(package, data, integrity)?;
  } else {
    // todo(dsherret): check shasum here
    bail!(
      "Errored on '{}': npm packages with no integrity are not implemented.",
      package
    );
  }

  fs::create_dir_all(output_folder).with_context(|| {
    format!("Error creating '{}'.", output_folder.display())
  })?;

  // This sync lock file is a way to ensure that partially created
  // npm package directories aren't considered valid. This could maybe
  // be a bit smarter in the future to not bother extracting here
  // if another process has taken the lock in the past X seconds and
  // wait for the other process to finish (it could try to create the
  // file with `create_new(true)` then if it exists, check the metadata
  // then wait until the other process finishes with a timeout), but
  // for now this is good enough.
  let sync_lock_path = output_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME);
  match fs::OpenOptions::new()
    .write(true)
    .create(true)
    .open(&sync_lock_path)
  {
    Ok(_) => {
      extract_tarball(data, output_folder)?;
      // extraction succeeded, so only now delete this file
      let _ignore = std::fs::remove_file(&sync_lock_path);
      Ok(())
    }
    Err(err) => {
      bail!(
        concat!(
          "Error creating package sync lock file at '{}'. ",
          "Maybe try manually deleting this folder.\n\n{:#}",
        ),
        output_folder.display(),
        err
      );
    }
  }
}

fn verify_tarball_integrity(
  package: &NpmPackageId,
  data: &[u8],
  npm_integrity: &str,
) -> Result<(), AnyError> {
  use ring::digest::Context;
  use ring::digest::SHA512;
  let (algo, expected_checksum) = match npm_integrity.split_once('-') {
    Some((hash_kind, checksum)) => {
      let algo = match hash_kind {
        "sha512" => &SHA512,
        hash_kind => bail!(
          "Not implemented hash function for {}: {}",
          package,
          hash_kind
        ),
      };
      (algo, checksum.to_lowercase())
    }
    None => bail!(
      "Not implemented integrity kind for {}: {}",
      package,
      npm_integrity
    ),
  };

  let mut hash_ctx = Context::new(algo);
  hash_ctx.update(data);
  let digest = hash_ctx.finish();
  let tarball_checksum = base64::encode(digest.as_ref()).to_lowercase();
  if tarball_checksum != expected_checksum {
    bail!(
      "Tarball checksum did not match what was provided by npm registry for {}.\n\nExpected: {}\nActual: {}",
      package,
      expected_checksum,
      tarball_checksum,
    )
  }
  Ok(())
}

fn extract_tarball(data: &[u8], output_folder: &Path) -> Result<(), AnyError> {
  fs::create_dir_all(output_folder)?;
  let output_folder = fs::canonicalize(output_folder)?;
  let tar = GzDecoder::new(data);
  let mut archive = Archive::new(tar);
  archive.set_overwrite(true);
  archive.set_preserve_permissions(true);
  let mut created_dirs = HashSet::new();

  for entry in archive.entries()? {
    let mut entry = entry?;
    let path = entry.path()?;
    let entry_type = entry.header().entry_type();
    // skip the first component which will be either "package" or the name of the package
    let relative_path = path.components().skip(1).collect::<PathBuf>();
    let absolute_path = output_folder.join(relative_path);
    let dir_path = if entry_type == EntryType::Directory {
      absolute_path.as_path()
    } else {
      absolute_path.parent().unwrap()
    };
    if created_dirs.insert(dir_path.to_path_buf()) {
      fs::create_dir_all(&dir_path)?;
      let canonicalized_dir = fs::canonicalize(&dir_path)?;
      if !canonicalized_dir.starts_with(&output_folder) {
        bail!(
          "Extracted directory '{}' of npm tarball was not in output directory.",
          canonicalized_dir.display()
        )
      }
    }
    if entry.header().entry_type() == EntryType::Regular {
      entry.unpack(&absolute_path)?;
    }
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  pub fn test_verify_tarball() {
    let package_id = NpmPackageId {
      name: "package".to_string(),
      version: semver::Version::parse("1.0.0").unwrap(),
    };
    let actual_checksum =
      "z4phnx7vul3xvchq1m2ab9yg5aulvxxcg/spidns6c5h0ne8xyxysp+dgnkhfuwvy7kxvudbeoglodj6+sfapg==";
    assert_eq!(
      verify_tarball_integrity(&package_id, &Vec::new(), "test")
        .unwrap_err()
        .to_string(),
      "Not implemented integrity kind for package@1.0.0: test",
    );
    assert_eq!(
      verify_tarball_integrity(&package_id, &Vec::new(), "sha1-test")
        .unwrap_err()
        .to_string(),
      "Not implemented hash function for package@1.0.0: sha1",
    );
    assert_eq!(
      verify_tarball_integrity(&package_id, &Vec::new(), "sha512-test")
        .unwrap_err()
        .to_string(),
      format!("Tarball checksum did not match what was provided by npm registry for package@1.0.0.\n\nExpected: test\nActual: {}", actual_checksum),
    );
    assert!(verify_tarball_integrity(
      &package_id,
      &Vec::new(),
      &format!("sha512-{}", actual_checksum)
    )
    .is_ok());
  }
}
