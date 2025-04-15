// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::rc::Rc;

use deno_core::error::ResourceError;
use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ResourceTable;
use deno_error::JsErrorBox;

use crate::io::TcpStreamResource;
use crate::ops_tls::TlsStreamResource;

pub trait NetworkStreamTrait: Into<NetworkStream> {
  type Resource;
  const RESOURCE_NAME: &'static str;
  fn local_address(&self) -> Result<NetworkStreamAddress, std::io::Error>;
  fn peer_address(&self) -> Result<NetworkStreamAddress, std::io::Error>;
}

#[allow(async_fn_in_trait)]
pub trait NetworkStreamListenerTrait:
  Into<NetworkStreamListener> + Send + Sync
{
  type Stream: NetworkStreamTrait + 'static;
  type Addr: Into<NetworkStreamAddress> + 'static;
  /// Additional data, if needed
  type ResourceData: Default;
  const RESOURCE_NAME: &'static str;
  async fn accept(&self) -> std::io::Result<(Self::Stream, Self::Addr)>;
  fn listen_address(&self) -> Result<Self::Addr, std::io::Error>;
}

/// A strongly-typed network listener resource for something that
/// implements `NetworkListenerTrait`.
pub struct NetworkListenerResource<T: NetworkStreamListenerTrait> {
  pub listener: AsyncRefCell<T>,
  /// Associated data for this resource. Not required.
  #[allow(unused)]
  pub data: T::ResourceData,
  pub cancel: CancelHandle,
}

impl<T: NetworkStreamListenerTrait + 'static> Resource
  for NetworkListenerResource<T>
{
  fn name(&self) -> Cow<str> {
    T::RESOURCE_NAME.into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

impl<T: NetworkStreamListenerTrait + 'static> NetworkListenerResource<T> {
  pub fn new(t: T) -> Self {
    Self {
      listener: AsyncRefCell::new(t),
      data: Default::default(),
      cancel: Default::default(),
    }
  }

  /// Returns a [`NetworkStreamListener`] from this resource if it is not in use elsewhere.
  fn take(
    resource_table: &mut ResourceTable,
    listener_rid: ResourceId,
  ) -> Result<Option<NetworkStreamListener>, JsErrorBox> {
    if let Ok(resource_rc) = resource_table.take::<Self>(listener_rid) {
      let resource = Rc::try_unwrap(resource_rc)
        .map_err(|_| JsErrorBox::new("Busy", "Listener is currently in use"))?;
      return Ok(Some(resource.listener.into_inner().into()));
    }
    Ok(None)
  }
}

/// Each of the network streams has the exact same pattern for listening, accepting, etc, so
/// we just codegen them all via macro to avoid repeating each one of these N times.
macro_rules! network_stream {
  ( $([$i:ident, $il:ident, $stream:path, $listener:path, $addr:path, $stream_resource:ty]),* ) => {
    /// A raw stream of one of the types handled by this extension.
    #[pin_project::pin_project(project = NetworkStreamProject)]
    pub enum NetworkStream {
      $( $i (#[pin] $stream), )*
    }

    /// A raw stream of one of the types handled by this extension.
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub enum NetworkStreamType {
      $( $i, )*
    }

    /// A raw stream listener of one of the types handled by this extension.
    pub enum NetworkStreamListener {
      $( $i( $listener ), )*
    }

    $(
      impl NetworkStreamListenerTrait for $listener {
        type Stream = $stream;
        type Addr = $addr;
        type ResourceData = ();
        const RESOURCE_NAME: &'static str = concat!(stringify!($il), "Listener");
        async fn accept(&self) -> std::io::Result<(Self::Stream, Self::Addr)> {
          <$listener> :: accept(self).await
        }
        fn listen_address(&self) -> std::io::Result<Self::Addr> {
          self.local_addr()
        }
      }

      impl From<$listener> for NetworkStreamListener {
        fn from(value: $listener) -> Self {
          Self::$i(value)
        }
      }

      impl NetworkStreamTrait for $stream {
        type Resource = $stream_resource;
        const RESOURCE_NAME: &'static str = concat!(stringify!($il), "Stream");
        fn local_address(&self) -> Result<NetworkStreamAddress, std::io::Error> {
          Ok(NetworkStreamAddress::from(self.local_addr()?))
        }
        fn peer_address(&self) -> Result<NetworkStreamAddress, std::io::Error> {
          Ok(NetworkStreamAddress::from(self.peer_addr()?))
        }
      }

      impl From<$stream> for NetworkStream {
        fn from(value: $stream) -> Self {
          Self::$i(value)
        }
      }
    )*

    impl NetworkStream {
      pub fn local_address(&self) -> Result<NetworkStreamAddress, std::io::Error> {
        match self {
          $( Self::$i(stm) => Ok(NetworkStreamAddress::from(stm.local_addr()?)), )*
        }
      }

      pub fn peer_address(&self) -> Result<NetworkStreamAddress, std::io::Error> {
        match self {
          $( Self::$i(stm) => Ok(NetworkStreamAddress::from(stm.peer_addr()?)), )*
        }
      }

      pub fn stream(&self) -> NetworkStreamType {
        match self {
          $( Self::$i(_) => NetworkStreamType::$i, )*
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
          $( NetworkStreamProject::$i(s) => s.poll_read(cx, buf), )*
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
          $( NetworkStreamProject::$i(s) => s.poll_write(cx, buf), )*
        }
      }

      fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
      ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.project() {
          $( NetworkStreamProject::$i(s) => s.poll_flush(cx), )*
        }
      }

      fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
      ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.project() {
          $( NetworkStreamProject::$i(s) => s.poll_shutdown(cx), )*
        }
      }

      fn is_write_vectored(&self) -> bool {
        match self {
          $( NetworkStream::$i(s) => s.is_write_vectored(), )*
        }
      }

      fn poll_write_vectored(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
      ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.project() {
          $( NetworkStreamProject::$i(s) => s.poll_write_vectored(cx, bufs), )*
        }
      }
    }

    impl NetworkStreamListener {
      /// Accepts a connection on this listener.
      pub async fn accept(&self) -> Result<(NetworkStream, NetworkStreamAddress), std::io::Error> {
        Ok(match self {
          $(
            Self::$i(s) => {
              let (stm, addr) = s.accept().await?;
              (NetworkStream::$i(stm), addr.into())
            }
          )*
        })
      }

      pub fn listen_address(&self) -> Result<NetworkStreamAddress, std::io::Error> {
        match self {
          $( Self::$i(s) => { Ok(NetworkStreamAddress::from(s.listen_address()?)) } )*
        }
      }

      pub fn stream(&self) -> NetworkStreamType {
        match self {
          $( Self::$i(_) => { NetworkStreamType::$i } )*
        }
      }

      /// Return a `NetworkStreamListener` if a resource exists for this `ResourceId` and it is currently
      /// not locked.
      pub fn take_resource(resource_table: &mut ResourceTable, listener_rid: ResourceId) -> Result<NetworkStreamListener, JsErrorBox> {
        $(
          if let Some(resource) = NetworkListenerResource::<$listener>::take(resource_table, listener_rid)? {
            return Ok(resource)
          }
        )*
        Err(JsErrorBox::from_err(ResourceError::BadResourceId))
      }
    }
  };
}

#[cfg(unix)]
network_stream!(
  [
    Tcp,
    tcp,
    tokio::net::TcpStream,
    crate::tcp::TcpListener,
    std::net::SocketAddr,
    TcpStreamResource
  ],
  [
    Tls,
    tls,
    crate::ops_tls::TlsStream,
    crate::ops_tls::TlsListener,
    std::net::SocketAddr,
    TlsStreamResource
  ],
  [
    Unix,
    unix,
    tokio::net::UnixStream,
    tokio::net::UnixListener,
    tokio::net::unix::SocketAddr,
    crate::io::UnixStreamResource
  ],
  [
    Vsock,
    vsock,
    tokio_vsock::VsockStream,
    tokio_vsock::VsockListener,
    tokio_vsock::VsockAddr,
    crate::io::VsockStreamResource
  ]
);

#[cfg(not(unix))]
network_stream!(
  [
    Tcp,
    tcp,
    tokio::net::TcpStream,
    crate::tcp::TcpListener,
    std::net::SocketAddr,
    TcpStreamResource
  ],
  [
    Tls,
    tls,
    crate::ops_tls::TlsStream,
    crate::ops_tls::TlsListener,
    std::net::SocketAddr,
    TlsStreamResource
  ]
);

pub enum NetworkStreamAddress {
  Ip(std::net::SocketAddr),
  #[cfg(unix)]
  Unix(tokio::net::unix::SocketAddr),
  #[cfg(unix)]
  Vsock(tokio_vsock::VsockAddr),
}

impl From<std::net::SocketAddr> for NetworkStreamAddress {
  fn from(value: std::net::SocketAddr) -> Self {
    NetworkStreamAddress::Ip(value)
  }
}

#[cfg(unix)]
impl From<tokio::net::unix::SocketAddr> for NetworkStreamAddress {
  fn from(value: tokio::net::unix::SocketAddr) -> Self {
    NetworkStreamAddress::Unix(value)
  }
}

#[cfg(unix)]
impl From<tokio_vsock::VsockAddr> for NetworkStreamAddress {
  fn from(value: tokio_vsock::VsockAddr) -> Self {
    NetworkStreamAddress::Vsock(value)
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum TakeNetworkStreamError {
  #[class("Busy")]
  #[error("TCP stream is currently in use")]
  TcpBusy,
  #[class("Busy")]
  #[error("TLS stream is currently in use")]
  TlsBusy,
  #[cfg(unix)]
  #[class("Busy")]
  #[error("Unix socket is currently in use")]
  UnixBusy,
  #[cfg(unix)]
  #[class("Busy")]
  #[error("Vsock socket is currently in use")]
  VsockBusy,
  #[class(generic)]
  #[error(transparent)]
  ReuniteTcp(#[from] tokio::net::tcp::ReuniteError),
  #[cfg(unix)]
  #[class(generic)]
  #[error(transparent)]
  ReuniteUnix(#[from] tokio::net::unix::ReuniteError),
  #[cfg(unix)]
  #[class(generic)]
  #[error("Cannot reunite halves from different streams")]
  ReuniteVsock,
  #[class(inherit)]
  #[error(transparent)]
  Resource(deno_core::error::ResourceError),
}

/// In some cases it may be more efficient to extract the resource from the resource table and use it directly (for example, an HTTP server).
/// This method will extract a stream from the resource table and return it, unwrapped.
pub fn take_network_stream_resource(
  resource_table: &mut ResourceTable,
  stream_rid: ResourceId,
) -> Result<NetworkStream, TakeNetworkStreamError> {
  // The stream we're attempting to unwrap may be in use somewhere else. If that's the case, we cannot proceed
  // with the process of unwrapping this connection, so we just return a bad resource error.
  // See also: https://github.com/denoland/deno/pull/16242

  if let Ok(resource_rc) = resource_table.take::<TcpStreamResource>(stream_rid)
  {
    // This TCP connection might be used somewhere else.
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| TakeNetworkStreamError::TcpBusy)?;
    let (read_half, write_half) = resource.into_inner();
    let tcp_stream = read_half.reunite(write_half)?;
    return Ok(NetworkStream::Tcp(tcp_stream));
  }

  if let Ok(resource_rc) = resource_table.take::<TlsStreamResource>(stream_rid)
  {
    // This TLS connection might be used somewhere else.
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| TakeNetworkStreamError::TlsBusy)?;
    let (read_half, write_half) = resource.into_inner();
    let tls_stream = read_half.unsplit(write_half);
    return Ok(NetworkStream::Tls(tls_stream));
  }

  #[cfg(unix)]
  if let Ok(resource_rc) =
    resource_table.take::<crate::io::UnixStreamResource>(stream_rid)
  {
    // This UNIX socket might be used somewhere else.
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| TakeNetworkStreamError::UnixBusy)?;
    let (read_half, write_half) = resource.into_inner();
    let unix_stream = read_half.reunite(write_half)?;
    return Ok(NetworkStream::Unix(unix_stream));
  }

  #[cfg(unix)]
  if let Ok(resource_rc) =
    resource_table.take::<crate::io::VsockStreamResource>(stream_rid)
  {
    // This Vsock socket might be used somewhere else.
    let resource = Rc::try_unwrap(resource_rc)
      .map_err(|_| TakeNetworkStreamError::VsockBusy)?;
    let (read_half, write_half) = resource.into_inner();
    if !read_half.is_pair_of(&write_half) {
      return Err(TakeNetworkStreamError::ReuniteVsock);
    }
    let vsock_stream = read_half.unsplit(write_half);
    return Ok(NetworkStream::Vsock(vsock_stream));
  }

  Err(TakeNetworkStreamError::Resource(
    ResourceError::BadResourceId,
  ))
}

/// In some cases it may be more efficient to extract the resource from the resource table and use it directly (for example, an HTTP server).
/// This method will extract a stream from the resource table and return it, unwrapped.
pub fn take_network_stream_listener_resource(
  resource_table: &mut ResourceTable,
  listener_rid: ResourceId,
) -> Result<NetworkStreamListener, JsErrorBox> {
  NetworkStreamListener::take_resource(resource_table, listener_rid)
}
