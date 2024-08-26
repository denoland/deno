// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_core::anyhow::bail;
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
    return Ok(Some((
      header::AUTHORIZATION,
      header::HeaderValue::from_str(&format!(
        "Basic {}",
        BASE64_STANDARD.encode(&format!(
          "{}:{}",
          username.unwrap(),
          password.unwrap()
        ))
      ))
      .unwrap(),
    )));
  }

  Ok(None)
}
