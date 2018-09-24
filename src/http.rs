// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use errors::DenoResult;

use futures::Future;
use futures::Stream;
use hyper;
use hyper::client::HttpConnector;
use hyper::Uri;
use hyper_rustls;
use std::sync::Mutex;
use tokio::runtime::current_thread::Runtime;

type HttpsConnector = hyper_rustls::HttpsConnector<HttpConnector>;

lazy_static! {
  static ref connector_mutex: Mutex<HttpsConnector> = {
    let num_dns_threads = 4;
    Mutex::new(hyper_rustls::HttpsConnector::new(num_dns_threads))
  };
}

pub fn get_client() -> hyper::Client<HttpsConnector, hyper::Body> {
  // TODO use Hyper's connection pool.
  //let connector = connector_mutex.lock().unwrap();
  let connector: HttpsConnector = hyper_rustls::HttpsConnector::new(4);
  let client = hyper::Client::builder().build(connector);
  return client;
}

// The CodeFetch message is used to load HTTP javascript resources and expects a
// synchronous response, this utility method supports that.
pub fn fetch_sync_string(module_name: &str) -> DenoResult<String> {
  let url = module_name.parse::<Uri>().unwrap();
  let client = get_client();

  // TODO Use Deno's RT
  let mut rt = Runtime::new().unwrap();
  let body = rt.block_on(
    client
      .get(url)
      .and_then(|response| response.into_body().concat2()),
  )?;
  Ok(String::from_utf8(body.to_vec()).unwrap())
}

#[test]
fn test_fetch_sync_string() {
  // Relies on external http server. See tools/http_server.py
  let p = fetch_sync_string("http://localhost:4545/package.json").unwrap();
  println!("package.json len {}", p.len());
  assert!(p.len() > 1);
}
