// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::resolve_addr::resolve_addr;
use crate::resources;
use crate::resources::Resource;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use futures::Future;
use std;
use std::convert::From;
use std::net::Shutdown;
use std::sync::Arc;
use tokio;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_rustls::{rustls::ClientConfig, TlsConnector};
use webpki;
use webpki::DNSNameRef;
use webpki_roots;

#[derive(Deserialize)]
struct AcceptArgs {
  rid: i32,
}

pub fn op_accept(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: AcceptArgs = serde_json::from_value(args)?;
  let server_rid = args.rid as u32;

  let server_resource = resources::lookup(server_rid)?;
  let op = tokio_util::accept(server_resource)
    .and_then(move |(tcp_stream, _socket_addr)| {
      let local_addr = tcp_stream.local_addr()?;
      let remote_addr = tcp_stream.peer_addr()?;
      let tcp_stream_resource = resources::add_tcp_stream(tcp_stream);
      Ok((tcp_stream_resource, local_addr, remote_addr))
    })
    .map_err(ErrBox::from)
    .and_then(move |(tcp_stream_resource, local_addr, remote_addr)| {
      futures::future::ok(json!({
        "rid": tcp_stream_resource.rid,
        "localAddr": local_addr.to_string(),
        "remoteAddr": remote_addr.to_string(),
      }))
    });

  Ok(JsonOp::Async(Box::new(op)))
}

#[derive(Deserialize)]
struct DialArgs {
  transport: String,
  hostname: String,
  port: u16,
}

pub fn op_dial(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: DialArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp"); // TODO Support others.

  // TODO(ry) Using format! is suboptimal here. Better would be if
  // state.check_net and resolve_addr() took hostname and port directly.
  let address = format!("{}:{}", args.hostname, args.port);

  state.check_net(&address)?;

  let op = resolve_addr(&address).and_then(move |addr| {
    TcpStream::connect(&addr)
      .map_err(ErrBox::from)
      .and_then(move |tcp_stream| {
        let local_addr = tcp_stream.local_addr()?;
        let remote_addr = tcp_stream.peer_addr()?;
        let tcp_stream_resource = resources::add_tcp_stream(tcp_stream);
        Ok((tcp_stream_resource, local_addr, remote_addr))
      })
      .map_err(ErrBox::from)
      .and_then(move |(tcp_stream_resource, local_addr, remote_addr)| {
        futures::future::ok(json!({
          "rid": tcp_stream_resource.rid,
          "localAddr": local_addr.to_string(),
          "remoteAddr": remote_addr.to_string(),
        }))
      })
  });

  Ok(JsonOp::Async(Box::new(op)))
}

pub fn op_dial_tls(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: DialArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp"); // TODO Support others.

  // TODO(ry) Using format! is suboptimal here. Better would be if
  // state.check_net and resolve_addr() took hostname and port directly.
  let address = format!("{}:{}", args.hostname, args.port);

  state.check_net(&address)?;

  let mut domain = args.hostname;
  if domain.is_empty() {
    domain.push_str("localhost");
  }

  let op = resolve_addr(&address).and_then(move |addr| {
    TcpStream::connect(&addr)
      .and_then(move |tcp_stream| {
        let local_addr = tcp_stream.local_addr()?;
        let remote_addr = tcp_stream.peer_addr()?;
        let mut config = ClientConfig::new();
        config
          .root_store
          .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

        let tls_connector = TlsConnector::from(Arc::new(config));
        Ok((tls_connector, tcp_stream, local_addr, remote_addr))
      })
      .map_err(ErrBox::from)
      .and_then(
        move |(tls_connector, tcp_stream, local_addr, remote_addr)| {
          let dnsname = DNSNameRef::try_from_ascii_str(&domain)
            .expect("Invalid DNS lookup");
          tls_connector
            .connect(dnsname, tcp_stream)
            .map_err(ErrBox::from)
            .and_then(move |tls_stream| {
              let tls_stream_resource = resources::add_tls_stream(tls_stream);
              futures::future::ok(json!({
                "rid": tls_stream_resource.rid,
                "localAddr": local_addr.to_string(),
                "remoteAddr": remote_addr.to_string(),
              }))
            })
        },
      )
  });

  Ok(JsonOp::Async(Box::new(op)))
}

#[derive(Deserialize)]
struct ShutdownArgs {
  rid: i32,
  how: i32,
}

pub fn op_shutdown(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ShutdownArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let how = args.how;
  let mut resource = resources::lookup(rid)?;

  let shutdown_mode = match how {
    0 => Shutdown::Read,
    1 => Shutdown::Write,
    _ => unimplemented!(),
  };

  // Use UFCS for disambiguation
  Resource::shutdown(&mut resource, shutdown_mode)?;
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct ListenArgs {
  transport: String,
  hostname: String,
  port: u16,
}

pub fn op_listen(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ListenArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp");

  // TODO(ry) Using format! is suboptimal here. Better would be if
  // state.check_net and resolve_addr() took hostname and port directly.
  let address = format!("{}:{}", args.hostname, args.port);

  state.check_net(&address)?;

  let addr = resolve_addr(&address).wait()?;
  let listener = TcpListener::bind(&addr)?;
  let local_addr = listener.local_addr()?;
  let resource = resources::add_tcp_listener(listener);

  Ok(JsonOp::Sync(json!({
    "rid": resource.rid,
    "localAddr": local_addr.to_string()
  })))
}
