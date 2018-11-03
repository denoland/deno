// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use errors;
use errors::{DenoError, DenoResult};
use tokio_util;

use futures::future::{loop_fn, Loop};
use futures::{future, Future, Stream};
use hyper;
use hyper::client::{Client, HttpConnector};
use hyper::header::CONTENT_TYPE;
use hyper::Uri;
use hyper_rustls;

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

// The CodeFetch message is used to load HTTP javascript resources and expects a
// synchronous response, this utility method supports that.
pub fn fetch_sync_string(module_name: &str) -> DenoResult<(String, String)> {
  let url = module_name.parse::<Uri>().unwrap();
  let client = get_client();
  // TODO(kevinkassimo): consider set a max redirection counter
  // to avoid bouncing between 2 or more urls
  let fetch_future = loop_fn((client, Some(url)), |(client, maybe_url)| {
    let url = maybe_url.expect("target url should not be None");
    client
      .get(url)
      .map_err(|err| DenoError::from(err))
      .and_then(|response| {
        if response.status().is_redirection() {
          let new_url_string = response
            .headers()
            .get("location")
            .expect("url redirection should provide 'location' header")
            .to_str()
            .unwrap()
            .to_string();
          debug!("Redirecting to {}...", &new_url_string);
          let maybe_new_url = Some(
            new_url_string
              .parse::<Uri>()
              .expect("provided redirect url should be a valid url"),
          );
          return Ok(Loop::Continue((client, maybe_new_url)));
        }
        if !response.status().is_success() {
          return Err(errors::new(
            errors::ErrorKind::NotFound,
            "module not found".to_string(),
          ));
        }
        Ok(Loop::Break(response))
      })
  }).and_then(|response| {
    let content_type = response
      .headers()
      .get(CONTENT_TYPE)
      .map(|content_type| content_type.to_str().unwrap().to_string());
    let body = response
      .into_body()
      .concat2()
      .map(|body| String::from_utf8(body.to_vec()).unwrap())
      .map_err(|err| DenoError::from(err));
    body.join(future::ok(content_type))
  }).and_then(|(body_string, maybe_content_type)| {
    future::ok((body_string, maybe_content_type.unwrap()))
  });

  tokio_util::block_on(fetch_future)
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
