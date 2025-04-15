// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_npm::registry::NpmPackageVersionDistInfo;
use deno_npm::registry::NpmPackageVersionDistInfoIntegrity;
use deno_semver::package::PackageNv;
use flate2::read::GzDecoder;
use tar::Archive;
use tar::EntryType;

#[derive(Debug, Copy, Clone)]
pub enum TarballExtractionMode {
  /// Overwrites the destination directory without deleting any files.
  Overwrite,
  /// Creates and writes to a sibling temporary directory. When done, moves
  /// it to the final destination.
  ///
  /// This is more robust than `Overwrite` as it better handles multiple
  /// processes writing to the directory at the same time.
  SiblingTempDir,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum VerifyAndExtractTarballError {
  #[class(inherit)]
  #[error(transparent)]
  TarballIntegrity(#[from] TarballIntegrityError),
  #[class(inherit)]
  #[error(transparent)]
  ExtractTarball(#[from] ExtractTarballError),
  #[class(inherit)]
  #[error("Failed moving extracted tarball to final destination")]
  MoveFailed(std::io::Error),
}

pub fn verify_and_extract_tarball(
  package_nv: &PackageNv,
  data: &[u8],
  dist_info: &NpmPackageVersionDistInfo,
  output_folder: &Path,
  extraction_mode: TarballExtractionMode,
) -> Result<(), VerifyAndExtractTarballError> {
  verify_tarball_integrity(package_nv, data, &dist_info.integrity())?;

  match extraction_mode {
    TarballExtractionMode::Overwrite => {
      extract_tarball(data, output_folder).map_err(Into::into)
    }
    TarballExtractionMode::SiblingTempDir => {
      let temp_dir = get_atomic_dir_path(output_folder);
      extract_tarball(data, &temp_dir)?;
      rename_with_retries(&temp_dir, output_folder)
        .map_err(VerifyAndExtractTarballError::MoveFailed)
    }
  }
}

fn rename_with_retries(
  temp_dir: &Path,
  output_folder: &Path,
) -> Result<(), std::io::Error> {
  fn already_exists(err: &std::io::Error, output_folder: &Path) -> bool {
    // Windows will do an "Access is denied" error
    err.kind() == ErrorKind::AlreadyExists || output_folder.exists()
  }

  let mut count = 0;
  // renaming might be flaky if a lot of processes are trying
  // to do this, so retry a few times
  loop {
    match fs::rename(temp_dir, output_folder) {
      Ok(_) => return Ok(()),
      Err(err) if already_exists(&err, output_folder) => {
        // another process copied here, just cleanup
        let _ = fs::remove_dir_all(temp_dir);
        return Ok(());
      }
      Err(err) => {
        count += 1;
        if count > 5 {
          // too many retries, cleanup and return the error
          let _ = fs::remove_dir_all(temp_dir);
          return Err(err);
        }

        // wait a bit before retrying... this should be very rare or only
        // in error cases, so ok to sleep a bit
        let sleep_ms = std::cmp::min(100, 20 * count);
        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
      }
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
pub enum TarballIntegrityError {
  #[error("Not implemented hash function for {package}: {hash_kind}")]
  NotImplementedHashFunction {
    package: Box<PackageNv>,
    hash_kind: String,
  },
  #[error("Not implemented integrity kind for {package}: {integrity}")]
  NotImplementedIntegrityKind {
    package: Box<PackageNv>,
    integrity: String,
  },
  #[error("Tarball checksum did not match what was provided by npm registry for {package}.\n\nExpected: {expected}\nActual: {actual}")]
  MismatchedChecksum {
    package: Box<PackageNv>,
    expected: String,
    actual: String,
  },
}

fn verify_tarball_integrity(
  package: &PackageNv,
  data: &[u8],
  npm_integrity: &NpmPackageVersionDistInfoIntegrity,
) -> Result<(), TarballIntegrityError> {
  use ring::digest::Context;
  let (tarball_checksum, expected_checksum) = match npm_integrity {
    NpmPackageVersionDistInfoIntegrity::Integrity {
      algorithm,
      base64_hash,
    } => {
      let algo = match *algorithm {
        "sha512" => &ring::digest::SHA512,
        "sha1" => &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        hash_kind => {
          return Err(TarballIntegrityError::NotImplementedHashFunction {
            package: Box::new(package.clone()),
            hash_kind: hash_kind.to_string(),
          });
        }
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
      let tarball_checksum = faster_hex::hex_string(digest.as_ref());
      (tarball_checksum, hex)
    }
    NpmPackageVersionDistInfoIntegrity::UnknownIntegrity(integrity) => {
      return Err(TarballIntegrityError::NotImplementedIntegrityKind {
        package: Box::new(package.clone()),
        integrity: integrity.to_string(),
      });
    }
    NpmPackageVersionDistInfoIntegrity::None => {
      return Ok(());
    }
  };

  if tarball_checksum != *expected_checksum {
    return Err(TarballIntegrityError::MismatchedChecksum {
      package: Box::new(package.clone()),
      expected: expected_checksum.to_string(),
      actual: tarball_checksum,
    });
  }
  Ok(())
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ExtractTarballError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(generic)]
  #[error(
    "Extracted directory '{0}' of npm tarball was not in output directory."
  )]
  NotInOutputDirectory(PathBuf),
}

fn extract_tarball(
  data: &[u8],
  output_folder: &Path,
) -> Result<(), ExtractTarballError> {
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
        return Err(ExtractTarballError::NotInOutputDirectory(
          canonicalized_dir.to_path_buf(),
        ));
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

fn get_atomic_dir_path(file_path: &Path) -> PathBuf {
  let rand = gen_rand_path_component();
  let new_file_name = format!(
    ".{}_{}",
    file_path
      .file_name()
      .map(|f| f.to_string_lossy())
      .unwrap_or(Cow::Borrowed("")),
    rand
  );
  file_path.with_file_name(new_file_name)
}

fn gen_rand_path_component() -> String {
  use std::fmt::Write;
  (0..4).fold(String::with_capacity(8), |mut output, _| {
    write!(&mut output, "{:02x}", rand::random::<u8>()).unwrap();
    output
  })
}

#[cfg(test)]
mod test {
  use deno_semver::Version;
  use tempfile::TempDir;

  use super::*;

  #[test]
  pub fn test_verify_tarball() {
    let package = PackageNv {
      name: "package".into(),
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

  #[test]
  fn rename_with_retries_succeeds_exists() {
    let temp_dir = TempDir::new().unwrap();
    let folder_1 = temp_dir.path().join("folder_1");
    let folder_2 = temp_dir.path().join("folder_2");

    std::fs::create_dir_all(&folder_1).unwrap();
    std::fs::write(folder_1.join("a.txt"), "test").unwrap();
    std::fs::create_dir_all(&folder_2).unwrap();
    // this will not end up in the output as rename_with_retries assumes
    // the folders ending up at the destination are the same
    std::fs::write(folder_2.join("b.txt"), "test2").unwrap();

    let dest_folder = temp_dir.path().join("dest_folder");

    rename_with_retries(folder_1.as_path(), &dest_folder).unwrap();
    rename_with_retries(folder_2.as_path(), &dest_folder).unwrap();
    assert!(dest_folder.join("a.txt").exists());
    assert!(!dest_folder.join("b.txt").exists());
  }
}
