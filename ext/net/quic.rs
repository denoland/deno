// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::resolve_addr::resolve_addr;
use crate::DefaultTlsOptions;
use crate::NetPermissions;
use crate::UnsafelyIgnoreCertificateErrors;
use deno_core::error::bad_resource;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::task::noop_waker_ref;
use deno_core::op2;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::GarbageCollected;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::WriteOutcome;
use deno_tls::create_client_config;
use deno_tls::SocketUse;
use deno_tls::TlsKeys;
use deno_tls::TlsKeysHolder;
use quinn::crypto::rustls::QuicClientConfig;
use quinn::crypto::rustls::QuicServerConfig;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddrV4;
use std::net::SocketAddrV6;
use std::pin::pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransportConfig {
  keep_alive_interval: Option<u64>,
  max_idle_timeout: Option<u64>,
  max_concurrent_bidirectional_streams: Option<u32>,
  max_concurrent_unidirectional_streams: Option<u32>,
  preferred_address_v4: Option<SocketAddrV4>,
  preferred_address_v6: Option<SocketAddrV6>,
}

impl TryInto<quinn::TransportConfig> for TransportConfig {
  type Error = AnyError;

  fn try_into(self) -> Result<quinn::TransportConfig, AnyError> {
    let mut cfg = quinn::TransportConfig::default();

    if let Some(interval) = self.keep_alive_interval {
      cfg.keep_alive_interval(Some(Duration::from_millis(interval)));
    }

    if let Some(timeout) = self.max_idle_timeout {
      cfg.max_idle_timeout(Some(Duration::from_millis(timeout).try_into()?));
    }

    if let Some(max) = self.max_concurrent_bidirectional_streams {
      cfg.max_concurrent_bidi_streams(max.into());
    }

    if let Some(max) = self.max_concurrent_unidirectional_streams {
      cfg.max_concurrent_uni_streams(max.into());
    }

    Ok(cfg)
  }
}

struct EndpointResource(quinn::Endpoint, Arc<QuicServerConfig>);

impl GarbageCollected for EndpointResource {}

#[op2(async)]
#[cppgc]
pub(crate) async fn op_quic_listen<NP>(
  state: Rc<RefCell<OpState>>,
  #[serde] addr: Addr,
  #[serde] args: ListenArgs,
  #[serde] transport_config: TransportConfig,
  #[cppgc] keys: &TlsKeysHolder,
) -> Result<EndpointResource, AnyError>
where
  NP: NetPermissions + 'static,
{
  state
    .borrow_mut()
    .borrow_mut::<NP>()
    .check_net(&(&addr.hostname, Some(addr.port)), "Deno.listenQuic()")?;

  let addr = resolve_addr(&addr.hostname, addr.port)
    .await?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;

  let TlsKeys::Static(deno_tls::TlsKey(cert, key)) = keys.take() else {
    unreachable!()
  };

  let mut crypto =
    quinn::rustls::ServerConfig::builder_with_protocol_versions(&[
      &quinn::rustls::version::TLS13,
    ])
    .with_no_client_auth()
    .with_single_cert(cert.clone(), key.clone_key())?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    crypto.alpn_protocols = alpn_protocols
      .into_iter()
      .map(|alpn| alpn.into_bytes())
      .collect();
  }

  let server_config = Arc::new(QuicServerConfig::try_from(crypto)?);
  let mut config = quinn::ServerConfig::with_crypto(server_config.clone());
  config.preferred_address_v4(transport_config.preferred_address_v4);
  config.preferred_address_v6(transport_config.preferred_address_v6);
  config.transport_config(Arc::new(transport_config.try_into()?));
  let endpoint = quinn::Endpoint::server(config, addr)?;

  Ok(EndpointResource(endpoint, server_config))
}

#[op2]
#[serde]
pub(crate) fn op_quic_endpoint_get_addr(
  #[cppgc] endpoint: &EndpointResource,
) -> Result<Addr, AnyError> {
  let addr = endpoint.0.local_addr()?;
  let addr = Addr {
    hostname: format!("{}", addr.ip()),
    port: addr.port(),
  };
  Ok(addr)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseInfo {
  close_code: u64,
  reason: String,
}

#[op2(fast)]
pub(crate) fn op_quic_close_endpoint(
  #[cppgc] endpoint: &EndpointResource,
  #[bigint] close_code: u64,
  #[string] reason: String,
) -> Result<(), AnyError> {
  endpoint
    .0
    .close(quinn::VarInt::from_u64(close_code)?, reason.as_bytes());
  Ok(())
}

struct ConnectionResource(quinn::Connection);

impl GarbageCollected for ConnectionResource {}

#[op2(async)]
#[cppgc]
pub(crate) async fn op_quic_accept(
  #[cppgc] endpoint: &EndpointResource,
) -> Result<ConnectionResource, AnyError> {
  match endpoint.0.accept().await {
    Some(incoming) => {
      let conn = incoming.accept()?.await?;
      Ok(ConnectionResource(conn))
    }
    None => Err(bad_resource("QuicListener is closed")),
  }
}

struct IncomingResource(
  RefCell<Option<quinn::Incoming>>,
  Arc<QuicServerConfig>,
);

impl GarbageCollected for IncomingResource {}

#[op2(async)]
#[cppgc]
pub(crate) async fn op_quic_accept_incoming(
  #[cppgc] endpoint: &EndpointResource,
) -> Result<IncomingResource, AnyError> {
  match endpoint.0.accept().await {
    Some(incoming) => Ok(IncomingResource(
      RefCell::new(Some(incoming)),
      endpoint.1.clone(),
    )),
    None => Err(bad_resource("QuicListener is closed")),
  }
}

#[op2(async)]
#[cppgc]
pub(crate) async fn op_quic_incoming_accept(
  #[cppgc] incoming_resource: &IncomingResource,
  #[serde] transport_config: Option<TransportConfig>,
) -> Result<ConnectionResource, AnyError> {
  let Some(incoming) = incoming_resource.0.borrow_mut().take() else {
    return Err(bad_resource("QuicIncoming already used"));
  };
  let conn = match transport_config {
    Some(transport_config) => {
      let mut config =
        quinn::ServerConfig::with_crypto(incoming_resource.1.clone());
      config.preferred_address_v4(transport_config.preferred_address_v4);
      config.preferred_address_v6(transport_config.preferred_address_v6);
      config.transport_config(Arc::new(transport_config.try_into()?));
      incoming.accept_with(Arc::new(config))?.await?
    }
    None => incoming.accept()?.await?,
  };
  Ok(ConnectionResource(conn))
}

#[op2]
#[serde]
pub(crate) fn op_quic_incoming_refuse(
  #[cppgc] incoming: &IncomingResource,
) -> Result<(), AnyError> {
  let Some(incoming) = incoming.0.borrow_mut().take() else {
    return Err(bad_resource("QuicIncoming already used"));
  };
  incoming.refuse();
  Ok(())
}

#[op2]
#[serde]
pub(crate) fn op_quic_incoming_ignore(
  #[cppgc] incoming: &IncomingResource,
) -> Result<(), AnyError> {
  let Some(incoming) = incoming.0.borrow_mut().take() else {
    return Err(bad_resource("QuicIncoming already used"));
  };
  incoming.ignore();
  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectArgs {
  ca_certs: Option<Vec<String>>,
  alpn_protocols: Option<Vec<String>>,
  server_name: Option<String>,
}

#[op2(async)]
#[cppgc]
pub(crate) async fn op_quic_connect<NP>(
  state: Rc<RefCell<OpState>>,
  #[serde] addr: Addr,
  #[serde] args: ConnectArgs,
  #[serde] transport_config: TransportConfig,
  #[cppgc] key_pair: &TlsKeysHolder,
) -> Result<ConnectionResource, AnyError>
where
  NP: NetPermissions + 'static,
{
  state
    .borrow_mut()
    .borrow_mut::<NP>()
    .check_net(&(&addr.hostname, Some(addr.port)), "Deno.connectQuic()")?;

  let sock_addr = resolve_addr(&addr.hostname, addr.port)
    .await?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;

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

  let client_config = QuicClientConfig::try_from(tls_config)?;
  let mut client_config = quinn::ClientConfig::new(Arc::new(client_config));
  client_config.transport_config(Arc::new(transport_config.try_into()?));

  let local_addr = match sock_addr.ip() {
    IpAddr::V4(_) => IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)),
    IpAddr::V6(_) => IpAddr::from(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
  };

  let conn = quinn::Endpoint::client((local_addr, 0).into())?
    .connect_with(
      client_config,
      sock_addr,
      &args.server_name.unwrap_or(addr.hostname),
    )?
    .await?;

  Ok(ConnectionResource(conn))
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
#[serde]
pub(crate) fn op_quic_connection_get_remote_addr(
  #[cppgc] connection: &ConnectionResource,
) -> Result<Addr, AnyError> {
  let addr = connection.0.remote_address();
  Ok(Addr {
    hostname: format!("{}", addr.ip()),
    port: addr.port(),
  })
}

#[op2(fast)]
pub(crate) fn op_quic_close_connection(
  #[cppgc] connection: &ConnectionResource,
  #[bigint] close_code: u64,
  #[string] reason: String,
) -> Result<(), AnyError> {
  connection
    .0
    .close(quinn::VarInt::from_u64(close_code)?, reason.as_bytes());
  Ok(())
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_connection_closed(
  #[cppgc] connection: &ConnectionResource,
) -> Result<CloseInfo, AnyError> {
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

struct SendStreamResource(AsyncRefCell<quinn::SendStream>);

impl SendStreamResource {
  fn new(stream: quinn::SendStream) -> Self {
    Self(AsyncRefCell::new(stream))
  }
}

impl Resource for SendStreamResource {
  fn name(&self) -> Cow<str> {
    "quicSendStream".into()
  }

  fn write(self: Rc<Self>, view: BufView) -> AsyncResult<WriteOutcome> {
    Box::pin(async move {
      let mut r = RcRef::map(self, |r| &r.0).borrow_mut().await;
      let nwritten = r.write(&view).await?;
      Ok(WriteOutcome::Partial { nwritten, view })
    })
  }
}

struct RecvStreamResource(AsyncRefCell<quinn::RecvStream>);

impl RecvStreamResource {
  fn new(stream: quinn::RecvStream) -> Self {
    Self(AsyncRefCell::new(stream))
  }
}

impl Resource for RecvStreamResource {
  fn name(&self) -> Cow<str> {
    "quicReceiveStream".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      let mut r = RcRef::map(self, |r| &r.0).borrow_mut().await;
      let mut data = vec![0; limit];
      let nread = r.read(&mut data).await?.unwrap_or(0);
      data.truncate(nread);
      Ok(BufView::from(data))
    })
  }
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_accept_bi(
  #[cppgc] connection: &ConnectionResource,
  state: Rc<RefCell<OpState>>,
) -> Result<(ResourceId, ResourceId), AnyError> {
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
        Err(bad_resource("QuicConn is closed"))
      }
      _ => Err(e.into()),
    },
  }
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_open_bi(
  #[cppgc] connection: &ConnectionResource,
  state: Rc<RefCell<OpState>>,
  wait_for_available: bool,
) -> Result<(ResourceId, ResourceId), AnyError> {
  let (tx, rx) = if wait_for_available {
    connection.0.open_bi().await?
  } else {
    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    match pin!(connection.0.open_bi()).poll(&mut cx) {
      Poll::Ready(r) => r?,
      Poll::Pending => {
        return Err(generic_error("Connection has reached the maximum number of outgoing concurrent bidirectional streams"));
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
pub(crate) async fn op_quic_accept_uni(
  #[cppgc] connection: &ConnectionResource,
  state: Rc<RefCell<OpState>>,
) -> Result<ResourceId, AnyError> {
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
        Err(bad_resource("QuicConn is closed"))
      }
      _ => Err(e.into()),
    },
  }
}

#[op2(async)]
#[serde]
pub(crate) async fn op_quic_open_uni(
  #[cppgc] connection: &ConnectionResource,
  state: Rc<RefCell<OpState>>,
  wait_for_available: bool,
) -> Result<ResourceId, AnyError> {
  let tx = if wait_for_available {
    connection.0.open_uni().await?
  } else {
    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    match pin!(connection.0.open_uni()).poll(&mut cx) {
      Poll::Ready(r) => r?,
      Poll::Pending => {
        return Err(generic_error("Connection has reached the maximum number of outgoing concurrent unidirectional streams"));
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
pub(crate) async fn op_quic_send_datagram(
  #[cppgc] connection: &ConnectionResource,
  #[buffer] buf: JsBuffer,
) -> Result<(), AnyError> {
  connection.0.send_datagram_wait(buf.to_vec().into()).await?;
  Ok(())
}

#[op2(async)]
pub(crate) async fn op_quic_read_datagram(
  #[cppgc] connection: &ConnectionResource,
  #[buffer] mut buf: JsBuffer,
) -> Result<u32, AnyError> {
  let data = connection.0.read_datagram().await?;
  buf[0..data.len()].copy_from_slice(&data);
  Ok(data.len() as _)
}

#[op2(fast)]
pub(crate) fn op_quic_max_datagram_size(
  #[cppgc] connection: &ConnectionResource,
) -> Result<u32, AnyError> {
  Ok(connection.0.max_datagram_size().unwrap_or(0) as _)
}

#[op2(fast)]
pub(crate) fn op_quic_get_send_stream_priority(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<i32, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<SendStreamResource>(rid)?;
  let r = RcRef::map(resource, |r| &r.0).try_borrow();
  match r {
    Some(s) => Ok(s.priority()?),
    None => Err(generic_error("Unable to get priority")),
  }
}

#[op2(fast)]
pub(crate) fn op_quic_set_send_stream_priority(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  priority: i32,
) -> Result<(), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<SendStreamResource>(rid)?;
  let r = RcRef::map(resource, |r| &r.0).try_borrow();
  match r {
    Some(s) => {
      s.set_priority(priority)?;
      Ok(())
    }
    None => Err(generic_error("Unable to set priority")),
  }
}
