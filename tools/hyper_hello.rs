// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Adapted from https://github.com/hyperium/hyper/blob/master/examples/hello.rs

#![deny(warnings)]
extern crate hyper;

use std::env;
use hyper::{Body, Response, Server};
use hyper::service::service_fn_ok;
use hyper::rt::{self, Future};

static PHRASE: &'static [u8] = b"Hello World!";

fn main() {
  let mut port: u16 = 4544;
  if let Some(custom_port) = env::args().nth(1) {
    port = custom_port.parse::<u16>().unwrap();
  }

  let addr = ([127, 0, 0, 1], port).into();

  // new_service is run for each connection, creating a 'service'
  // to handle requests for that specific connection.
  let new_service = || {
      // This is the `Service` that will handle the connection.
      // `service_fn_ok` is a helper to convert a function that
      // returns a Response into a `Service`.
      service_fn_ok(|_| {
          Response::new(Body::from(PHRASE))
      })
  };

  let server = Server::bind(&addr)
      .serve(new_service)
      .map_err(|e| eprintln!("server error: {}", e));

  println!("Listening on http://{}", addr);

  rt::run(server);
}
