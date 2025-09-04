// Copyright 2018-2025 the Deno authors. MIT license.

use std::rc::Rc;

use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::error::ResourceError;
use deno_core::op2;
use deno_http::http_create_conn_resource;
use deno_net::io::TcpStreamResource;
use deno_net::ops_tls::TlsStreamResource;

pub const UNSTABLE_FEATURE_NAME: &str = "http";

deno_core::extension!(deno_http_runtime, ops = [op_http_start],);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HttpStartError {
  #[class("Busy")]
  #[error("TCP stream is currently in use")]
  TcpStreamInUse,
  #[class("Busy")]
  #[error("TLS stream is currently in use")]
  TlsStreamInUse,
  #[class("Busy")]
  #[error("Unix socket is currently in use")]
  UnixSocketInUse,
  #[class(generic)]
  #[error(transparent)]
  ReuniteTcp(#[from] tokio::net::tcp::ReuniteError),
  #[cfg(unix)]
  #[class(generic)]
  #[error(transparent)]
  ReuniteUnix(#[from] tokio::net::unix::ReuniteError),
  #[class(inherit)]
  #[error("{0}")]
  Io(
    #[from]
    #[inherit]
    std::io::Error,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[inherit] ResourceError),
}

#[op2(fast)]
#[smi]
fn op_http_start(
  state: &mut OpState,
  #[smi] tcp_stream_rid: ResourceId,
) -> Result<ResourceId, HttpStartError> {
  if let Ok(resource_rc) = state
    .resource_table
    .take::<TcpStreamResource>(tcp_stream_rid)
  {
    // This TCP connection might be used somewhere else. If it's the case, we cannot proceed with the
    // process of starting a HTTP server on top of this TCP connection, so we just return a Busy error.
    // See also: https://github.com/denoland/deno/pull/16242
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| HttpStartError::TcpStreamInUse)?;
    let (read_half, write_half) = resource.into_inner();
    let tcp_stream = read_half.reunite(write_half)?;
    let addr = tcp_stream.local_addr()?;
    return Ok(http_create_conn_resource(state, tcp_stream, addr, "http"));
  }

  if let Ok(resource_rc) = state
    .resource_table
    .take::<TlsStreamResource>(tcp_stream_rid)
  {
    // This TLS connection might be used somewhere else. If it's the case, we cannot proceed with the
    // process of starting a HTTP server on top of this TLS connection, so we just return a Busy error.
    // See also: https://github.com/denoland/deno/pull/16242
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| HttpStartError::TlsStreamInUse)?;
    let tls_stream = resource.into_tls_stream();
    let addr = tls_stream.local_addr()?;
    return Ok(http_create_conn_resource(state, tls_stream, addr, "https"));
  }

  #[cfg(unix)]
  if let Ok(resource_rc) = state
    .resource_table
    .take::<deno_net::io::UnixStreamResource>(tcp_stream_rid)
  {
    // This UNIX socket might be used somewhere else. If it's the case, we cannot proceed with the
    // process of starting a HTTP server on top of this UNIX socket, so we just return a Busy error.
    // See also: https://github.com/denoland/deno/pull/16242
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| HttpStartError::UnixSocketInUse)?;
    let (read_half, write_half) = resource.into_inner();
    let unix_stream = read_half.reunite(write_half)?;
    let addr = unix_stream.local_addr()?;
    return Ok(http_create_conn_resource(
      state,
      unix_stream,
      addr,
      "http+unix",
    ));
  }

  Err(HttpStartError::Resource(ResourceError::BadResourceId))
}
