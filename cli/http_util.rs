// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::deno_error::DenoError;
use crate::version;
use bytes::Bytes;
use deno_core::ErrBox;
use futures::future::FutureExt;
use reqwest;
use reqwest::header::HeaderMap;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::LOCATION;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use reqwest::Client;
use reqwest::Response;
use std::cmp::min;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;
use url::Url;

lazy_static! {
  static ref HTTP_CLIENT: Client = {
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
  };
}

/// Get instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn get_client() -> &'static Client {
  &HTTP_CLIENT
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
pub enum FetchOnceResult {
  // (code, maybe_content_type)
  Code(String, Option<String>),
  Redirect(Url),
}

/// Asynchronously fetches the given HTTP URL one pass only.
/// If no redirect is present and no error occurs,
/// yields Code(code, maybe_content_type).
/// If redirect occurs, does not follow and
/// yields Redirect(url).
pub fn fetch_string_once(
  url: &Url,
) -> impl Future<Output = Result<FetchOnceResult, ErrBox>> {
  let url = url.clone();
  let client = get_client();

  let fut = async move {
    let response = client.get(url.clone()).send().await?;

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

    let body = response.text().await?;
    return Ok(FetchOnceResult::Code(body, content_type));
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
  use crate::tokio_util;

  #[test]
  fn test_fetch_sync_string() {
    let http_server_guard = crate::test_util::http_server();
    // Relies on external http server. See tools/http_server.py
    let url =
      Url::parse("http://127.0.0.1:4545/cli/tests/fixture.json").unwrap();

    let fut = fetch_string_once(&url).map(|result| match result {
      Ok(FetchOnceResult::Code(code, maybe_content_type)) => {
        assert!(!code.is_empty());
        assert_eq!(maybe_content_type, Some("application/json".to_string()));
      }
      _ => panic!(),
    });

    tokio_util::run(fut);
    drop(http_server_guard);
  }

  #[test]
  fn test_fetch_string_once_with_redirect() {
    let http_server_guard = crate::test_util::http_server();
    // Relies on external http server. See tools/http_server.py
    let url =
      Url::parse("http://127.0.0.1:4546/cli/tests/fixture.json").unwrap();
    // Dns resolver substitutes `127.0.0.1` with `localhost`
    let target_url =
      Url::parse("http://localhost:4545/cli/tests/fixture.json").unwrap();
    let fut = fetch_string_once(&url).map(move |result| match result {
      Ok(FetchOnceResult::Redirect(url)) => {
        assert_eq!(url, target_url);
      }
      _ => panic!(),
    });

    tokio_util::run(fut);
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
