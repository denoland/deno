// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::fmt::Write;
use std::io::IsTerminal;
use std::rc::Rc;
use std::sync::Arc;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_config::ConfigFile;
use deno_core::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_runtime::colors;
use deno_runtime::deno_fetch::reqwest;
use http::header::AUTHORIZATION;
use http::header::CONTENT_ENCODING;
use hyper::body::Bytes;
use import_map::ImportMap;
use lsp_types::Url;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::Digest;

use crate::args::Flags;
use crate::args::PublishFlags;
use crate::factory::CliFactory;
use crate::http_util::HttpClient;
use crate::util::import_map::ImportMapUnfurler;

mod publish_order;
mod tar;

enum AuthMethod {
  Interactive,
  Token(String),
  Oidc(OidcConfig),
}

struct OidcConfig {
  url: String,
  token: String,
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
  deno_json: &ConfigFile,
  import_map: Arc<ImportMap>,
) -> Result<PreparedPublishPackage, AnyError> {
  let Some(version) = deno_json.json.version.clone() else {
    bail!("{} is missing 'version' field", deno_json.specifier);
  };
  let Some(name) = deno_json.json.name.clone() else {
    bail!("{} is missing 'name' field", deno_json.specifier);
  };
  let Some(name) = name.strip_prefix('@') else {
    bail!("Invalid package name, use '@<scope_name>/<package_name> format");
  };
  let Some((scope, package_name)) = name.split_once('/') else {
    bail!("Invalid package name, use '@<scope_name>/<package_name> format");
  };

  let config_path = deno_json.specifier.to_file_path().unwrap();
  let dir_path = config_path.parent().unwrap().to_path_buf();

  let tarball = deno_core::unsync::spawn_blocking(move || {
    let unfurler = ImportMapUnfurler::new(&import_map);
    tar::create_gzipped_tarball(&dir_path, unfurler)
      .context("Failed to create a tarball")
  })
  .await??;

  let tarball_hash_bytes: Vec<u8> =
    sha2::Sha256::digest(&tarball).iter().cloned().collect();
  let mut tarball_hash = "sha256-".to_string();
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
  http_client: &Arc<HttpClient>,
  package_batches: Vec<Vec<PreparedPublishPackage>>,
  auth_method: AuthMethod,
) -> Result<(), AnyError> {
  let client = http_client.client()?;
  let registry_url = crate::cache::DENO_REGISTRY_URL.to_string();

  let permissions = package_batches
    .iter()
    .flatten()
    .map(|package| Permission::VersionPublish {
      scope: &package.scope,
      package: &package.package,
      version: &package.version,
      tarball_hash: &package.tarball_hash,
    })
    .collect::<Vec<_>>();
  let packages_len = package_batches.iter().map(|b| b.len()).sum::<usize>();

  let authorizations = match auth_method {
    AuthMethod::Interactive => {
      let verifier = uuid::Uuid::new_v4().to_string();
      let challenge = BASE64_STANDARD.encode(sha2::Sha256::digest(&verifier));

      let response = client
        .post(format!("{}authorizations", registry_url))
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
      if packages_len > 1 {
        println!(" {} packages", packages_len);
      } else {
        println!(
          " @{}/{}",
          package_batches[0][0].scope, package_batches[0][0].package
        );
      }

      println!("{}", colors::gray("Waiting..."));

      let interval = std::time::Duration::from_secs(auth.poll_interval);

      loop {
        tokio::time::sleep(interval).await;
        let response = client
          .post(format!("{}authorizations/exchange", registry_url))
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
            let authorization: Rc<str> = format!("Bearer {}", res.token).into();
            let mut authorizations = Vec::new();
            for _ in &package_batches {
              authorizations.push(authorization.clone());
            }
            break authorizations;
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
    AuthMethod::Token(token) => {
      let authorization: Rc<str> = format!("Bearer {}", token).into();
      let mut authorizations = Vec::new();
      for _ in &package_batches {
        authorizations.push(authorization.clone());
      }
      authorizations
    }
    AuthMethod::Oidc(oidc_config) => {
      let mut authorizations = Vec::new();
      for permissions in permissions.chunks(16) {
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
          .bearer_auth(&oidc_config.token)
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
            format!(
              "Failed to parse OIDC token: '{}' (status {})",
              text, status
            )
          })?;

        let authorization: Rc<str> = format!("githuboidc {}", value).into();
        for _ in permissions {
          authorizations.push(authorization.clone());
        }
      }
      authorizations
    }
  };

  assert_eq!(packages_len, authorizations.len());
  let mut authorizations = authorizations.into_iter();
  for batch in package_batches {
    let futures = batch
      .into_iter()
      .map(|package| {
        let authorization = authorizations.next().unwrap();
        let registry_url = registry_url.clone();
        let http_client = http_client.clone();
        deno_core::unsync::spawn(async move {
          let package_name = format!(
            "@{}/{}@{}",
            package.scope, package.package, package.version
          );
          publish_package(&http_client, package, &registry_url, &authorization)
            .await
            .with_context(|| format!("Failed to publish {}", package_name))
        })
      })
      .collect::<Vec<_>>();
    let results = deno_core::futures::future::join_all(futures).await;
    for result in results {
      result??;
    }
  }

  Ok(())
}

async fn publish_package(
  http_client: &HttpClient,
  package: PreparedPublishPackage,
  registry_url: &str,
  authorization: &str,
) -> Result<(), AnyError> {
  let client = http_client.client()?;
  println!(
    "{} @{}/{}@{} ...",
    colors::intense_blue("Publishing"),
    package.scope,
    package.package,
    package.version
  );

  let url = format!(
    "{}scopes/{}/packages/{}/versions/{}",
    registry_url, package.scope, package.package, package.version
  );

  let response = client
    .post(url)
    .header(AUTHORIZATION, authorization)
    .header(CONTENT_ENCODING, "gzip")
    .body(package.tarball)
    .send()
    .await?;

  let res = parse_response::<PublishingTask>(response).await;
  let mut task = match res {
    Ok(task) => task,
    Err(err) if err.code == "duplicateVersionPublish" => {
      println!(
        "{} @{}/{}@{}",
        colors::yellow("Skipping, already published"),
        package.scope,
        package.package,
        package.version
      );
      return Ok(());
    }
    Err(err) => {
      return Err(err).with_context(|| {
        format!(
          "Failed to publish @{}/{} at {}",
          package.scope, package.package, package.version
        )
      })
    }
  };

  let interval = std::time::Duration::from_secs(2);
  while task.status != "success" && task.status != "failure" {
    tokio::time::sleep(interval).await;
    let resp = client
      .get(format!("{}publish_status/{}", registry_url, task.id))
      .send()
      .await
      .with_context(|| {
        format!(
          "Failed to get publishing status for @{}/{} at {}",
          package.scope, package.package, package.version
        )
      })?;
    task = parse_response::<PublishingTask>(resp)
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
    "{}/@{}/{}/{}_meta.json",
    registry_url, package.scope, package.package, package.version
  );
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
  flags: Flags,
  publish_flags: PublishFlags,
) -> Result<(), AnyError> {
  let cli_factory = CliFactory::from_flags(flags).await?;

  let auth_method = match publish_flags.token {
    Some(token) => AuthMethod::Token(token),
    None => match get_gh_oidc_env_vars() {
      Some(Ok((url, token))) => AuthMethod::Oidc(OidcConfig { url, token }),
      Some(Err(err)) => return Err(err),
      None if std::io::stdin().is_terminal() => AuthMethod::Interactive,
      None => {
        bail!("No means to authenticate. Pass a token to `--token`.")
      }
    },
  };

  let import_map = cli_factory
    .maybe_import_map()
    .await?
    .clone()
    .unwrap_or_else(|| {
      Arc::new(ImportMap::new(Url::parse("file:///dev/null").unwrap()))
    });

  let initial_cwd =
    std::env::current_dir().with_context(|| "Failed getting cwd.")?;

  let directory_path = initial_cwd.join(publish_flags.directory);
  // TODO: doesn't handle jsonc
  let deno_json_path = directory_path.join("deno.json");
  let deno_json = ConfigFile::read(&deno_json_path).with_context(|| {
    format!(
      "Failed to read deno.json file at {}",
      deno_json_path.display()
    )
  })?;

  let mut package_batches =
    Vec::with_capacity(std::cmp::max(1, deno_json.json.workspaces.len()));

  let workspace_config = deno_json.to_workspace_config()?;
  match workspace_config {
    Some(workspace_config) => {
      println!("Publishing a workspace...");
      let batched_packages = publish_order::analyze_workspace_publish_order(
        &workspace_config,
        cli_factory.module_graph_builder().await?.as_ref(),
      )
      .await?;

      let members_by_name = workspace_config
        .members
        .iter()
        .map(|m| (&m.package_name, m))
        .collect::<HashMap<_, _>>();
      for batch in batched_packages {
        let results = batch
          .iter()
          .map(|name| {
            let member = (*members_by_name.get(name).unwrap()).clone();
            let import_map = import_map.clone();
            deno_core::unsync::spawn(async move {
              prepare_publish(&member.config_file, import_map)
                .await
                .with_context(|| {
                  format!("Failed preparing '{}'.", member.member_name)
                })
            })
          })
          .collect::<Vec<_>>();
        let results = deno_core::futures::future::join_all(results).await;
        let mut batch = Vec::with_capacity(results.len());
        for result in results {
          batch.push(result??);
        }
        package_batches.push(batch);
      }
    }
    None => {
      package_batches
        .push(vec![prepare_publish(&deno_json, import_map).await?]);
    }
  }

  if package_batches.is_empty() {
    bail!("No packages to publish");
  }

  perform_publish(cli_factory.http_client(), package_batches, auth_method).await
}
