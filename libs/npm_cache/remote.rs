// Copyright 2018-2025 the Deno authors. MIT license.

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use deno_npm::npm_rc::RegistryConfig;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum AuthHeaderForNpmRegistryError {
  #[class(type)]
  #[error("Both the username and password must be provided for basic auth")]
  Both,
  #[class(type)]
  #[error("The password in npmrc is an invalid base64 string: {0}")]
  Base64(base64::DecodeError),
}

// TODO(bartlomieju): support more auth methods besides token and basic auth
pub fn maybe_auth_header_value_for_npm_registry(
  registry_config: &RegistryConfig,
) -> Result<Option<String>, AuthHeaderForNpmRegistryError> {
  if let Some(token) = registry_config.auth_token.as_ref() {
    return Ok(Some(format!("Bearer {}", token)));
  }

  if let Some(auth) = registry_config.auth.as_ref() {
    return Ok(Some(format!("Basic {}", auth)));
  }

  let (username, password) = (
    registry_config.username.as_ref(),
    registry_config.password.as_ref(),
  );
  if (username.is_some() && password.is_none())
    || (username.is_none() && password.is_some())
  {
    return Err(AuthHeaderForNpmRegistryError::Both);
  }

  if let Some(username) = username
    && let Some(password) = password
  {
    // The npm client does some double encoding when generating the
    // bearer token value, see
    // https://github.com/npm/cli/blob/780afc50e3a345feb1871a28e33fa48235bc3bd5/workspaces/config/lib/index.js#L846-L851
    let pw_base64 = BASE64_STANDARD
      .decode(password)
      .map_err(AuthHeaderForNpmRegistryError::Base64)?;
    let bearer = BASE64_STANDARD.encode(format!(
      "{}:{}",
      username,
      String::from_utf8_lossy(&pw_base64)
    ));

    return Ok(Some(format!("Basic {}", bearer)));
  }

  Ok(None)
}
