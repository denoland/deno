// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::deno_error::DenoError;
use crate::version;
use brotli2::read::BrotliDecoder;
use bytes::Bytes;
use deno_core::ErrBox;
use futures::future::FutureExt;
use reqwest;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::ACCEPT_ENCODING;
use reqwest::header::CONTENT_ENCODING;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::ETAG;
use reqwest::header::IF_NONE_MATCH;
use reqwest::header::LOCATION;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use reqwest::Client;
use reqwest::Response;
use reqwest::StatusCode;
use std::cmp::min;
use std::future::Future;
use std::io;
use std::io::Read;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;
use url::Url;

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn create_http_client() -> Client {
  let mut headers = HeaderMap::new();
  headers.insert(
    USER_AGENT,
    format!("Deno/{}", version::DENO).parse().unwrap(),
  );
  Client::builder()
    .redirect(Policy::none())
    .default_headers(headers)
    .use_rustls_tls()
    .build()
    .unwrap()
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

#[derive(Debug, PartialEq)]
pub struct ResultPayload {
  pub body: Vec<u8>,
  pub content_type: Option<String>,
  pub etag: Option<String>,
  pub x_typescript_types: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum FetchOnceResult {
  Code(ResultPayload),
  NotModified,
  Redirect(Url),
}

/// Asynchronously fetches the given HTTP URL one pass only.
/// If no redirect is present and no error occurs,
/// yields Code(ResultPayload).
/// If redirect occurs, does not follow and
/// yields Redirect(url).
pub fn fetch_once(
  client: Client,
  url: &Url,
  cached_etag: Option<String>,
) -> impl Future<Output = Result<FetchOnceResult, ErrBox>> {
  let url = url.clone();

  let fut = async move {
    let mut request = client
      .get(url.clone())
      .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip, br"));

    if let Some(etag) = cached_etag {
      let if_none_match_val = HeaderValue::from_str(&etag).unwrap();
      request = request.header(IF_NONE_MATCH, if_none_match_val);
    }
    let response = request.send().await?;

    if response.status() == StatusCode::NOT_MODIFIED {
      return Ok(FetchOnceResult::NotModified);
    }

    if response.status().is_redirection() {
      let location_string = response
        .headers()
        .get(LOCATION)
        .expect("url redirection should provide 'location' header")
        .to_str()
        .unwrap();

      debug!("Redirecting to {:?}...", &location_string);
      let new_url = resolve_url_from_location(&url, location_string);
      return Ok(FetchOnceResult::Redirect(new_url));
    }

    if response.status().is_client_error()
      || response.status().is_server_error()
    {
      let err = DenoError::new(
        deno_error::ErrorKind::Other,
        format!("Import '{}' failed: {}", &url, response.status()),
      );
      return Err(err.into());
    }

    let content_type = response
      .headers()
      .get(CONTENT_TYPE)
      .map(|content_type| content_type.to_str().unwrap().to_owned());

    let etag = response
      .headers()
      .get(ETAG)
      .map(|etag| etag.to_str().unwrap().to_owned());

    let content_encoding = response
      .headers()
      .get(CONTENT_ENCODING)
      .map(|content_encoding| content_encoding.to_str().unwrap().to_owned());

    const X_TYPESCRIPT_TYPES: &str = "X-TypeScript-Types";

    let x_typescript_types =
      response
        .headers()
        .get(X_TYPESCRIPT_TYPES)
        .map(|x_typescript_types| {
          x_typescript_types.to_str().unwrap().to_owned()
        });

    let body;
    if let Some(content_encoding) = content_encoding {
      body = match content_encoding {
        _ if content_encoding == "br" => {
          let full_bytes = response.bytes().await?;
          let mut decoder = BrotliDecoder::new(full_bytes.as_ref());
          let mut body = vec![];
          decoder.read_to_end(&mut body)?;
          body
        }
        _ => response.bytes().await?.to_vec(),
      }
    } else {
      body = response.bytes().await?.to_vec();
    }

    return Ok(FetchOnceResult::Code(ResultPayload {
      body,
      content_type,
      etag,
      x_typescript_types,
    }));
  };

  fut.boxed()
}

/// Wraps reqwest `Response` so that it can be exposed as an `AsyncRead` and integrated
/// into resources more easily.
pub struct HttpBody {
  response: Response,
  chunk: Option<Bytes>,
  pos: usize,
}

impl HttpBody {
  pub fn from(body: Response) -> Self {
    Self {
      response: body,
      chunk: None,
      pos: 0,
    }
  }
}

impl AsyncRead for HttpBody {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, io::Error>> {
    let mut inner = self.get_mut();
    if let Some(chunk) = inner.chunk.take() {
      debug!(
        "HttpBody Fake Read buf {} chunk {} pos {}",
        buf.len(),
        chunk.len(),
        inner.pos
      );
      let n = min(buf.len(), chunk.len() - inner.pos);
      {
        let rest = &chunk[inner.pos..];
        buf[..n].clone_from_slice(&rest[..n]);
      }
      inner.pos += n;
      if inner.pos == chunk.len() {
        inner.pos = 0;
      } else {
        inner.chunk = Some(chunk);
      }
      return Poll::Ready(Ok(n));
    } else {
      assert_eq!(inner.pos, 0);
    }

    let chunk_future = &mut inner.response.chunk();
    // Safety: `chunk_future` lives only for duration of this poll. So, it doesn't move.
    let chunk_future = unsafe { Pin::new_unchecked(chunk_future) };
    match chunk_future.poll(cx) {
      Poll::Ready(Err(e)) => {
        Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
      }
      Poll::Ready(Ok(Some(chunk))) => {
        debug!(
          "HttpBody Real Read buf {} chunk {} pos {}",
          buf.len(),
          chunk.len(),
          inner.pos
        );
        let n = min(buf.len(), chunk.len());
        buf[..n].clone_from_slice(&chunk[..n]);
        if buf.len() < chunk.len() {
          inner.pos = n;
          inner.chunk = Some(chunk);
        }
        Poll::Ready(Ok(n))
      }
      Poll::Ready(Ok(None)) => Poll::Ready(Ok(0)),
      Poll::Pending => Poll::Pending,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_fetch_sync_string() {
    let http_server_guard = crate::test_util::http_server();
    // Relies on external http server. See tools/http_server.py
    let url =
      Url::parse("http://127.0.0.1:4545/cli/tests/fixture.json").unwrap();
    let client = create_http_client();
    let result = fetch_once(client, &url, None).await;
    if let Ok(FetchOnceResult::Code(payload)) = result {
      assert!(!payload.body.is_empty());
      assert_eq!(payload.content_type, Some("application/json".to_string()));
      assert_eq!(payload.etag, None);
      assert_eq!(payload.x_typescript_types, None);
    } else {
      panic!();
    }
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_fetch_gzip() {
    let http_server_guard = crate::test_util::http_server();
    // Relies on external http server. See tools/http_server.py
    let url = Url::parse(
      "http://127.0.0.1:4545/cli/tests/053_import_compression/gziped",
    )
    .unwrap();
    let client = create_http_client();
    let result = fetch_once(client, &url, None).await;
    if let Ok(FetchOnceResult::Code(payload)) = result {
      assert_eq!(
        String::from_utf8(payload.body).unwrap(),
        "console.log('gzip')"
      );
      assert_eq!(
        payload.content_type,
        Some("application/javascript".to_string())
      );
      assert_eq!(payload.etag, None);
      assert_eq!(payload.x_typescript_types, None);
    } else {
      panic!();
    }
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_fetch_with_etag() {
    let http_server_guard = crate::test_util::http_server();
    let url = Url::parse("http://127.0.0.1:4545/etag_script.ts").unwrap();
    let client = create_http_client();
    let result = fetch_once(client.clone(), &url, None).await;
    if let Ok(FetchOnceResult::Code(ResultPayload {
      body,
      content_type,
      etag,
      x_typescript_types,
    })) = result
    {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('etag')");
      assert_eq!(content_type, Some("application/typescript".to_string()));
      assert_eq!(etag, Some("33a64df551425fcc55e".to_string()));
      assert_eq!(x_typescript_types, None);
    } else {
      panic!();
    }

    let res =
      fetch_once(client, &url, Some("33a64df551425fcc55e".to_string())).await;
    assert_eq!(res.unwrap(), FetchOnceResult::NotModified);

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_fetch_brotli() {
    let http_server_guard = crate::test_util::http_server();
    // Relies on external http server. See tools/http_server.py
    let url = Url::parse(
      "http://127.0.0.1:4545/cli/tests/053_import_compression/brotli",
    )
    .unwrap();
    let client = create_http_client();
    let result = fetch_once(client, &url, None).await;
    if let Ok(FetchOnceResult::Code(payload)) = result {
      assert!(!payload.body.is_empty());
      assert_eq!(
        String::from_utf8(payload.body).unwrap(),
        "console.log('brotli');"
      );
      assert_eq!(
        payload.content_type,
        Some("application/javascript".to_string())
      );
      assert_eq!(payload.etag, None);
      assert_eq!(payload.x_typescript_types, None);
    } else {
      panic!();
    }
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_fetch_once_with_redirect() {
    let http_server_guard = crate::test_util::http_server();
    // Relies on external http server. See tools/http_server.py
    let url =
      Url::parse("http://127.0.0.1:4546/cli/tests/fixture.json").unwrap();
    // Dns resolver substitutes `127.0.0.1` with `localhost`
    let target_url =
      Url::parse("http://localhost:4545/cli/tests/fixture.json").unwrap();
    let client = create_http_client();
    let result = fetch_once(client, &url, None).await;
    if let Ok(FetchOnceResult::Redirect(url)) = result {
      assert_eq!(url, target_url);
    } else {
      panic!();
    }
    drop(http_server_guard);
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
