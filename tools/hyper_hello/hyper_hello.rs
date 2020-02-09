// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Adapted from https://github.com/hyperium/hyper/blob/master/examples/hello.rs

#![deny(warnings)]

use std::convert::Infallible;
use std::env;
use std::error::Error;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Response, Server};

type Just<T> = Result<T, Infallible>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let mut port: u16 = 4544;
  if let Some(custom_port) = env::args().nth(1) {
    port = custom_port.parse::<u16>().unwrap();
  }

  let addr = ([127, 0, 0, 1], port).into();

  // For every connection, we must make a `Service` to handle all
  // incoming HTTP requests on said connection.
  let new_service = make_service_fn(|_| {
    // This is the `Service` that will handle the connection.
    // `service_fn` is a helper to convert a function that
    // returns a Response into a `Service`.
    async {
      Just::Ok(service_fn(|_req| async {
        Just::Ok(Response::new(Body::from(&b"Hello World!"[..])))
      }))
    }
  });

  let server = Server::bind(&addr).tcp_nodelay(true).serve(new_service);
  println!("Listening on http://{}", addr);
  Ok(server.await?)
}
