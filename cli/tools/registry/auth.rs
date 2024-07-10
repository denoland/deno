// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::IsTerminal;

use deno_core::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;

pub enum AuthMethod {
  Interactive,
  Token(String),
  Oidc(OidcConfig),
}

pub struct OidcConfig {
  pub url: String,
  pub token: String,
}

pub(crate) fn is_gha() -> bool {
  std::env::var("GITHUB_ACTIONS").unwrap_or_default() == "true"
}

pub(crate) fn gha_oidc_token() -> Option<String> {
  std::env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").ok()
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

pub fn get_auth_method(
  maybe_token: Option<String>,
  dry_run: bool,
) -> Result<AuthMethod, AnyError> {
  if dry_run {
    // We don't authenticate in dry-run mode.
    return Ok(AuthMethod::Interactive);
  }

  if let Some(token) = maybe_token {
    return Ok(AuthMethod::Token(token));
  }

  match get_gh_oidc_env_vars() {
    Some(Ok((url, token))) => Ok(AuthMethod::Oidc(OidcConfig { url, token })),
    Some(Err(err)) => Err(err),
    None if std::io::stdin().is_terminal() => Ok(AuthMethod::Interactive),
    None => {
      bail!("No means to authenticate. Pass a token to `--token`.")
    }
  }
}
