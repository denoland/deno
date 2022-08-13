// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! This module provides feature to upgrade deno executable

use crate::args::UpgradeFlags;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::deno_fetch::reqwest::Client;
use once_cell::sync::Lazy;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

static ARCHIVE_NAME: Lazy<String> =
  Lazy::new(|| format!("deno-{}.zip", env!("TARGET")));

const RELEASE_URL: &str = "https://github.com/denoland/deno/releases";

pub async fn upgrade(upgrade_flags: UpgradeFlags) -> Result<(), AnyError> {
  let old_exe_path = std::env::current_exe()?;
  let permissions = fs::metadata(&old_exe_path)?.permissions();

  if permissions.readonly() {
    bail!("You do not have write permission to {:?}", old_exe_path);
  }

  let mut client_builder = Client::builder();

  // If we have been provided a CA Certificate, add it into the HTTP client
  let ca_file = upgrade_flags.ca_file.or_else(|| env::var("DENO_CERT").ok());
  if let Some(ca_file) = ca_file {
    let buf = std::fs::read(ca_file)?;
    let cert = reqwest::Certificate::from_pem(&buf)?;
    client_builder = client_builder.add_root_certificate(cert);
  }

  let client = client_builder.build()?;

  let install_version = match upgrade_flags.version {
    Some(passed_version) => {
      if upgrade_flags.canary
        && !regex::Regex::new("^[0-9a-f]{40}$")?.is_match(&passed_version)
      {
        bail!("Invalid commit hash passed");
      } else if !upgrade_flags.canary
        && semver::Version::parse(&passed_version).is_err()
      {
        bail!("Invalid semver passed");
      }

      let current_is_passed = if upgrade_flags.canary {
        crate::version::GIT_COMMIT_HASH == passed_version
      } else if !crate::version::is_canary() {
        crate::version::deno() == passed_version
      } else {
        false
      };

      if !upgrade_flags.force
        && upgrade_flags.output.is_none()
        && current_is_passed
      {
        println!("Version {} is already installed", crate::version::deno());
        return Ok(());
      } else {
        passed_version
      }
    }
    None => {
      let latest_version = if upgrade_flags.canary {
        get_latest_canary_version(&client).await?
      } else {
        get_latest_release_version(&client).await?
      };

      let current_is_most_recent = if upgrade_flags.canary {
        let mut latest_hash = latest_version.clone();
        latest_hash.truncate(7);
        crate::version::GIT_COMMIT_HASH == latest_hash
      } else if !crate::version::is_canary() {
        let current = semver::Version::parse(&crate::version::deno()).unwrap();
        let latest = semver::Version::parse(&latest_version).unwrap();
        current >= latest
      } else {
        false
      };

      if !upgrade_flags.force
        && upgrade_flags.output.is_none()
        && current_is_most_recent
      {
        println!(
          "Local deno version {} is the most recent release",
          crate::version::deno()
        );
        return Ok(());
      } else {
        println!("Found latest version {}", latest_version);
        latest_version
      }
    }
  };

  let download_url = if upgrade_flags.canary {
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

  let archive_data = download_package(client, &download_url).await?;

  println!("Deno is upgrading to version {}", &install_version);

  let new_exe_path = unpack(archive_data, cfg!(windows))?;
  fs::set_permissions(&new_exe_path, permissions)?;
  check_exe(&new_exe_path)?;

  if !upgrade_flags.dry_run {
    match upgrade_flags.output {
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

  Ok(version.replace('v', ""))
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
    let total_size = res.content_length().unwrap() as f64;
    let mut current_size = 0.0;
    let mut data = Vec::with_capacity(total_size as usize);
    let mut stream = res.bytes_stream();
    let mut skip_print = 0;
    let is_tty = atty::is(atty::Stream::Stdout);
    const MEBIBYTE: f64 = 1024.0 * 1024.0;
    while let Some(item) = stream.next().await {
      let bytes = item?;
      current_size += bytes.len() as f64;
      data.extend_from_slice(&bytes);
      if skip_print == 0 {
        if is_tty {
          print!("\u{001b}[1G\u{001b}[2K");
        }
        print!(
          "{:>4.1} MiB / {:.1} MiB ({:^5.1}%)",
          current_size / MEBIBYTE,
          total_size / MEBIBYTE,
          (current_size / total_size) * 100.0,
        );
        std::io::stdout().flush()?;
        skip_print = 10;
      } else {
        skip_print -= 1;
      }
    }
    if is_tty {
      print!("\u{001b}[1G\u{001b}[2K");
    }
    println!(
      "{:.1} MiB / {:.1} MiB (100.0%)",
      current_size / MEBIBYTE,
      total_size / MEBIBYTE
    );

    Ok(data)
  } else {
    println!("Download could not be found, aborting");
    std::process::exit(1)
  }
}

pub fn unpack(
  archive_data: Vec<u8>,
  is_windows: bool,
) -> Result<PathBuf, std::io::Error> {
  const EXE_NAME: &str = "deno";
  // We use into_path so that the tempdir is not automatically deleted. This is
  // useful for debugging upgrade, but also so this function can return a path
  // to the newly uncompressed file without fear of the tempdir being deleted.
  let temp_dir = secure_tempfile::TempDir::new()?.into_path();
  let exe_ext = if is_windows { "exe" } else { "" };
  let archive_path = temp_dir.join(EXE_NAME).with_extension("zip");
  let exe_path = temp_dir.join(EXE_NAME).with_extension(exe_ext);
  assert!(!exe_path.exists());

  let archive_ext = Path::new(&*ARCHIVE_NAME)
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap();
  let unpack_status = match archive_ext {
    "zip" if cfg!(windows) => {
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
      fs::write(&archive_path, &archive_data)?;
      Command::new("unzip")
        .current_dir(&temp_dir)
        .arg(archive_path)
        .spawn()
        .map_err(|err| {
          if err.kind() == std::io::ErrorKind::NotFound {
            std::io::Error::new(
              std::io::ErrorKind::NotFound,
              "`unzip` was not found on your PATH, please install `unzip`",
            )
          } else {
            err
          }
        })?
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
