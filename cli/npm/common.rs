// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_npm::npm_rc::RegistryConfig;
use http::header;

// TODO(bartlomieju): support more auth methods besides token and basic auth
pub fn maybe_auth_header_for_npm_registry(
  registry_config: &RegistryConfig,
) -> Option<(header::HeaderName, header::HeaderValue)> {
  if let Some(token) = registry_config.auth_token.as_ref() {
    return Some((
      header::AUTHORIZATION,
      header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    ));
  }

  if let Some(auth) = registry_config.auth.as_ref() {
    return Some((
      header::AUTHORIZATION,
      header::HeaderValue::from_str(&format!("Basic {}", auth)).unwrap(),
    ));
  }

  None
}
