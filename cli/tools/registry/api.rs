// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::http_util;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_runtime::deno_fetch;
use serde::de::DeserializeOwned;

use crate::http_util::HttpClient;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAuthorizationResponse {
  pub verification_url: String,
  pub code: String,
  pub exchange_token: String,
  pub poll_interval: u64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeAuthorizationResponse {
  pub token: String,
  pub user: User,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
  pub name: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OidcTokenResponse {
  pub value: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishingTaskError {
  #[allow(dead_code)]
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
  pub code: String,
  pub message: String,
  #[serde(flatten)]
  pub data: serde_json::Value,
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

pub async fn parse_response<T: DeserializeOwned>(
  response: http::Response<deno_fetch::ResBody>,
) -> Result<T, ApiError> {
  let status = response.status();
  let x_deno_ray = response
    .headers()
    .get("x-deno-ray")
    .and_then(|value| value.to_str().ok())
    .map(|s| s.to_string());
  let text = http_util::body_to_string(response).await.unwrap();

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
          data: serde_json::json!({}),
        };
        return Err(err);
      }
    }
  }

  serde_json::from_str(&text).map_err(|err| ApiError {
    code: "unknown".to_string(),
    message: format!("Failed to parse response: {}, response: '{}'", err, text),
    x_deno_ray,
    data: serde_json::json!({}),
  })
}

pub async fn get_scope(
  client: &HttpClient,
  registry_api_url: &Url,
  scope: &str,
) -> Result<http::Response<deno_fetch::ResBody>, AnyError> {
  let scope_url = format!("{}scopes/{}", registry_api_url, scope);
  let response = client.get(scope_url.parse()?)?.send().await?;
  Ok(response)
}

pub fn get_package_api_url(
  registry_api_url: &Url,
  scope: &str,
  package: &str,
) -> String {
  format!("{}scopes/{}/packages/{}", registry_api_url, scope, package)
}

pub async fn get_package(
  client: &HttpClient,
  registry_api_url: &Url,
  scope: &str,
  package: &str,
) -> Result<http::Response<deno_fetch::ResBody>, AnyError> {
  let package_url = get_package_api_url(registry_api_url, scope, package);
  let response = client.get(package_url.parse()?)?.send().await?;
  Ok(response)
}

pub fn get_jsr_alternative(imported: &Url) -> Option<String> {
  if matches!(imported.host_str(), Some("esm.sh")) {
    let mut segments = imported.path_segments()?;
    match segments.next()? {
      "gh" => None,
      module => Some(format!("\"npm:{module}\"")),
    }
  } else if imported.as_str().starts_with("https://deno.land/") {
    let mut segments = imported.path_segments()?;
    let maybe_std = segments.next()?;
    if maybe_std != "std" && !maybe_std.starts_with("std@") {
      return None;
    }
    let module = segments.next()?;
    let export = segments
      .next()
      .filter(|s| *s != "mod.ts")
      .map(|s| s.strip_suffix(".ts").unwrap_or(s).replace("_", "-"));
    Some(format!(
      "\"jsr:@std/{}@1{}\"",
      module,
      export.map(|s| format!("/{}", s)).unwrap_or_default()
    ))
  } else {
    None
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_jsr_alternative() {
    #[track_caller]
    fn run_test(imported: &str, output: Option<&str>) {
      let imported = Url::parse(imported).unwrap();
      let output = output.map(|s| s.to_string());
      assert_eq!(get_jsr_alternative(&imported), output);
    }

    run_test("https://esm.sh/ts-morph", Some("\"npm:ts-morph\""));
    run_test(
      "https://deno.land/std/path/mod.ts",
      Some("\"jsr:@std/path@1\""),
    );
    run_test(
      "https://deno.land/std/path/join.ts",
      Some("\"jsr:@std/path@1/join\""),
    );
    run_test(
      "https://deno.land/std@0.229.0/path/join.ts",
      Some("\"jsr:@std/path@1/join\""),
    );
    run_test(
      "https://deno.land/std@0.229.0/path/something_underscore.ts",
      Some("\"jsr:@std/path@1/something-underscore\""),
    );
  }
}
