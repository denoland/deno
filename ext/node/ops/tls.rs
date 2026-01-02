// Copyright 2018-2025 the Deno authors. MIT license.
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::io::Error;
use std::io::ErrorKind;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
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
use deno_net::DefaultTlsOptions;
use deno_net::UnsafelyIgnoreCertificateErrors;
use deno_net::ops::NetError;
use deno_net::ops::TlsHandshakeInfo;
use deno_net::ops_tls::TlsStreamResource;
use deno_tls::SocketUse;
use deno_tls::TlsClientConfigOptions;
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
pub struct JSStreamSocket {
  readable: Arc<Mutex<tokio::sync::mpsc::Receiver<Bytes>>>,
  writable: tokio::sync::mpsc::Sender<Bytes>,
  read_buffer: Arc<Mutex<VecDeque<Bytes>>>,
  closed: AtomicBool,
}

impl JSStreamSocket {
  pub fn new(
    readable: tokio::sync::mpsc::Receiver<Bytes>,
    writable: tokio::sync::mpsc::Sender<Bytes>,
  ) -> Self {
    Self {
      readable: Arc::new(Mutex::new(readable)),
      writable,
      read_buffer: Arc::new(Mutex::new(VecDeque::new())),
      closed: AtomicBool::new(false),
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
    if let Ok(buffer) = self.read_buffer.lock()
      && !buffer.is_empty()
    {
      return Poll::Ready(Ok(()));
    }

    if self.closed.load(Ordering::Relaxed) {
      return Poll::Ready(Err(Error::new(
        ErrorKind::UnexpectedEof,
        "Stream closed",
      )));
    }

    // Try to poll for data without consuming it
    if let Ok(mut receiver) = self.readable.lock() {
      match receiver.poll_recv(cx) {
        Poll::Ready(Some(data)) => {
          // Store the data in buffer for try_read
          if let Ok(mut buffer) = self.read_buffer.lock() {
            buffer.push_back(data);
          }
          Poll::Ready(Ok(()))
        }
        Poll::Ready(None) => {
          // Channel closed
          self.closed.store(true, Ordering::Relaxed);
          Poll::Ready(Err(Error::new(
            ErrorKind::UnexpectedEof,
            "Channel closed",
          )))
        }
        Poll::Pending => Poll::Pending,
      }
    } else {
      panic!("Failed to acquire lock")
    }
  }

  fn poll_write_ready(
    &self,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    if self.closed.load(Ordering::Relaxed) {
      return Poll::Ready(Err(Error::new(
        ErrorKind::BrokenPipe,
        "Stream closed",
      )));
    }

    // For bounded sender, check if channel is ready
    if self.writable.is_closed() {
      self.closed.store(true, Ordering::Relaxed);
      Poll::Ready(Err(Error::new(ErrorKind::BrokenPipe, "Channel closed")))
    } else {
      Poll::Ready(Ok(()))
    }
  }

  fn try_read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
    if self.closed.load(Ordering::Relaxed) {
      return Err(Error::new(ErrorKind::UnexpectedEof, "Stream closed"));
    }

    // Check if we have buffered data first
    if let Ok(mut buffer) = self.read_buffer.lock()
      && let Some(data) = buffer.pop_front()
    {
      let len = std::cmp::min(buf.len(), data.len());
      buf[..len].copy_from_slice(&data[..len]);

      // If there's leftover data, put it back in the buffer
      if data.len() > len {
        buffer.push_front(data.slice(len..));
      }

      return Ok(len);
    }

    // Try to read from channel non-blocking
    if let Ok(mut receiver) = self.readable.lock() {
      match receiver.try_recv() {
        Ok(data) => {
          let len = std::cmp::min(buf.len(), data.len());
          buf[..len].copy_from_slice(&data[..len]);

          // If there's leftover data, store it in buffer
          if data.len() > len
            && let Ok(mut buffer) = self.read_buffer.lock()
          {
            buffer.push_front(data.slice(len..));
          }

          Ok(len)
        }
        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
          Err(Error::new(ErrorKind::WouldBlock, "No data available"))
        }
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
          self.closed.store(true, Ordering::Relaxed);
          Err(Error::new(ErrorKind::UnexpectedEof, "Channel closed"))
        }
      }
    } else {
      Err(Error::other("Failed to acquire lock"))
    }
  }

  fn try_write(&self, buf: &[u8]) -> std::io::Result<usize> {
    if self.closed.load(Ordering::Relaxed) {
      return Err(Error::new(ErrorKind::BrokenPipe, "Stream closed"));
    }

    if self.writable.is_closed() {
      self.closed.store(true, Ordering::Relaxed);
      return Err(Error::new(ErrorKind::BrokenPipe, "Channel closed"));
    }

    let data = Bytes::copy_from_slice(buf);
    match self.writable.try_send(data) {
      Ok(()) => Ok(buf.len()),
      Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
        Err(Error::new(ErrorKind::WouldBlock, "Channel full"))
      }
      Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
        self.closed.store(true, Ordering::Relaxed);
        Err(Error::new(ErrorKind::BrokenPipe, "Channel closed"))
      }
    }
  }

  fn readable(&self) -> impl Future<Output = std::io::Result<()>> + Send {
    ReadableFuture { socket: self }
  }

  fn writable(&self) -> impl Future<Output = std::io::Result<()>> + Send {
    WritableFuture { socket: self }
  }

  fn shutdown(&self, _: std::net::Shutdown) -> std::io::Result<()> {
    self.closed.store(true, Ordering::Relaxed);
    Ok(())
  }

  fn into_std(self) -> Option<std::io::Result<Self::StdType>> {
    None
  }
}

struct JSDuplexResource {
  readable: Arc<Mutex<tokio::sync::mpsc::Receiver<Bytes>>>,
  writable: tokio::sync::mpsc::Sender<Bytes>,
  read_buffer: Arc<Mutex<VecDeque<Bytes>>>,
}

impl JSDuplexResource {
  pub fn new(
    readable: tokio::sync::mpsc::Receiver<Bytes>,
    writable: tokio::sync::mpsc::Sender<Bytes>,
  ) -> Self {
    Self {
      readable: Arc::new(Mutex::new(readable)),
      writable,
      read_buffer: Arc::new(Mutex::new(VecDeque::new())),
    }
  }

  #[allow(clippy::await_holding_lock)]
  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    // First check if we have buffered data from previous partial read
    if let Ok(mut buffer) = self.read_buffer.lock()
      && let Some(buffered_data) = buffer.pop_front()
    {
      let len = std::cmp::min(data.len(), buffered_data.len());
      data[..len].copy_from_slice(&buffered_data[..len]);

      // If there's remaining data, put it back in buffer
      if buffered_data.len() > len {
        buffer.push_front(buffered_data.slice(len..));
      }

      return Ok(len);
    }

    // No buffered data, receive new data from channel
    let bytes = {
      let mut receiver = self
        .readable
        .lock()
        .map_err(|_| Error::other("Failed to acquire lock"))?;
      receiver.recv().await
    };

    match bytes {
      Some(bytes) => {
        let len = std::cmp::min(data.len(), bytes.len());
        data[..len].copy_from_slice(&bytes[..len]);

        // If there's remaining data, buffer it for next read
        if bytes.len() > len
          && let Ok(mut buffer) = self.read_buffer.lock()
        {
          buffer.push_back(bytes.slice(len..));
        }

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
    let bytes = Bytes::copy_from_slice(data);

    self
      .writable
      .send(bytes)
      .await
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

#[derive(Debug)]
pub struct JSStreamTlsResource {
  rd: AsyncRefCell<TlsStreamRead<JSStreamSocket>>,
  wr: AsyncRefCell<TlsStreamWrite<JSStreamSocket>>,
}

impl JSStreamTlsResource {
  pub fn new(
    (rd, wr): (
      TlsStreamRead<JSStreamSocket>,
      TlsStreamWrite<JSStreamSocket>,
    ),
  ) -> Self {
    Self {
      rd: AsyncRefCell::new(rd),
      wr: AsyncRefCell::new(wr),
    }
  }

  pub async fn handshake(
    self: &Rc<Self>,
  ) -> Result<TlsHandshakeInfo, std::io::Error> {
    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;

    let handshake = wr.handshake().await?;

    let alpn_protocol = handshake.alpn.map(|alpn| alpn.into());
    let peer_certificates = handshake.peer_certificates.clone();
    let tls_info = TlsHandshakeInfo {
      alpn_protocol,
      peer_certificates,
    };

    Ok(tls_info)
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    use tokio::io::AsyncReadExt;

    let mut rd = RcRef::map(&self, |r| &r.rd).borrow_mut().await;
    rd.read(data).await
  }

  pub async fn write(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, std::io::Error> {
    use tokio::io::AsyncWriteExt;

    let mut wr = RcRef::map(&self, |r| &r.wr).borrow_mut().await;
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

#[op2]
pub fn op_node_tls_start(
  state: Rc<RefCell<OpState>>,
  #[serde] args: StartJSTlsArgs,
  #[buffer] output: &mut [u32],
) -> Result<(), NetError> {
  let reject_unauthorized = args.reject_unauthorized.unwrap_or(true);
  let hostname = match &*args.hostname {
    "" => "localhost".to_string(),
    n => n.to_string(),
  };

  assert_eq!(output.len(), 2);

  let ca_certs = args
    .ca_certs
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  let hostname_dns = ServerName::try_from(hostname.to_string())
    .map_err(|_| NetError::InvalidHostname(hostname))?;
  // --unsafely-ignore-certificate-errors overrides the `rejectUnauthorized` option.
  let unsafely_ignore_certificate_errors = if reject_unauthorized {
    state
      .borrow()
      .try_borrow::<UnsafelyIgnoreCertificateErrors>()
      .and_then(|it| it.0.clone())
  } else {
    Some(Vec::new())
  };

  let root_cert_store = state
    .borrow()
    .borrow::<DefaultTlsOptions>()
    .root_cert_store()
    .map_err(NetError::RootCertStore)?;

  let (network_to_tls_tx, network_to_tls_rx) =
    tokio::sync::mpsc::channel::<Bytes>(10);
  let (tls_to_network_tx, tls_to_network_rx) =
    tokio::sync::mpsc::channel::<Bytes>(10);

  let js_stream = JSStreamSocket::new(network_to_tls_rx, tls_to_network_tx);

  let tls_null = TlsKeysHolder::from(TlsKeys::Null);
  let mut tls_config = create_client_config(TlsClientConfigOptions {
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    unsafely_disable_hostname_verification: false,
    cert_chain_and_key: tls_null.take(),
    socket_use: SocketUse::GeneralSsl,
  })?;

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

  let tls_resource = JSStreamTlsResource::new(tls_stream.into_split());
  let user_duplex = JSDuplexResource::new(tls_to_network_rx, network_to_tls_tx);

  let (tls_rid, duplex_rid) = {
    let mut state = state.borrow_mut();
    let tls_rid = state.resource_table.add(tls_resource);
    let duplex_rid = state.resource_table.add(user_duplex);
    (tls_rid, duplex_rid)
  };

  output[0] = tls_rid;
  output[1] = duplex_rid;

  Ok(())
}

#[op2(async)]
#[serde]
pub async fn op_node_tls_handshake(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<TlsHandshakeInfo, NetError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<JSStreamTlsResource>(rid)
    .map_err(|_| NetError::ListenerClosed)?;
  resource.handshake().await.map_err(Into::into)
}
