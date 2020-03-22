// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module provides feature to upgrade deno executable
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

extern crate semver_parser;
use crate::futures::FutureExt;
use crate::{
  http_util::{fetch_once, FetchOnceResult},
  ErrBox,
};
use regex::Regex;
use reqwest::{redirect::Policy, Client};
use semver_parser::version::parse as semver_parse;
use semver_parser::version::Version;
use std::future::Future;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Command;
use std::string::String;
use tempfile::TempDir;
use url::Url;

// TODO(ry) we should really be using target triples for the uploaded files.
#[cfg(windows)]
const EXEC_FILE_NAME: &str = "deno_win_x64.zip";
#[cfg(target_os = "macos")]
const EXEC_FILE_NAME: &str = "deno_osx_x64.gz";
#[cfg(target_os = "linux")]
const EXEC_FILE_NAME: &str = "deno_linux_x64.gz";

struct ErrorMsg(String);

impl ErrorMsg {
  fn to_err_box(&self) -> ErrBox {
    ErrBox::from(std::io::Error::new(
      std::io::ErrorKind::Other,
      self.0.clone(),
    ))
  }
}

async fn get_latest_version(client: &Client) -> Result<Version, ErrBox> {
  println!("Checking for latest version");
  let body = client
    .get(Url::parse(
      "https://github.com/denoland/deno/releases/latest",
    )?)
    .send()
    .await?
    .text()
    .await?;
  let v = find_version(&body)?;
  Ok(semver_parse(&v).unwrap())
}

/// Asynchronously updates deno executable to greatest version
/// if greatest version is available.
pub async fn upgrade_command(dry_run: bool) -> Result<(), ErrBox> {
  let force = dry_run; // TODO(ry) Should this be a CLI flag?

  let client = Client::builder().redirect(Policy::none()).build()?;
  let latest_version = get_latest_version(&client).await?;
  let current_version = semver_parse(crate::version::DENO).unwrap();

  if !force && current_version >= latest_version {
    println!(
      "Local deno version {} is the most recent release",
      &crate::version::DENO
    );
  } else {
    println!(
      "New version has been found\nDeno is upgrading to version {}",
      &latest_version
    );
    let archive =
      download_package(&compose_url_to_exec(&latest_version)?, client).await?;

    let new_exe_path = unpack(archive)?;
    check_exe(&new_exe_path, &latest_version)?;
    let old_exe_path = std::env::current_exe()?;

    if !dry_run {
      replace_exe(&new_exe_path, &old_exe_path)?;
    }

    println!("Upgrade done successfully")
  }
  Ok(())
}

fn download_package(
  url: &Url,
  client: Client,
) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, ErrBox>>>> {
  println!("downloading {}", url);
  let url = url.clone();
  let fut = async move {
    match fetch_once(client.clone(), &url, None).await? {
      FetchOnceResult::Code(source, _) => Ok(source),
      FetchOnceResult::NotModified => unreachable!(),
      FetchOnceResult::Redirect(_url, _) => {
        download_package(&_url, client).await
      }
    }
  };
  fut.boxed_local()
}

fn compose_url_to_exec(version: &Version) -> Result<Url, ErrBox> {
  let s = format!(
    "https://github.com/denoland/deno/releases/download/v{}/{}",
    version, EXEC_FILE_NAME
  );
  Ok(Url::parse(&s)?)
}

fn find_version(text: &str) -> Result<String, ErrBox> {
  let re = Regex::new(r#"v([^\?]+)?""#)?;
  if let Some(_mat) = re.find(text) {
    let mat = _mat.as_str();
    return Ok(mat[1..mat.len() - 1].to_string());
  }
  Err(ErrorMsg("Cannot read latest tag version".to_string()).to_err_box())
}

fn unpack(archive: Vec<u8>) -> Result<PathBuf, ErrBox> {
  // We use into_path so that the tempdir is not automatically deleted. This is
  // useful for debugging upgrade, but also so this function can return a path
  // to the newly uncompressed file without fear of the tempdir being deleted.
  let tmp = TempDir::new().unwrap().into_path();
  let ar_path = tmp.join(EXEC_FILE_NAME);
  {
    let mut ar_file = std::fs::File::create(&ar_path)?;
    ar_file.write_all(&archive)?;
  }

  if cfg!(windows) {
    todo!()
  } else {
    let status = Command::new("gunzip")
      .arg(&ar_path)
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());

    let new_exe_path = ar_path.with_extension("");
    assert!(new_exe_path.exists());

    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(&new_exe_path)?.permissions();
    perms.set_mode(perms.mode() | 0o100); // make executable
    std::fs::set_permissions(&new_exe_path, perms)?;
    Ok(new_exe_path)
  }
}

fn replace_exe(new: &Path, old: &Path) -> Result<(), ErrBox> {
  std::fs::remove_file(old)?;
  std::fs::rename(new, old)?;
  Ok(())
}

fn check_exe(
  exe_path: &Path,
  expected_version: &Version,
) -> Result<(), ErrBox> {
  let output = Command::new(exe_path)
    .arg("-V")
    .stderr(std::process::Stdio::inherit())
    .output()?;
  let stdout = String::from_utf8(output.stdout).unwrap();
  assert!(output.status.success());
  assert_eq!(stdout.trim(), format!("deno {}", expected_version));
  Ok(())
}

#[test]
fn test_find_version() {
  let url = "<html><body>You are being <a href=\"https://github.com/denoland/deno/releases/tag/v0.36.0\">redirected</a>.</body></html>";
  assert_eq!(find_version(url).unwrap(), "0.36.0".to_string());
}

#[test]
fn test_compose_url_to_exec() {
  use semver_parser::version::parse as semver_parse;
  let v = semver_parse("0.0.1").unwrap();
  let url = compose_url_to_exec(&v).unwrap();
  #[cfg(windows)]
  assert_eq!(url.to_string(), "https://github.com/denoland/deno/releases/download/v0.0.1/deno_win_x64.zip".to_string());
  #[cfg(target_os = "macos")]
  assert_eq!(
    url.to_string(),
    "https://github.com/denoland/deno/releases/download/v0.0.1/deno_osx_x64.gz"
      .to_string()
  );
  #[cfg(target_os = "linux")]
  assert_eq!(url.to_string(), "https://github.com/denoland/deno/releases/download/v0.0.1/deno_linux_x64.gz".to_string());
}
