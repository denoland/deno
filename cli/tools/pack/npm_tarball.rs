// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;

use deno_config::deno_json::ConfigFile;
use deno_core::error::AnyError;
use flate2::Compression;
use flate2::write::GzEncoder;

use super::ProcessedFile;
use super::ReadmeOrLicense;
use super::extensions::js_to_dts_extension;

/// Tar archive paths must use forward slashes, even on Windows. Output paths
/// are computed with platform separators when they pass through `Path::display`,
/// so normalize before writing the tar header. Unconditional — backslash is
/// not a valid path character on POSIX, so a plain replace is a no-op there.
fn to_tar_path(relative: &str) -> String {
  relative.replace('\\', "/")
}

/// Append a file entry with a fixed mode/mtime/uid/gid so two pack runs over
/// the same source produce a bit-identical tarball. Matches `npm pack`'s
/// reproducibility guarantees.
fn append_reproducible(
  tar: &mut tar::Builder<impl std::io::Write>,
  path: &str,
  bytes: &[u8],
) -> std::io::Result<()> {
  let mut header = tar::Header::new_gnu();
  header.set_path(to_tar_path(path))?;
  header.set_size(bytes.len() as u64);
  header.set_mode(0o644);
  header.set_mtime(0);
  header.set_uid(0);
  header.set_gid(0);
  header.set_cksum();
  tar.append(&header, bytes)
}

pub fn create_npm_tarball(
  config_file: &ConfigFile,
  version: &str,
  files: &[ProcessedFile],
  package_json: &str,
  readme_license_files: &[ReadmeOrLicense],
  output_path: Option<&str>,
  dry_run: bool,
) -> Result<PathBuf, AnyError> {
  let name = config_file
    .json
    .name
    .as_ref()
    .ok_or_else(|| deno_core::anyhow::anyhow!("Missing name"))?;

  // Compute output filename
  let filename = if let Some(path) = output_path {
    let p = PathBuf::from(path);
    if p.components().any(|c| c == std::path::Component::ParentDir) {
      log::warn!(
        "Output path '{}' contains '..' components",
        path
      );
    }
    p
  } else {
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
    PathBuf::from(format!("{}-{}.tgz", normalized, version))
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

  tar.finish()?;

  Ok(filename)
}
