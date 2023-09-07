// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fmt::Write;
use std::io::IsTerminal;
use std::path::PathBuf;

use deno_config::ConfigFile;
use deno_core::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_runtime::colors;
use deno_runtime::deno_fetch::reqwest;
use http::header::AUTHORIZATION;
use http::header::CONTENT_ENCODING;
use hyper::body::Bytes;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::Digest;

use crate::args::Flags;
use crate::args::PublishFlags;
use crate::factory::CliFactory;

mod tar;
mod urls;

struct OidcConfig {
  url: String,
  token: String,
}

enum AuthMethod {
  Interactive,
  Token(String),
  Oidc(OidcConfig),
}

struct PreparedPublishPackage {
  scope: String,
  package: String,
  version: String,
  tarball_hash: String,
  tarball: Bytes,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishingTaskError {
  pub code: String,
  pub message: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishingTask {
  pub id: String,
  pub status: String,
  pub error: Option<PublishingTaskError>,
}

async fn prepare_publish(
  directory: PathBuf,
) -> Result<PreparedPublishPackage, AnyError> {
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

  let tarball = tar::create_tarball(directory_path, unfurler)
    .context("Failed to create a tarball")?;

  let tarball_hash_bytes: Vec<u8> =
    sha2::Sha256::digest(&tarball).iter().cloned().collect();
  let mut tarball_hash = format!("sha256-");
  for byte in tarball_hash_bytes {
    write!(&mut tarball_hash, "{:02x}", byte).unwrap();
  }

  Ok(PreparedPublishPackage {
    scope: scope.to_string(),
    package: package_name.to_string(),
    version: version.to_string(),
    tarball_hash,
    tarball,
  })
}

#[derive(Serialize)]
#[serde(tag = "permission")]
pub enum Permission<'s> {
  #[serde(rename = "package/publish", rename_all = "camelCase")]
  VersionPublish {
    scope: &'s str,
    package: &'s str,
    version: &'s str,
    tarball_hash: &'s str,
  },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAuthorizationResponse {
  verification_url: String,
  code: String,
  exchange_token: String,
  poll_interval: u64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangeAuthorizationResponse {
  token: String,
  user: User,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
  name: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiError {
  pub code: String,
  pub message: String,
  #[serde(skip)]
  pub x_deno_ray: Option<String>,
}

impl std::fmt::Display for ApiError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{} ({})", self.message, self.code)?;
    if let Some(x_deno_ray) = &self.x_deno_ray {
      write!(f, "[x-deno-ray: {}]", x_deno_ray)?;
    }
    Ok(())
  }
}

impl std::fmt::Debug for ApiError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Display::fmt(self, f)
  }
}

impl std::error::Error for ApiError {}

async fn parse_response<T: DeserializeOwned>(
  response: reqwest::Response,
) -> Result<T, ApiError> {
  let status = response.status();
  let x_deno_ray = response
    .headers()
    .get("x-deno-ray")
    .and_then(|value| value.to_str().ok())
    .map(|s| s.to_string());
  let text = response.text().await.unwrap();

  if !status.is_success() {
    match serde_json::from_str::<ApiError>(&text) {
      Ok(mut err) => {
        err.x_deno_ray = x_deno_ray;
        return Err(err);
      }
      Err(_) => {
        let err = ApiError {
          code: "unknown".to_string(),
          message: format!("{}: {}", status, text),
          x_deno_ray,
        };
        return Err(err);
      }
    }
  }

  serde_json::from_str(&text).map_err(|err| ApiError {
    code: "unknown".to_string(),
    message: format!("Failed to parse response: {}, response: '{}'", err, text),
    x_deno_ray,
  })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OidcTokenResponse {
  value: String,
}

async fn perform_publish(
  packages: Vec<PreparedPublishPackage>,
  auth_method: AuthMethod,
) -> Result<(), AnyError> {
  let client = reqwest::Client::new();

  let authorization = match auth_method {
    AuthMethod::Interactive => {
      let verifier = uuid::Uuid::new_v4().to_string();
      let challenge = base64::encode(sha2::Sha256::digest(&verifier));

      let permissions = packages
        .iter()
        .map(|package| Permission::VersionPublish {
          scope: &package.scope,
          package: &package.package,
          version: &package.version,
          tarball_hash: &package.tarball_hash,
        })
        .collect::<Vec<_>>();

      let response = client
        .post(format!("{}/authorizations", urls::REGISTRY_URL))
        .json(&serde_json::json!({
          "challenge": challenge,
          "permissions": permissions,
        }))
        .send()
        .await
        .context("Failed to create interactive authorization")?;
      let auth = parse_response::<CreateAuthorizationResponse>(response)
        .await
        .context("Failed to create interactive authorization")?;

      print!(
        "Visit {} to authorize publishing of",
        colors::cyan(format!("{}?code={}", auth.verification_url, auth.code))
      );
      if packages.len() > 1 {
        println!(" {} packages", packages.len());
      } else {
        println!(" @{}/{}", packages[0].scope, packages[0].package);
      }

      println!("{}", colors::gray("Waiting..."));

      let interval = std::time::Duration::from_secs(auth.poll_interval);

      loop {
        tokio::time::sleep(interval).await;
        let response = client
          .post(format!("{}/authorizations/exchange", urls::REGISTRY_URL))
          .json(&serde_json::json!({
            "exchangeToken": auth.exchange_token,
            "verifier": verifier,
          }))
          .send()
          .await
          .context("Failed to exchange authorization")?;
        let res =
          parse_response::<ExchangeAuthorizationResponse>(response).await;
        match res {
          Ok(res) => {
            println!(
              "{} {} {}",
              colors::green("Authorization successful."),
              colors::gray("Authenticated as"),
              colors::cyan(res.user.name)
            );
            break format!("Bearer {}", res.token);
          }
          Err(err) => {
            if err.code == "authorizationPending" {
              continue;
            } else {
              return Err(err).context("Failed to exchange authorization");
            }
          }
        }
      }
    }
    AuthMethod::Token(token) => format!("Bearer {}", token),
    AuthMethod::Oidc(oidc_config) => {
      let permissions = packages
        .iter()
        .map(|package| Permission::VersionPublish {
          scope: &package.scope,
          package: &package.package,
          version: &package.version,
          tarball_hash: &package.tarball_hash,
        })
        .collect::<Vec<_>>();
      let audience = json!({ "permissions": permissions }).to_string();

      let url = format!(
        "{}&audience={}",
        oidc_config.url,
        percent_encoding::percent_encode(
          audience.as_bytes(),
          percent_encoding::NON_ALPHANUMERIC
        )
      );

      let response = client
        .get(url)
        .bearer_auth(oidc_config.token)
        .send()
        .await
        .context("Failed to get OIDC token")?;
      let status = response.status();
      let text = response.text().await.with_context(|| {
        format!("Failed to get OIDC token: status {}", status)
      })?;
      if !status.is_success() {
        bail!(
          "Failed to get OIDC token: status {}, response: '{}'",
          status,
          text
        );
      }
      let OidcTokenResponse { value } = serde_json::from_str(&text)
        .with_context(|| {
          format!("Failed to parse OIDC token: '{}' (status {})", text, status)
        })?;
      format!("githuboidc {}", value)
    }
  };

  for package in packages {
    println!(
      "{} @{}/{}@{} ...",
      colors::intense_blue("Publishing"),
      package.scope,
      package.package,
      package.version
    );

    let url = format!(
      "{}/scopes/{}/packages/{}/versions/{}",
      urls::REGISTRY_URL,
      package.scope,
      package.package,
      package.version
    );

    let response = client
      .post(url)
      .header(AUTHORIZATION, &authorization)
      .header(CONTENT_ENCODING, "gzip")
      .body(package.tarball)
      .send()
      .await?;

    let mut task = parse_response::<PublishingTask>(response)
      .await
      .with_context(|| {
        format!(
          "Failed to publish @{}/{} at {}",
          package.scope, package.package, package.version
        )
      })?;

    let interval = std::time::Duration::from_secs(2);
    while task.status != "success" && task.status != "failure" {
      tokio::time::sleep(interval).await;
      let resp = client
        .get(format!("{}/publish_status/{}", urls::REGISTRY_URL, task.id))
        .send()
        .await
        .with_context(|| {
          format!(
            "Failed to get publishing status for @{}/{} at {}",
            package.scope, package.package, package.version
          )
        })?;
      task =
        parse_response::<PublishingTask>(resp)
          .await
          .with_context(|| {
            format!(
              "Failed to get publishing status for @{}/{} at {}",
              package.scope, package.package, package.version
            )
          })?;
    }

    if let Some(error) = task.error {
      bail!(
        "{} @{}/{} at {}: {}",
        colors::red("Failed to publish"),
        package.scope,
        package.package,
        package.version,
        error.message
      );
    }

    println!(
      "{} @{}/{}@{}",
      colors::green("Successfully published"),
      package.scope,
      package.package,
      package.version
    );
    println!(
      "https://deno-registry-staging.net/@{}/{}/{}_meta.json",
      package.scope, package.package, package.version
    );
  }

  Ok(())
}

fn get_gh_oidc_env_vars() -> Option<Result<(String, String), AnyError>> {
  if std::env::var("GITHUB_ACTIONS").unwrap_or_default() == "true" {
    let url = std::env::var("ACTIONS_ID_TOKEN_REQUEST_URL");
    let token = std::env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN");
    match (url, token) {
      (Ok(url), Ok(token)) => Some(Ok((url, token))),
      (Err(_), Err(_)) => Some(Err(anyhow::anyhow!(
        "No means to authenticate. Pass a token to `--token`, or enable tokenless publishing from GitHub Actions using OIDC. Learn more at https://deno.co/ghoidc"
      ))),
      _ => None,
    }
  } else {
    None
  }
}

pub async fn publish(
  _flags: Flags,
  publish_flags: PublishFlags,
) -> Result<(), AnyError> {
  let auth_method = match publish_flags.token {
    Some(token) => AuthMethod::Token(token),
    None => match get_gh_oidc_env_vars() {
      Some(Ok((url, token))) => AuthMethod::Oidc(OidcConfig { url, token }),
      Some(Err(err)) => return Err(err),
      None if std::io::stdin().is_terminal() => AuthMethod::Interactive,
      None => {
        bail!("No means to authenticate. Please pass a token to `--token`.")
      }
    },
  };

  let initial_cwd =
    std::env::current_dir().with_context(|| "Failed getting cwd.")?;
  // TODO: handle publishing without deno.json

  let directory_path = initial_cwd.join(publish_flags.directory);
  // TODO: doesn't handle jsonc
  let deno_json_path = directory_path.join("deno.json");
  let deno_json = ConfigFile::read(&deno_json_path).with_context(|| {
    format!(
      "Failed to read deno.json file at {}",
      deno_json_path.display()
    )
  })?;

  let mut packages = Vec::new();

  let members = deno_json.json.members.clone();
  if members.is_empty() {
    packages.push(prepare_publish(directory_path).await?);
  } else {
    println!("Publishing a workspace...");
    for member in members {
      let member_dir = directory_path.join(member);
      packages.push(prepare_publish(member_dir).await?);
    }
  }

  if packages.is_empty() {
    bail!("No packages to publish");
  }

  perform_publish(packages, auth_method).await
}

pub async fn deps_add(flags: Flags, specifier: String) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags).await?;
  let cli_options = factory.cli_options();

  let name = specifier
    .strip_prefix("deno:")
    .unwrap_or_else(|| specifier.strip_prefix("npm:").unwrap_or(&specifier));

  let (config_file_path, content) = if let Some(config_file) =
    cli_options.maybe_config_file()
  {
    let Ok(path) = config_file.specifier.to_file_path() else {
      bail!("Can't add a dependency to a remote configuration file");
    };

    let mut json = config_file.json.clone();
    json.imports = Some(json.imports.unwrap_or_else(|| serde_json::json!({})));
    json
      .imports
      .as_mut()
      .unwrap()
      .as_object_mut()
      .unwrap()
      .insert(name.to_string(), specifier.to_string().into());

    let content = serde_json::to_string_pretty(&json).unwrap();
    (path, content)
  } else {
    // Generate deno.json in initial_cwd
    let path = cli_options.initial_cwd().join("./deno.json");
    let mut json = serde_json::json!({
      "imports": {}
    });
    json["imports"]
      .as_object_mut()
      .unwrap()
      .insert(name.to_string(), specifier.to_string().into());

    let content = serde_json::to_string_pretty(&json).unwrap();
    (path, content)
  };

  std::fs::write(&config_file_path, content)?;
  println!("Added '{}' to '{}'", name, &config_file_path.display());
  Ok(())
}
