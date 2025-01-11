// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::net::SocketAddrV6;
use std::pin::pin;
use std::rc::Rc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use deno_core::error::ResourceError;
use deno_core::futures::task::noop_waker_ref;
use deno_core::op2;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::GarbageCollected;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::WriteOutcome;
use deno_error::JsError;
use deno_error::JsErrorBox;
use deno_permissions::PermissionCheckError;
use deno_tls::create_client_config;
use deno_tls::SocketUse;
use deno_tls::TlsError;
use deno_tls::TlsKeys;
use deno_tls::TlsKeysHolder;
use quinn::crypto::rustls::QuicClientConfig;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::rustls::client::ClientSessionMemoryCache;
use quinn::rustls::client::ClientSessionStore;
use quinn::rustls::client::Resumption;
use serde::Deserialize;
use serde::Serialize;

use crate::resolve_addr::resolve_addr_sync;
use crate::DefaultTlsOptions;
use crate::NetPermissions;
use crate::UnsafelyIgnoreCertificateErrors;

#[derive(Debug, thiserror::Error, JsError)]
pub enum QuicError {
  #[class(generic)]
  #[error("Endpoint created by 'connectQuic' cannot be used for listening")]
  CannotListen,
  #[class(type)]
  #[error("key and cert are required")]
  MissingTlsKey,
  #[class(type)]
  #[error("Duration is invalid")]
  InvalidDuration,
  #[class(generic)]
  #[error("Unable to resolve hostname")]
  UnableToResolve,
  #[class(inherit)]
  #[error("{0}")]
  StdIo(#[from] std::io::Error),
  #[class(inherit)]
  #[error("{0}")]
  PermissionCheck(#[from] PermissionCheckError),
  #[class(range)]
  #[error("{0}")]
  VarIntBoundsExceeded(#[from] quinn::VarIntBoundsExceeded),
  #[class(generic)]
  #[error("{0}")]
  Rustls(#[from] quinn::rustls::Error),
  #[class(inherit)]
  #[error("{0}")]
  Tls(#[from] TlsError),
  #[class(generic)]
  #[error("{0}")]
  ConnectionError(#[from] quinn::ConnectionError),
  #[class(generic)]
  #[error("{0}")]
  ConnectError(#[from] quinn::ConnectError),
  #[class(generic)]
  #[error("{0}")]
  SendDatagramError(#[from] quinn::SendDatagramError),
  #[class("BadResource")]
  #[error("{0}")]
  ClosedStream(#[from] quinn::ClosedStream),
  #[class("BadResource")]
  #[error("Invalid {0} resource")]
  BadResource(&'static str),
  #[class(range)]
  #[error("Connection has reached the maximum number of concurrent outgoing {0} streams")]
  MaxStreams(&'static str),
  #[class(generic)]
  #[error("{0}")]
  Core(#[from] deno_core::error::AnyError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseInfo {
  close_code: u64,
  reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Addr {
  hostname: String,
  port: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListenArgs {
  alpn_protocols: Option<Vec<String>>,
}

#[derive(Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TransportConfig {
  keep_alive_interval: Option<u64>,
  max_idle_timeout: Option<u64>,
  max_concurrent_bidirectional_streams: Option<u32>,
  max_concurrent_unidirectional_streams: Option<u32>,
  preferred_address_v4: Option<SocketAddrV4>,
  preferred_address_v6: Option<SocketAddrV6>,
  congestion_control: Option<String>,
}

impl TryInto<quinn::TransportConfig> for TransportConfig {
  type Error = QuicError;

  fn try_into(self) -> Result<quinn::TransportConfig, Self::Error> {
    let mut cfg = quinn::TransportConfig::default();

    if let Some(interval) = self.keep_alive_interval {
      cfg.keep_alive_interval(Some(Duration::from_millis(interval)));
    }

    if let Some(timeout) = self.max_idle_timeout {
      cfg.max_idle_timeout(Some(
        Duration::from_millis(timeout)
          .try_into()
          .map_err(|_| QuicError::InvalidDuration)?,
      ));
    }

    if let Some(max) = self.max_concurrent_bidirectional_streams {
      cfg.max_concurrent_bidi_streams(max.into());
    }

    if let Some(max) = self.max_concurrent_unidirectional_streams {
      cfg.max_concurrent_uni_streams(max.into());
    }

    if let Some(v) = self.congestion_control {
      let controller: Option<
        Arc<dyn quinn::congestion::ControllerFactory + Send + Sync + 'static>,
      > = match v.as_str() {
        "low-latency" => {
          Some(Arc::new(quinn::congestion::BbrConfig::default()))
        }
        "throughput" => {
          Some(Arc::new(quinn::congestion::CubicConfig::default()))
        }
        _ => None,
      };
      if let Some(controller) = controller {
        cfg.congestion_controller_factory(controller);
      }
    }

    Ok(cfg)
  }
}

fn apply_server_transport_config(
  config: &mut quinn::ServerConfig,
  transport_config: TransportConfig,
) -> Result<(), QuicError> {
  config.preferred_address_v4(transport_config.preferred_address_v4);
  config.preferred_address_v6(transport_config.preferred_address_v6);
  config.transport_config(Arc::new(transport_config.try_into()?));
  Ok(())
}

struct EndpointResource {
  endpoint: quinn::Endpoint,
  can_listen: bool,
  session_store: Arc<dyn ClientSessionStore>,
}

impl GarbageCollected for EndpointResource {}

#[op2]
#[cppgc]
pub(crate) fn op_quic_endpoint_create<NP>(
  state: Rc<RefCell<OpState>>,
  #[serde] addr: Addr,
  can_listen: bool,
) -> Result<EndpointResource, QuicError>
where
  NP: NetPermissions + 'static,
{
  let addr = resolve_addr_sync(&addr.hostname, addr.port)?
    .next()
    .ok_or_else(|| QuicError::UnableToResolve)?;

  if can_listen {
    state.borrow_mut().borrow_mut::<NP>().check_net(
      &(&addr.ip().to_string(), Some(addr.port())),
      "new Deno.QuicEndpoint()",
    )?;
  } else {
    // If this is not a can-listen, assert that we will bind to an ephemeral port.
    assert_eq!(
      addr,
      SocketAddr::from((
        IpAddr::from(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
        0
      ))
    );
  }

  let config = quinn::EndpointConfig::default();
  let socket = std::net::UdpSocket::bind(addr)?;
  let endpoint = quinn::Endpoint::new(
    config,
    None,
    socket,
    quinn::default_runtime().unwrap(),
  )?;

  Ok(EndpointResource {
    endpoint,
    can_listen,
    session_store: Arc::new(ClientSessionMemoryCache::new(256)),
  })
}

#[op2]
#[serde]
pub(crate) fn op_quic_endpoint_get_addr(
  #[cppgc] endpoint: &EndpointResource,
) -> Result<Addr, QuicError> {
  let addr = endpoint.endpoint.local_addr()?;
  let addr = Addr {
    hostname: format!("{}", addr.ip()),
    port: addr.port(),
  };
  Ok(addr)
}

#[op2(fast)]
pub(crate) fn op_quic_endpoint_close(
  #[cppgc] endpoint: &EndpointResource,
  #[bigint] close_code: u64,
  #[string] reason: String,
) -> Result<(), QuicError> {
  endpoint
    .endpoint
    .close(quinn::VarInt::from_u64(close_code)?, reason.as_bytes());
  Ok(())
}

struct ListenerResource(quinn::Endpoint, Arc<QuicServerConfig>);

impl Drop for ListenerResource {
  fn drop(&mut self) {
    self.0.set_server_config(None);
  }
}

impl GarbageCollected for ListenerResource {}

#[op2]
#[cppgc]
pub(crate) fn op_quic_endpoint_listen(
  #[cppgc] endpoint: &EndpointResource,
  #[serde] args: ListenArgs,
  #[serde] transport_config: TransportConfig,
  #[cppgc] keys: &TlsKeysHolder,
) -> Result<ListenerResource, QuicError> {
  if !endpoint.can_listen {
    return Err(QuicError::CannotListen);
  }

  let TlsKeys::Static(deno_tls::TlsKey(cert, key)) = keys.take() else {
    return Err(QuicError::MissingTlsKey);
  };

  let mut crypto =
    quinn::rustls::ServerConfig::builder_with_protocol_versions(&[
      &quinn::rustls::version::TLS13,
    ])
    .with_no_client_auth()
    .with_single_cert(cert.clone(), key.clone_key())?;

  // required by QUIC spec.
  crypto.max_early_data_size = u32::MAX;

  if let Some(alpn_protocols) = args.alpn_protocols {
    crypto.alpn_protocols = alpn_protocols
      .into_iter()
      .map(|alpn| alpn.into_bytes())
      .collect();
  }

  let server_config = Arc::new(
    QuicServerConfig::try_from(crypto).expect("TLS13 is explicitly configured"),
  );
  let mut config = quinn::ServerConfig::with_crypto(server_config.clone());
  apply_server_transport_config(&mut config, transport_config)?;

  endpoint.endpoint.set_server_config(Some(config));

  Ok(ListenerResource(endpoint.endpoint.clone(), server_config))
}

struct ConnectionResource(
  quinn::Connection,
  RefCell<Option<quinn::ZeroRttAccepted>>,
);

impl GarbageCollected for ConnectionResource {}

struct IncomingResource(
  RefCell<Option<quinn::Incoming>>,
  Arc<QuicServerConfig>,
);

impl GarbageCollected for IncomingResource {}

#[op2(async)]
#[cppgc]
pub(crate) async fn op_quic_listener_accept(
  #[cppgc] resource: &ListenerResource,
) -> Result<IncomingResource, QuicError> {
  match resource.0.accept().await {
    Some(incoming) => Ok(IncomingResource(
      RefCell::new(Some(incoming)),
      resource.1.clone(),
    )),
    None => Err(QuicError::BadResource("QuicListener")),
  }
}

#[op2(fast)]
pub(crate) fn op_quic_listener_stop(#[cppgc] resource: &ListenerResource) {
  resource.0.set_server_config(None);
}

#[op2]
#[string]
pub(crate) fn op_quic_incoming_local_ip(
  #[cppgc] incoming_resource: &IncomingResource,
) -> Result<Option<String>, QuicError> {
  let Some(incoming) = incoming_resource.0.borrow_mut().take() else {
    return Err(QuicError::BadResource("QuicIncoming"));
  };
  Ok(incoming.local_ip().map(|ip| ip.to_string()))
}

#[op2]
#[serde]
pub(crate) fn op_quic_incoming_remote_addr(
  #[cppgc] incoming_resource: &IncomingResource,
) -> Result<Addr, QuicError> {
  let Some(incoming) = incoming_resource.0.borrow_mut().take() else {
    return Err(QuicError::BadResource("QuicIncoming"));
  };
  let addr = incoming.remote_address();
  Ok(Addr {
    hostname: format!("{}", addr.ip()),
    port: addr.port(),
  })
}

#[op2(fast)]
pub(crate) fn op_quic_incoming_remote_addr_validated(
  #[cppgc] incoming_resource: &IncomingResource,
) -> Result<bool, QuicError> {
  let Some(incoming) = incoming_resource.0.borrow_mut().take() else {
    return Err(QuicError::BadResource("QuicIncoming"));
  };
  Ok(incoming.remote_address_validated())
}

fn quic_incoming_accept(
  incoming_resource: &IncomingResource,
  transport_config: Option<TransportConfig>,
) -> Result<quinn::Connecting, QuicError> {
  let Some(incoming) = incoming_resource.0.borrow_mut().take() else {
    return Err(QuicError::BadResource("QuicIncoming"));
  };
  match transport_config {
    Some(transport_config) if transport_config != Default::default() => {
      let mut config =
        quinn::ServerConfig::with_crypto(incoming_resource.1.clone());
      apply_server_transport_config(&mut config, transport_config)?;
      Ok(incoming.accept_with(Arc::new(config))?)
    }
    _ => Ok(incoming.accept()?),
  }
}

#[op2(async)]
#[cppgc]
pub(crate) async fn op_quic_incoming_accept(
  #[cppgc] incoming_resource: &IncomingResource,
  #[serde] transport_config: Option<TransportConfig>,
) -> Result<ConnectionResource, QuicError> {
  let connecting = quic_incoming_accept(incoming_resource, transport_config)?;
  let conn = connecting.await?;
  Ok(ConnectionResource(conn, RefCell::new(None)))
}

#[op2]
#[cppgc]
pub(crate) fn op_quic_incoming_accept_0rtt(
  #[cppgc] incoming_resource: &IncomingResource,
  #[serde] transport_config: Option<TransportConfig>,
) -> Result<ConnectionResource, QuicError> {
  let connecting = quic_incoming_accept(incoming_resource, transport_config)?;
  match connecting.into_0rtt() {
    Ok((conn, zrtt_accepted)) => {
      Ok(ConnectionResource(conn, RefCell::new(Some(zrtt_accepted))))
    }
    Err(_connecting) => {
      unreachable!("0.5-RTT always succeeds");
    }
  }
}

#[op2]
#[serde]
pub(crate) fn op_quic_incoming_refuse(
  #[cppgc] incoming: &IncomingResource,
) -> Result<(), QuicError> {
  let Some(incoming) = incoming.0.borrow_mut().take() else {
    return Err(QuicError::BadResource("QuicIncoming"));
  };
  incoming.refuse();
  Ok(())
}

#[op2]
#[serde]
pub(crate) fn op_quic_incoming_ignore(
  #[cppgc] incoming: &IncomingResource,
) -> Result<(), QuicError> {
  let Some(incoming) = incoming.0.borrow_mut().take() else {
    return Err(QuicError::BadResource("QuicIncoming"));
  };
  incoming.ignore();
  Ok(())
}

struct ConnectingResource(RefCell<Option<quinn::Connecting>>);

impl GarbageCollected for ConnectingResource {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectArgs {
  addr: Addr,
  ca_certs: Option<Vec<String>>,
  alpn_protocols: Option<Vec<String>>,
  server_name: Option<String>,
}

#[op2]
#[cppgc]
pub(crate) fn op_quic_endpoint_connect<NP>(
  state: Rc<RefCell<OpState>>,
  #[cppgc] endpoint: &EndpointResource,
  #[serde] args: ConnectArgs,
  #[serde] transport_config: TransportConfig,
  #[cppgc] key_pair: &TlsKeysHolder,
) -> Result<ConnectingResource, QuicError>
where
  NP: NetPermissions + 'static,
{
  state.borrow_mut().borrow_mut::<NP>().check_net(
    &(&args.addr.hostname, Some(args.addr.port)),
    "Deno.connectQuic()",
  )?;

  let sock_addr = resolve_addr_sync(&args.addr.hostname, args.addr.port)?
    .next()
    .ok_or_else(|| QuicError::UnableToResolve)?;

  let root_cert_store = state
    .borrow()
    .borrow::<DefaultTlsOptions>()
    .root_cert_store()?;

  let unsafely_ignore_certificate_errors = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());

  let ca_certs = args
    .ca_certs
    .unwrap_or_default()
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  let mut tls_config = create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    key_pair.take(),
    SocketUse::GeneralSsl,
  )?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  tls_config.enable_early_data = true;
  tls_config.resumption = Resumption::store(endpoint.session_store.clone());

  let client_config =
    QuicClientConfig::try_from(tls_config).expect("TLS13 supported");
  let mut client_config = quinn::ClientConfig::new(Arc::new(client_config));
  client_config.transport_config(Arc::new(transport_config.try_into()?));

  let connecting = endpoint.endpoint.connect_with(
    client_config,
    sock_addr,
    &args.server_name.unwrap_or(args.addr.hostname),
  )?;

  Ok(ConnectingResource(RefCell::new(Some(connecting))))
}

#[op2(async)]
#[cppgc]
pub(crate) async fn op_quic_connecting_1rtt(
  #[cppgc] connecting: &ConnectingResource,
) -> Result<ConnectionResource, QuicError> {
  let Some(connecting) = connecting.0.borrow_mut().take() else {
    return Err(QuicError::BadResource("QuicConnecting"));
  };
  let conn = connecting.await?;
  Ok(ConnectionResource(conn, RefCell::new(None)))
}

#[op2]
#[cppgc]
pub(crate) fn op_quic_connecting_0rtt(
  #[cppgc] connecting_res: &ConnectingResource,
) -> Option<ConnectionResource> {
  let connecting = connecting_res.0.borrow_mut().take()?;
  match connecting.into_0rtt() {
    Ok((conn, zrtt_accepted)) => {
      Some(ConnectionResource(conn, RefCell::new(Some(zrtt_accepted))))
    }
    Err(connecting) => {
      *connecting_res.0.borrow_mut() = Some(connecting);
      None
    }
  }
}

#[op2]
#[string]
pub(crate) fn op_quic_connection_get_protocol(
  #[cppgc] connection: &ConnectionResource,
) -> Option<String> {
  connection
    .0
    .handshake_data()
    .and_then(|h| h.downcast::<quinn::crypto::rustls::HandshakeData>().ok())
    .and_then(|h| h.protocol)
    .map(|p| String::from_utf8_lossy(&p).into_owned())
}

#[op2]
#[string]
pub(crate) fn op_quic_connection_get_server_name(
  #[cppgc] connection: &ConnectionResource,
) -> Option<String> {
  connection
    .0
    .handshake_data()
    .and_then(|h| h.downcast::<quinn::crypto::rustls::HandshakeData>().ok())
    .and_then(|h| h.server_name)
}

#[op2]
#[serde]
pub(crate) fn op_quic_connection_get_remote_addr(
  #[cppgc] connection: &ConnectionResource,
) -> Result<Addr, QuicError> {
  let addr = connection.0.remote_address();
  Ok(Addr {
    hostname: format!("{}", addr.ip()),
    port: addr.port(),
  })
}

#[op2(fast)]
pub(crate) fn op_quic_connection_close(
  #[cppgc] connection: &ConnectionResource,
  #[bigint] close_code: u64,
  #[string] reason: String,
) -> Result<(), QuicError> {
  connection
    .0
    .close(quinn::VarInt::from_u64(close_code)?, reason.as_bytes());
  Ok(())
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_connection_closed(
  #[cppgc] connection: &ConnectionResource,
) -> Result<CloseInfo, QuicError> {
  let e = connection.0.closed().await;
  match e {
    quinn::ConnectionError::LocallyClosed => Ok(CloseInfo {
      close_code: 0,
      reason: "".into(),
    }),
    quinn::ConnectionError::ApplicationClosed(i) => Ok(CloseInfo {
      close_code: i.error_code.into(),
      reason: String::from_utf8_lossy(&i.reason).into_owned(),
    }),
    e => Err(e.into()),
  }
}

#[op2(async)]
pub(crate) async fn op_quic_connection_handshake(
  #[cppgc] connection: &ConnectionResource,
) {
  let Some(zrtt_accepted) = connection.1.borrow_mut().take() else {
    return;
  };
  zrtt_accepted.await;
}

struct SendStreamResource {
  stream: AsyncRefCell<quinn::SendStream>,
  stream_id: quinn::StreamId,
  priority: AtomicI32,
}

impl SendStreamResource {
  fn new(stream: quinn::SendStream) -> Self {
    Self {
      stream_id: stream.id(),
      priority: AtomicI32::new(stream.priority().unwrap_or(0)),
      stream: AsyncRefCell::new(stream),
    }
  }
}

impl Resource for SendStreamResource {
  fn name(&self) -> Cow<str> {
    "quicSendStream".into()
  }

  fn write(self: Rc<Self>, view: BufView) -> AsyncResult<WriteOutcome> {
    Box::pin(async move {
      let mut stream =
        RcRef::map(self.clone(), |r| &r.stream).borrow_mut().await;
      stream
        .set_priority(self.priority.load(Ordering::Relaxed))
        .map_err(|e| JsErrorBox::from_err(std::io::Error::from(e)))?;
      let nwritten = stream
        .write(&view)
        .await
        .map_err(|e| JsErrorBox::from_err(std::io::Error::from(e)))?;
      Ok(WriteOutcome::Partial { nwritten, view })
    })
  }

  fn close(self: Rc<Self>) {}
}

struct RecvStreamResource {
  stream: AsyncRefCell<quinn::RecvStream>,
  stream_id: quinn::StreamId,
}

impl RecvStreamResource {
  fn new(stream: quinn::RecvStream) -> Self {
    Self {
      stream_id: stream.id(),
      stream: AsyncRefCell::new(stream),
    }
  }
}

impl Resource for RecvStreamResource {
  fn name(&self) -> Cow<str> {
    "quicReceiveStream".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      let mut r = RcRef::map(self, |r| &r.stream).borrow_mut().await;
      let mut data = vec![0; limit];
      let nread = r
        .read(&mut data)
        .await
        .map_err(|e| JsErrorBox::from_err(std::io::Error::from(e)))?
        .unwrap_or(0);
      data.truncate(nread);
      Ok(BufView::from(data))
    })
  }

  fn read_byob(
    self: Rc<Self>,
    mut buf: BufMutView,
  ) -> AsyncResult<(usize, BufMutView)> {
    Box::pin(async move {
      let mut r = RcRef::map(self, |r| &r.stream).borrow_mut().await;
      let nread = r
        .read(&mut buf)
        .await
        .map_err(|e| JsErrorBox::from_err(std::io::Error::from(e)))?
        .unwrap_or(0);
      Ok((nread, buf))
    })
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(async move {
      let mut r = RcRef::map(self, |r| &r.stream).borrow_mut().await;
      r.stop(quinn::VarInt::from(0u32))
        .map_err(|e| JsErrorBox::from_err(std::io::Error::from(e)))?;
      Ok(())
    })
  }
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_connection_accept_bi(
  #[cppgc] connection: &ConnectionResource,
  state: Rc<RefCell<OpState>>,
) -> Result<(ResourceId, ResourceId), QuicError> {
  match connection.0.accept_bi().await {
    Ok((tx, rx)) => {
      let mut state = state.borrow_mut();
      let tx_rid = state.resource_table.add(SendStreamResource::new(tx));
      let rx_rid = state.resource_table.add(RecvStreamResource::new(rx));
      Ok((tx_rid, rx_rid))
    }
    Err(e) => match e {
      quinn::ConnectionError::LocallyClosed
      | quinn::ConnectionError::ApplicationClosed(..) => {
        Err(QuicError::BadResource("QuicConnection"))
      }
      _ => Err(e.into()),
    },
  }
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_connection_open_bi(
  #[cppgc] connection: &ConnectionResource,
  state: Rc<RefCell<OpState>>,
  wait_for_available: bool,
) -> Result<(ResourceId, ResourceId), QuicError> {
  let (tx, rx) = if wait_for_available {
    connection.0.open_bi().await?
  } else {
    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    match pin!(connection.0.open_bi()).poll(&mut cx) {
      Poll::Ready(r) => r?,
      Poll::Pending => {
        return Err(QuicError::MaxStreams("bidirectional"));
      }
    }
  };
  let mut state = state.borrow_mut();
  let tx_rid = state.resource_table.add(SendStreamResource::new(tx));
  let rx_rid = state.resource_table.add(RecvStreamResource::new(rx));
  Ok((tx_rid, rx_rid))
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_connection_accept_uni(
  #[cppgc] connection: &ConnectionResource,
  state: Rc<RefCell<OpState>>,
) -> Result<ResourceId, QuicError> {
  match connection.0.accept_uni().await {
    Ok(rx) => {
      let rid = state
        .borrow_mut()
        .resource_table
        .add(RecvStreamResource::new(rx));
      Ok(rid)
    }
    Err(e) => match e {
      quinn::ConnectionError::LocallyClosed
      | quinn::ConnectionError::ApplicationClosed(..) => {
        Err(QuicError::BadResource("QuicConnection"))
      }
      _ => Err(e.into()),
    },
  }
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_connection_open_uni(
  #[cppgc] connection: &ConnectionResource,
  state: Rc<RefCell<OpState>>,
  wait_for_available: bool,
) -> Result<ResourceId, QuicError> {
  let tx = if wait_for_available {
    connection.0.open_uni().await?
  } else {
    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    match pin!(connection.0.open_uni()).poll(&mut cx) {
      Poll::Ready(r) => r?,
      Poll::Pending => {
        return Err(QuicError::MaxStreams("unidirectional"));
      }
    }
  };
  let rid = state
    .borrow_mut()
    .resource_table
    .add(SendStreamResource::new(tx));
  Ok(rid)
}

#[op2(async)]
pub(crate) async fn op_quic_connection_send_datagram(
  #[cppgc] connection: &ConnectionResource,
  #[buffer] buf: JsBuffer,
) -> Result<(), QuicError> {
  connection.0.send_datagram_wait(buf.to_vec().into()).await?;
  Ok(())
}

#[op2(async)]
#[buffer]
pub(crate) async fn op_quic_connection_read_datagram(
  #[cppgc] connection: &ConnectionResource,
) -> Result<Vec<u8>, QuicError> {
  let data = connection.0.read_datagram().await?;
  Ok(data.into())
}

#[op2(fast)]
pub(crate) fn op_quic_connection_get_max_datagram_size(
  #[cppgc] connection: &ConnectionResource,
) -> u32 {
  connection.0.max_datagram_size().unwrap_or(0) as _
}

#[op2(fast)]
pub(crate) fn op_quic_send_stream_get_priority(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<i32, ResourceError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<SendStreamResource>(rid)?;
  Ok(resource.priority.load(Ordering::Relaxed))
}

#[op2(fast)]
pub(crate) fn op_quic_send_stream_set_priority(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  priority: i32,
) -> Result<(), ResourceError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<SendStreamResource>(rid)?;
  resource.priority.store(priority, Ordering::Relaxed);
  Ok(())
}

#[op2(fast)]
#[bigint]
pub(crate) fn op_quic_send_stream_get_id(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<u64, ResourceError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<SendStreamResource>(rid)?;
  let stream_id = quinn::VarInt::from(resource.stream_id).into_inner();
  Ok(stream_id)
}

#[op2(fast)]
#[bigint]
pub(crate) fn op_quic_recv_stream_get_id(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<u64, ResourceError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<RecvStreamResource>(rid)?;
  let stream_id = quinn::VarInt::from(resource.stream_id).into_inner();
  Ok(stream_id)
}
