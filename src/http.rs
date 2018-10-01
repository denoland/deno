// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use errors::{DenoError, DenoResult};
use futures;
use futures::future::Either;
use futures::Future;
use tokio_util;

use futures::Stream;
use hyper;
use hyper::client::Client;
use hyper::client::HttpConnector;
use hyper::Uri;
use hyper_rustls;
use std::io;

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

enum HyperOrIOError {
  IO(io::Error),
  Hyper(hyper::Error),
}

fn response_future(
  response: hyper::Response<hyper::Body>,
) -> impl Future<Item = String, Error = HyperOrIOError> {
  if !response.status().is_success() {
    return Either::A(futures::future::err(HyperOrIOError::IO(io::Error::new(
      io::ErrorKind::NotFound,
      format!("module not found"),
    ))));
  }
  Either::B(
    response
      .into_body()
      .concat2()
      .map(|body| String::from_utf8(body.to_vec()).unwrap())
      .map_err(|err| HyperOrIOError::Hyper(err)),
  )
}

// The CodeFetch message is used to load HTTP javascript resources and expects a
// synchronous response, this utility method supports that.
pub fn fetch_sync_string(module_name: &str) -> DenoResult<String> {
  let url = module_name.parse::<Uri>().unwrap();
  let client = get_client();
  let fetch_future = client
    .get(url)
    .map_err(|err| HyperOrIOError::Hyper(err))
    .and_then(response_future);
  match tokio_util::block_on(fetch_future) {
    Ok(s) => Ok(s),
    Err(HyperOrIOError::Hyper(err)) => Err(DenoError::from(err)),
    Err(HyperOrIOError::IO(err)) => Err(DenoError::from(err)),
  }
}

/* TODO(ry) Re-enabled this test. Disabling to work around bug in #782.

#[test]
fn test_fetch_sync_string() {
  // Relies on external http server. See tools/http_server.py
  use futures;

  tokio_util::init(|| {
    tokio_util::block_on(futures::future::lazy(|| -> DenoResult<()> {
      let p = fetch_sync_string("http://127.0.0.1:4545/package.json")?;
      println!("package.json len {}", p.len());
      assert!(p.len() > 1);
      Ok(())
    })).unwrap();
  });
}

*/
