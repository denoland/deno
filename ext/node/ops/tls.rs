// Copyright 2018-2025 the Deno authors. MIT license.
use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::io::Error;
use std::io::ErrorKind;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use base64::Engine;
use bytes::Bytes;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_net::ops_tls::TlsStreamResource;
use deno_tls::SocketUse;
use deno_tls::TlsKeys;
use deno_tls::TlsKeysHolder;
use deno_tls::create_client_config;
use deno_tls::rustls::ClientConnection;
use deno_tls::rustls::pki_types::ServerName;
use rustls_tokio_stream::TlsStream;
use rustls_tokio_stream::TlsStreamRead;
use rustls_tokio_stream::TlsStreamWrite;
use rustls_tokio_stream::UnderlyingStream;
use serde::Deserialize;
use webpki_root_certs;

use super::crypto::x509::Certificate;
use super::crypto::x509::CertificateObject;

#[op2]
#[serde]
pub fn op_get_root_certificates() -> Vec<String> {
  webpki_root_certs::TLS_SERVER_ROOT_CERTS
    .iter()
    .map(|cert| {
      let b64 = base64::engine::general_purpose::STANDARD.encode(cert);
      let pem_lines = b64
        .chars()
        .collect::<Vec<char>>()
        // Node uses 72 characters per line, so we need to follow node even though
        // it's not spec compliant https://datatracker.ietf.org/doc/html/rfc7468#section-2
        .chunks(72)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<String>>()
        .join("\n");
      let pem = format!(
        "-----BEGIN CERTIFICATE-----\n{pem_lines}\n-----END CERTIFICATE-----\n",
      );
      pem
    })
    .collect::<Vec<String>>()
}

#[op2]
#[serde]
pub fn op_tls_peer_certificate(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  detailed: bool,
) -> Option<CertificateObject> {
  let resource = state.resource_table.get::<TlsStreamResource>(rid).ok()?;
  let certs = resource.peer_certificates()?;

  if certs.is_empty() {
    return None;
  }

  // For Node.js compatibility, return the peer certificate (first in chain)
  let cert_der = &certs[0];

  let cert = Certificate::from_der(cert_der.as_ref()).ok()?;
  cert.to_object(detailed).ok()
}

#[op2]
#[string]
pub fn op_tls_canonicalize_ipv4_address(
  #[string] hostname: String,
) -> Option<String> {
  let ip = hostname.parse::<std::net::IpAddr>().ok()?;

  let canonical_ip = match ip {
    std::net::IpAddr::V4(ipv4) => ipv4.to_string(),
    std::net::IpAddr::V6(ipv6) => ipv6.to_string(),
  };

  Some(canonical_ip)
}

// Two pairs of byte channels
use std::sync::Arc;
use std::sync::Mutex;

struct ReadableFuture<'a> {
  socket: &'a JSStreamSocket,
}

impl<'a> Future for ReadableFuture<'a> {
  type Output = std::io::Result<()>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    self.socket.poll_read_ready(cx)
  }
}

struct WritableFuture<'a> {
  socket: &'a JSStreamSocket,
}

impl<'a> Future for WritableFuture<'a> {
  type Output = std::io::Result<()>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    self.socket.poll_write_ready(cx)
  }
}

#[derive(Debug)]
struct JSStreamSocket {
  readable: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<Bytes>>>,
  writable: tokio::sync::mpsc::UnboundedSender<Bytes>,
  read_buffer: Arc<Mutex<Option<Bytes>>>,
  closed: Arc<Mutex<bool>>,
}

impl JSStreamSocket {
  pub fn new(
    readable: tokio::sync::mpsc::UnboundedReceiver<Bytes>,
    writable: tokio::sync::mpsc::UnboundedSender<Bytes>,
  ) -> Self {
    Self {
      readable: Arc::new(Mutex::new(readable)),
      writable,
      read_buffer: Arc::new(Mutex::new(None)),
      closed: Arc::new(Mutex::new(false)),
    }
  }
}

impl UnderlyingStream for JSStreamSocket {
  type StdType = ();

  fn poll_read_ready(
    &self,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    // Check if we have buffered data
    if let Ok(buffer) = self.read_buffer.lock() {
      if buffer.is_some() {
        return Poll::Ready(Ok(()));
      }
    }

    if let Ok(closed) = self.closed.lock() {
      if *closed {
        return Poll::Ready(Err(Error::new(
          ErrorKind::UnexpectedEof,
          "Stream closed",
        )));
      }
    }

    // Try to poll for data without consuming it
    if let Ok(mut receiver) = self.readable.lock() {
      match receiver.poll_recv(cx) {
        Poll::Ready(Some(data)) => {
          // Store the data in buffer for try_read
          if let Ok(mut buffer) = self.read_buffer.lock() {
            *buffer = Some(data);
          }
          Poll::Ready(Ok(()))
        }
        Poll::Ready(None) => {
          // Channel closed
          if let Ok(mut closed) = self.closed.lock() {
            *closed = true;
          }
          Poll::Ready(Err(Error::new(
            ErrorKind::UnexpectedEof,
            "Channel closed",
          )))
        }
        Poll::Pending => Poll::Pending,
      }
    } else {
      Poll::Ready(Err(Error::new(ErrorKind::Other, "Failed to acquire lock")))
    }
  }

  fn poll_write_ready(
    &self,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    if let Ok(closed) = self.closed.lock() {
      if *closed {
        return Poll::Ready(Err(Error::new(
          ErrorKind::BrokenPipe,
          "Stream closed",
        )));
      }
    }

    // For unbounded sender, we're always ready to write unless closed
    if self.writable.is_closed() {
      if let Ok(mut closed) = self.closed.lock() {
        *closed = true;
      }
      Poll::Ready(Err(Error::new(ErrorKind::BrokenPipe, "Channel closed")))
    } else {
      Poll::Ready(Ok(()))
    }
  }

  fn try_read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
    if let Ok(closed) = self.closed.lock() {
      if *closed {
        return Err(Error::new(ErrorKind::UnexpectedEof, "Stream closed"));
      }
    }

    // Check if we have buffered data first
    if let Ok(mut buffer) = self.read_buffer.lock() {
      if let Some(data) = buffer.take() {
        let len = std::cmp::min(buf.len(), data.len());
        buf[..len].copy_from_slice(&data[..len]);

        // If there's leftover data, put it back in the buffer
        if data.len() > len {
          *buffer = Some(data.slice(len..));
        }

        return Ok(len);
      }
    }

    // Try to read from channel non-blocking
    if let Ok(mut receiver) = self.readable.lock() {
      match receiver.try_recv() {
        Ok(data) => {
          let len = std::cmp::min(buf.len(), data.len());
          buf[..len].copy_from_slice(&data[..len]);

          // If there's leftover data, store it in buffer
          if data.len() > len {
            if let Ok(mut buffer) = self.read_buffer.lock() {
              *buffer = Some(data.slice(len..));
            }
          }

          Ok(len)
        }
        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
          Err(Error::new(ErrorKind::WouldBlock, "No data available"))
        }
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
          if let Ok(mut closed) = self.closed.lock() {
            *closed = true;
          }
          Err(Error::new(ErrorKind::UnexpectedEof, "Channel closed"))
        }
      }
    } else {
      Err(Error::new(ErrorKind::Other, "Failed to acquire lock"))
    }
  }

  fn try_write(&self, buf: &[u8]) -> std::io::Result<usize> {
    if let Ok(closed) = self.closed.lock() {
      if *closed {
        return Err(Error::new(ErrorKind::BrokenPipe, "Stream closed"));
      }
    }

    if self.writable.is_closed() {
      if let Ok(mut closed) = self.closed.lock() {
        *closed = true;
      }
      return Err(Error::new(ErrorKind::BrokenPipe, "Channel closed"));
    }

    // Convert buffer to Bytes and send
    let data = Bytes::copy_from_slice(buf);
    match self.writable.send(data) {
      Ok(()) => Ok(buf.len()),
      Err(_) => {
        if let Ok(mut closed) = self.closed.lock() {
          *closed = true;
        }
        Err(Error::new(ErrorKind::BrokenPipe, "Failed to send data"))
      }
    }
  }

  fn readable(&self) -> impl Future<Output = std::io::Result<()>> + Send {
    ReadableFuture { socket: self }
  }

  fn writable(&self) -> impl Future<Output = std::io::Result<()>> + Send {
    WritableFuture { socket: self }
  }

  fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
    match how {
      std::net::Shutdown::Read => {
        // Close the read side - mark as closed
        if let Ok(mut closed) = self.closed.lock() {
          *closed = true;
        }
        // We can't actually close the receiver side of the channel
        // but we can mark it as closed for our purposes
        Ok(())
      }
      std::net::Shutdown::Write => {
        // Close the write side
        // The UnboundedSender doesn't have a direct close method,
        // but dropping it will close the channel
        if let Ok(mut closed) = self.closed.lock() {
          *closed = true;
        }
        Ok(())
      }
      std::net::Shutdown::Both => {
        if let Ok(mut closed) = self.closed.lock() {
          *closed = true;
        }
        Ok(())
      }
    }
  }

  fn into_std(self) -> Option<std::io::Result<Self::StdType>> {
    None
  }
}

struct JSDuplexResource {
  readable: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<Bytes>>>,
  writable: tokio::sync::mpsc::UnboundedSender<Bytes>,
}

impl JSDuplexResource {
  pub fn new(
    readable: tokio::sync::mpsc::UnboundedReceiver<Bytes>,
    writable: tokio::sync::mpsc::UnboundedSender<Bytes>,
  ) -> Self {
    Self {
      readable: Arc::new(Mutex::new(readable)),
      writable,
    }
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    // Try to receive data from the readable channel
    let mut receiver = self
      .readable
      .lock()
      .map_err(|_| Error::new(ErrorKind::Other, "Failed to acquire lock"))?;

      dbg!("JSDuplexResource::read - waiting for data");
    match receiver.recv().await {
      Some(bytes) => {
        let len = std::cmp::min(data.len(), bytes.len());
        data[..len].copy_from_slice(&bytes[..len]);
        dbg!("JSDuplexResource::read - received data: {:?}", &data[..len]);
        // If there's more data than our buffer can hold, we lose it
        // This is a limitation of the current design
        Ok(len)
      }
      None => {
        // Channel closed
        Ok(0)
      }
    }
  }

  pub async fn write(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, std::io::Error> {
    // Convert data to Bytes and send through the writable channel
    let bytes = Bytes::copy_from_slice(data);

    self
      .writable
      .send(bytes)
      .map_err(|_| Error::new(ErrorKind::BrokenPipe, "Channel closed"))?;

    Ok(data.len())
  }
}

impl Resource for JSDuplexResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<'_, str> {
    "JSDuplexResource".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartJSTlsArgs {
  ca_certs: Vec<String>,
  hostname: String,
  alpn_protocols: Option<Vec<String>>,
  reject_unauthorized: Option<bool>,
}

// TLS stream resource similar to TlsStreamResource but for JSStreamSocket
#[derive(Debug)]
enum JSStreamTlsInner {
  JSSocket {
    rd: AsyncRefCell<TlsStreamRead<JSStreamSocket>>,
    wr: AsyncRefCell<TlsStreamWrite<JSStreamSocket>>,
  },
}

#[derive(Debug)]
pub struct JSStreamTlsResource {
  inner: JSStreamTlsInner,
}

impl JSStreamTlsResource {
  pub fn new_js(
    (rd, wr): (
      TlsStreamRead<JSStreamSocket>,
      TlsStreamWrite<JSStreamSocket>,
    ),
  ) -> Self {
    Self {
      inner: JSStreamTlsInner::JSSocket {
        rd: AsyncRefCell::new(rd),
        wr: AsyncRefCell::new(wr),
      },
    }
  }

  pub async fn handshake(self: &Rc<Self>) -> Result<(), std::io::Error> {
    let mut wr = RcRef::map(self, |r| match &r.inner {
      JSStreamTlsInner::JSSocket { wr, .. } => wr,
    })
    .borrow_mut()
    .await;

    wr.handshake().await?;
    Ok(())
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    use tokio::io::AsyncReadExt;
    let mut rd = RcRef::map(&self, |r| match &r.inner {
      JSStreamTlsInner::JSSocket { rd, .. } => rd,
    })
    .borrow_mut()
    .await;
    rd.read(data).await
  }

  pub async fn write(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, std::io::Error> {
    use tokio::io::AsyncWriteExt;
    let mut wr = RcRef::map(&self, |r| match &r.inner {
      JSStreamTlsInner::JSSocket { wr, .. } => wr,
    })
    .borrow_mut()
    .await;
    let nwritten = wr.write(data).await?;
    wr.flush().await?;
    Ok(nwritten)
  }
}

impl Resource for JSStreamTlsResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<'_, str> {
    "JSStreamTlsResource".into()
  }
}

// TLS op that creates both a TLS stream and user duplex, returning both RIDs
#[op2]
pub fn op_node_tls_start(
  state: Rc<RefCell<OpState>>,
  #[serde] args: StartJSTlsArgs,
  #[buffer] output: &mut [u32],
) -> Result<(), deno_tls::TlsError> {
  let reject_unauthorized = args.reject_unauthorized.unwrap_or(true);
  let hostname = match &*args.hostname {
    "" => "localhost".to_string(),
    n => n.to_string(),
  };

  if output.len() < 2 {
    return Err(deno_tls::TlsError::UnableAddPemFileToCert(Error::new(
      ErrorKind::InvalidInput,
      "Output buffer must have at least 2 elements",
    )));
  }

  let ca_certs = args
    .ca_certs
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  let hostname_dns =
    ServerName::try_from(hostname.to_string()).map_err(|_| {
      deno_tls::TlsError::UnableAddPemFileToCert(Error::new(
        ErrorKind::InvalidInput,
        format!("Invalid hostname: {}", hostname),
      ))
    })?;

  // Create channels for bidirectional communication between JS and TLS
  // network_to_tls: Raw network data from JS to TLS (ServerHello, etc.)
  // tls_to_network: TLS data from TLS to JS network (ClientHello, etc.)
  let (network_to_tls_tx, network_to_tls_rx) =
    tokio::sync::mpsc::unbounded_channel::<Bytes>();
  let (tls_to_network_tx, tls_to_network_rx) =
    tokio::sync::mpsc::unbounded_channel::<Bytes>();

  // JSStreamSocket acts as the network interface for the TLS connection
  // TLS reads network data and writes network data through this
  let js_stream = JSStreamSocket::new(network_to_tls_rx, tls_to_network_tx);

  // Set up TLS configuration using deno_tls utilities
  let tls_null = TlsKeysHolder::from(TlsKeys::Null);
  let mut tls_config = create_client_config(
    Default::default(), // root_cert_store
    ca_certs,
    if reject_unauthorized {
      None
    } else {
      Some(Vec::new())
    }, // unsafely_ignore_certificate_errors
    tls_null.take(),
    SocketUse::GeneralSsl,
  )?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let tls_config = Arc::new(tls_config);
  let tls_stream = TlsStream::new_client_side(
    js_stream,
    ClientConnection::new(tls_config, hostname_dns)?,
    NonZeroUsize::new(65536),
  );

  // Create the TLS resource
  let tls_resource = JSStreamTlsResource::new_js(tls_stream.into_split());

  // Create the duplex resource that JS will interact with
  // JS writes network data (from socket) here, reads network data (to socket) from here
  let user_duplex = JSDuplexResource::new(tls_to_network_rx, network_to_tls_tx);

  let (tls_rid, duplex_rid) = {
    let mut state_ = state.borrow_mut();
    let tls_rid = state_.resource_table.add(tls_resource);
    let duplex_rid = state_.resource_table.add(user_duplex);
    (tls_rid, duplex_rid)
  };

  // Return both RIDs in the output buffer
  output[0] = tls_rid;
  output[1] = duplex_rid;

  Ok(())
}

#[op2(async)]
pub async fn op_node_tls_handshake(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) {
  let resource = state
    .borrow()
    .resource_table
    .get::<JSStreamTlsResource>(rid)
    .unwrap();

  resource.handshake().await.unwrap()
}
