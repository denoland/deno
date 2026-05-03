// Copyright 2018-2026 the Deno authors. MIT license.
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
use deno_core::FromV8;
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
use deno_node_crypto::x509::Certificate;
use deno_node_crypto::x509::CertificateObject;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;
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
use sys_traits::EnvVar;
use sys_traits::FsRead;
use webpki_root_certs;

use crate::ExtNodeSys;

#[derive(Clone)]
pub(crate) struct NodeTlsState {
  pub(crate) custom_ca_certs: Option<Vec<String>>,
  pub(crate) client_session_store:
    Arc<dyn deno_tls::rustls::client::ClientSessionStore>,
  /// Process-shared TLS session ticketer used for every `node:tls` server
  /// config in this isolate.  Sharing the ticketer across servers in a
  /// process keeps RFC 5077 ticket keys consistent for all incoming
  /// connections to a given `tls.createServer()`, which is what the upstream
  /// `parallel/test-tls-ticket-cluster` test relies on.  Node maintains
  /// per-server keys; this is a pragmatic simplification.
  pub(crate) server_ticketer:
    Option<Arc<dyn deno_tls::rustls::server::ProducesTickets>>,
  /// Cached TLS-1.3 client cert verifier and the shared "no client cert"
  /// resolver, used when a client connection is built without custom CA
  /// certs or a client cert.  Reusing these `Arc`s across connections keeps
  /// rustls's session-resumption identity check (`Arc::downgrade(&verifier)`)
  /// stable, which is what allows `tls.TLSSocket#isSessionReused()` to
  /// return true on subsequent connections.  Without identity stability the
  /// cached session is dropped at handshake start and resumption never
  /// succeeds.
  pub(crate) cached_default_verifier: Option<CachedClientVerifier>,
  pub(crate) cached_no_client_auth:
    Option<Arc<dyn deno_tls::rustls::client::ResolvesClientCert>>,
}

fn der_to_pem(der: &[u8]) -> String {
  let b64 = base64::engine::general_purpose::STANDARD.encode(der);
  let pem_lines = b64
    .chars()
    .collect::<Vec<char>>()
    // Node uses 72 characters per line, so we need to follow node even though
    // it's not spec compliant https://datatracker.ietf.org/doc/html/rfc7468#section-2
    .chunks(72)
    .map(|c| c.iter().collect::<String>())
    .collect::<Vec<String>>()
    .join("\n");
  format!("-----BEGIN CERTIFICATE-----\n{pem_lines}\n-----END CERTIFICATE-----",)
}

fn get_bundled_root_certificates() -> Vec<String> {
  webpki_root_certs::TLS_SERVER_ROOT_CERTS
    .iter()
    .map(|cert| der_to_pem(cert))
    .collect()
}

pub(crate) type CachedClientVerifier = (
  Arc<dyn deno_tls::rustls::client::danger::ServerCertVerifier>,
  Arc<std::sync::Mutex<Option<String>>>,
);

#[op2]
pub fn op_get_root_certificates(
  state: &mut OpState,
) -> Result<Vec<String>, PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("ca", "node:tls.rootCertificates")?;

  if let Some(tls_state) = state.try_borrow::<NodeTlsState>()
    && let Some(certs) = &tls_state.custom_ca_certs
  {
    return Ok(certs.clone());
  }

  Ok(get_bundled_root_certificates())
}

fn parse_extra_ca_certs(sys: &(impl EnvVar + FsRead)) -> Vec<String> {
  let Ok(extra_ca_certs_file) = sys.env_var("NODE_EXTRA_CA_CERTS") else {
    return vec![];
  };
  let Ok(contents) = sys.fs_read_to_string(&extra_ca_certs_file) else {
    return vec![];
  };
  contents
    .split("-----END CERTIFICATE-----")
    .filter_map(|s| {
      let trimmed = s.trim();
      if trimmed.contains("-----BEGIN CERTIFICATE-----") {
        Some(format!("{trimmed}\n-----END CERTIFICATE-----\n"))
      } else {
        None
      }
    })
    .collect()
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CaCertificatesError {
  #[class(type)]
  #[error(
    "The argument 'type' must be one of 'default', 'system', 'bundled', or 'extra'. Received '{0}'"
  )]
  InvalidType(String),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] PermissionCheckError),
}

#[op2]
pub fn op_get_ca_certificates<TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
  #[string] cert_type: String,
) -> Result<Vec<String>, CaCertificatesError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("ca", "node:tls.getCACertificates()")?;

  let sys = state.borrow::<TSys>();
  match cert_type.as_str() {
    "bundled" => Ok(get_bundled_root_certificates()),
    "system" => {
      let native_certs =
        deno_tls::deno_native_certs::load_native_certs().unwrap_or_default();
      Ok(
        native_certs
          .into_iter()
          .map(|cert| der_to_pem(&cert.0))
          .collect(),
      )
    }
    "extra" => Ok(parse_extra_ca_certs(sys)),
    "default" => {
      let mut certs = get_bundled_root_certificates();
      certs.extend(parse_extra_ca_certs(sys));
      Ok(certs)
    }
    _ => Err(CaCertificatesError::InvalidType(cert_type)),
  }
}

#[op2]
pub fn op_set_default_ca_certificates(
  state: &mut OpState,
  #[serde] certs: Vec<String>,
) {
  // Treat `setDefaultCACertificates([])` as "use defaults" (None) rather
  // than "use no custom CAs" (Some(vec![])).  The two are semantically
  // identical for cert validation, but `Some(vec![])` would force the
  // default-path verifier cache off in `build_client_config` and silently
  // disable session resumption.
  let normalized = if certs.is_empty() { None } else { Some(certs) };
  if let Some(tls_state) = state.try_borrow_mut::<NodeTlsState>() {
    tls_state.custom_ca_certs = normalized;
    // Custom CA list changed; previously cached verifier no longer matches.
    tls_state.cached_default_verifier = None;
  } else {
    state.put(NodeTlsState {
      custom_ca_certs: normalized,
      client_session_store: Arc::new(
        deno_tls::rustls::client::ClientSessionMemoryCache::new(256),
      ),
      server_ticketer: None,
      cached_default_verifier: None,
      cached_no_client_auth: None,
    });
  }
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
  closed: AtomicBool,
  close_notify: tokio::sync::Notify,
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
      closed: AtomicBool::new(false),
      close_notify: tokio::sync::Notify::new(),
    }
  }

  #[allow(
    clippy::await_holding_lock,
    reason = "lock is dropped before await points"
  )]
  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    if self.closed.load(Ordering::Relaxed) {
      return Ok(0);
    }

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

    // No buffered data, receive new data from channel.
    // We use select! so that close() can wake us up via close_notify
    // even though we hold the readable mutex across the await (the
    // close() method uses try_lock to avoid deadlock).
    let bytes = {
      let mut receiver = self
        .readable
        .lock()
        .map_err(|_| Error::other("Failed to acquire lock"))?;
      tokio::select! {
        result = receiver.recv() => result,
        _ = self.close_notify.notified() => None,
      }
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
        // Channel closed or resource closing
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

  fn close(self: Rc<Self>) {
    // Signal that this resource is closing.  The read() method checks
    // this flag and the close_notify to break out of pending recv().
    //
    // Without this cleanup, a circular Rc dependency between
    // JSDuplexResource and JSStreamTlsResource prevents either from
    // being dropped, keeping the event loop alive indefinitely.
    self.closed.store(true, Ordering::Relaxed);

    // Wake up any pending read via Notify.  We use notify_one() which
    // stores a permit if no one is currently waiting, so the next
    // notified().await will complete immediately.
    self.close_notify.notify_one();

    // Also try to close the receiver directly.  We use try_lock()
    // because read() holds the mutex across an await point; using
    // lock() here would deadlock.
    if let Ok(mut rx) = self.readable.try_lock() {
      rx.close();
    }
  }
}

#[derive(FromV8)]
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
  #[scoped] args: StartJSTlsArgs,
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

#[op2]
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
