// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use crate::auth_tokens::AuthToken;

use cache_control::Cachability;
use cache_control::CacheControl;
use chrono::DateTime;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_runtime::deno_fetch::reqwest::header::HeaderValue;
use deno_runtime::deno_fetch::reqwest::header::ACCEPT;
use deno_runtime::deno_fetch::reqwest::header::AUTHORIZATION;
use deno_runtime::deno_fetch::reqwest::header::IF_NONE_MATCH;
use deno_runtime::deno_fetch::reqwest::header::LOCATION;
use deno_runtime::deno_fetch::reqwest::Client;
use deno_runtime::deno_fetch::reqwest::StatusCode;
use log::debug;
use std::collections::HashMap;
use std::time::Duration;
use std::time::SystemTime;

/// Construct the next uri based on base uri and location header fragment
/// See <https://tools.ietf.org/html/rfc3986#section-4.2>
fn resolve_url_from_location(base_url: &Url, location: &str) -> Url {
  if location.starts_with("http://") || location.starts_with("https://") {
    // absolute uri
    Url::parse(location).expect("provided redirect url should be a valid url")
  } else if location.starts_with("//") {
    // "//" authority path-abempty
    Url::parse(&format!("{}:{}", base_url.scheme(), location))
      .expect("provided redirect url should be a valid url")
  } else if location.starts_with('/') {
    // path-absolute
    base_url
      .join(location)
      .expect("provided redirect url should be a valid url")
  } else {
    // assuming path-noscheme | path-empty
    let base_url_path_str = base_url.path().to_owned();
    // Pop last part or url (after last slash)
    let segs: Vec<&str> = base_url_path_str.rsplitn(2, '/').collect();
    let new_path = format!("{}/{}", segs.last().unwrap_or(&""), location);
    base_url
      .join(&new_path)
      .expect("provided redirect url should be a valid url")
  }
}

// TODO(ry) HTTP headers are not unique key, value pairs. There may be more than
// one header line with the same key. This should be changed to something like
// Vec<(String, String)>
pub type HeadersMap = HashMap<String, String>;

/// A structure used to determine if a entity in the http cache can be used.
///
/// This is heavily influenced by
/// <https://github.com/kornelski/rusty-http-cache-semantics> which is BSD
/// 2-Clause Licensed and copyright Kornel Lesiński
pub struct CacheSemantics {
  cache_control: CacheControl,
  cached: SystemTime,
  headers: HashMap<String, String>,
  now: SystemTime,
}

impl CacheSemantics {
  pub fn new(
    headers: HashMap<String, String>,
    cached: SystemTime,
    now: SystemTime,
  ) -> Self {
    let cache_control = headers
      .get("cache-control")
      .map(|v| CacheControl::from_value(v).unwrap_or_default())
      .unwrap_or_default();
    Self {
      cache_control,
      cached,
      headers,
      now,
    }
  }

  fn age(&self) -> Duration {
    let mut age = self.age_header_value();

    if let Ok(resident_time) = self.now.duration_since(self.cached) {
      age += resident_time;
    }

    age
  }

  fn age_header_value(&self) -> Duration {
    Duration::from_secs(
      self
        .headers
        .get("age")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0),
    )
  }

  fn is_stale(&self) -> bool {
    self.max_age() <= self.age()
  }

  fn max_age(&self) -> Duration {
    if self.cache_control.cachability == Some(Cachability::NoCache) {
      return Duration::from_secs(0);
    }

    if self.headers.get("vary").map(|s| s.trim()) == Some("*") {
      return Duration::from_secs(0);
    }

    if let Some(max_age) = self.cache_control.max_age {
      return max_age;
    }

    let default_min_ttl = Duration::from_secs(0);

    let server_date = self.raw_server_date();
    if let Some(expires) = self.headers.get("expires") {
      return match DateTime::parse_from_rfc2822(expires) {
        Err(_) => Duration::from_secs(0),
        Ok(expires) => {
          let expires = SystemTime::UNIX_EPOCH
            + Duration::from_secs(expires.timestamp().max(0) as _);
          return default_min_ttl
            .max(expires.duration_since(server_date).unwrap_or_default());
        }
      };
    }

    if let Some(last_modified) = self.headers.get("last-modified") {
      if let Ok(last_modified) = DateTime::parse_from_rfc2822(last_modified) {
        let last_modified = SystemTime::UNIX_EPOCH
          + Duration::from_secs(last_modified.timestamp().max(0) as _);
        if let Ok(diff) = server_date.duration_since(last_modified) {
          let secs_left = diff.as_secs() as f64 * 0.1;
          return default_min_ttl.max(Duration::from_secs(secs_left as _));
        }
      }
    }

    default_min_ttl
  }

  fn raw_server_date(&self) -> SystemTime {
    self
      .headers
      .get("date")
      .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
      .and_then(|d| {
        SystemTime::UNIX_EPOCH
          .checked_add(Duration::from_secs(d.timestamp() as _))
      })
      .unwrap_or(self.cached)
  }

  /// Returns true if the cached value is "fresh" respecting cached headers,
  /// otherwise returns false.
  pub fn should_use(&self) -> bool {
    if self.cache_control.cachability == Some(Cachability::NoCache) {
      return false;
    }

    if let Some(max_age) = self.cache_control.max_age {
      if self.age() > max_age {
        return false;
      }
    }

    if let Some(min_fresh) = self.cache_control.min_fresh {
      if self.time_to_live() < min_fresh {
        return false;
      }
    }

    if self.is_stale() {
      let has_max_stale = self.cache_control.max_stale.is_some();
      let allows_stale = has_max_stale
        && self
          .cache_control
          .max_stale
          .map_or(true, |val| val > self.age() - self.max_age());
      if !allows_stale {
        return false;
      }
    }

    true
  }

  fn time_to_live(&self) -> Duration {
    self.max_age().checked_sub(self.age()).unwrap_or_default()
  }
}

#[derive(Debug, PartialEq)]
pub enum FetchOnceResult {
  Code(Vec<u8>, HeadersMap),
  NotModified,
  Redirect(Url, HeadersMap),
}

#[derive(Debug)]
pub struct FetchOnceArgs {
  pub client: Client,
  pub url: Url,
  pub maybe_accept: Option<String>,
  pub maybe_etag: Option<String>,
  pub maybe_auth_token: Option<AuthToken>,
}

/// Asynchronously fetches the given HTTP URL one pass only.
/// If no redirect is present and no error occurs,
/// yields Code(ResultPayload).
/// If redirect occurs, does not follow and
/// yields Redirect(url).
pub async fn fetch_once(
  args: FetchOnceArgs,
) -> Result<FetchOnceResult, AnyError> {
  let mut request = args.client.get(args.url.clone());

  if let Some(etag) = args.maybe_etag {
    let if_none_match_val = HeaderValue::from_str(&etag)?;
    request = request.header(IF_NONE_MATCH, if_none_match_val);
  }
  if let Some(auth_token) = args.maybe_auth_token {
    let authorization_val = HeaderValue::from_str(&auth_token.to_string())?;
    request = request.header(AUTHORIZATION, authorization_val);
  }
  if let Some(accept) = args.maybe_accept {
    let accepts_val = HeaderValue::from_str(&accept)?;
    request = request.header(ACCEPT, accepts_val);
  }
  let response = request.send().await?;

  if response.status() == StatusCode::NOT_MODIFIED {
    return Ok(FetchOnceResult::NotModified);
  }

  let mut headers_: HashMap<String, String> = HashMap::new();
  let headers = response.headers();

  if let Some(warning) = headers.get("X-Deno-Warning") {
    eprintln!(
      "{} {}",
      crate::colors::yellow("Warning"),
      warning.to_str().unwrap()
    );
  }

  for key in headers.keys() {
    let key_str = key.to_string();
    let values = headers.get_all(key);
    let values_str = values
      .iter()
      .map(|e| e.to_str().unwrap().to_string())
      .collect::<Vec<String>>()
      .join(",");
    headers_.insert(key_str, values_str);
  }

  if response.status().is_redirection() {
    if let Some(location) = response.headers().get(LOCATION) {
      let location_string = location.to_str().unwrap();
      debug!("Redirecting to {:?}...", &location_string);
      let new_url = resolve_url_from_location(&args.url, location_string);
      return Ok(FetchOnceResult::Redirect(new_url, headers_));
    } else {
      return Err(generic_error(format!(
        "Redirection from '{}' did not provide location header",
        args.url
      )));
    }
  }

  if response.status().is_client_error() || response.status().is_server_error()
  {
    let err = if response.status() == StatusCode::NOT_FOUND {
      custom_error(
        "NotFound",
        format!("Import '{}' failed, not found.", args.url),
      )
    } else {
      generic_error(format!(
        "Import '{}' failed: {}",
        args.url,
        response.status()
      ))
    };
    return Err(err);
  }

  let body = response.bytes().await?.to_vec();

  Ok(FetchOnceResult::Code(body, headers_))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::version;
  use deno_runtime::deno_fetch::create_http_client;
  use std::fs::read;

  fn create_test_client() -> Client {
    create_http_client(
      "test_client".to_string(),
      None,
      vec![],
      None,
      None,
      None,
    )
    .unwrap()
  }

  #[tokio::test]
  async fn test_fetch_string() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/fixture.json").unwrap();
    let client = create_test_client();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(headers.get("content-type").unwrap(), "application/json");
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_gzip() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/053_import_compression/gziped")
      .unwrap();
    let client = create_test_client();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('gzip')");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
      );
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_etag() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("http://127.0.0.1:4545/etag_script.ts").unwrap();
    let client = create_test_client();
    let result = fetch_once(FetchOnceArgs {
      client: client.clone(),
      url: url.clone(),
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('etag')");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/typescript"
      );
      assert_eq!(headers.get("etag").unwrap(), "33a64df551425fcc55e");
    } else {
      panic!();
    }

    let res = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: Some("33a64df551425fcc55e".to_string()),
      maybe_auth_token: None,
    })
    .await;
    assert_eq!(res.unwrap(), FetchOnceResult::NotModified);
  }

  #[tokio::test]
  async fn test_fetch_brotli() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/053_import_compression/brotli")
      .unwrap();
    let client = create_test_client();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('brotli');");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
      );
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_accept() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/echo_accept").unwrap();
    let client = create_test_client();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: Some("application/json".to_string()),
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, _)) = result {
      assert_eq!(body, r#"{"accept":"application/json"}"#.as_bytes());
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_once_with_redirect() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4546/fixture.json").unwrap();
    // Dns resolver substitutes `127.0.0.1` with `localhost`
    let target_url = Url::parse("http://localhost:4545/fixture.json").unwrap();
    let client = create_test_client();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Redirect(url, _)) = result {
      assert_eq!(url, target_url);
    } else {
      panic!();
    }
  }

  #[test]
  fn test_resolve_url_from_location_full_1() {
    let url = "http://deno.land".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "http://golang.org");
    assert_eq!(new_uri.host_str().unwrap(), "golang.org");
  }

  #[test]
  fn test_resolve_url_from_location_full_2() {
    let url = "https://deno.land".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "https://golang.org");
    assert_eq!(new_uri.host_str().unwrap(), "golang.org");
  }

  #[test]
  fn test_resolve_url_from_location_relative_1() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "//rust-lang.org/en-US");
    assert_eq!(new_uri.host_str().unwrap(), "rust-lang.org");
    assert_eq!(new_uri.path(), "/en-US");
  }

  #[test]
  fn test_resolve_url_from_location_relative_2() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "/y");
    assert_eq!(new_uri.host_str().unwrap(), "deno.land");
    assert_eq!(new_uri.path(), "/y");
  }

  #[test]
  fn test_resolve_url_from_location_relative_3() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "z");
    assert_eq!(new_uri.host_str().unwrap(), "deno.land");
    assert_eq!(new_uri.path(), "/z");
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_string() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("https://localhost:5545/fixture.json").unwrap();

    let client = create_http_client(
      version::get_user_agent(),
      None,
      vec![read(
        test_util::testdata_path()
          .join("tls/RootCA.pem")
          .to_str()
          .unwrap(),
      )
      .unwrap()],
      None,
      None,
      None,
    )
    .unwrap();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(headers.get("content-type").unwrap(), "application/json");
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_default_certificate_store() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server with a valid mozilla root CA cert.
    let url = Url::parse("https://deno.land").unwrap();
    let client = create_http_client(
      version::get_user_agent(),
      None, // This will load mozilla certs by default
      vec![],
      None,
      None,
      None,
    )
    .unwrap();

    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;

    println!("{:?}", result);
    if let Ok(FetchOnceResult::Code(body, _headers)) = result {
      assert!(!body.is_empty());
    } else {
      panic!();
    }
  }

  // TODO(@justinmchase): Windows should verify certs too and fail to make this request without ca certs
  #[cfg(not(windows))]
  #[tokio::test]
  #[ignore] // https://github.com/denoland/deno/issues/12561
  async fn test_fetch_with_empty_certificate_store() {
    use deno_runtime::deno_tls::rustls::RootCertStore;

    let _http_server_guard = test_util::http_server();
    // Relies on external http server with a valid mozilla root CA cert.
    let url = Url::parse("https://deno.land").unwrap();
    let client = create_http_client(
      version::get_user_agent(),
      Some(RootCertStore::empty()), // no certs loaded at all
      vec![],
      None,
      None,
      None,
    )
    .unwrap();

    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;

    if let Ok(FetchOnceResult::Code(_body, _headers)) = result {
      // This test is expected to fail since to CA certs have been loaded
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_gzip() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url =
      Url::parse("https://localhost:5545/053_import_compression/gziped")
        .unwrap();
    let client = create_http_client(
      version::get_user_agent(),
      None,
      vec![read(
        test_util::testdata_path()
          .join("tls/RootCA.pem")
          .to_str()
          .unwrap(),
      )
      .unwrap()],
      None,
      None,
      None,
    )
    .unwrap();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('gzip')");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
      );
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_with_etag() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("https://localhost:5545/etag_script.ts").unwrap();
    let client = create_http_client(
      version::get_user_agent(),
      None,
      vec![read(
        test_util::testdata_path()
          .join("tls/RootCA.pem")
          .to_str()
          .unwrap(),
      )
      .unwrap()],
      None,
      None,
      None,
    )
    .unwrap();
    let result = fetch_once(FetchOnceArgs {
      client: client.clone(),
      url: url.clone(),
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('etag')");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/typescript"
      );
      assert_eq!(headers.get("etag").unwrap(), "33a64df551425fcc55e");
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }

    let res = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: Some("33a64df551425fcc55e".to_string()),
      maybe_auth_token: None,
    })
    .await;
    assert_eq!(res.unwrap(), FetchOnceResult::NotModified);
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_brotli() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url =
      Url::parse("https://localhost:5545/053_import_compression/brotli")
        .unwrap();
    let client = create_http_client(
      version::get_user_agent(),
      None,
      vec![read(
        test_util::testdata_path()
          .join("tls/RootCA.pem")
          .to_str()
          .unwrap(),
      )
      .unwrap()],
      None,
      None,
      None,
    )
    .unwrap();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('brotli');");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
      );
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn bad_redirect() {
    let _g = test_util::http_server();
    let url_str = "http://127.0.0.1:4545/bad_redirect";
    let url = Url::parse(url_str).unwrap();
    let client = create_test_client();
    let result = fetch_once(FetchOnceArgs {
      client,
      url,
      maybe_accept: None,
      maybe_etag: None,
      maybe_auth_token: None,
    })
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    // Check that the error message contains the original URL
    assert!(err.to_string().contains(url_str));
  }
}
