// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::error::bad_resource;
use deno_core::error::bad_resource_id;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::ResourceId;
use deno_core::ToJsBuffer;
use deno_http::http_create_conn_resource;
use deno_http::HttpRequestReader;
use deno_http::HttpStreamResource;
use deno_net::io::TcpStreamResource;
use deno_net::ops_tls::TlsStream;
use deno_net::ops_tls::TlsStreamResource;
use hyper_v014::upgrade::Parts;
use serde::Serialize;
use tokio::net::TcpStream;

#[cfg(unix)]
use deno_net::io::UnixStreamResource;
#[cfg(unix)]
use tokio::net::UnixStream;

pub const UNSTABLE_FEATURE_NAME: &str = "http";

deno_core::extension!(
  deno_http_runtime,
  ops = [op_http_start, op_http_upgrade],
);

#[op2(fast)]
#[smi]
fn op_http_start(
  state: &mut OpState,
  #[smi] tcp_stream_rid: ResourceId,
) -> Result<ResourceId, AnyError> {
  if let Ok(resource_rc) = state
    .resource_table
    .take::<TcpStreamResource>(tcp_stream_rid)
  {
    // This TCP connection might be used somewhere else. If it's the case, we cannot proceed with the
    // process of starting a HTTP server on top of this TCP connection, so we just return a bad
    // resource error. See also: https://github.com/denoland/deno/pull/16242
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("TCP stream is currently in use"))?;
    let (read_half, write_half) = resource.into_inner();
    let tcp_stream = read_half.reunite(write_half)?;
    let addr = tcp_stream.local_addr()?;
    return http_create_conn_resource(state, tcp_stream, addr, "http");
  }

  if let Ok(resource_rc) = state
    .resource_table
    .take::<TlsStreamResource>(tcp_stream_rid)
  {
    // This TLS connection might be used somewhere else. If it's the case, we cannot proceed with the
    // process of starting a HTTP server on top of this TLS connection, so we just return a bad
    // resource error. See also: https://github.com/denoland/deno/pull/16242
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("TLS stream is currently in use"))?;
    let (read_half, write_half) = resource.into_inner();
    let tls_stream = read_half.unsplit(write_half);
    let addr = tls_stream.local_addr()?;
    return http_create_conn_resource(state, tls_stream, addr, "https");
  }

  #[cfg(unix)]
  if let Ok(resource_rc) = state
    .resource_table
    .take::<deno_net::io::UnixStreamResource>(tcp_stream_rid)
  {
    super::check_unstable(state, UNSTABLE_FEATURE_NAME, "Deno.serveHttp");

    // This UNIX socket might be used somewhere else. If it's the case, we cannot proceed with the
    // process of starting a HTTP server on top of this UNIX socket, so we just return a bad
    // resource error. See also: https://github.com/denoland/deno/pull/16242
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("UNIX stream is currently in use"))?;
    let (read_half, write_half) = resource.into_inner();
    let unix_stream = read_half.reunite(write_half)?;
    let addr = unix_stream.local_addr()?;
    return http_create_conn_resource(state, unix_stream, addr, "http+unix");
  }

  Err(bad_resource_id())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpUpgradeResult {
  conn_rid: ResourceId,
  conn_type: &'static str,
  read_buf: ToJsBuffer,
}

#[op2(async)]
#[serde]
async fn op_http_upgrade(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<HttpUpgradeResult, AnyError> {
  let stream = state
    .borrow_mut()
    .resource_table
    .get::<HttpStreamResource>(rid)?;
  let mut rd = RcRef::map(&stream, |r| &r.rd).borrow_mut().await;

  let request = match &mut *rd {
    HttpRequestReader::Headers(request) => request,
    _ => {
      return Err(custom_error(
        "Http",
        "cannot upgrade because request body was used",
      ))
    }
  };

  let transport = hyper_v014::upgrade::on(request).await?;
  let transport = match transport.downcast::<TcpStream>() {
    Ok(Parts {
      io: tcp_stream,
      read_buf,
      ..
    }) => {
      return Ok(HttpUpgradeResult {
        conn_type: "tcp",
        conn_rid: state
          .borrow_mut()
          .resource_table
          .add(TcpStreamResource::new(tcp_stream.into_split())),
        read_buf: read_buf.to_vec().into(),
      });
    }
    Err(transport) => transport,
  };
  #[cfg(unix)]
  let transport = match transport.downcast::<UnixStream>() {
    Ok(Parts {
      io: unix_stream,
      read_buf,
      ..
    }) => {
      return Ok(HttpUpgradeResult {
        conn_type: "unix",
        conn_rid: state
          .borrow_mut()
          .resource_table
          .add(UnixStreamResource::new(unix_stream.into_split())),
        read_buf: read_buf.to_vec().into(),
      });
    }
    Err(transport) => transport,
  };
  match transport.downcast::<TlsStream>() {
    Ok(Parts {
      io: tls_stream,
      read_buf,
      ..
    }) => Ok(HttpUpgradeResult {
      conn_type: "tls",
      conn_rid: state
        .borrow_mut()
        .resource_table
        .add(TlsStreamResource::new(tls_stream.into_split())),
      read_buf: read_buf.to_vec().into(),
    }),
    Err(_) => Err(custom_error(
      "Http",
      "encountered unsupported transport while upgrading",
    )),
  }
}
