// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::auth_tokens::AuthToken;
use crate::util::progress_bar::UpdateGuard;
use crate::version;

use cache_control::Cachability;
use cache_control::CacheControl;
use chrono::DateTime;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_runtime::deno_fetch;
use deno_runtime::deno_fetch::create_http_client;
use deno_runtime::deno_fetch::CreateHttpClientOptions;
use deno_runtime::deno_tls::RootCertStoreProvider;
use http::header;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::header::ACCEPT;
use http::header::AUTHORIZATION;
use http::header::CONTENT_LENGTH;
use http::header::IF_NONE_MATCH;
use http::header::LOCATION;
use http::StatusCode;
use http_body_util::BodyExt;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread::ThreadId;
use std::time::Duration;
use std::time::SystemTime;
use thiserror::Error;

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

#[derive(Debug, Eq, PartialEq)]
pub enum FetchOnceResult {
  Code(Vec<u8>, HeadersMap),
  NotModified,
  Redirect(Url, HeadersMap),
  RequestError(String),
  ServerError(StatusCode),
}

#[derive(Debug)]
pub struct FetchOnceArgs<'a> {
  pub url: Url,
  pub maybe_accept: Option<String>,
  pub maybe_etag: Option<String>,
  pub maybe_auth_token: Option<AuthToken>,
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_progress_guard: Option<&'a UpdateGuard>,
}

pub struct HttpClientProvider {
  options: CreateHttpClientOptions,
  root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
  // it's not safe to share a reqwest::Client across tokio runtimes,
  // so we store these Clients keyed by thread id
  // https://github.com/seanmonstar/reqwest/issues/1148#issuecomment-910868788
  clients_by_thread_id: Mutex<HashMap<ThreadId, deno_fetch::Client>>,
}

impl std::fmt::Debug for HttpClientProvider {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("HttpClient")
      .field("options", &self.options)
      .finish()
  }
}

impl HttpClientProvider {
  pub fn new(
    root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
    unsafely_ignore_certificate_errors: Option<Vec<String>>,
  ) -> Self {
    Self {
      options: CreateHttpClientOptions {
        unsafely_ignore_certificate_errors,
        ..Default::default()
      },
      root_cert_store_provider,
      clients_by_thread_id: Default::default(),
    }
  }

  pub fn get_or_create(&self) -> Result<HttpClient, AnyError> {
    use std::collections::hash_map::Entry;
    let thread_id = std::thread::current().id();
    let mut clients = self.clients_by_thread_id.lock();
    let entry = clients.entry(thread_id);
    match entry {
      Entry::Occupied(entry) => Ok(HttpClient::new(entry.get().clone())),
      Entry::Vacant(entry) => {
        let client = create_http_client(
          version::DENO_VERSION_INFO.user_agent,
          CreateHttpClientOptions {
            root_cert_store: match &self.root_cert_store_provider {
              Some(provider) => Some(provider.get_or_try_init()?.clone()),
              None => None,
            },
            ..self.options.clone()
          },
        )?;
        entry.insert(client.clone());
        Ok(HttpClient::new(client))
      }
    }
  }
}

#[derive(Debug, Error)]
#[error("Bad response: {:?}{}", .status_code, .response_text.as_ref().map(|s| format!("\n\n{}", s)).unwrap_or_else(String::new))]
pub struct BadResponseError {
  pub status_code: StatusCode,
  pub response_text: Option<String>,
}

#[derive(Debug, Error)]
pub enum DownloadError {
  #[error(transparent)]
  Fetch(AnyError),
  #[error(transparent)]
  UrlParse(#[from] deno_core::url::ParseError),
  #[error(transparent)]
  HttpParse(#[from] http::Error),
  #[error(transparent)]
  Json(#[from] serde_json::Error),
  #[error(transparent)]
  ToStr(#[from] http::header::ToStrError),
  #[error("Redirection from '{}' did not provide location header", .request_url)]
  NoRedirectHeader { request_url: Url },
  #[error("Too many redirects.")]
  TooManyRedirects,
  #[error(transparent)]
  BadResponse(#[from] BadResponseError),
}

#[derive(Debug)]
pub struct HttpClient {
  client: deno_fetch::Client,
  // don't allow sending this across threads because then
  // it might be shared accidentally across tokio runtimes
  // which will cause issues
  // https://github.com/seanmonstar/reqwest/issues/1148#issuecomment-910868788
  _unsend_marker: deno_core::unsync::UnsendMarker,
}

impl HttpClient {
  // DO NOT make this public. You should always be creating one of these from
  // the HttpClientProvider
  fn new(client: deno_fetch::Client) -> Self {
    Self {
      client,
      _unsend_marker: deno_core::unsync::UnsendMarker::default(),
    }
  }

  pub fn get(&self, url: Url) -> Result<RequestBuilder, http::Error> {
    let body = http_body_util::Empty::new()
      .map_err(|never| match never {})
      .boxed();
    let mut req = http::Request::new(body);
    *req.uri_mut() = url.as_str().parse()?;
    Ok(RequestBuilder {
      client: self.client.clone(),
      req,
    })
  }

  pub fn post(
    &self,
    url: Url,
    body: deno_fetch::ReqBody,
  ) -> Result<RequestBuilder, http::Error> {
    let mut req = http::Request::new(body);
    *req.method_mut() = http::Method::POST;
    *req.uri_mut() = url.as_str().parse()?;
    Ok(RequestBuilder {
      client: self.client.clone(),
      req,
    })
  }

  pub fn post_json<S>(
    &self,
    url: Url,
    ser: &S,
  ) -> Result<RequestBuilder, DownloadError>
  where
    S: serde::Serialize,
  {
    let json = deno_core::serde_json::to_vec(ser)?;
    let body = http_body_util::Full::new(json.into())
      .map_err(|never| match never {})
      .boxed();
    let builder = self.post(url, body)?;
    Ok(builder.header(
      http::header::CONTENT_TYPE,
      "application/json".parse().map_err(http::Error::from)?,
    ))
  }

  /// Asynchronously fetches the given HTTP URL one pass only.
  /// If no redirect is present and no error occurs,
  /// yields Code(ResultPayload).
  /// If redirect occurs, does not follow and
  /// yields Redirect(url).
  pub async fn fetch_no_follow<'a>(
    &self,
    args: FetchOnceArgs<'a>,
  ) -> Result<FetchOnceResult, AnyError> {
    let body = http_body_util::Empty::new()
      .map_err(|never| match never {})
      .boxed();
    let mut request = http::Request::new(body);
    *request.uri_mut() = args.url.as_str().parse()?;

    if let Some(etag) = args.maybe_etag {
      let if_none_match_val = HeaderValue::from_str(&etag)?;
      request
        .headers_mut()
        .insert(IF_NONE_MATCH, if_none_match_val);
    }
    if let Some(auth_token) = args.maybe_auth_token {
      let authorization_val = HeaderValue::from_str(&auth_token.to_string())?;
      request
        .headers_mut()
        .insert(AUTHORIZATION, authorization_val);
    } else if let Some((header, value)) = args.maybe_auth {
      request.headers_mut().insert(header, value);
    }
    if let Some(accept) = args.maybe_accept {
      let accepts_val = HeaderValue::from_str(&accept)?;
      request.headers_mut().insert(ACCEPT, accepts_val);
    }
    let response = match self.client.clone().send(request).await {
      Ok(resp) => resp,
      Err(err) => {
        if err.is_connect_error() {
          return Ok(FetchOnceResult::RequestError(err.to_string()));
        }
        return Err(err.into());
      }
    };

    if response.status() == StatusCode::NOT_MODIFIED {
      return Ok(FetchOnceResult::NotModified);
    }

    let mut result_headers = HashMap::new();
    let response_headers = response.headers();

    if let Some(warning) = response_headers.get("X-Deno-Warning") {
      log::warn!(
        "{} {}",
        crate::colors::yellow("Warning"),
        warning.to_str().unwrap()
      );
    }

    for key in response_headers.keys() {
      let key_str = key.to_string();
      let values = response_headers.get_all(key);
      let values_str = values
        .iter()
        .map(|e| e.to_str().unwrap().to_string())
        .collect::<Vec<String>>()
        .join(",");
      result_headers.insert(key_str, values_str);
    }

    if response.status().is_redirection() {
      let new_url = resolve_redirect_from_response(&args.url, &response)?;
      return Ok(FetchOnceResult::Redirect(new_url, result_headers));
    }

    let status = response.status();

    if status.is_server_error() {
      return Ok(FetchOnceResult::ServerError(status));
    }

    if status.is_client_error() {
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

    let body =
      get_response_body_with_progress(response, args.maybe_progress_guard)
        .await?;

    Ok(FetchOnceResult::Code(body, result_headers))
  }

  pub async fn download_text(&self, url: Url) -> Result<String, AnyError> {
    let bytes = self.download(url).await?;
    Ok(String::from_utf8(bytes)?)
  }

  pub async fn download(&self, url: Url) -> Result<Vec<u8>, AnyError> {
    let maybe_bytes = self.download_inner(url, None, None).await?;
    match maybe_bytes {
      Some(bytes) => Ok(bytes),
      None => Err(custom_error("Http", "Not found.")),
    }
  }

  pub async fn download_with_progress_and_retries(
    &self,
    url: Url,
    maybe_header: Option<(HeaderName, HeaderValue)>,
    progress_guard: &UpdateGuard,
  ) -> Result<Option<Vec<u8>>, DownloadError> {
    crate::util::retry::retry(
      || {
        self.download_inner(
          url.clone(),
          maybe_header.clone(),
          Some(progress_guard),
        )
      },
      |e| matches!(e, DownloadError::BadResponse(_) | DownloadError::Fetch(_)),
    )
    .await
  }

  pub async fn get_redirected_url(
    &self,
    url: Url,
    maybe_header: Option<(HeaderName, HeaderValue)>,
  ) -> Result<Url, AnyError> {
    let (_, url) = self.get_redirected_response(url, maybe_header).await?;
    Ok(url)
  }

  async fn download_inner(
    &self,
    url: Url,
    maybe_header: Option<(HeaderName, HeaderValue)>,
    progress_guard: Option<&UpdateGuard>,
  ) -> Result<Option<Vec<u8>>, DownloadError> {
    let (response, _) = self.get_redirected_response(url, maybe_header).await?;

    if response.status() == 404 {
      return Ok(None);
    } else if !response.status().is_success() {
      let status = response.status();
      let maybe_response_text = body_to_string(response).await.ok();
      return Err(DownloadError::BadResponse(BadResponseError {
        status_code: status,
        response_text: maybe_response_text
          .map(|s| s.trim().to_string())
          .filter(|s| !s.is_empty()),
      }));
    }

    get_response_body_with_progress(response, progress_guard)
      .await
      .map(Some)
      .map_err(DownloadError::Fetch)
  }

  async fn get_redirected_response(
    &self,
    mut url: Url,
    mut maybe_header: Option<(HeaderName, HeaderValue)>,
  ) -> Result<(http::Response<deno_fetch::ResBody>, Url), DownloadError> {
    let mut req = self.get(url.clone())?.build();
    if let Some((header_name, header_value)) = maybe_header.as_ref() {
      req.headers_mut().append(header_name, header_value.clone());
    }
    let mut response = self
      .client
      .clone()
      .send(req)
      .await
      .map_err(|e| DownloadError::Fetch(e.into()))?;
    let status = response.status();
    if status.is_redirection() {
      for _ in 0..5 {
        let new_url = resolve_redirect_from_response(&url, &response)?;
        let mut req = self.get(new_url.clone())?.build();

        if new_url.origin() == url.origin() {
          if let Some((header_name, header_value)) = maybe_header.as_ref() {
            req.headers_mut().append(header_name, header_value.clone());
          }
        } else {
          maybe_header = None;
        }

        let new_response = self
          .client
          .clone()
          .send(req)
          .await
          .map_err(|e| DownloadError::Fetch(e.into()))?;
        let status = new_response.status();
        if status.is_redirection() {
          response = new_response;
          url = new_url;
        } else {
          return Ok((new_response, new_url));
        }
      }
      Err(DownloadError::TooManyRedirects)
    } else {
      Ok((response, url))
    }
  }
}

async fn get_response_body_with_progress(
  response: http::Response<deno_fetch::ResBody>,
  progress_guard: Option<&UpdateGuard>,
) -> Result<Vec<u8>, AnyError> {
  use http_body::Body as _;
  if let Some(progress_guard) = progress_guard {
    let mut total_size = response.body().size_hint().exact();
    if total_size.is_none() {
      total_size = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|val| val.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    }
    if let Some(total_size) = total_size {
      progress_guard.set_total_size(total_size);
      let mut current_size = 0;
      let mut data = Vec::with_capacity(total_size as usize);
      let mut stream = response.into_body().into_data_stream();
      while let Some(item) = stream.next().await {
        let bytes = item?;
        current_size += bytes.len() as u64;
        progress_guard.set_position(current_size);
        data.extend(bytes.into_iter());
      }
      return Ok(data);
    }
  }
  let bytes = response.collect().await?.to_bytes();
  Ok(bytes.into())
}

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

fn resolve_redirect_from_response<B>(
  request_url: &Url,
  response: &http::Response<B>,
) -> Result<Url, DownloadError> {
  debug_assert!(response.status().is_redirection());
  if let Some(location) = response.headers().get(LOCATION) {
    let location_string = location.to_str()?;
    log::debug!("Redirecting to {:?}...", &location_string);
    let new_url = resolve_url_from_location(request_url, location_string);
    Ok(new_url)
  } else {
    Err(DownloadError::NoRedirectHeader {
      request_url: request_url.clone(),
    })
  }
}

pub async fn body_to_string<B>(body: B) -> Result<String, AnyError>
where
  B: http_body::Body,
  AnyError: From<B::Error>,
{
  let bytes = body.collect().await?.to_bytes();
  let s = std::str::from_utf8(&bytes)?;
  Ok(s.into())
}

pub async fn body_to_json<B, D>(body: B) -> Result<D, AnyError>
where
  B: http_body::Body,
  AnyError: From<B::Error>,
  D: serde::de::DeserializeOwned,
{
  let bytes = body.collect().await?.to_bytes();
  let val = deno_core::serde_json::from_slice(&bytes)?;
  Ok(val)
}

pub struct RequestBuilder {
  client: deno_fetch::Client,
  req: http::Request<deno_fetch::ReqBody>,
}

impl RequestBuilder {
  pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
    self.req.headers_mut().append(name, value);
    self
  }

  pub async fn send(
    self,
  ) -> Result<http::Response<deno_fetch::ResBody>, AnyError> {
    self.client.send(self.req).await.map_err(Into::into)
  }

  pub fn build(self) -> http::Request<deno_fetch::ReqBody> {
    self.req
  }
}

#[allow(clippy::print_stdout)]
#[allow(clippy::print_stderr)]
#[cfg(test)]
mod test {
  use std::collections::HashSet;
  use std::hash::RandomState;

  use deno_runtime::deno_tls::rustls::RootCertStore;

  use crate::version;

  use super::*;

  #[tokio::test]
  async fn test_http_client_download_redirect() {
    let _http_server_guard = test_util::http_server();
    let client = HttpClientProvider::new(None, None).get_or_create().unwrap();

    // make a request to the redirect server
    let text = client
      .download_text(
        Url::parse("http://localhost:4546/subdir/redirects/redirect1.js")
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(text, "export const redirect = 1;\n");

    // now make one to the infinite redirects server
    let err = client
      .download_text(
        Url::parse("http://localhost:4549/subdir/redirects/redirect1.js")
          .unwrap(),
      )
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

  fn create_test_client() -> HttpClient {
    HttpClient::new(
      create_http_client("test_client", CreateHttpClientOptions::default())
        .unwrap(),
    )
  }

  #[tokio::test]
  async fn test_fetch_string() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/assets/fixture.json").unwrap();
    let client = create_test_client();
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
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
    let url = Url::parse("http://127.0.0.1:4545/run/import_compression/gziped")
      .unwrap();
    let client = create_test_client();
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
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
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url: url.clone(),
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
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

    let res = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: Some("33a64df551425fcc55e".to_string()),
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
      })
      .await;
    assert_eq!(res.unwrap(), FetchOnceResult::NotModified);
  }

  #[tokio::test]
  async fn test_fetch_brotli() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/run/import_compression/brotli")
      .unwrap();
    let client = create_test_client();
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
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
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: Some("application/json".to_string()),
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
      })
      .await;
    if let Ok(FetchOnceResult::Code(body, _)) = result {
      assert_eq!(body, r#"{"accept":"application/json"}"#.as_bytes());
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_no_follow_with_redirect() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4546/assets/fixture.json").unwrap();
    // Dns resolver substitutes `127.0.0.1` with `localhost`
    let target_url =
      Url::parse("http://localhost:4545/assets/fixture.json").unwrap();
    let client = create_test_client();
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
      })
      .await;
    if let Ok(FetchOnceResult::Redirect(url, _)) = result {
      assert_eq!(url, target_url);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_string() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("https://localhost:5545/assets/fixture.json").unwrap();

    let client = HttpClient::new(
      create_http_client(
        version::DENO_VERSION_INFO.user_agent,
        CreateHttpClientOptions {
          ca_certs: vec![std::fs::read(
            test_util::testdata_path().join("tls/RootCA.pem"),
          )
          .unwrap()],
          ..Default::default()
        },
      )
      .unwrap(),
    );
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
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

  static PUBLIC_HTTPS_URLS: &[&str] = &[
    "https://deno.com/",
    "https://example.com/",
    "https://github.com/",
    "https://www.w3.org/",
  ];

  /// This test depends on external servers, so we need to be careful to avoid mistaking an offline machine with a
  /// test failure.
  #[tokio::test]
  async fn test_fetch_with_default_certificate_store() {
    let urls: HashSet<_, RandomState> =
      HashSet::from_iter(PUBLIC_HTTPS_URLS.iter());

    // Rely on the randomization of hashset iteration
    for url in urls {
      // Relies on external http server with a valid mozilla root CA cert.
      let url = Url::parse(url).unwrap();
      eprintln!("Attempting to fetch {url}...");

      let client = HttpClient::new(
        create_http_client(
          version::DENO_VERSION_INFO.user_agent,
          CreateHttpClientOptions::default(),
        )
        .unwrap(),
      );

      let result = client
        .fetch_no_follow(FetchOnceArgs {
          url,
          maybe_accept: None,
          maybe_etag: None,
          maybe_auth_token: None,
          maybe_progress_guard: None,
          maybe_auth: None,
        })
        .await;

      match result {
        Err(_) => {
          eprintln!("Fetch error: {result:?}");
          continue;
        }
        Ok(
          FetchOnceResult::Code(..)
          | FetchOnceResult::NotModified
          | FetchOnceResult::Redirect(..),
        ) => return,
        Ok(
          FetchOnceResult::RequestError(_) | FetchOnceResult::ServerError(_),
        ) => {
          eprintln!("HTTP error: {result:?}");
          continue;
        }
      };
    }

    // Use 1.1.1.1 and 8.8.8.8 as our last-ditch internet check
    if std::net::TcpStream::connect("8.8.8.8:80").is_err()
      && std::net::TcpStream::connect("1.1.1.1:80").is_err()
    {
      return;
    }

    panic!("None of the expected public URLs were available but internet appears to be available");
  }

  #[tokio::test]
  async fn test_fetch_with_empty_certificate_store() {
    let root_cert_store = RootCertStore::empty();
    let urls: HashSet<_, RandomState> =
      HashSet::from_iter(PUBLIC_HTTPS_URLS.iter());

    // Rely on the randomization of hashset iteration
    let url = urls.into_iter().next().unwrap();
    // Relies on external http server with a valid mozilla root CA cert.
    let url = Url::parse(url).unwrap();
    eprintln!("Attempting to fetch {url}...");

    let client = HttpClient::new(
      create_http_client(
        version::DENO_VERSION_INFO.user_agent,
        CreateHttpClientOptions {
          root_cert_store: Some(root_cert_store),
          ..Default::default()
        },
      )
      .unwrap(),
    );

    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
      })
      .await;

    match result {
      Err(_) => {
        eprintln!("Fetch error (expected): {result:?}");
        return;
      }
      Ok(
        FetchOnceResult::Code(..)
        | FetchOnceResult::NotModified
        | FetchOnceResult::Redirect(..),
      ) => {
        panic!("Should not have successfully fetched a URL");
      }
      Ok(
        FetchOnceResult::RequestError(_) | FetchOnceResult::ServerError(_),
      ) => {
        eprintln!("HTTP error (expected): {result:?}");
        return;
      }
    };
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_gzip() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url =
      Url::parse("https://localhost:5545/run/import_compression/gziped")
        .unwrap();
    let client = HttpClient::new(
      create_http_client(
        version::DENO_VERSION_INFO.user_agent,
        CreateHttpClientOptions {
          ca_certs: vec![std::fs::read(
            test_util::testdata_path()
              .join("tls/RootCA.pem")
              .to_string(),
          )
          .unwrap()],
          ..Default::default()
        },
      )
      .unwrap(),
    );
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
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
    let client = HttpClient::new(
      create_http_client(
        version::DENO_VERSION_INFO.user_agent,
        CreateHttpClientOptions {
          ca_certs: vec![std::fs::read(
            test_util::testdata_path()
              .join("tls/RootCA.pem")
              .to_string(),
          )
          .unwrap()],
          ..Default::default()
        },
      )
      .unwrap(),
    );
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url: url.clone(),
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
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

    let res = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: Some("33a64df551425fcc55e".to_string()),
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
      })
      .await;
    assert_eq!(res.unwrap(), FetchOnceResult::NotModified);
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_brotli() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url =
      Url::parse("https://localhost:5545/run/import_compression/brotli")
        .unwrap();
    let client = HttpClient::new(
      create_http_client(
        version::DENO_VERSION_INFO.user_agent,
        CreateHttpClientOptions {
          ca_certs: vec![std::fs::read(
            test_util::testdata_path()
              .join("tls/RootCA.pem")
              .to_string(),
          )
          .unwrap()],
          ..Default::default()
        },
      )
      .unwrap(),
    );
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
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
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
      })
      .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    // Check that the error message contains the original URL
    assert!(err.to_string().contains(url_str));
  }

  #[tokio::test]
  async fn server_error() {
    let _g = test_util::http_server();
    let url_str = "http://127.0.0.1:4545/server_error";
    let url = Url::parse(url_str).unwrap();
    let client = create_test_client();
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
      })
      .await;

    if let Ok(FetchOnceResult::ServerError(status)) = result {
      assert_eq!(status, 500);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn request_error() {
    let _g = test_util::http_server();
    let url_str = "http://127.0.0.1:9999/";
    let url = Url::parse(url_str).unwrap();
    let client = create_test_client();
    let result = client
      .fetch_no_follow(FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
        maybe_auth: None,
      })
      .await;

    assert!(matches!(result, Ok(FetchOnceResult::RequestError(_))));
  }
}
