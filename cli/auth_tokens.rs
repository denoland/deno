// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::ModuleSpecifier;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthToken {
  host: String,
  token: String,
}

impl fmt::Display for AuthToken {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Bearer {}", self.token)
  }
}

/// A structure which contains bearer tokens that can be used when sending
/// requests to websites, intended to authorize access to private resources
/// such as remote modules.
#[derive(Debug, Clone)]
pub struct AuthTokens(Vec<AuthToken>);

impl AuthTokens {
  /// Create a new set of tokens based on the provided string. It is intended
  /// that the string be the value of an environment variable and the string is
  /// parsed for token values.  The string is expected to be a semi-colon
  /// separated string, where each value is `{token}@{hostname}`.
  pub fn new(maybe_tokens_str: Option<String>) -> Self {
    let mut tokens = Vec::new();
    if let Some(tokens_str) = maybe_tokens_str {
      for token_str in tokens_str.split(';') {
        if token_str.contains('@') {
          let pair: Vec<&str> = token_str.rsplitn(2, '@').collect();
          let token = pair[1].to_string();
          let host = pair[0].to_lowercase();
          tokens.push(AuthToken { host, token });
        } else {
          error!("Badly formed auth token discarded.");
        }
      }
      debug!("Parsed {} auth token(s).", tokens.len());
    }

    Self(tokens)
  }

  /// Attempt to match the provided specifier to the tokens in the set.  The
  /// matching occurs from the right of the hostname plus port, irrespective of
  /// scheme.  For example `https://www.deno.land:8080/` would match a token
  /// with a host value of `deno.land:8080` but not match `www.deno.land`.  The
  /// matching is case insensitive.
  pub fn get(&self, specifier: &ModuleSpecifier) -> Option<AuthToken> {
    self.0.iter().find_map(|t| {
      let hostname = if let Some(port) = specifier.port() {
        format!("{}:{}", specifier.host_str()?, port)
      } else {
        specifier.host_str()?.to_string()
      };
      if hostname.to_lowercase().ends_with(&t.host) {
        Some(t.clone())
      } else {
        None
      }
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url;

  #[test]
  fn test_auth_token() {
    let auth_tokens = AuthTokens::new(Some("abc123@deno.land".to_string()));
    let fixture = resolve_url("https://deno.land/x/mod.ts").unwrap();
    assert_eq!(
      auth_tokens.get(&fixture).unwrap().to_string(),
      "Bearer abc123"
    );
    let fixture = resolve_url("https://www.deno.land/x/mod.ts").unwrap();
    assert_eq!(
      auth_tokens.get(&fixture).unwrap().to_string(),
      "Bearer abc123".to_string()
    );
    let fixture = resolve_url("http://127.0.0.1:8080/x/mod.ts").unwrap();
    assert_eq!(auth_tokens.get(&fixture), None);
    let fixture =
      resolve_url("https://deno.land.example.com/x/mod.ts").unwrap();
    assert_eq!(auth_tokens.get(&fixture), None);
    let fixture = resolve_url("https://deno.land:8080/x/mod.ts").unwrap();
    assert_eq!(auth_tokens.get(&fixture), None);
  }

  #[test]
  fn test_auth_tokens_multiple() {
    let auth_tokens =
      AuthTokens::new(Some("abc123@deno.land;def456@example.com".to_string()));
    let fixture = resolve_url("https://deno.land/x/mod.ts").unwrap();
    assert_eq!(
      auth_tokens.get(&fixture).unwrap().to_string(),
      "Bearer abc123".to_string()
    );
    let fixture = resolve_url("http://example.com/a/file.ts").unwrap();
    assert_eq!(
      auth_tokens.get(&fixture).unwrap().to_string(),
      "Bearer def456".to_string()
    );
  }

  #[test]
  fn test_auth_tokens_port() {
    let auth_tokens =
      AuthTokens::new(Some("abc123@deno.land:8080".to_string()));
    let fixture = resolve_url("https://deno.land/x/mod.ts").unwrap();
    assert_eq!(auth_tokens.get(&fixture), None);
    let fixture = resolve_url("http://deno.land:8080/x/mod.ts").unwrap();
    assert_eq!(
      auth_tokens.get(&fixture).unwrap().to_string(),
      "Bearer abc123".to_string()
    );
  }

  #[test]
  fn test_auth_tokens_contain_at() {
    let auth_tokens = AuthTokens::new(Some("abc@123@deno.land".to_string()));
    let fixture = resolve_url("https://deno.land/x/mod.ts").unwrap();
    assert_eq!(
      auth_tokens.get(&fixture).unwrap().to_string(),
      "Bearer abc@123".to_string()
    );
  }
}
