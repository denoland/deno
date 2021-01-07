// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module provides feature to upgrade deno executable

use crate::AnyError;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::deno_fetch::reqwest::Client;
use semver_parser::version::parse as semver_parse;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

lazy_static! {
  static ref ARCHIVE_NAME: String = format!("deno-{}.zip", env!("TARGET"));
}

const RELEASE_URL: &str = "https://github.com/denoland/deno/releases";

pub async fn upgrade_command(
  dry_run: bool,
  force: bool,
  canary: bool,
  version: Option<String>,
  output: Option<PathBuf>,
  ca_file: Option<String>,
) -> Result<(), AnyError> {
  let mut client_builder = Client::builder();

  // If we have been provided a CA Certificate, add it into the HTTP client
  if let Some(ca_file) = ca_file {
    let buf = std::fs::read(ca_file)?;
    let cert = reqwest::Certificate::from_pem(&buf)?;
    client_builder = client_builder.add_root_certificate(cert);
  }

  let client = client_builder.build()?;

  let install_version = match version {
    Some(passed_version) => {
      let current_is_passed = if canary {
        let mut passed_hash = passed_version.clone();
        passed_hash.truncate(7);
        crate::version::GIT_COMMIT_HASH == passed_hash
      } else if !crate::version::is_canary() {
        crate::version::deno() == passed_version
      } else {
        false
      };

      if !force && output.is_none() && current_is_passed {
        println!("Version {} is already installed", crate::version::deno());
        return Ok(());
      } else {
        passed_version
      }
    }
    None => {
      let latest_version = if canary {
        get_latest_canary_version(&client).await?
      } else {
        get_latest_release_version(&client).await?
      };

      let current_is_most_recent = if canary {
        let mut latest_hash = latest_version.clone();
        latest_hash.truncate(7);
        crate::version::GIT_COMMIT_HASH == latest_hash
      } else if !crate::version::is_canary() {
        let current = semver_parse(&*crate::version::deno()).unwrap();
        let latest = match semver_parse(&latest_version) {
          Ok(v) => v,
          Err(_) => {
            eprintln!("Invalid semver passed");
            std::process::exit(1)
          }
        };
        current >= latest
      } else {
        false
      };

      if !force && output.is_none() && current_is_most_recent {
        println!(
          "Local deno version {} is the most recent release",
          crate::version::deno()
        );
        return Ok(());
      } else {
        println!("Found latest version {}", &latest_version);
        latest_version
      }
    }
  };

  let download_url = if canary {
    format!(
      "https://dl.deno.land/canary/{}/{}",
      install_version, *ARCHIVE_NAME
    )
  } else {
    format!(
      "{}/download/v{}/{}",
      RELEASE_URL, install_version, *ARCHIVE_NAME
    )
  };

  let archive_data = download_package(client, &*download_url).await?;

  println!("Deno is upgrading to version {}", &install_version);

  let old_exe_path = std::env::current_exe()?;
  let new_exe_path = unpack(archive_data)?;
  let permissions = fs::metadata(&old_exe_path)?.permissions();
  fs::set_permissions(&new_exe_path, permissions)?;
  check_exe(&new_exe_path)?;

  if !dry_run {
    match output {
      Some(path) => {
        fs::rename(&new_exe_path, &path)
          .or_else(|_| fs::copy(&new_exe_path, &path).map(|_| ()))?;
      }
      None => replace_exe(&new_exe_path, &old_exe_path)?,
    }
  }

  println!("Upgraded successfully");

  Ok(())
}

async fn get_latest_release_version(
  client: &Client,
) -> Result<String, AnyError> {
  println!("Looking up latest version");

  let res = client
    .get(&format!("{}/latest", RELEASE_URL))
    .send()
    .await?;
  let version = res.url().path_segments().unwrap().last().unwrap();

  Ok(version.replace("v", ""))
}

async fn get_latest_canary_version(
  client: &Client,
) -> Result<String, AnyError> {
  println!("Looking up latest version");

  let res = client
    .get("https://dl.deno.land/canary-latest.txt")
    .send()
    .await?;
  let version = res.text().await?.trim().to_string();

  Ok(version)
}

async fn download_package(
  client: Client,
  download_url: &str,
) -> Result<Vec<u8>, AnyError> {
  println!("Checking {}", &download_url);

  let res = client.get(download_url).send().await?;

  if res.status().is_success() {
    println!("Download has been found");
    Ok(res.bytes().await?.to_vec())
  } else {
    println!("Download could not be found, aborting");
    std::process::exit(1)
  }
}

fn unpack(archive_data: Vec<u8>) -> Result<PathBuf, std::io::Error> {
  // We use into_path so that the tempdir is not automatically deleted. This is
  // useful for debugging upgrade, but also so this function can return a path
  // to the newly uncompressed file without fear of the tempdir being deleted.
  let temp_dir = TempDir::new()?.into_path();
  let exe_ext = if cfg!(windows) { "exe" } else { "" };
  let exe_path = temp_dir.join("deno").with_extension(exe_ext);
  assert!(!exe_path.exists());

  let archive_ext = Path::new(&*ARCHIVE_NAME)
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap();
  let unpack_status = match archive_ext {
    "zip" if cfg!(windows) => {
      let archive_path = temp_dir.join("deno.zip");
      fs::write(&archive_path, &archive_data)?;
      Command::new("powershell.exe")
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(
          "& {
            param($Path, $DestinationPath)
            trap { $host.ui.WriteErrorLine($_.Exception); exit 1 }
            Add-Type -AssemblyName System.IO.Compression.FileSystem
            [System.IO.Compression.ZipFile]::ExtractToDirectory(
              $Path,
              $DestinationPath
            );
          }",
        )
        .arg("-Path")
        .arg(format!("'{}'", &archive_path.to_str().unwrap()))
        .arg("-DestinationPath")
        .arg(format!("'{}'", &temp_dir.to_str().unwrap()))
        .spawn()?
        .wait()?
    }
    "zip" => {
      let archive_path = temp_dir.join("deno.zip");
      fs::write(&archive_path, &archive_data)?;
      Command::new("unzip")
        .current_dir(&temp_dir)
        .arg(archive_path)
        .spawn()?
        .wait()?
    }
    ext => panic!("Unsupported archive type: '{}'", ext),
  };
  assert!(unpack_status.success());
  assert!(exe_path.exists());
  Ok(exe_path)
}

fn replace_exe(new: &Path, old: &Path) -> Result<(), std::io::Error> {
  if cfg!(windows) {
    // On windows you cannot replace the currently running executable.
    // so first we rename it to deno.old.exe
    fs::rename(old, old.with_extension("old.exe"))?;
  } else {
    fs::remove_file(old)?;
  }
  // Windows cannot rename files across device boundaries, so if rename fails,
  // we try again with copy.
  fs::rename(new, old).or_else(|_| fs::copy(new, old).map(|_| ()))?;
  Ok(())
}

fn check_exe(exe_path: &Path) -> Result<(), AnyError> {
  let output = Command::new(exe_path)
    .arg("-V")
    .stderr(std::process::Stdio::inherit())
    .output()?;
  assert!(output.status.success());
  Ok(())
}
