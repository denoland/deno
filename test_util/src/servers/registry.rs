// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use futures::Future;
use futures::FutureExt;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::Empty;
use http_body_util::Full;
use hyper1::body::Incoming;
use hyper1::service::service_fn;
use hyper1::Request;
use hyper1::Response;
use hyper1::StatusCode;
use hyper_util::rt::TokioIo;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use tokio::net::TcpListener;

async fn run_server<F, S>(
  addr: SocketAddr,
  service_fn_handler: F,
  error_msg: &'static str,
) where
  F: Fn(Request<Incoming>) -> S + Copy + 'static,
  S: Future<
    Output = Result<
      Response<UnsyncBoxBody<Bytes, Infallible>>,
      hyper1::http::Error,
    >,
  >,
{
  let fut: Pin<Box<dyn Future<Output = Result<(), anyhow::Error>>>> =
    async move {
      let listener = TcpListener::bind(addr).await?;
      loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let service = service_fn(service_fn_handler);
        deno_unsync::spawn(async move {
          if let Err(e) = hyper1::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
          {
            eprintln!("{}: {:?}", error_msg, e);
          }
        });
      }
    }
    .boxed_local();

  if let Err(e) = fut.await {
    eprintln!("{}: {:?}", error_msg, e);
  }
}

pub async fn registry_server(port: u16) {
  let registry_server_addr = SocketAddr::from(([127, 0, 0, 1], port));

  run_server(
    registry_server_addr,
    registry_server_handler,
    "Registry server error",
  )
  .await
}

async fn registry_server_handler(
  req: Request<Incoming>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, hyper1::http::Error> {
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
  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(empty_body)
}
