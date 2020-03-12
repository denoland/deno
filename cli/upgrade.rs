// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module provides feature to upgrade deno executable
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

extern crate flate2;
use crate::fs::write_file;
use crate::futures::FutureExt;
use crate::{
  http_util::{fetch_once, FetchOnceResult},
  version, ErrBox,
};
use flate2::write::GzDecoder;
use regex::Regex;
use reqwest::{redirect::Policy, Client};
use std::env::current_exe;
use std::fs::{remove_file, rename};
use std::future::Future;
use std::io::prelude::*;
use std::path::Path;
use std::pin::Pin;
use std::string::String;
use url::Url;

lazy_static! {
  static ref LATEST_VERSION_URL: String =
    "https://github.com/denoland/deno/releases/latest".to_string();
  static ref EXEC_DOWNLOAD_URL: String =
    "https://github.com/denoland/deno/releases/download/v".to_string();
  static ref REGEX_STRING: String = r#"v([^\?]+)?""#.to_string();
  static ref DENO_EXEC_TEMP_NAME: String = "deno_temp".to_string();
}

#[cfg(windows)]
const EXEC_FILE_NAME: &str = "deno_win_x64.zip";
#[cfg(macos)]
const EXEC_FILE_NAME: &str = "deno_osx_x64.gz";
#[cfg(unix)]
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

/// Asynchronously updates deno executable to greatest version
/// if newest version is available.
pub async fn exec_upgrade() -> Result<(), ErrBox> {
  let client = Client::builder().redirect(Policy::none()).build()?;
  println!("Checking for latest version");
  let body = client
    .get(Url::parse(&LATEST_VERSION_URL)?)
    .send()
    .await?
    .text()
    .await?;
  let checked_version = find_version(&body)?;
  if is_latest_version_greater(&version::DENO.to_string(), &checked_version) {
    println!(
      "New version has been found.\nDeno is upgrading to version {}",
      &checked_version
    );
    let archive =
      download_package(&compose_url_to_exec(&checked_version)?, client).await?;
    let path = current_exe()?;
    unpack(archive, &path)?;
    replace_exec(&path)?;
    println!("Upgrade done successfully")
  } else {
    println!("Local deno version {} is the greatest one", &version::DENO);
  }
  Ok(())
}

fn download_package(
  url: &Url,
  client: Client,
) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, ErrBox>>>> {
  let url = url.clone();
  let fut = async move {
    match fetch_once(client.clone(), &url, None).await? {
      FetchOnceResult::Code(source, _) => Ok(source),
      FetchOnceResult::NotModified => Err(
        ErrorMsg(format!("Cannot fetch data from url: {}", &url)).to_err_box(),
      ),
      FetchOnceResult::Redirect(_url, _) => {
        download_package(&_url, client).await
      }
    }
  };
  fut.boxed_local()
}

fn compose_url_to_exec(version: &String) -> Result<Url, ErrBox> {
  let mut url_str = EXEC_DOWNLOAD_URL.clone();
  url_str.push_str(&format!("{}/", version));
  url_str.push_str(&EXEC_FILE_NAME);
  Ok(Url::parse(&url_str[..])?)
}

fn find_version(text: &String) -> Result<String, ErrBox> {
  let re = Regex::new(&REGEX_STRING)?;
  if let Some(_mat) = re.find(text) {
    let mat = _mat.as_str();
    return Ok(mat[1..mat.len() - 1].to_string());
  }
  Err(ErrorMsg("Cannot read latest tag version".to_string()).to_err_box())
}

fn is_latest_version_greater(old_v: &String, new_v: &String) -> bool {
  let mut power = 4;
  let (mut old_v_num, mut new_v_num) = (0, 0);
  old_v
    .split(".")
    .into_iter()
    .zip(new_v.split(".").into_iter())
    .for_each(|(old, new)| {
      old_v_num += old.parse::<i32>().unwrap() * (10_f32.powi(power) as i32);
      new_v_num += new.parse::<i32>().unwrap() * (10_f32.powi(power) as i32);
      power -= 2;
    });
  old_v_num < new_v_num
}

fn unpack(archive: Vec<u8>, path: &Path) -> Result<(), ErrBox> {
  let mut exec = Vec::new();
  let mut decoder = GzDecoder::new(exec);
  decoder.write_all(&archive[..])?;
  decoder.try_finish()?;
  exec = decoder.finish()?;
  write_file::<Vec<u8>>(
    &path.with_file_name(DENO_EXEC_TEMP_NAME.as_str()),
    exec,
    0o777,
  )?;
  Ok(())
}

fn replace_exec(path: &Path) -> Result<(), ErrBox> {
  remove_file(path)?;
  rename(path.with_file_name(DENO_EXEC_TEMP_NAME.as_str()), path)?;
  Ok(())
}

#[cfg(test)]
mod test {
  #[test]
  fn test_is_latest_version_greater() {
    let mut version = "0.0.0".to_string();
    let versions = [
      "0.0.1".to_string(),
      "0.1.0".to_string(),
      "1.0.0".to_string(),
      "11.22.33".to_string(),
      "22.0.44".to_string(),
      "30.0.0".to_string(),
    ];
    for v in versions.iter() {
      assert_eq!(super::is_latest_version_greater(&version, v), true);
      version = v.clone();
    }
    assert_eq!(super::is_latest_version_greater(&version, &version), false);
  }

  #[test]
  fn test_find_version() {
    let url = "<html><body>You are being <a href=\"https://github.com/denoland/deno/releases/tag/v0.36.0\">redirected</a>.</body></html>".to_string();
    assert_eq!(super::find_version(&url).unwrap(), "0.36.0".to_string());
  }

  #[test]
  fn test_compose_url_to_exec() {
    let url = super::compose_url_to_exec(&"0.0.1".to_string()).unwrap();
    #[cfg(windows)]
    assert_eq!(url.to_string(), "https://github.com/denoland/deno/releases/download/v0.0.1/deno_win_x64.zip".to_string());
    #[cfg(macos)]
    assert_eq!(url.to_string(), "https://github.com/denoland/deno/releases/download/v0.0.1/deno_osx_x64.gz".to_string());
    #[cfg(unix)]
    assert_eq!(url.to_string(), "https://github.com/denoland/deno/releases/download/v0.0.1/deno_linux_x64.gz".to_string());
  }
}
