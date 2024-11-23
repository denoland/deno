// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_npm::npm_rc::RegistryConfig;
use http::header;

// TODO(bartlomieju): support more auth methods besides token and basic auth
pub fn maybe_auth_header_for_npm_registry(
  registry_config: &RegistryConfig,
) -> Result<Option<(header::HeaderName, header::HeaderValue)>, AnyError> {
  if let Some(token) = registry_config.auth_token.as_ref() {
    return Ok(Some((
      header::AUTHORIZATION,
      header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    )));
  }

  if let Some(auth) = registry_config.auth.as_ref() {
    return Ok(Some((
      header::AUTHORIZATION,
      header::HeaderValue::from_str(&format!("Basic {}", auth)).unwrap(),
    )));
  }

  let (username, password) = (
    registry_config.username.as_ref(),
    registry_config.password.as_ref(),
  );
  if (username.is_some() && password.is_none())
    || (username.is_none() && password.is_some())
  {
    bail!("Both the username and password must be provided for basic auth")
  }

  if username.is_some() && password.is_some() {
    // The npm client does some double encoding when generating the
    // bearer token value, see
    // https://github.com/npm/cli/blob/780afc50e3a345feb1871a28e33fa48235bc3bd5/workspaces/config/lib/index.js#L846-L851
    let pw_base64 = BASE64_STANDARD
      .decode(password.unwrap())
      .with_context(|| "The password in npmrc is an invalid base64 string")?;
    let bearer = BASE64_STANDARD.encode(format!(
      "{}:{}",
      username.unwrap(),
      String::from_utf8_lossy(&pw_base64)
    ));

    return Ok(Some((
      header::AUTHORIZATION,
      header::HeaderValue::from_str(&format!("Basic {}", bearer)).unwrap(),
    )));
  }

  Ok(None)
}
