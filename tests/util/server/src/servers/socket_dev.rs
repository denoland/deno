// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV6;
use std::path::PathBuf;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bytes::Bytes;
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use http::HeaderMap;
use http::HeaderValue;
use http_body_util::BodyExt;
use http_body_util::combinators::UnsyncBoxBody;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::body::Incoming;
use serde_json::json;
use sha2::Digest;

use super::ServerKind;
use super::ServerOptions;
use super::custom_headers;
use super::empty_body;
use super::hyper_utils::HandlerOutput;
use super::run_server;
use super::string_body;
use crate::npm;
use crate::root_path;

pub fn api(port: u16) -> Vec<LocalBoxFuture<'static, ()>> {
  run_socket_dev_server(port, "socket.dev server error", socket_dev_handler)
}

fn run_socket_dev_server<F, S>(
  port: u16,
  error_msg: &'static str,
  handler: F,
) -> Vec<LocalBoxFuture<'static, ()>>
where
  F: Fn(Request<hyper::body::Incoming>) -> S + Copy + 'static,
  S: Future<Output = HandlerOutput> + 'static,
{
  let npm_registry_addr = SocketAddr::from(([127, 0, 0, 1], port));
  let ipv6_loopback = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
  let npm_registry_ipv6_addr =
    SocketAddr::V6(SocketAddrV6::new(ipv6_loopback, port, 0, 0));
  vec![
    run_socket_dev_server_for_addr(npm_registry_addr, error_msg, handler)
      .boxed_local(),
  ]
}

async fn run_socket_dev_server_for_addr<F, S>(
  addr: SocketAddr,
  error_msg: &'static str,
  handler: F,
) where
  F: Fn(Request<hyper::body::Incoming>) -> S + Copy + 'static,
  S: Future<Output = HandlerOutput> + 'static,
{
  run_server(
    ServerOptions {
      addr,
      kind: ServerKind::Auto,
      error_msg,
    },
    handler,
  )
  .await
}

async fn socket_dev_handler(
  req: Request<hyper::body::Incoming>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  // { id: "81646", name: "weak-lru-cache", version: "1.2.2", score: Some(FirewallScore { license: 1.0, maintenance: 0.77, overall: 0.77, quality: 0.94, supply_chain: 1.0, vulnerability: 1.0 }), alerts: [] }
}
