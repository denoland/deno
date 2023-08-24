// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use deno_config::ConfigFile;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_runtime::deno_fetch::reqwest;
use serde::Deserialize;

use crate::args::Flags;

fn save_token(token: String) -> Result<(), AnyError> {
  let contents = vec![format!("deno-registry-token:{}", REGISTRY_URL), token];
  std::fs::write("./deno.token", contents.join("\n"))?;
  Ok(())
}

fn read_token() -> Result<Option<String>, AnyError> {
  let contents = std::fs::read_to_string("./deno.token")?;
  let Some((_url, token)) = contents.split_once('\n') else {
    return Ok(None)
  };
  Ok(Some(token.to_string()))
}

fn ensure_token() -> Result<String, AnyError> {
  let maybe_token = read_token()?;
  let Some(token) = maybe_token else {
        bail!("Not logged in. Use `deno reg login` and try again.");
    };
  Ok(token)
}

pub async fn info(_flags: Flags) -> Result<(), AnyError> {
  let token = ensure_token()?;
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
    .get(format!("{}/user", REGISTRY_URL))
    .bearer_auth(token)
    .send()
    .await?
    .json()
    .await?;
  Ok(user_info)
}

// TODO(bartlomieju): support configuring these
static REGISTRY_URL: &str = "https://api.deno-registry-staging.net";
static AUTH_REGISTRY_URL: &str = "https://manage.deno-registry-staging.net";

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
    reqwest::get(format!("{}/login/device", AUTH_REGISTRY_URL))
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
      AUTH_REGISTRY_URL, device_login.id
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

  save_token(token)?;
  println!("Sign in successful. Authenticated as {}", user_info.name);

  Ok(())
}

pub async fn publish(
  _flags: Flags,
  directory: PathBuf,
) -> Result<(), AnyError> {
  let initial_cwd =
    std::env::current_dir().with_context(|| "Failed getting cwd.")?;
  // TODO: handle publishing without deno.json

  // TODO: doesn't handle jsonc
  let deno_json_path = initial_cwd.join(directory).join("deno.json");
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

  eprintln!(
    "deno.json {}",
    serde_json::to_string_pretty(&deno_json.json).unwrap()
  );
  eprintln!("deno reg publish is not yet implemented");
  Ok(())
}

pub async fn scope(_flags: Flags) -> Result<(), AnyError> {
  eprintln!("deno reg scope is not yet implemented");
  Ok(())
}
