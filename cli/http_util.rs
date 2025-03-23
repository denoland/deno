// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread::ThreadId;

use boxed_error::Boxed;
use deno_cache_dir::file_fetcher::RedirectHeaderParseError;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsError;
use deno_error::JsErrorBox;
use deno_lib::version::DENO_VERSION_INFO;
use deno_runtime::deno_fetch;
use deno_runtime::deno_fetch::create_http_client;
use deno_runtime::deno_fetch::CreateHttpClientOptions;
use deno_runtime::deno_fetch::ResBody;
use deno_runtime::deno_tls::RootCertStoreProvider;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::header::CONTENT_LENGTH;
use http::HeaderMap;
use http::StatusCode;
use http_body_util::BodyExt;
use thiserror::Error;

use crate::util::progress_bar::UpdateGuard;

#[derive(Debug, Error)]
pub enum SendError {
  #[error(transparent)]
  Send(#[from] deno_fetch::ClientSendError),
  #[error(transparent)]
  InvalidUri(#[from] http::uri::InvalidUri),
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

  pub fn get_or_create(&self) -> Result<HttpClient, JsErrorBox> {
    use std::collections::hash_map::Entry;
    let thread_id = std::thread::current().id();
    let mut clients = self.clients_by_thread_id.lock();
    let entry = clients.entry(thread_id);
    match entry {
      Entry::Occupied(entry) => Ok(HttpClient::new(entry.get().clone())),
      Entry::Vacant(entry) => {
        let client = create_http_client(
          DENO_VERSION_INFO.user_agent,
          CreateHttpClientOptions {
            root_cert_store: match &self.root_cert_store_provider {
              Some(provider) => Some(provider.get_or_try_init()?.clone()),
              None => None,
            },
            ..self.options.clone()
          },
        )
        .map_err(JsErrorBox::from_err)?;
        entry.insert(client.clone());
        Ok(HttpClient::new(client))
      }
    }
  }
}

#[derive(Debug, Error, JsError)]
#[class(type)]
#[error("Bad response: {:?}{}", .status_code, .response_text.as_ref().map(|s| format!("\n\n{}", s)).unwrap_or_else(String::new))]
pub struct BadResponseError {
  pub status_code: StatusCode,
  pub response_text: Option<String>,
}

#[derive(Debug, Boxed, JsError)]
pub struct DownloadError(pub Box<DownloadErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum DownloadErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  Fetch(deno_fetch::ClientSendError),
  #[class(inherit)]
  #[error(transparent)]
  UrlParse(#[from] deno_core::url::ParseError),
  #[class(generic)]
  #[error(transparent)]
  HttpParse(#[from] http::Error),
  #[class(inherit)]
  #[error(transparent)]
  Json(#[from] serde_json::Error),
  #[class(generic)]
  #[error(transparent)]
  ToStr(#[from] http::header::ToStrError),
  #[class(inherit)]
  #[error(transparent)]
  RedirectHeaderParse(RedirectHeaderParseError),
  #[class(type)]
  #[error("Too many redirects.")]
  TooManyRedirects,
  #[class(inherit)]
  #[error(transparent)]
  BadResponse(#[from] BadResponseError),
  #[class("Http")]
  #[error("Not Found.")]
  NotFound,
  #[class(inherit)]
  #[error(transparent)]
  Other(JsErrorBox),
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
    let body = deno_fetch::ReqBody::empty();
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
    let body = deno_fetch::ReqBody::full(json.into());
    let builder = self.post(url, body)?;
    Ok(builder.header(
      http::header::CONTENT_TYPE,
      "application/json".parse().map_err(http::Error::from)?,
    ))
  }

  pub async fn send(
    &self,
    url: &Url,
    headers: HeaderMap,
  ) -> Result<http::Response<ResBody>, SendError> {
    let body = deno_fetch::ReqBody::empty();
    let mut request = http::Request::new(body);
    *request.uri_mut() = http::Uri::try_from(url.as_str())?;
    *request.headers_mut() = headers;

    self
      .client
      .clone()
      .send(request)
      .await
      .map_err(SendError::Send)
  }

  pub async fn download_text(&self, url: Url) -> Result<String, AnyError> {
    let bytes = self.download(url).await?;
    Ok(String::from_utf8(bytes)?)
  }

  pub async fn download(&self, url: Url) -> Result<Vec<u8>, DownloadError> {
    let maybe_bytes = self.download_inner(url, None, None).await?;
    match maybe_bytes {
      Some(bytes) => Ok(bytes),
      None => Err(DownloadErrorKind::NotFound.into_box()),
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
      |e| {
        matches!(
          e.as_kind(),
          DownloadErrorKind::BadResponse(_) | DownloadErrorKind::Fetch(_)
        )
      },
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
      return Err(
        DownloadErrorKind::BadResponse(BadResponseError {
          status_code: status,
          response_text: maybe_response_text
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        })
        .into_box(),
      );
    }

    get_response_body_with_progress(response, progress_guard)
      .await
      .map(|(_, body)| Some(body))
      .map_err(|err| DownloadErrorKind::Other(err).into_box())
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
      .map_err(|e| DownloadErrorKind::Fetch(e).into_box())?;
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
          .map_err(|e| DownloadErrorKind::Fetch(e).into_box())?;
        let status = new_response.status();
        if status.is_redirection() {
          response = new_response;
          url = new_url;
        } else {
          return Ok((new_response, new_url));
        }
      }
      Err(DownloadErrorKind::TooManyRedirects.into_box())
    } else {
      Ok((response, url))
    }
  }
}

pub async fn get_response_body_with_progress(
  response: http::Response<deno_fetch::ResBody>,
  progress_guard: Option<&UpdateGuard>,
) -> Result<(HeaderMap, Vec<u8>), JsErrorBox> {
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
      let (parts, body) = response.into_parts();
      let mut stream = body.into_data_stream();
      while let Some(item) = stream.next().await {
        let bytes = item?;
        current_size += bytes.len() as u64;
        progress_guard.set_position(current_size);
        data.extend(bytes.into_iter());
      }
      return Ok((parts.headers, data));
    }
  }

  let (parts, body) = response.into_parts();
  let bytes = body.collect().await?.to_bytes();
  Ok((parts.headers, bytes.into()))
}

fn resolve_redirect_from_response<B>(
  request_url: &Url,
  response: &http::Response<B>,
) -> Result<Url, DownloadError> {
  debug_assert!(response.status().is_redirection());
  deno_cache_dir::file_fetcher::resolve_redirect_from_headers(
    request_url,
    response.headers(),
  )
  .map_err(|err| DownloadErrorKind::RedirectHeaderParse(*err).into_box())
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

  #[tokio::test]
  async fn test_fetch_with_cafile_string() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("https://localhost:5545/assets/fixture.json").unwrap();

    let client = HttpClient::new(
      create_http_client(
        DENO_VERSION_INFO.user_agent,
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
    let response = client.send(&url, Default::default()).await.unwrap();
    assert!(response.status().is_success());
    let (parts, body) = response.into_parts();
    let headers = parts.headers;
    let body = body.collect().await.unwrap().to_bytes();
    assert!(!body.is_empty());
    assert_eq!(headers.get("content-type").unwrap(), "application/json");
    assert_eq!(headers.get("etag"), None);
    assert_eq!(headers.get("x-typescript-types"), None);
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
          DENO_VERSION_INFO.user_agent,
          CreateHttpClientOptions::default(),
        )
        .unwrap(),
      );

      let result = client.send(&url, Default::default()).await;
      match result {
        Ok(response) if response.status().is_success() => {
          return; // success
        }
        _ => {
          // keep going
        }
      }
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
        DENO_VERSION_INFO.user_agent,
        CreateHttpClientOptions {
          root_cert_store: Some(root_cert_store),
          ..Default::default()
        },
      )
      .unwrap(),
    );

    let result = client.send(&url, HeaderMap::new()).await;
    assert!(result.is_err() || !result.unwrap().status().is_success());
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_gzip() {
    let _http_server_guard = test_util::http_server();
    let url =
      Url::parse("https://localhost:5545/run/import_compression/gziped")
        .unwrap();
    let client = HttpClient::new(
      create_http_client(
        DENO_VERSION_INFO.user_agent,
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
    let response = client.send(&url, Default::default()).await.unwrap();
    assert!(response.status().is_success());
    let (parts, body) = response.into_parts();
    let headers = parts.headers;
    let body = body.collect().await.unwrap().to_bytes().to_vec();
    assert_eq!(String::from_utf8(body).unwrap(), "console.log('gzip')");
    assert_eq!(
      headers.get("content-type").unwrap(),
      "application/javascript"
    );
    assert_eq!(headers.get("etag"), None);
    assert_eq!(headers.get("x-typescript-types"), None);
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_with_etag() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("https://localhost:5545/etag_script.ts").unwrap();
    let client = HttpClient::new(
      create_http_client(
        DENO_VERSION_INFO.user_agent,
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
    let response = client.send(&url, Default::default()).await.unwrap();
    assert!(response.status().is_success());
    let (parts, body) = response.into_parts();
    let headers = parts.headers;
    let body = body.collect().await.unwrap().to_bytes().to_vec();
    assert!(!body.is_empty());
    assert_eq!(String::from_utf8(body).unwrap(), "console.log('etag')");
    assert_eq!(
      headers.get("content-type").unwrap(),
      "application/typescript"
    );
    assert_eq!(headers.get("etag").unwrap(), "33a64df551425fcc55e");
    assert_eq!(headers.get("x-typescript-types"), None);

    let mut headers = HeaderMap::new();
    headers.insert("If-None-Match", "33a64df551425fcc55e".parse().unwrap());
    let res = client.send(&url, headers).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_brotli() {
    let _http_server_guard = test_util::http_server();
    let url =
      Url::parse("https://localhost:5545/run/import_compression/brotli")
        .unwrap();
    let client = HttpClient::new(
      create_http_client(
        DENO_VERSION_INFO.user_agent,
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
    let response = client.send(&url, Default::default()).await.unwrap();
    assert!(response.status().is_success());
    let (parts, body) = response.into_parts();
    let headers = parts.headers;
    let body = body.collect().await.unwrap().to_bytes().to_vec();
    assert!(!body.is_empty());
    assert_eq!(String::from_utf8(body).unwrap(), "console.log('brotli');");
    assert_eq!(
      headers.get("content-type").unwrap(),
      "application/javascript"
    );
    assert_eq!(headers.get("etag"), None);
    assert_eq!(headers.get("x-typescript-types"), None);
  }
}
