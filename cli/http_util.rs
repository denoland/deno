// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::deno_error::DenoError;
use deno::ErrBox;
use futures::compat::Future01CompatExt;
use futures::compat::Stream01CompatExt;
use futures::future;
use futures::future::TryFutureExt;
use futures::stream::TryStreamExt;
use hyper;
use hyper::client::{Client, HttpConnector};
use hyper::header::CONTENT_TYPE;
use hyper::Uri;
use hyper_rustls;
use std::future::Future;

type Connector = hyper_rustls::HttpsConnector<HttpConnector>;

lazy_static! {
  static ref CONNECTOR: Connector = {
    let num_dns_threads = 4;
    Connector::new(num_dns_threads)
  };
}

pub fn get_client() -> Client<Connector, hyper::Body> {
  // TODO use Hyper's connection pool.
  let c = CONNECTOR.clone();
  Client::builder().build(c)
}

/// Construct the next uri based on base uri and location header fragment
/// See <https://tools.ietf.org/html/rfc3986#section-4.2>
fn resolve_uri_from_location(base_uri: &Uri, location: &str) -> Uri {
  if location.starts_with("http://") || location.starts_with("https://") {
    // absolute uri
    location
      .parse::<Uri>()
      .expect("provided redirect url should be a valid url")
  } else if location.starts_with("//") {
    // "//" authority path-abempty
    format!("{}:{}", base_uri.scheme_part().unwrap().as_str(), location)
      .parse::<Uri>()
      .expect("provided redirect url should be a valid url")
  } else if location.starts_with('/') {
    // path-absolute
    let mut new_uri_parts = base_uri.clone().into_parts();
    new_uri_parts.path_and_query = Some(location.parse().unwrap());
    Uri::from_parts(new_uri_parts).unwrap()
  } else {
    // assuming path-noscheme | path-empty
    let mut new_uri_parts = base_uri.clone().into_parts();
    let base_uri_path_str = base_uri.path().to_owned();
    let segs: Vec<&str> = base_uri_path_str.rsplitn(2, '/').collect();
    new_uri_parts.path_and_query = Some(
      format!("{}/{}", segs.last().unwrap_or(&""), location)
        .parse()
        .unwrap(),
    );
    Uri::from_parts(new_uri_parts).unwrap()
  }
}

#[cfg(test)]
use crate::tokio_util;
#[cfg(test)]
use futures::future::FutureExt;
#[cfg(test)]
use hyper::Body;
#[cfg(test)]
use hyper::Response;
#[cfg(test)]
use std::pin::Pin;
#[cfg(test)]
use std::task::Context;
#[cfg(test)]
use std::task::Poll;

#[cfg(test)]
/// Synchronously fetchs the given HTTP URL. Returns (content, media_type).
pub fn fetch_sync_string(
  module_name: &str,
) -> Result<(String, String), ErrBox> {
  tokio_util::block_on(fetch_string(module_name))
}

pub enum FetchOnceResult {
  // (code, maybe_content_type)
  Code(String, Option<String>),
  Redirect(http::uri::Uri),
}

/// Asynchronously fetchs the given HTTP URL one pass only.
/// If no redirect is present and no error occurs,
/// yields Code(code, maybe_content_type).
/// If redirect occurs, does not follow and
/// yields Redirect(url).
pub fn fetch_string_once(
  url: http::uri::Uri,
) -> impl Future<Output = Result<FetchOnceResult, ErrBox>> {
  let client = get_client();
  client
    .get(url.clone())
    .compat()
    .map_err(ErrBox::from)
    .and_then(move |response| {
      if response.status().is_redirection() {
        let location_string = response
          .headers()
          .get("location")
          .expect("url redirection should provide 'location' header")
          .to_str()
          .unwrap()
          .to_string();
        debug!("Redirecting to {}...", &location_string);
        let new_url = resolve_uri_from_location(&url, &location_string);
        // Boxed trait object turns out to be the savior for 2+ types yielding same results.
        return future::try_join3(
          future::Either::Left(future::ok(None)),
          future::ok(None),
          future::ok(Some(FetchOnceResult::Redirect(new_url))),
        );
      } else if response.status().is_client_error()
        || response.status().is_server_error()
      {
        return future::try_join3(
          future::Either::Left(future::err(
            DenoError::new(
              deno_error::ErrorKind::Other,
              format!("Import '{}' failed: {}", &url, response.status()),
            )
            .into(),
          )),
          future::ok(None),
          future::ok(None),
        );
      }
      let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .map(|content_type| content_type.to_str().unwrap().to_owned());
      let body = response
        .into_body()
        .compat()
        .try_concat()
        .map_ok(|body| String::from_utf8(body.to_vec()).ok())
        .map_err(ErrBox::from);
      future::try_join3(
        future::Either::Right(body),
        future::ok(content_type),
        future::ok(None),
      )
    })
    .and_then(move |(maybe_code, maybe_content_type, maybe_redirect)| {
      if let Some(redirect) = maybe_redirect {
        future::ok(redirect)
      } else {
        // maybe_code should always contain code here!
        future::ok(FetchOnceResult::Code(
          maybe_code.unwrap(),
          maybe_content_type,
        ))
      }
    })
}

#[cfg(test)]
type FetchStringGetResult = Result<Response<Body>, ErrBox>;
#[cfg(test)]
type FetchStringGetFuture =
  dyn Future<Output = FetchStringGetResult> + Send + 'static;

#[cfg(test)]
struct FetchStringFuture {
  pub client: Client<Connector, hyper::Body>,
  pub url: Uri,
  pub maybe_get_future: Option<Pin<Box<FetchStringGetFuture>>>,
}

#[cfg(test)]
impl Future for FetchStringFuture {
  type Output = FetchStringGetResult;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = Pin::get_mut(self);

    if inner.maybe_get_future.is_none() {
      inner.maybe_get_future = Some(
        inner
          .client
          .get(inner.url.clone())
          .compat()
          .map_err(ErrBox::from)
          .boxed(),
      );
    }

    let mut get_future = inner.maybe_get_future.take().unwrap();

    match get_future.poll_unpin(cx) {
      Poll::Ready(Ok(response)) => {
        if response.status().is_redirection() {
          let location_string = response
            .headers()
            .get("location")
            .expect("url redirection should provide 'location' header")
            .to_str()
            .unwrap()
            .to_string();
          debug!("Redirecting to {}...", &location_string);
          let new_url = resolve_uri_from_location(&inner.url, &location_string);
          inner.url = new_url;
          return Poll::Pending;
        }
        if !response.status().is_success() {
          return Poll::Ready(Err(
            DenoError::new(
              deno_error::ErrorKind::NotFound,
              "module not found".to_string(),
            )
            .into(),
          ));
        }
        Poll::Ready(Ok(response))
      }
      Poll::Ready(Err(err)) => Poll::Ready(Err(err.into())),
      Poll::Pending => {
        inner.maybe_get_future = Some(get_future);

        Poll::Pending
      }
    }
  }
}

#[cfg(test)]
/// Asynchronously fetchs the given HTTP URL. Returns (content, media_type).
pub fn fetch_string(
  module_name: &str,
) -> impl Future<Output = Result<(String, String), ErrBox>> {
  let url = module_name.parse::<Uri>().unwrap();
  let client = get_client();

  let future = FetchStringFuture {
    client,
    url,
    maybe_get_future: None,
  };
  // TODO(kevinkassimo): consider set a max redirection counter
  // to avoid bouncing between 2 or more urls
  future
    .and_then(|response| {
      let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .map(|content_type| content_type.to_str().unwrap().to_string());
      let body = response
        .into_body()
        .compat()
        .try_concat()
        .map_ok(|body| String::from_utf8(body.to_vec()).unwrap())
        .map_err(ErrBox::from);
      future::try_join(body, future::ok(content_type))
    })
    .and_then(|(body_string, maybe_content_type)| {
      future::ok((body_string, maybe_content_type.unwrap()))
    })
}

#[test]
fn test_fetch_sync_string() {
  // Relies on external http server. See tools/http_server.py
  tokio_util::init(|| {
    let (p, m) =
      fetch_sync_string("http://127.0.0.1:4545/package.json").unwrap();
    println!("package.json len {}", p.len());
    assert!(p.len() > 1);
    assert!(m == "application/json")
  });
}

#[test]
fn test_fetch_string() {
  // Relies on external http server. See tools/http_server.py
  tokio_util::init(|| {
    let (p, m) = futures::executor::block_on(fetch_string(
      "http://127.0.0.1:4545/package.json",
    ))
    .unwrap();
    println!("package.json len {}", p.len());
    assert!(p.len() > 1);
    assert!(m == "application/json")
  });
}

#[test]
fn test_fetch_sync_string_with_redirect() {
  // Relies on external http server. See tools/http_server.py
  tokio_util::init(|| {
    let (p, m) =
      fetch_sync_string("http://127.0.0.1:4546/package.json").unwrap();
    println!("package.json len {}", p.len());
    assert!(p.len() > 1);
    assert!(m == "application/json")
  });
}

#[test]
fn test_resolve_uri_from_location_full_1() {
  let url = "http://deno.land".parse::<Uri>().unwrap();
  let new_uri = resolve_uri_from_location(&url, "http://golang.org");
  assert_eq!(new_uri.host().unwrap(), "golang.org");
}

#[test]
fn test_resolve_uri_from_location_full_2() {
  let url = "https://deno.land".parse::<Uri>().unwrap();
  let new_uri = resolve_uri_from_location(&url, "https://golang.org");
  assert_eq!(new_uri.host().unwrap(), "golang.org");
}

#[test]
fn test_resolve_uri_from_location_relative_1() {
  let url = "http://deno.land/x".parse::<Uri>().unwrap();
  let new_uri = resolve_uri_from_location(&url, "//rust-lang.org/en-US");
  assert_eq!(new_uri.host().unwrap(), "rust-lang.org");
  assert_eq!(new_uri.path(), "/en-US");
}

#[test]
fn test_resolve_uri_from_location_relative_2() {
  let url = "http://deno.land/x".parse::<Uri>().unwrap();
  let new_uri = resolve_uri_from_location(&url, "/y");
  assert_eq!(new_uri.host().unwrap(), "deno.land");
  assert_eq!(new_uri.path(), "/y");
}

#[test]
fn test_resolve_uri_from_location_relative_3() {
  let url = "http://deno.land/x".parse::<Uri>().unwrap();
  let new_uri = resolve_uri_from_location(&url, "z");
  assert_eq!(new_uri.host().unwrap(), "deno.land");
  assert_eq!(new_uri.path(), "/z");
}
