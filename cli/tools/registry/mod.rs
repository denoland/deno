// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use deno_config::ConfigFile;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_runtime::deno_fetch::reqwest;
use http::header::CONTENT_ENCODING;
use serde::Deserialize;

use crate::args::Flags;
use crate::tools::registry::auth::ensure_token;

mod auth;
mod tar;
mod urls;

pub async fn info(_flags: Flags) -> Result<(), AnyError> {
  let token = auth::ensure_token()?;
  let user_info = get_user_info(token).await?;
  eprintln!("{:#?}", user_info);
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
  pub id: uuid::Uuid,
  pub name: String,
  pub email: Option<String>,
  pub avatar_url: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub is_admin: bool,
}

async fn get_user_info(token: String) -> Result<UserInfo, AnyError> {
  let client = reqwest::Client::new();
  let user_info: UserInfo = client
    .get(format!("{}/user", urls::REGISTRY_URL))
    .bearer_auth(token)
    .send()
    .await?
    .json()
    .await?;
  Ok(user_info)
}

#[derive(Debug, Deserialize)]
struct DeviceLoginResponse {
  uri: String,
  code: String,
  id: uuid::Uuid,
  interval: f32,
  expires_in: f32,
}

pub async fn login(_flags: Flags) -> Result<(), AnyError> {
  let device_login_response =
    reqwest::get(format!("{}/login/device", urls::AUTH_REGISTRY_URL))
      .await
      .context("Failed to obtain device login info")?;

  let device_login: DeviceLoginResponse = device_login_response.json().await?;

  println!("Copy the code {}", device_login.code);
  println!("And enter it at {} to sign in", device_login.uri);
  println!("\nWaiting for login to complete...");

  let start = std::time::Instant::now();

  let token = loop {
    tokio::time::sleep(std::time::Duration::from_secs(
      device_login.interval as u64,
    ))
    .await;
    let res = reqwest::get(format!(
      "{}/login/device/exchange?id={}",
      urls::AUTH_REGISTRY_URL,
      device_login.id
    ))
    .await?;
    if res.status().is_success() {
      let token: String = res.json().await?;
      break token;
    }
    if std::time::Instant::now() - start
      > std::time::Duration::from_secs(device_login.expires_in as u64)
    {
      bail!("Login took too long, please try again");
    }
  };

  let user_info = match get_user_info(token.clone()).await {
    Ok(info) => info,
    Err(err) => {
      bail!(
        "Failed to obtain user info. Please try logging in again. Reason: {}",
        err
      );
    }
  };

  auth::save_token(token)?;
  println!("Sign in successful. Authenticated as {}", user_info.name);

  Ok(())
}

async fn do_publish(directory: PathBuf) -> Result<(), AnyError> {
  let token = ensure_token()?;
  let initial_cwd =
    std::env::current_dir().with_context(|| "Failed getting cwd.")?;
  // TODO: handle publishing without deno.json

  let directory_path = initial_cwd.join(directory);
  // TODO: doesn't handle jsonc
  let deno_json_path = directory_path.join("deno.json");
  let deno_json = ConfigFile::read(&deno_json_path).with_context(|| {
    format!(
      "Failed to read deno.json file at {}",
      deno_json_path.display()
    )
  })?;

  if deno_json.json.name.is_none() || deno_json.json.version.is_none() {
    bail!(
      "{} is missing 'name' and 'version' fields",
      deno_json_path.display()
    );
  }
  let name = deno_json.json.name.unwrap();

  if !name.starts_with('@') || name.find('/').is_none() {
    bail!("Invalid package name, user '@<scope_name>/<package_name> format");
  }
  let scope_and_package_name = name[1..].to_string();
  let (scope, package_name) = scope_and_package_name.split_once('/').unwrap();
  let version = deno_json.json.version.unwrap();

  let unfurler = tar::Unfurler::new(
    Url::from_file_path(&deno_json_path).unwrap(),
    std::fs::read_to_string(&deno_json_path).unwrap(),
  )?;

  let url = format!(
    "{}/publish/{}/{}/{}",
    urls::REGISTRY_URL,
    scope,
    package_name,
    version
  );
  let tarball = tar::create_tarball(directory_path, unfurler)
    .context("Failed to create a tarball")?;

  let client = reqwest::Client::new();
  let response = client
    .post(url)
    .bearer_auth(token.to_string())
    .header(CONTENT_ENCODING, "gzip")
    .body(tarball)
    .send()
    .await?;

  let status = response.status();
  let data: serde_json::Value = response.json().await?;

  if !status.is_success() {
    bail!("Failed to publish, status: {} {}", status, data);
  }

  let task_id = data["id"].as_str().unwrap();

  loop {
    let resp = client
      .get(format!("{}/publish_status/{}", urls::REGISTRY_URL, task_id))
      .bearer_auth(token.to_string())
      .send()
      .await?;

    let status = resp.status();
    let data: serde_json::Value = resp.json().await?;
    if !status.is_success() {
      bail!("Failed to get publishing status {:?}", data);
    }

    let data_status = data["status"].as_str().unwrap();
    if data_status == "success" {
      println!(
        "Successfully published @{}/{}@{}",
        data["packageScope"].as_str().unwrap(),
        data["packageName"].as_str().unwrap(),
        data["packageVersion"].as_str().unwrap()
      );
      println!(
        "https://deno-registry-staging.net/@{}/{}/{}_meta.json",
        data["packageScope"].as_str().unwrap(),
        data["packageName"].as_str().unwrap(),
        data["packageVersion"].as_str().unwrap()
      );
      break;
    } else if data_status == "failure" {
      bail!(
        "Publishing failed {}",
        serde_json::to_string_pretty(&data).unwrap()
      );
    } else {
      println!("Waiting");
      tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
  }

  Ok(())
}

pub async fn publish(
  _flags: Flags,
  directory: PathBuf,
) -> Result<(), AnyError> {
  let _token = auth::ensure_token()?;

  let initial_cwd =
    std::env::current_dir().with_context(|| "Failed getting cwd.")?;
  // TODO: handle publishing without deno.json

  let directory_path = initial_cwd.join(directory);
  // TODO: doesn't handle jsonc
  let deno_json_path = directory_path.join("deno.json");
  let deno_json = ConfigFile::read(&deno_json_path).with_context(|| {
    format!(
      "Failed to read deno.json file at {}",
      deno_json_path.display()
    )
  })?;

  let members = deno_json.json.members.clone();
  if !members.is_empty() {
    // TODO(bartlomieju): this should be smart enough to figure out dependencies
    // between workspace members and publish in correct order. Or error out
    // if there are circular dependencies between the packages.
    println!("Publishing a workspace...");
    for member in members {
      let member_dir = directory_path.join(member);
      println!("Publishing {}", member_dir.display());
      do_publish(member_dir).await?;
    }
    return Ok(());
  }

  do_publish(directory_path).await?;
  Ok(())
}

pub async fn scope(_flags: Flags) -> Result<(), AnyError> {
  eprintln!("deno reg scope is not yet implemented");
  Ok(())
}
