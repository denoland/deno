// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use crate::io::TcpStreamResource;
#[cfg(unix)]
use crate::io::UnixStreamResource;
use crate::ops::TcpListenerResource;
use crate::ops_tls::TlsListenerResource;
use crate::ops_tls::TlsStreamResource;
use crate::ops_tls::TLS_BUFFER_SIZE;
#[cfg(unix)]
use crate::ops_unix::UnixListenerResource;
use deno_core::error::bad_resource;
use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::ResourceId;
use deno_core::ResourceTable;
use deno_tls::rustls::ServerConfig;
use pin_project::pin_project;
use rustls_tokio_stream::TlsStream;
use std::rc::Rc;
use std::sync::Arc;
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;

/// A raw stream of one of the types handled by this extension.
#[pin_project(project = NetworkStreamProject)]
pub enum NetworkStream {
  Tcp(#[pin] TcpStream),
  Tls(#[pin] TlsStream),
  #[cfg(unix)]
  Unix(#[pin] UnixStream),
}

impl From<TcpStream> for NetworkStream {
  fn from(value: TcpStream) -> Self {
    NetworkStream::Tcp(value)
  }
}

impl From<TlsStream> for NetworkStream {
  fn from(value: TlsStream) -> Self {
    NetworkStream::Tls(value)
  }
}

#[cfg(unix)]
impl From<UnixStream> for NetworkStream {
  fn from(value: UnixStream) -> Self {
    NetworkStream::Unix(value)
  }
}

/// A raw stream of one of the types handled by this extension.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum NetworkStreamType {
  Tcp,
  Tls,
  #[cfg(unix)]
  Unix,
}

impl NetworkStream {
  pub fn local_address(&self) -> Result<NetworkStreamAddress, std::io::Error> {
    match self {
      Self::Tcp(tcp) => Ok(NetworkStreamAddress::Ip(tcp.local_addr()?)),
      Self::Tls(tls) => Ok(NetworkStreamAddress::Ip(tls.local_addr()?)),
      #[cfg(unix)]
      Self::Unix(unix) => Ok(NetworkStreamAddress::Unix(unix.local_addr()?)),
    }
  }

  pub fn peer_address(&self) -> Result<NetworkStreamAddress, std::io::Error> {
    match self {
      Self::Tcp(tcp) => Ok(NetworkStreamAddress::Ip(tcp.peer_addr()?)),
      Self::Tls(tls) => Ok(NetworkStreamAddress::Ip(tls.peer_addr()?)),
      #[cfg(unix)]
      Self::Unix(unix) => Ok(NetworkStreamAddress::Unix(unix.peer_addr()?)),
    }
  }

  pub fn stream(&self) -> NetworkStreamType {
    match self {
      Self::Tcp(_) => NetworkStreamType::Tcp,
      Self::Tls(_) => NetworkStreamType::Tls,
      #[cfg(unix)]
      Self::Unix(_) => NetworkStreamType::Unix,
    }
  }
}

impl tokio::io::AsyncRead for NetworkStream {
  fn poll_read(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    match self.project() {
      NetworkStreamProject::Tcp(s) => s.poll_read(cx, buf),
      NetworkStreamProject::Tls(s) => s.poll_read(cx, buf),
      #[cfg(unix)]
      NetworkStreamProject::Unix(s) => s.poll_read(cx, buf),
    }
  }
}

impl tokio::io::AsyncWrite for NetworkStream {
  fn poll_write(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<Result<usize, std::io::Error>> {
    match self.project() {
      NetworkStreamProject::Tcp(s) => s.poll_write(cx, buf),
      NetworkStreamProject::Tls(s) => s.poll_write(cx, buf),
      #[cfg(unix)]
      NetworkStreamProject::Unix(s) => s.poll_write(cx, buf),
    }
  }

  fn poll_flush(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    match self.project() {
      NetworkStreamProject::Tcp(s) => s.poll_flush(cx),
      NetworkStreamProject::Tls(s) => s.poll_flush(cx),
      #[cfg(unix)]
      NetworkStreamProject::Unix(s) => s.poll_flush(cx),
    }
  }

  fn poll_shutdown(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    match self.project() {
      NetworkStreamProject::Tcp(s) => s.poll_shutdown(cx),
      NetworkStreamProject::Tls(s) => s.poll_shutdown(cx),
      #[cfg(unix)]
      NetworkStreamProject::Unix(s) => s.poll_shutdown(cx),
    }
  }

  fn is_write_vectored(&self) -> bool {
    match self {
      Self::Tcp(s) => s.is_write_vectored(),
      Self::Tls(s) => s.is_write_vectored(),
      #[cfg(unix)]
      Self::Unix(s) => s.is_write_vectored(),
    }
  }

  fn poll_write_vectored(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    bufs: &[std::io::IoSlice<'_>],
  ) -> std::task::Poll<Result<usize, std::io::Error>> {
    match self.project() {
      NetworkStreamProject::Tcp(s) => s.poll_write_vectored(cx, bufs),
      NetworkStreamProject::Tls(s) => s.poll_write_vectored(cx, bufs),
      #[cfg(unix)]
      NetworkStreamProject::Unix(s) => s.poll_write_vectored(cx, bufs),
    }
  }
}

/// A raw stream listener of one of the types handled by this extension.
pub enum NetworkStreamListener {
  Tcp(tokio::net::TcpListener),
  Tls(tokio::net::TcpListener, Arc<ServerConfig>),
  #[cfg(unix)]
  Unix(tokio::net::UnixListener),
}

pub enum NetworkStreamAddress {
  Ip(std::net::SocketAddr),
  #[cfg(unix)]
  Unix(tokio::net::unix::SocketAddr),
}

impl NetworkStreamListener {
  /// Accepts a connection on this listener.
  pub async fn accept(&self) -> Result<NetworkStream, std::io::Error> {
    Ok(match self {
      Self::Tcp(tcp) => {
        let (stream, _addr) = tcp.accept().await?;
        NetworkStream::Tcp(stream)
      }
      Self::Tls(tcp, config) => {
        let (stream, _addr) = tcp.accept().await?;
        NetworkStream::Tls(TlsStream::new_server_side(
          stream,
          config.clone(),
          TLS_BUFFER_SIZE,
        ))
      }
      #[cfg(unix)]
      Self::Unix(unix) => {
        let (stream, _addr) = unix.accept().await?;
        NetworkStream::Unix(stream)
      }
    })
  }

  pub fn listen_address(&self) -> Result<NetworkStreamAddress, std::io::Error> {
    match self {
      Self::Tcp(tcp) => Ok(NetworkStreamAddress::Ip(tcp.local_addr()?)),
      Self::Tls(tcp, _) => Ok(NetworkStreamAddress::Ip(tcp.local_addr()?)),
      #[cfg(unix)]
      Self::Unix(unix) => Ok(NetworkStreamAddress::Unix(unix.local_addr()?)),
    }
  }

  pub fn stream(&self) -> NetworkStreamType {
    match self {
      Self::Tcp(..) => NetworkStreamType::Tcp,
      Self::Tls(..) => NetworkStreamType::Tls,
      #[cfg(unix)]
      Self::Unix(..) => NetworkStreamType::Unix,
    }
  }
}

/// In some cases it may be more efficient to extract the resource from the resource table and use it directly (for example, an HTTP server).
/// This method will extract a stream from the resource table and return it, unwrapped.
pub fn take_network_stream_resource(
  resource_table: &mut ResourceTable,
  stream_rid: ResourceId,
) -> Result<NetworkStream, AnyError> {
  // The stream we're attempting to unwrap may be in use somewhere else. If that's the case, we cannot proceed
  // with the process of unwrapping this connection, so we just return a bad resource error.
  // See also: https://github.com/denoland/deno/pull/16242

  if let Ok(resource_rc) = resource_table.take::<TcpStreamResource>(stream_rid)
  {
    // This TCP connection might be used somewhere else.
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("TCP stream is currently in use"))?;
    let (read_half, write_half) = resource.into_inner();
    let tcp_stream = read_half.reunite(write_half)?;
    return Ok(NetworkStream::Tcp(tcp_stream));
  }

  if let Ok(resource_rc) = resource_table.take::<TlsStreamResource>(stream_rid)
  {
    // This TLS connection might be used somewhere else.
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("TLS stream is currently in use"))?;
    let (read_half, write_half) = resource.into_inner();
    let tls_stream = read_half.unsplit(write_half);
    return Ok(NetworkStream::Tls(tls_stream));
  }

  #[cfg(unix)]
  if let Ok(resource_rc) = resource_table.take::<UnixStreamResource>(stream_rid)
  {
    // This UNIX socket might be used somewhere else.
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("UNIX stream is currently in use"))?;
    let (read_half, write_half) = resource.into_inner();
    let unix_stream = read_half.reunite(write_half)?;
    return Ok(NetworkStream::Unix(unix_stream));
  }

  Err(bad_resource_id())
}

/// In some cases it may be more efficient to extract the resource from the resource table and use it directly (for example, an HTTP server).
/// This method will extract a stream from the resource table and return it, unwrapped.
pub fn take_network_stream_listener_resource(
  resource_table: &mut ResourceTable,
  listener_rid: ResourceId,
) -> Result<NetworkStreamListener, AnyError> {
  if let Ok(resource_rc) =
    resource_table.take::<TcpListenerResource>(listener_rid)
  {
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("TCP socket listener is currently in use"))?;
    return Ok(NetworkStreamListener::Tcp(resource.listener.into_inner()));
  }

  if let Ok(resource_rc) =
    resource_table.take::<TlsListenerResource>(listener_rid)
  {
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("TLS socket listener is currently in use"))?;
    return Ok(NetworkStreamListener::Tls(
      resource.tcp_listener.into_inner(),
      resource.tls_config,
    ));
  }

  #[cfg(unix)]
  if let Ok(resource_rc) =
    resource_table.take::<UnixListenerResource>(listener_rid)
  {
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| bad_resource("UNIX socket listener is currently in use"))?;
    return Ok(NetworkStreamListener::Unix(resource.listener.into_inner()));
  }

  Err(bad_resource_id())
}
