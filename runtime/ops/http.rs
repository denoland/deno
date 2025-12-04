// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::error::ResourceError;

deno_core::extension!(deno_http_runtime);

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
