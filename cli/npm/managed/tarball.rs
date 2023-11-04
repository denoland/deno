// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_npm::registry::NpmPackageVersionDistInfo;
use deno_npm::registry::NpmPackageVersionDistInfoIntegrity;
use deno_semver::package::PackageNv;
use flate2::read::GzDecoder;
use tar::Archive;
use tar::EntryType;

use super::cache::with_folder_sync_lock;

pub fn verify_and_extract_tarball(
  package: &PackageNv,
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
  package: &PackageNv,
  data: &[u8],
  npm_integrity: &NpmPackageVersionDistInfoIntegrity,
) -> Result<(), AnyError> {
  use ring::digest::Context;
  let (tarball_checksum, expected_checksum) = match npm_integrity {
    NpmPackageVersionDistInfoIntegrity::Integrity {
      algorithm,
      base64_hash,
    } => {
      let algo = match *algorithm {
        "sha512" => &ring::digest::SHA512,
        "sha1" => &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        hash_kind => bail!(
          "Not implemented hash function for {}: {}",
          package,
          hash_kind
        ),
      };
      let mut hash_ctx = Context::new(algo);
      hash_ctx.update(data);
      let digest = hash_ctx.finish();
      let tarball_checksum = BASE64_STANDARD.encode(digest.as_ref());
      (tarball_checksum, base64_hash)
    }
    NpmPackageVersionDistInfoIntegrity::LegacySha1Hex(hex) => {
      let mut hash_ctx = Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY);
      hash_ctx.update(data);
      let digest = hash_ctx.finish();
      let tarball_checksum = hex::encode(digest.as_ref());
      (tarball_checksum, hex)
    }
    NpmPackageVersionDistInfoIntegrity::UnknownIntegrity(integrity) => {
      bail!(
        "Not implemented integrity kind for {}: {}",
        package,
        integrity
      )
    }
  };

  if tarball_checksum != *expected_checksum {
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

    let entry_type = entry.header().entry_type();
    match entry_type {
      EntryType::Regular => {
        entry.unpack(&absolute_path)?;
      }
      EntryType::Symlink | EntryType::Link => {
        // At the moment, npm doesn't seem to support uploading hardlinks or
        // symlinks to the npm registry. If ever adding symlink or hardlink
        // support, we will need to validate that the hardlink and symlink
        // target are within the package directory.
        log::warn!(
          "Ignoring npm tarball entry type {:?} for '{}'",
          entry_type,
          absolute_path.display()
        )
      }
      _ => {
        // ignore
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use deno_semver::Version;

  use super::*;

  #[test]
  pub fn test_verify_tarball() {
    let package = PackageNv {
      name: "package".to_string(),
      version: Version::parse_from_npm("1.0.0").unwrap(),
    };
    let actual_checksum =
      "z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==";
    assert_eq!(
      verify_tarball_integrity(
        &package,
        &Vec::new(),
        &NpmPackageVersionDistInfoIntegrity::UnknownIntegrity("test")
      )
      .unwrap_err()
      .to_string(),
      "Not implemented integrity kind for package@1.0.0: test",
    );
    assert_eq!(
      verify_tarball_integrity(
        &package,
        &Vec::new(),
        &NpmPackageVersionDistInfoIntegrity::Integrity {
          algorithm: "notimplemented",
          base64_hash: "test"
        }
      )
      .unwrap_err()
      .to_string(),
      "Not implemented hash function for package@1.0.0: notimplemented",
    );
    assert_eq!(
      verify_tarball_integrity(
        &package,
        &Vec::new(),
        &NpmPackageVersionDistInfoIntegrity::Integrity {
          algorithm: "sha1",
          base64_hash: "test"
        }
      )
      .unwrap_err()
      .to_string(),
      concat!(
        "Tarball checksum did not match what was provided by npm ",
        "registry for package@1.0.0.\n\nExpected: test\nActual: 2jmj7l5rSw0yVb/vlWAYkK/YBwk=",
      ),
    );
    assert_eq!(
      verify_tarball_integrity(
        &package,
        &Vec::new(),
        &NpmPackageVersionDistInfoIntegrity::Integrity {
          algorithm: "sha512",
          base64_hash: "test"
        }
      )
      .unwrap_err()
      .to_string(),
      format!("Tarball checksum did not match what was provided by npm registry for package@1.0.0.\n\nExpected: test\nActual: {actual_checksum}"),
    );
    assert!(verify_tarball_integrity(
      &package,
      &Vec::new(),
      &NpmPackageVersionDistInfoIntegrity::Integrity {
        algorithm: "sha512",
        base64_hash: actual_checksum,
      },
    )
    .is_ok());
    let actual_hex = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
    assert_eq!(
      verify_tarball_integrity(
        &package,
        &Vec::new(),
        &NpmPackageVersionDistInfoIntegrity::LegacySha1Hex("test"),
      )
      .unwrap_err()
      .to_string(),
      format!("Tarball checksum did not match what was provided by npm registry for package@1.0.0.\n\nExpected: test\nActual: {actual_hex}"),
    );
    assert!(verify_tarball_integrity(
      &package,
      &Vec::new(),
      &NpmPackageVersionDistInfoIntegrity::LegacySha1Hex(actual_hex),
    )
    .is_ok());
  }
}
