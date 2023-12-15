// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use hyper::server::Server;
use hyper::service::make_service_fn;
use hyper::service::service_fn;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;

pub async fn registry_server(port: u16) {
  let registry_server_addr = SocketAddr::from(([127, 0, 0, 1], port));
  let registry_server_svc = make_service_fn(|_| async {
    Ok::<_, Infallible>(service_fn(registry_server_handler))
  });
  let registry_server =
    Server::bind(&registry_server_addr).serve(registry_server_svc);
  if let Err(e) = registry_server.await {
    eprintln!("Registry server error: {:?}", e);
  }
}

async fn registry_server_handler(
  req: Request<Body>,
) -> Result<Response<Body>, hyper::http::Error> {
  let path = req.uri().path();

  if path.starts_with("/scopes/") {
    let body = serde_json::to_string_pretty(&json!({
      "id": "sdfwqer-sffg-qwerasdf",
      "status": "success",
      "error": null
    }))
    .unwrap();
    let res = Response::new(Body::from(body));
    return Ok(res);
  } else if path.starts_with("/publish_status/") {
    let body = serde_json::to_string_pretty(&json!({
      "id": "sdfwqer-qwer-qwerasdf",
      "status": "success",
      "error": null
    }))
    .unwrap();
    let res = Response::new(Body::from(body));
    return Ok(res);
  }

  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(Body::empty())
}
