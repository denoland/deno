// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use flate2::read::GzDecoder;
use tar::Archive;
use tar::EntryType;

use super::cache::with_folder_sync_lock;
use super::registry::NpmPackageVersionDistInfo;
use super::semver::NpmVersion;

pub fn verify_and_extract_tarball(
  package: (&str, &NpmVersion),
  data: &[u8],
  dist_info: &NpmPackageVersionDistInfo,
  output_folder: &Path,
) -> Result<(), AnyError> {
  verify_tarball_integrity(package, data, &dist_info.integrity())?;

  with_folder_sync_lock(package, output_folder, || {
    extract_tarball(data, output_folder)
  })
}

fn verify_tarball_integrity(
  package: (&str, &NpmVersion),
  data: &[u8],
  npm_integrity: &str,
) -> Result<(), AnyError> {
  use ring::digest::Context;
  let (algo, expected_checksum) = match npm_integrity.split_once('-') {
    Some((hash_kind, checksum)) => {
      let algo = match hash_kind {
        "sha512" => &ring::digest::SHA512,
        "sha1" => &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        hash_kind => bail!(
          "Not implemented hash function for {}@{}: {}",
          package.0,
          package.1,
          hash_kind
        ),
      };
      (algo, checksum.to_lowercase())
    }
    None => bail!(
      "Not implemented integrity kind for {}@{}: {}",
      package.0,
      package.1,
      npm_integrity
    ),
  };

  let mut hash_ctx = Context::new(algo);
  hash_ctx.update(data);
  let digest = hash_ctx.finish();
  let tarball_checksum = base64::encode(digest.as_ref()).to_lowercase();
  if tarball_checksum != expected_checksum {
    bail!(
      "Tarball checksum did not match what was provided by npm registry for {}@{}.\n\nExpected: {}\nActual: {}",
      package.0,
      package.1,
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

    // Some package tarballs contain "pax_global_header", these entries
    // should be skipped.
    if entry_type == EntryType::XGlobalHeader {
      continue;
    }

    // skip the first component which will be either "package" or the name of the package
    let relative_path = path.components().skip(1).collect::<PathBuf>();
    let absolute_path = output_folder.join(relative_path);
    let dir_path = if entry_type == EntryType::Directory {
      absolute_path.as_path()
    } else {
      absolute_path.parent().unwrap()
    };
    if created_dirs.insert(dir_path.to_path_buf()) {
      fs::create_dir_all(dir_path)?;
      let canonicalized_dir = fs::canonicalize(dir_path)?;
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
  use crate::npm::semver::NpmVersion;

  #[test]
  pub fn test_verify_tarball() {
    let package_name = "package".to_string();
    let package_version = NpmVersion::parse("1.0.0").unwrap();
    let package = (package_name.as_str(), &package_version);
    let actual_checksum =
      "z4phnx7vul3xvchq1m2ab9yg5aulvxxcg/spidns6c5h0ne8xyxysp+dgnkhfuwvy7kxvudbeoglodj6+sfapg==";
    assert_eq!(
      verify_tarball_integrity(package, &Vec::new(), "test")
        .unwrap_err()
        .to_string(),
      "Not implemented integrity kind for package@1.0.0: test",
    );
    assert_eq!(
      verify_tarball_integrity(package, &Vec::new(), "notimplemented-test")
        .unwrap_err()
        .to_string(),
      "Not implemented hash function for package@1.0.0: notimplemented",
    );
    assert_eq!(
      verify_tarball_integrity(package, &Vec::new(), "sha1-test")
        .unwrap_err()
        .to_string(),
      concat!(
        "Tarball checksum did not match what was provided by npm ",
        "registry for package@1.0.0.\n\nExpected: test\nActual: 2jmj7l5rsw0yvb/vlwaykk/ybwk=",
      ),
    );
    assert_eq!(
      verify_tarball_integrity(package, &Vec::new(), "sha512-test")
        .unwrap_err()
        .to_string(),
      format!("Tarball checksum did not match what was provided by npm registry for package@1.0.0.\n\nExpected: test\nActual: {}", actual_checksum),
    );
    assert!(verify_tarball_integrity(
      package,
      &Vec::new(),
      &format!("sha512-{}", actual_checksum)
    )
    .is_ok());
  }
}
