// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module provides feature to upgrade deno executable
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

use crate::http_util::fetch_once;
use crate::http_util::FetchOnceResult;
use crate::AnyError;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_fetch::reqwest;
use deno_fetch::reqwest::redirect::Policy;
use deno_fetch::reqwest::Client;
use semver_parser::version::parse as semver_parse;
use std::fs;
use std::future::Future;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Command;
use std::process::Stdio;
use std::string::String;
use tempfile::TempDir;

// TODO(ry) Auto detect target triples for the uploaded files.
#[cfg(windows)]
const ARCHIVE_NAME: &str = "deno-x86_64-pc-windows-msvc.zip";
#[cfg(target_os = "macos")]
const ARCHIVE_NAME: &str = "deno-x86_64-apple-darwin.zip";
#[cfg(target_os = "linux")]
const ARCHIVE_NAME: &str = "deno-x86_64-unknown-linux-gnu.zip";

async fn get_latest_version(
  client: &Client,
  release_url: &str,
) -> Result<String, AnyError> {
  println!("Checking for latest version");
  let res = client
    .get(Url::parse(&*format!("{}/latest", release_url))?)
    .send()
    .await?;
  let version = res
    .url()
    .path_segments()
    .unwrap()
    .last()
    .unwrap();

  Ok(version.replace("v", ""))
}

/// Asynchronously updates deno executable to greatest version
/// if greatest version is available.
pub async fn upgrade_command(
  dry_run: bool,
  force: bool,
  nightly: bool,
  version: Option<String>,
  output: Option<PathBuf>,
  ca_file: Option<String>,
) -> Result<(), AnyError> {
  let mut client_builder = Client::builder().redirect(Policy::none());

  // If we have been provided a CA Certificate, add it into the HTTP client
  if let Some(ca_file) = ca_file {
    let buf = std::fs::read(ca_file);
    let cert = reqwest::Certificate::from_pem(&buf.unwrap())?;
    client_builder = client_builder.add_root_certificate(cert);
  }

  let client = client_builder.build()?;

  let current_version = crate::version::deno();

  let release_url = if nightly {
    "https://github.com/denoland/deno/releases"
  } else {
    "https://github.com/denoland/deno_nightly/releases"
  };

  let install_version = match version {
    Some(passed_version) => {
      if !force && current_version == passed_version {
        println!("Version {} is already installed", &passed_version);
        return Ok(());
      } else {
        passed_version
      }
    }
    None => {
      let latest_version = get_latest_version(&client, release_url).await?;

      let current_is_most_recent = if nightly && crate::version::is_nightly() {
        let current = current_version.replace(".", "").parse::<u32>().unwrap();
        let latest = latest_version.replace(".", "").parse::<u32>().unwrap();
        current >= latest
      } else if !nightly && !crate::version::is_nightly() {
        let current = semver_parse(current_version).unwrap();
        let latest = semver_parse(&*latest_version).unwrap();
        current >= latest
      } else {
        false
      };

      if !force && current_is_most_recent {
        println!(
          "Local deno version {} is the most recent release",
          &current_version
        );
        return Ok(())
      } else {
        latest_version
      }
    }
  };

  let archive_data = download_package(
    &compose_url_to_exec(release_url, &install_version)?,
    client,
    &install_version,
  )
  .await?;
  let old_exe_path = std::env::current_exe()?;
  let new_exe_path = unpack(archive_data)?;
  let permissions = fs::metadata(&old_exe_path)?.permissions();
  fs::set_permissions(&new_exe_path, permissions)?;
  check_exe(&new_exe_path, &install_version)?;

  if !dry_run {
    match output {
      Some(path) => {
        fs::rename(&new_exe_path, &path)
          .or_else(|_| fs::copy(&new_exe_path, &path).map(|_| ()))?;
      }
      None => replace_exe(&new_exe_path, &old_exe_path)?,
    }
  }

  println!("Upgrade done successfully");

  Ok(())
}

fn download_package(
  url: &Url,
  client: Client,
  version: &str,
) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, AnyError>>>> {
  println!("downloading {}", url);
  let url = url.clone();
  let version = version.clone();
  let fut = async move {
    match fetch_once(client.clone(), &url, None).await {
      Ok(result) => {
        println!(
          "Version has been found\nDeno is upgrading to version {}",
          &version
        );
        match result {
          FetchOnceResult::Code(source, _) => Ok(source),
          FetchOnceResult::NotModified => unreachable!(),
          FetchOnceResult::Redirect(url, _) => {
            download_package(&url, client, version).await
          }
        }
      }
      Err(_) => {
        println!("Version has not been found, aborting");
        std::process::exit(1)
      }
    }
  };
  fut.boxed_local()
}

fn compose_url_to_exec(
  release_url: &str,
  version: &str,
) -> Result<Url, AnyError> {
  let s = format!("{}/download/v{}/{}", release_url, version, ARCHIVE_NAME);
  Url::parse(&s).map_err(AnyError::from)
}

fn unpack(archive_data: Vec<u8>) -> Result<PathBuf, std::io::Error> {
  // We use into_path so that the tempdir is not automatically deleted. This is
  // useful for debugging upgrade, but also so this function can return a path
  // to the newly uncompressed file without fear of the tempdir being deleted.
  let temp_dir = TempDir::new()?.into_path();
  let exe_ext = if cfg!(windows) { "exe" } else { "" };
  let exe_path = temp_dir.join("deno").with_extension(exe_ext);
  assert!(!exe_path.exists());

  let archive_ext = Path::new(ARCHIVE_NAME)
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap();
  let unpack_status = match archive_ext {
    "gz" => {
      let exe_file = fs::File::create(&exe_path)?;
      let mut cmd = Command::new("gunzip")
        .arg("-c")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(exe_file))
        .spawn()?;
      cmd.stdin.as_mut().unwrap().write_all(&archive_data)?;
      cmd.wait()?
    }
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

fn check_exe(
  exe_path: &Path,
  expected_version: &str,
) -> Result<(), AnyError> {
  let output = Command::new(exe_path)
    .arg("-V")
    .stderr(std::process::Stdio::inherit())
    .output()?;
  let stdout = String::from_utf8(output.stdout)?;
  assert!(output.status.success());
  assert_eq!(stdout.trim(), format!("deno {}", expected_version));
  Ok(())
}

#[test]
fn test_compose_url_to_exec() {
  let v = String::from("0.0.1");
  let url =
    compose_url_to_exec("https://github.com/denoland/deno/releases/", &v)
      .unwrap();
  #[cfg(windows)]
  assert_eq!(url.as_str(), "https://github.com/denoland/deno/releases/download/v0.0.1/deno-x86_64-pc-windows-msvc.zip");
  #[cfg(target_os = "macos")]
  assert_eq!(
    url.as_str(),
    "https://github.com/denoland/deno/releases/download/v0.0.1/deno-x86_64-apple-darwin.zip"
  );
  #[cfg(target_os = "linux")]
  assert_eq!(url.as_str(), "https://github.com/denoland/deno/releases/download/v0.0.1/deno-x86_64-unknown-linux-gnu.zip");
}
