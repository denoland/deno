// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::util::progress_bar::UpdateGuard;
use crate::version::get_user_agent;

use cache_control::Cachability;
use cache_control::CacheControl;
use chrono::DateTime;
use deno_core::anyhow::bail;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::url::Url;
use deno_runtime::deno_fetch::create_http_client;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::deno_fetch::reqwest::header::LOCATION;
use deno_runtime::deno_fetch::reqwest::Response;
use deno_runtime::deno_tls::rustls::RootCertStore;
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

pub fn resolve_redirect_from_response(
  request_url: &Url,
  response: &Response,
) -> Result<Url, AnyError> {
  debug_assert!(response.status().is_redirection());
  if let Some(location) = response.headers().get(LOCATION) {
    let location_string = location.to_str()?;
    log::debug!("Redirecting to {:?}...", &location_string);
    let new_url = resolve_url_from_location(request_url, location_string);
    Ok(new_url)
  } else {
    Err(generic_error(format!(
      "Redirection from '{request_url}' did not provide location header"
    )))
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
          .map(|val| val > self.age() - self.max_age())
          .unwrap_or(true);
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

#[derive(Debug, Clone)]
pub struct HttpClient(reqwest::Client);

impl HttpClient {
  pub fn new(
    root_cert_store: Option<RootCertStore>,
    unsafely_ignore_certificate_errors: Option<Vec<String>>,
  ) -> Result<Self, AnyError> {
    Ok(HttpClient::from_client(create_http_client(
      get_user_agent(),
      root_cert_store,
      vec![],
      None,
      unsafely_ignore_certificate_errors,
      None,
      None,
    )?))
  }

  pub fn from_client(client: reqwest::Client) -> Self {
    Self(client)
  }

  /// Do a GET request without following redirects.
  pub fn get_no_redirect<U: reqwest::IntoUrl>(
    &self,
    url: U,
  ) -> reqwest::RequestBuilder {
    self.0.get(url)
  }

  pub async fn download_text<U: reqwest::IntoUrl>(
    &self,
    url: U,
  ) -> Result<String, AnyError> {
    let bytes = self.download(url).await?;
    Ok(String::from_utf8(bytes)?)
  }

  pub async fn download<U: reqwest::IntoUrl>(
    &self,
    url: U,
  ) -> Result<Vec<u8>, AnyError> {
    let maybe_bytes = self.inner_download(url, None).await?;
    match maybe_bytes {
      Some(bytes) => Ok(bytes),
      None => Err(custom_error("Http", "Not found.")),
    }
  }

  pub async fn download_with_progress<U: reqwest::IntoUrl>(
    &self,
    url: U,
    progress_guard: &UpdateGuard,
  ) -> Result<Option<Vec<u8>>, AnyError> {
    self.inner_download(url, Some(progress_guard)).await
  }

  async fn inner_download<U: reqwest::IntoUrl>(
    &self,
    url: U,
    progress_guard: Option<&UpdateGuard>,
  ) -> Result<Option<Vec<u8>>, AnyError> {
    let response = self.get_redirected_response(url).await?;

    if response.status() == 404 {
      return Ok(None);
    } else if !response.status().is_success() {
      let status = response.status();
      let maybe_response_text = response.text().await.ok();
      bail!(
        "Bad response: {:?}{}",
        status,
        match maybe_response_text {
          Some(text) => format!("\n\n{text}"),
          None => String::new(),
        }
      );
    }

    get_response_body_with_progress(response, progress_guard)
      .await
      .map(Some)
  }

  pub async fn get_redirected_response<U: reqwest::IntoUrl>(
    &self,
    url: U,
  ) -> Result<Response, AnyError> {
    let mut url = url.into_url()?;
    let mut response = self.get_no_redirect(url.clone()).send().await?;
    let status = response.status();
    if status.is_redirection() {
      for _ in 0..5 {
        let new_url = resolve_redirect_from_response(&url, &response)?;
        let new_response = self.get_no_redirect(new_url.clone()).send().await?;
        let status = new_response.status();
        if status.is_redirection() {
          response = new_response;
          url = new_url;
        } else {
          return Ok(new_response);
        }
      }
      Err(custom_error("Http", "Too many redirects."))
    } else {
      Ok(response)
    }
  }
}

pub async fn get_response_body_with_progress(
  response: reqwest::Response,
  progress_guard: Option<&UpdateGuard>,
) -> Result<Vec<u8>, AnyError> {
  if let Some(progress_guard) = progress_guard {
    if let Some(total_size) = response.content_length() {
      progress_guard.set_total_size(total_size);
      let mut current_size = 0;
      let mut data = Vec::with_capacity(total_size as usize);
      let mut stream = response.bytes_stream();
      while let Some(item) = stream.next().await {
        let bytes = item?;
        current_size += bytes.len() as u64;
        progress_guard.set_position(current_size);
        data.extend(bytes.into_iter());
      }
      return Ok(data);
    }
  }
  let bytes = response.bytes().await?;
  Ok(bytes.into())
}

#[cfg(test)]
mod test {
  use super::*;

  #[tokio::test]
  async fn test_http_client_download_redirect() {
    let _http_server_guard = test_util::http_server();
    let client = HttpClient::new(None, None).unwrap();

    // make a request to the redirect server
    let text = client
      .download_text("http://localhost:4546/subdir/redirects/redirect1.js")
      .await
      .unwrap();
    assert_eq!(text, "export const redirect = 1;\n");

    // now make one to the infinite redirects server
    let err = client
      .download_text("http://localhost:4549/subdir/redirects/redirect1.js")
      .await
      .err()
      .unwrap();
    assert_eq!(err.to_string(), "Too many redirects.");
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
}
