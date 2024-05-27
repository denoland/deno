// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_npm::npm_rc::RegistryConfig;
use reqwest::header;

/// Gets the corresponding @types package for the provided package name.
pub fn types_package_name(package_name: &str) -> String {
  debug_assert!(!package_name.starts_with("@types/"));
  // Scoped packages will get two underscores for each slash
  // https://github.com/DefinitelyTyped/DefinitelyTyped/tree/15f1ece08f7b498f4b9a2147c2a46e94416ca777#what-about-scoped-packages
  format!("@types/{}", package_name.replace('/', "__"))
}

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

#[cfg(test)]
mod test {
  use super::types_package_name;

  #[test]
  fn test_types_package_name() {
    assert_eq!(types_package_name("name"), "@types/name");
    assert_eq!(
      types_package_name("@scoped/package"),
      "@types/@scoped__package"
    );
  }
}
