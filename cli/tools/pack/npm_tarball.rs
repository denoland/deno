// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;

use deno_config::deno_json::ConfigFile;
use deno_core::error::AnyError;
use flate2::write::GzEncoder;
use flate2::Compression;

use super::ProcessedFile;
use super::ReadmeOrLicense;

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
    PathBuf::from(path)
  } else {
    // Convert @scope/name to scope-name
    let normalized = name.replace('@', "").replace('/', "-");
    PathBuf::from(format!("{}-{}.tgz", normalized, version))
  };

  if dry_run {
    println!("Dry run - would create: {}", filename.display());
    println!("\nPackage contents:");
    println!("  package.json");
    for file in readme_license_files {
      println!("  {}", file.relative_path);
    }
    for file in files {
      println!("  {}", file.output_path);
      if file.dts_content.is_some() {
        let dts_path = convert_js_to_dts_path(&file.output_path);
        println!("  {}", dts_path);
      }
    }
    return Ok(filename);
  }

  // Create tarball
  let tar_file = std::fs::File::create(&filename)?;
  let enc = GzEncoder::new(tar_file, Compression::default());
  let mut tar = tar::Builder::new(enc);

  // Add package.json
  let package_json_bytes = package_json.as_bytes();
  let mut header = tar::Header::new_gnu();
  header.set_path("package/package.json")?;
  header.set_size(package_json_bytes.len() as u64);
  header.set_mode(0o644);
  header.set_cksum();
  tar.append(&header, package_json_bytes)?;

  // Add README and LICENSE files
  for file in readme_license_files {
    let mut header = tar::Header::new_gnu();
    header.set_path(format!("package/{}", file.relative_path))?;
    header.set_size(file.content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, file.content.as_slice())?;
  }

  // Add each file
  for file in files {
    // Add JS file
    let js_bytes = file.js_content.as_bytes();
    let mut header = tar::Header::new_gnu();
    header.set_path(format!("package/{}", file.output_path))?;
    header.set_size(js_bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, js_bytes)?;

    // Add .d.ts file if present
    if let Some(ref dts) = file.dts_content {
      let dts_bytes = dts.as_bytes();
      let dts_path = convert_js_to_dts_path(&file.output_path);
      let mut header = tar::Header::new_gnu();
      header.set_path(format!("package/{}", dts_path))?;
      header.set_size(dts_bytes.len() as u64);
      header.set_mode(0o644);
      header.set_cksum();
      tar.append(&header, dts_bytes)?;
    }
  }

  tar.finish()?;

  Ok(filename)
}

fn convert_js_to_dts_path(path: &str) -> String {
  if path.ends_with(".mjs") {
    format!("{}.d.mts", &path[..path.len() - 4])
  } else if path.ends_with(".js") {
    format!("{}.d.ts", &path[..path.len() - 3])
  } else {
    format!("{}.d.ts", path)
  }
}
