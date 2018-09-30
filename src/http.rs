// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use errors::{DenoError, DenoResult};
use futures::stream::Concat2;
use futures::{Async, Future};
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

struct FetchedBodyFuture {
  body: Concat2<hyper::Body>,
  status: hyper::StatusCode,
}

struct FetchedBody {
  body: hyper::Chunk,
  status: hyper::StatusCode,
}

impl Future for FetchedBodyFuture {
  type Item = FetchedBody;
  type Error = hyper::Error;
  fn poll(&mut self) -> Result<Async<FetchedBody>, hyper::Error> {
    match self.body.poll()? {
      Async::Ready(body) => Ok(Async::Ready(FetchedBody {
        body,
        status: self.status.clone(),
      })),
      Async::NotReady => Ok(Async::NotReady),
    }
  }
}

// The CodeFetch message is used to load HTTP javascript resources and expects a
// synchronous response, this utility method supports that.
pub fn fetch_sync_string(module_name: &str) -> DenoResult<String> {
  let url = module_name.parse::<Uri>().unwrap();
  let client = get_client();
  let fetch_future = client.get(url).and_then(|response| {
    let status = response.status();
    FetchedBodyFuture {
      body: response.into_body().concat2(),
      status,
    }
  });

  let fetch_result = tokio_util::block_on(fetch_future)?;
  if !fetch_result.status.is_success() {
    return Err(DenoError::from(io::Error::new(
      io::ErrorKind::NotFound,
      format!("cannot load from '{}'", module_name),
    )));
  }
  Ok(String::from_utf8(fetch_result.body.to_vec()).unwrap())
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
