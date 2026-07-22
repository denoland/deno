// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;

use deno_config::deno_json::ConfigFile;
use deno_core::error::AnyError;
use flate2::Compression;
use flate2::write::GzEncoder;

use super::AssetFile;
use super::ProcessedFile;
use super::ReadmeOrLicense;
use super::extensions::js_to_dts_extension;

/// Compute the default tarball filename (`scope-name-version.tgz`) that
/// `pack` writes when no `--output` is given. Extracted so the asset
/// collector can exclude this exact path (instead of guessing by `.tgz`
/// extension) and so the name is computed in exactly one place.
pub fn default_tarball_filename(
  config_file: &ConfigFile,
  version: &str,
) -> Result<PathBuf, AnyError> {
  let name = config_file
    .json
    .name
    .as_ref()
    .ok_or_else(|| deno_core::anyhow::anyhow!("Missing name"))?;
  // Convert @scope/name to scope-name
  let normalized = name.replace('@', "").replace('/', "-");
  // The package name shape is checked against `@scope/name` higher up
  // (see `pack` in mod.rs), but that check is loose — it does not
  // forbid path-traversal sequences. Treat this as a hard safety
  // boundary right before we open a file, rejecting any derived
  // tarball name that contains `..` or path separators so we never
  // escape the cwd regardless of upstream validation drift.
  if normalized.contains("..") || normalized.contains('/') {
    return Err(deno_core::anyhow::anyhow!(
      "refusing to write tarball with unsafe name derived from package: {}",
      name
    ));
  }
  Ok(PathBuf::from(format!("{}-{}.tgz", normalized, version)))
}

/// Tar archive paths must use forward slashes, even on Windows. Output paths
/// are computed with platform separators when they pass through `Path::display`,
/// so normalize before writing the tar header. The replace is unconditional, so
/// on POSIX — where a backslash is a legal filename character — it can split one
/// legal name into several archive segments. `validate_tar_path` runs on the
/// result to catch any `..` this introduces.
fn to_tar_path(relative: &str) -> String {
  relative.replace('\\', "/")
}

/// Reject archive paths that could escape the extraction root.
///
/// `tar::Builder::append_data` only validates the truncated 100-byte header
/// name when it emits a GNU LongLink entry; the full path is written to the
/// LongLink data unchecked. So a path >= 100 bytes can smuggle `..` past the
/// crate's own checks, while a short path is still rejected by `set_path`.
/// Validate here so both lengths behave the same.
///
/// Segments are split off the normalized `&str` rather than via
/// `std::path::Path::components()`: `Path` parsing is platform-dependent and
/// would treat these inputs differently on Windows.
fn validate_tar_path(path: &str) -> std::io::Result<()> {
  let invalid = |reason: &str| {
    std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      format!("refusing to add {} to tarball: {}", path, reason),
    )
  };
  let mut segments = path.split('/').peekable();
  // A leading "./" is fine; a leading empty segment means the path is absolute.
  if segments.peek() == Some(&".") {
    segments.next();
  }
  for segment in segments {
    match segment {
      ".." => return Err(invalid("path contains a '..' component")),
      "" => return Err(invalid("path is absolute or empty")),
      "." => return Err(invalid("path contains a '.' component")),
      _ => {}
    }
  }
  Ok(())
}

/// Append a file entry with a fixed mode/mtime/uid/gid so two pack runs over
/// the same source produce a bit-identical tarball. Matches `npm pack`'s
/// reproducibility guarantees.
fn append_reproducible(
  tar: &mut tar::Builder<impl std::io::Write>,
  path: &str,
  bytes: &[u8],
) -> std::io::Result<()> {
  let path = to_tar_path(path);
  validate_tar_path(&path)?;
  let mut header = tar::Header::new_gnu();
  header.set_size(bytes.len() as u64);
  header.set_mode(0o644);
  header.set_mtime(0);
  header.set_uid(0);
  header.set_gid(0);
  // `append_data` (not `Header::set_path`) so paths that don't fit the 100-byte
  // tar name field transparently get a GNU `././@LongLink` entry instead of an
  // error. The LongLink header it emits is itself fixed mode/uid/gid/mtime 0,
  // so the tarball stays byte-reproducible.
  tar.append_data(&mut header, path, bytes)
}

#[allow(clippy::too_many_arguments, reason = "tarball assembly inputs")]
pub fn create_npm_tarball(
  config_file: &ConfigFile,
  version: &str,
  files: &[ProcessedFile],
  package_json: &str,
  readme_license_files: &[ReadmeOrLicense],
  asset_files: &[AssetFile],
  output_path: Option<&str>,
  dry_run: bool,
) -> Result<PathBuf, AnyError> {
  // Compute output filename
  let filename = if let Some(path) = output_path {
    let p = PathBuf::from(path);
    if p.components().any(|c| c == std::path::Component::ParentDir) {
      log::warn!("Output path '{}' contains '..' components", path);
    }
    p
  } else {
    default_tarball_filename(config_file, version)?
  };

  if dry_run {
    log::info!("Dry run - would create: {}", filename.display());
    log::info!("\nPackage contents:");
    log::info!("  package.json");
    for file in readme_license_files {
      log::info!("  {}", file.relative_path);
    }
    for file in files {
      log::info!("  {}", file.output_path);
      if file.dts_content.is_some() {
        let dts_path = js_to_dts_extension(&file.output_path);
        log::info!("  {}", dts_path);
      }
    }
    for asset in asset_files {
      log::info!("  {}", asset.relative_path);
    }
    return Ok(filename);
  }

  // Create tarball
  let tar_file = std::fs::File::create(&filename)?;
  let enc = GzEncoder::new(tar_file, Compression::default());
  let mut tar = tar::Builder::new(enc);

  append_reproducible(
    &mut tar,
    "package/package.json",
    package_json.as_bytes(),
  )?;

  for file in readme_license_files {
    append_reproducible(
      &mut tar,
      &format!("package/{}", file.relative_path),
      file.content.as_slice(),
    )?;
  }

  for file in files {
    append_reproducible(
      &mut tar,
      &format!("package/{}", file.output_path),
      file.js_content.as_bytes(),
    )?;

    if let Some(ref dts) = file.dts_content {
      let dts_path = js_to_dts_extension(&file.output_path);
      append_reproducible(
        &mut tar,
        &format!("package/{}", dts_path),
        dts.as_bytes(),
      )?;
    }
  }

  for asset in asset_files {
    append_reproducible(
      &mut tar,
      &format!("package/{}", asset.relative_path),
      asset.content.as_slice(),
    )?;
  }

  tar.finish()?;

  Ok(filename)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// 114 bytes — longer than the 100-byte tar `name` field, so this only
  /// round-trips if a GNU LongLink entry is emitted. See issue #36008.
  const LONG_PATH: &str = "package/src/kubernetes/models/_schemas/IoK8sApiAdmissionregistrationV1beta1ValidatingAdmissionPolicyBindingList.js";

  #[test]
  fn long_traversal_path_is_rejected() {
    // >= 100 bytes, so `append_data` takes the GNU LongLink branch where the
    // tar crate does not validate the full path itself.
    let path = "package/src/kubernetes/models/_schemas/IoK8sApiAdmissionregistrationV1beta1ValidatingAdmissionPolicy\\..\\..\\outside.js";
    assert!(to_tar_path(path).len() >= 100);
    let mut tar = tar::Builder::new(Vec::new());
    let err = append_reproducible(&mut tar, path, b"x").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("'..'"), "{}", err);
  }

  #[test]
  fn short_traversal_path_is_rejected() {
    // Short paths error inside the tar crate; they must error here too, so long
    // and short agree instead of only the short case being safe.
    let mut tar = tar::Builder::new(Vec::new());
    let err =
      append_reproducible(&mut tar, "package/../outside.js", b"x").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
  }

  #[test]
  fn absolute_path_is_rejected() {
    let mut tar = tar::Builder::new(Vec::new());
    let err = append_reproducible(&mut tar, "/etc/passwd", b"x").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
  }

  fn build_tarball(paths: &[&str]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    for path in paths {
      append_reproducible(&mut tar, path, b"export const a = 1;\n").unwrap();
    }
    tar.into_inner().unwrap()
  }

  #[test]
  fn appends_path_longer_than_tar_name_field() {
    assert!(LONG_PATH.len() > 100);
    let bytes = build_tarball(&["package/package.json", LONG_PATH]);

    let paths: Vec<String> = tar::Archive::new(bytes.as_slice())
      .entries()
      .unwrap()
      .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned())
      .collect();

    assert_eq!(
      paths,
      vec!["package/package.json".to_string(), LONG_PATH.to_string()]
    );
  }

  #[test]
  fn tarball_is_reproducible_with_long_paths() {
    // The LongLink extension header must carry fixed mode/uid/gid/mtime too,
    // or long-path packages would lose byte-reproducibility.
    assert_eq!(build_tarball(&[LONG_PATH]), build_tarball(&[LONG_PATH]));
  }
}
