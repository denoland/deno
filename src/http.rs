// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use errors;
use errors::{DenoError, DenoResult};
use tokio_util;

use futures;
use futures::future::Either;
use futures::{Future, Stream};
use hyper;
use hyper::client::Client;
use hyper::client::HttpConnector;
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
pub fn fetch_sync_string(module_name: &str) -> DenoResult<String> {
  let url = module_name.parse::<Uri>().unwrap();
  let client = get_client();
  let fetch_future = client
    .get(url)
    .map_err(|err| DenoError::from(err))
    .and_then(|response| {
      if !response.status().is_success() {
        return Either::A(futures::future::err(errors::new(
          errors::ErrorKind::NotFound,
          "module not found".to_string(),
        )));
      }
      Either::B(
        response
          .into_body()
          .concat2()
          .map(|body| String::from_utf8(body.to_vec()).unwrap())
          .map_err(|err| DenoError::from(err)),
      )
    });
  tokio_util::block_on(fetch_future)
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
