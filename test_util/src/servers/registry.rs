// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::run_server;
use super::ServerKind;
use super::ServerOptions;
use bytes::Bytes;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::Empty;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;

pub async fn registry_server(port: u16) {
  let registry_server_addr = SocketAddr::from(([127, 0, 0, 1], port));

  run_server(
    ServerOptions {
      addr: registry_server_addr,
      error_msg: "Registry server error",
      kind: ServerKind::Auto,
    },
    registry_server_handler,
  )
  .await
}

async fn registry_server_handler(
  req: Request<Incoming>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  let path = req.uri().path();

  // TODO(bartlomieju): add a proper router here
  if path.starts_with("/api/scope/") {
    let body = serde_json::to_string_pretty(&json!({})).unwrap();
    let res = Response::new(UnsyncBoxBody::new(Full::from(body)));
    return Ok(res);
  } else if path.starts_with("/api/scopes/") {
    let body = serde_json::to_string_pretty(&json!({
      "id": "sdfwqer-sffg-qwerasdf",
      "status": "success",
      "error": null
    }))
    .unwrap();
    let res = Response::new(UnsyncBoxBody::new(Full::from(body)));
    return Ok(res);
  } else if path.starts_with("/api/publish_status/") {
    let body = serde_json::to_string_pretty(&json!({
      "id": "sdfwqer-qwer-qwerasdf",
      "status": "success",
      "error": null
    }))
    .unwrap();
    let res = Response::new(UnsyncBoxBody::new(Full::from(body)));
    return Ok(res);
  }

  let empty_body = UnsyncBoxBody::new(Empty::new());
  let res = Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(empty_body)?;
  Ok(res)
}
