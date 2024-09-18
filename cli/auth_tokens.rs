// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_core::ModuleSpecifier;
use log::debug;
use log::error;
use std::borrow::Cow;
use std::fmt;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthTokenData {
  Bearer(String),
  Basic { username: String, password: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthToken {
  host: AuthDomain,
  token: AuthTokenData,
}

impl fmt::Display for AuthToken {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match &self.token {
      AuthTokenData::Bearer(token) => write!(f, "Bearer {token}"),
      AuthTokenData::Basic { username, password } => {
        let credentials = format!("{username}:{password}");
        write!(f, "Basic {}", BASE64_STANDARD.encode(credentials))
      }
    }
  }
}

/// A structure which contains bearer tokens that can be used when sending
/// requests to websites, intended to authorize access to private resources
/// such as remote modules.
#[derive(Debug, Clone)]
pub struct AuthTokens(Vec<AuthToken>);

/// An authorization domain, either an exact or suffix match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthDomain {
  Ip(IpAddr),
  IpPort(SocketAddr),
  /// Suffix match, no dot. May include a port.
  Suffix(Cow<'static, str>),
}

impl<T: ToString> From<T> for AuthDomain {
  fn from(value: T) -> Self {
    let s = value.to_string().to_lowercase();
    if let Ok(ip) = SocketAddr::from_str(&s) {
      return AuthDomain::IpPort(ip);
    };
    if s.starts_with('[') && s.ends_with(']') {
      if let Ok(ip) = Ipv6Addr::from_str(&s[1..s.len() - 1]) {
        return AuthDomain::Ip(ip.into());
      }
    } else if let Ok(ip) = Ipv4Addr::from_str(&s) {
      return AuthDomain::Ip(ip.into());
    }
    if let Some(s) = s.strip_prefix('.') {
      AuthDomain::Suffix(Cow::Owned(s.to_owned()))
    } else {
      AuthDomain::Suffix(Cow::Owned(s))
    }
  }
}

impl AuthDomain {
  pub fn matches(&self, specifier: &ModuleSpecifier) -> bool {
    let Some(host) = specifier.host_str() else {
      return false;
    };
    match *self {
      Self::Ip(ip) => {
        let AuthDomain::Ip(parsed) = AuthDomain::from(host) else {
          return false;
        };
        ip == parsed && specifier.port().is_none()
      }
      Self::IpPort(ip) => {
        let AuthDomain::Ip(parsed) = AuthDomain::from(host) else {
          return false;
        };
        ip.ip() == parsed && specifier.port() == Some(ip.port())
      }
      Self::Suffix(ref suffix) => {
        let hostname = if let Some(port) = specifier.port() {
          Cow::Owned(format!("{}:{}", host, port))
        } else {
          Cow::Borrowed(host)
        };

        if suffix.len() == hostname.len() {
          return suffix == &hostname;
        }

        // If it's a suffix match, ensure a dot
        if hostname.ends_with(suffix.as_ref())
          && hostname.ends_with(&format!(".{suffix}"))
        {
          return true;
        }

        false
      }
    }
  }
}

impl AuthTokens {
  /// Create a new set of tokens based on the provided string. It is intended
  /// that the string be the value of an environment variable and the string is
  /// parsed for token values.  The string is expected to be a semi-colon
  /// separated string, where each value is `{token}@{hostname}`.
  pub fn new(maybe_tokens_str: Option<String>) -> Self {
    let mut tokens = Vec::new();
    if let Some(tokens_str) = maybe_tokens_str {
      for token_str in tokens_str.trim().split(';') {
        if token_str.contains('@') {
          let mut iter = token_str.rsplitn(2, '@');
          let host = AuthDomain::from(iter.next().unwrap());
          let token = iter.next().unwrap();
          if token.contains(':') {
            let mut iter = token.rsplitn(2, ':');
            let password = iter.next().unwrap().to_owned();
            let username = iter.next().unwrap().to_owned();
            tokens.push(AuthToken {
              host,
              token: AuthTokenData::Basic { username, password },
            });
          } else {
            tokens.push(AuthToken {
              host,
              token: AuthTokenData::Bearer(token.to_string()),
            });
          }
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
      if t.host.matches(specifier) {
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
  fn test_auth_tokens_space() {
    let auth_tokens = AuthTokens::new(Some(
      " abc123@deno.land;def456@example.com\t".to_string(),
    ));
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
  fn test_auth_tokens_newline() {
    let auth_tokens = AuthTokens::new(Some(
      "\nabc123@deno.land;def456@example.com\n".to_string(),
    ));
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

  #[test]
  fn test_auth_token_basic() {
    let auth_tokens = AuthTokens::new(Some("abc:123@deno.land".to_string()));
    let fixture = resolve_url("https://deno.land/x/mod.ts").unwrap();
    assert_eq!(
      auth_tokens.get(&fixture).unwrap().to_string(),
      "Basic YWJjOjEyMw=="
    );
    let fixture = resolve_url("https://www.deno.land/x/mod.ts").unwrap();
    assert_eq!(
      auth_tokens.get(&fixture).unwrap().to_string(),
      "Basic YWJjOjEyMw==".to_string()
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
  fn test_parse_ip() {
    let ip = AuthDomain::from("[2001:db8:a::123]");
    assert_eq!("Ip(2001:db8:a::123)", format!("{ip:?}"));
    let ip = AuthDomain::from("[2001:db8:a::123]:8080");
    assert_eq!("IpPort([2001:db8:a::123]:8080)", format!("{ip:?}"));
    let ip = AuthDomain::from("1.1.1.1");
    assert_eq!("Ip(1.1.1.1)", format!("{ip:?}"));
  }

  #[test]
  fn test_case_insensitive() {
    let domain = AuthDomain::from("EXAMPLE.com");
    assert!(
      domain.matches(&ModuleSpecifier::parse("http://example.com").unwrap())
    );
    assert!(
      domain.matches(&ModuleSpecifier::parse("http://example.COM").unwrap())
    );
  }

  #[test]
  fn test_matches() {
    let candidates = [
      "example.com",
      "www.example.com",
      "1.1.1.1",
      "[2001:db8:a::123]",
      // These will never match
      "example.com.evil.com",
      "1.1.1.1.evil.com",
      "notexample.com",
      "www.notexample.com",
    ];
    let domains = [
      ("example.com", vec!["example.com", "www.example.com"]),
      (".example.com", vec!["example.com", "www.example.com"]),
      ("www.example.com", vec!["www.example.com"]),
      ("1.1.1.1", vec!["1.1.1.1"]),
      ("[2001:db8:a::123]", vec!["[2001:db8:a::123]"]),
    ];
    let url = |c: &str| ModuleSpecifier::parse(&format!("http://{c}")).unwrap();
    let url_port =
      |c: &str| ModuleSpecifier::parse(&format!("http://{c}:8080")).unwrap();

    // Generate each candidate with and without a port
    let candidates = candidates
      .into_iter()
      .flat_map(|c| [url(c), url_port(c)])
      .collect::<Vec<_>>();

    for (domain, expected_domain) in domains {
      // Test without a port -- all candidates return without a port
      let auth_domain = AuthDomain::from(domain);
      let actual = candidates
        .iter()
        .filter(|c| auth_domain.matches(c))
        .cloned()
        .collect::<Vec<_>>();
      let expected = expected_domain.iter().map(|u| url(u)).collect::<Vec<_>>();
      assert_eq!(actual, expected);

      // Test with a port, all candidates return with a port
      let auth_domain = AuthDomain::from(&format!("{domain}:8080"));
      let actual = candidates
        .iter()
        .filter(|c| auth_domain.matches(c))
        .cloned()
        .collect::<Vec<_>>();
      let expected = expected_domain
        .iter()
        .map(|u| url_port(u))
        .collect::<Vec<_>>();
      assert_eq!(actual, expected);
    }
  }
}
