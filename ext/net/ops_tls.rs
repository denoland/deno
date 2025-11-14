// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::fs::File;
use std::io::BufReader;
use std::io::ErrorKind;
use std::io::Read;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::futures::TryFutureExt;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use deno_tls::ServerConfigProvider;
use deno_tls::SocketUse;
use deno_tls::TlsClientConfigOptions;
use deno_tls::TlsKey;
use deno_tls::TlsKeyLookup;
use deno_tls::TlsKeys;
use deno_tls::TlsKeysHolder;
use deno_tls::create_client_config;
use deno_tls::load_certs;
use deno_tls::load_private_keys;
use deno_tls::new_resolver;
use deno_tls::rustls::ClientConnection;
use deno_tls::rustls::ServerConfig;
use deno_tls::rustls::pki_types::ServerName;
pub use rustls_tokio_stream::TlsStream;
pub use rustls_tokio_stream::TlsStreamRead;
pub use rustls_tokio_stream::TlsStreamWrite;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::DefaultTlsOptions;
use crate::UnsafelyIgnoreCertificateErrors;
use crate::io::TcpStreamResource;
use crate::ops::IpAddr;
use crate::ops::NetError;
use crate::ops::TlsHandshakeInfo;
use crate::raw::NetworkListenerResource;
use crate::resolve_addr::resolve_addr;
use crate::resolve_addr::resolve_addr_sync;
use crate::tcp::TcpListener;

pub(crate) const TLS_BUFFER_SIZE: Option<NonZeroUsize> =
  NonZeroUsize::new(65536);

pub struct TlsListener {
  pub(crate) tcp_listener: TcpListener,
  pub(crate) tls_config: Option<Arc<ServerConfig>>,
  pub(crate) server_config_provider: Option<ServerConfigProvider>,
}

impl TlsListener {
  pub async fn accept(
    &self,
  ) -> std::io::Result<(TlsStream<TcpStream>, SocketAddr)> {
    let (tcp, addr) = self.tcp_listener.accept().await?;
    let tls = if let Some(provider) = &self.server_config_provider {
      TlsStream::new_server_side_acceptor(
        tcp,
        provider.clone(),
        TLS_BUFFER_SIZE,
      )
    } else {
      TlsStream::new_server_side(
        tcp,
        self.tls_config.clone().unwrap(),
        TLS_BUFFER_SIZE,
      )
    };
    Ok((tls, addr))
  }
  pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
    self.tcp_listener.local_addr()
  }
}

#[derive(Debug)]
enum TlsStreamInner {
  Tcp {
    rd: AsyncRefCell<TlsStreamRead<TcpStream>>,
    wr: AsyncRefCell<TlsStreamWrite<TcpStream>>,
  },
}

#[derive(Debug)]
pub struct TlsStreamResource {
  inner: TlsStreamInner,
  // `None` when a TLS handshake hasn't been done.
  handshake_info: RefCell<Option<TlsHandshakeInfo>>,
  cancel_handle: CancelHandle, // Only read and handshake ops get canceled.
}

impl TlsStreamResource {
  pub fn new_tcp(
    (rd, wr): (TlsStreamRead<TcpStream>, TlsStreamWrite<TcpStream>),
  ) -> Self {
    Self {
      inner: TlsStreamInner::Tcp {
        rd: AsyncRefCell::new(rd),
        wr: AsyncRefCell::new(wr),
      },
      handshake_info: RefCell::new(None),
      cancel_handle: Default::default(),
    }
  }

  pub fn into_tls_stream(self) -> TlsStream<TcpStream> {
    match self.inner {
      TlsStreamInner::Tcp { rd, wr } => {
        let read_half = rd.into_inner();
        let write_half = wr.into_inner();
        read_half.unsplit(write_half)
      }
    }
  }

  pub fn peer_certificates(
    &self,
  ) -> Option<
    Vec<rustls_tokio_stream::rustls::pki_types::CertificateDer<'static>>,
  > {
    self
      .handshake_info
      .borrow()
      .as_ref()
      .and_then(|info| info.peer_certificates.clone())
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    let mut rd = RcRef::map(&self, |r| match r.inner {
      TlsStreamInner::Tcp { ref rd, .. } => rd,
    })
    .borrow_mut()
    .await;
    let cancel_handle = RcRef::map(&self, |r| &r.cancel_handle);
    rd.read(data).try_or_cancel(cancel_handle).await
  }

  pub async fn write(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, std::io::Error> {
    let mut wr = RcRef::map(&self, |r| match r.inner {
      TlsStreamInner::Tcp { ref wr, .. } => wr,
    })
    .borrow_mut()
    .await;
    let nwritten = wr.write(data).await?;
    wr.flush().await?;
    Ok(nwritten)
  }

  pub async fn shutdown(self: Rc<Self>) -> Result<(), std::io::Error> {
    let mut wr = RcRef::map(&self, |r| match r.inner {
      TlsStreamInner::Tcp { ref wr, .. } => wr,
    })
    .borrow_mut()
    .await;
    wr.shutdown().await?;
    Ok(())
  }

  pub async fn handshake(
    self: &Rc<Self>,
  ) -> Result<TlsHandshakeInfo, std::io::Error> {
    if let Some(tls_info) = &*self.handshake_info.borrow() {
      return Ok(tls_info.clone());
    }

    let mut wr = RcRef::map(self, |r| match r.inner {
      TlsStreamInner::Tcp { ref wr, .. } => wr,
    })
    .borrow_mut()
    .await;
    let cancel_handle = RcRef::map(self, |r| &r.cancel_handle);
    let handshake = wr.handshake().try_or_cancel(cancel_handle).await?;

    let alpn_protocol = handshake.alpn.map(|alpn| alpn.into());
    let peer_certificates = handshake.peer_certificates.clone();
    let tls_info = TlsHandshakeInfo {
      alpn_protocol,
      peer_certificates,
    };
    self.handshake_info.replace(Some(tls_info.clone()));
    Ok(tls_info)
  }
}

impl Resource for TlsStreamResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<'_, str> {
    "tlsStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown().map_err(JsErrorBox::from_err))
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectTlsArgs {
  cert_file: Option<String>,
  ca_certs: Vec<String>,
  alpn_protocols: Option<Vec<String>>,
  server_name: Option<String>,
  unsafely_disable_hostname_verification: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTlsArgs {
  rid: ResourceId,
  ca_certs: Vec<String>,
  hostname: String,
  alpn_protocols: Option<Vec<String>>,
  reject_unauthorized: Option<bool>,
  unsafely_disable_hostname_verification: Option<bool>,
}

#[op2]
#[cppgc]
pub fn op_tls_key_null() -> TlsKeysHolder {
  TlsKeysHolder::from(TlsKeys::Null)
}

#[op2(reentrant)]
#[cppgc]
pub fn op_tls_key_static(
  #[string] cert: &str,
  #[string] key: &str,
) -> Result<TlsKeysHolder, deno_tls::TlsError> {
  let cert = load_certs(&mut BufReader::new(cert.as_bytes()))?;
  let key = load_private_keys(key.as_bytes())?
    .into_iter()
    .next()
    .unwrap();
  Ok(TlsKeysHolder::from(TlsKeys::Static(TlsKey(cert, key))))
}

#[op2]
pub fn op_tls_cert_resolver_create<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Array> {
  let (resolver, lookup) = new_resolver();
  let resolver = deno_core::cppgc::make_cppgc_object(
    scope,
    TlsKeysHolder::from(TlsKeys::Resolver(resolver)),
  );
  let lookup = deno_core::cppgc::make_cppgc_object(scope, lookup);
  v8::Array::new_with_elements(scope, &[resolver.into(), lookup.into()])
}

#[op2(async)]
#[string]
pub async fn op_tls_cert_resolver_poll(
  #[cppgc] lookup: &TlsKeyLookup,
) -> Option<String> {
  lookup.poll().await
}

#[op2(fast)]
pub fn op_tls_cert_resolver_resolve(
  #[cppgc] lookup: &TlsKeyLookup,
  #[string] sni: String,
  #[cppgc] key: &TlsKeysHolder,
) -> Result<(), NetError> {
  let TlsKeys::Static(key) = key.take() else {
    return Err(NetError::UnexpectedKeyType);
  };
  lookup.resolve(sni, Ok(key));
  Ok(())
}

#[op2(fast)]
pub fn op_tls_cert_resolver_resolve_error(
  #[cppgc] lookup: &TlsKeyLookup,
  #[string] sni: String,
  #[string] error: String,
) {
  lookup.resolve(sni, Err(error))
}

#[op2(stack_trace)]
#[serde]
pub fn op_tls_start(
  state: Rc<RefCell<OpState>>,
  #[serde] args: StartTlsArgs,
  #[cppgc] key_pair: Option<&TlsKeysHolder>,
) -> Result<(ResourceId, IpAddr, IpAddr), NetError> {
  let rid = args.rid;
  let reject_unauthorized = args.reject_unauthorized.unwrap_or(true);
  let hostname = match &*args.hostname {
    "" => "localhost".to_string(),
    n => n.to_string(),
  };

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

  let unsafely_disable_hostname_verification =
    args.unsafely_disable_hostname_verification.unwrap_or(false);

  let root_cert_store = state
    .borrow()
    .borrow::<DefaultTlsOptions>()
    .root_cert_store()
    .map_err(NetError::RootCertStore)?;

  let resource_rc = state
    .borrow_mut()
    .resource_table
    .take::<TcpStreamResource>(rid)
    .map_err(NetError::Resource)?;
  // This TCP connection might be used somewhere else. If it's the case, we cannot proceed with the
  // process of starting a TLS connection on top of this TCP connection, so we just return a Busy error.
  // See also: https://github.com/denoland/deno/pull/16242
  let resource =
    Rc::try_unwrap(resource_rc).map_err(|_| NetError::TcpStreamBusy)?;
  let (read_half, write_half) = resource.into_inner();
  let tcp_stream = read_half.reunite(write_half).map_err(NetError::Reunite)?;

  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let tls_null = TlsKeysHolder::from(TlsKeys::Null);
  let key_pair = key_pair.unwrap_or(&tls_null);
  let mut tls_config = create_client_config(TlsClientConfigOptions {
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    unsafely_disable_hostname_verification,
    cert_chain_and_key: key_pair.take(),
    socket_use: SocketUse::GeneralSsl,
  })?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let tls_config = Arc::new(tls_config);
  let tls_stream = TlsStream::new_client_side(
    tcp_stream,
    ClientConnection::new(tls_config, hostname_dns)?,
    TLS_BUFFER_SIZE,
  );

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new_tcp(tls_stream.into_split()))
  };

  Ok((rid, IpAddr::from(local_addr), IpAddr::from(remote_addr)))
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_net_connect_tls(
  state: Rc<RefCell<OpState>>,
  #[serde] addr: IpAddr,
  #[serde] args: ConnectTlsArgs,
  #[cppgc] key_pair: &TlsKeysHolder,
) -> Result<(ResourceId, IpAddr, IpAddr), NetError> {
  let cert_file = args.cert_file.as_deref();
  let unsafely_ignore_certificate_errors = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());
  let unsafely_disable_hostname_verification =
    args.unsafely_disable_hostname_verification.unwrap_or(false);

  let cert_file = {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<PermissionsContainer>();
    permissions
      .check_net(&(&addr.hostname, Some(addr.port)), "Deno.connectTls()")
      .map_err(NetError::Permission)?;
    if let Some(path) = cert_file {
      Some(
        permissions
          .check_open(
            Cow::Borrowed(Path::new(path)),
            OpenAccessKind::ReadNoFollow,
            Some("Deno.connectTls()"),
          )
          .map_err(NetError::Permission)?,
      )
    } else {
      None
    }
  };

  let mut ca_certs = args
    .ca_certs
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  if let Some(path) = cert_file {
    let mut buf = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    ca_certs.push(buf);
  };

  let root_cert_store = state
    .borrow()
    .borrow::<DefaultTlsOptions>()
    .root_cert_store()
    .map_err(NetError::RootCertStore)?;
  let hostname_dns = if let Some(server_name) = args.server_name {
    ServerName::try_from(server_name)
  } else {
    ServerName::try_from(addr.hostname.clone())
  }
  .map_err(|_| NetError::InvalidHostname(addr.hostname.clone()))?;
  let connect_addr = resolve_addr(&addr.hostname, addr.port)
    .await?
    .next()
    .ok_or_else(|| NetError::NoResolvedAddress)?;
  let tcp_stream = TcpStream::connect(connect_addr).await?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let mut tls_config = create_client_config(TlsClientConfigOptions {
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    unsafely_disable_hostname_verification,
    cert_chain_and_key: key_pair.take(),
    socket_use: SocketUse::GeneralSsl,
  })?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let tls_config = Arc::new(tls_config);

  let tls_stream = TlsStream::new_client_side(
    tcp_stream,
    ClientConnection::new(tls_config, hostname_dns)?,
    TLS_BUFFER_SIZE,
  );

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new_tcp(tls_stream.into_split()))
  };

  Ok((rid, IpAddr::from(local_addr), IpAddr::from(remote_addr)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenTlsArgs {
  alpn_protocols: Option<Vec<String>>,
  reuse_port: bool,
  #[serde(default)]
  load_balanced: bool,
  tcp_backlog: i32,
}

#[op2(stack_trace)]
#[serde]
pub fn op_net_listen_tls(
  state: &mut OpState,
  #[serde] addr: IpAddr,
  #[serde] args: ListenTlsArgs,
  #[cppgc] keys: &TlsKeysHolder,
) -> Result<(ResourceId, IpAddr), NetError> {
  if args.reuse_port {
    super::check_unstable(state, "Deno.listenTls({ reusePort: true })");
  }

  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions
      .check_net(&(&addr.hostname, Some(addr.port)), "Deno.listenTls()")
      .map_err(NetError::Permission)?;
  }

  let bind_addr = resolve_addr_sync(&addr.hostname, addr.port)?
    .next()
    .ok_or(NetError::NoResolvedAddress)?;

  let tcp_listener = if args.load_balanced {
    TcpListener::bind_load_balanced(bind_addr, args.tcp_backlog)
  } else {
    TcpListener::bind_direct(bind_addr, args.reuse_port, args.tcp_backlog)
  }?;
  let local_addr = tcp_listener.local_addr()?;
  let alpn = args
    .alpn_protocols
    .unwrap_or_default()
    .into_iter()
    .map(|s| s.into_bytes())
    .collect();
  let listener = match keys.take() {
    TlsKeys::Null => return Err(NetError::ListenTlsRequiresKey),
    TlsKeys::Static(TlsKey(cert, key)) => {
      let mut tls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)?;
      tls_config.alpn_protocols = alpn;
      TlsListener {
        tcp_listener,
        tls_config: Some(tls_config.into()),
        server_config_provider: None,
      }
    }
    TlsKeys::Resolver(resolver) => TlsListener {
      tcp_listener,
      tls_config: None,
      server_config_provider: Some(resolver.into_server_config_provider(alpn)),
    },
  };

  let tls_listener_resource = NetworkListenerResource::new(listener);

  let rid = state.resource_table.add(tls_listener_resource);

  Ok((rid, IpAddr::from(local_addr)))
}

#[op2(async)]
#[serde]
pub async fn op_net_accept_tls(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(ResourceId, IpAddr, IpAddr), NetError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NetworkListenerResource<TlsListener>>(rid)
    .map_err(|_| NetError::ListenerClosed)?;

  let cancel_handle = RcRef::map(&resource, |r| &r.cancel);
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or_else(|| NetError::AcceptTaskOngoing)?;

  let (tls_stream, remote_addr) =
    match listener.accept().try_or_cancel(&cancel_handle).await {
      Ok(tuple) => tuple,
      Err(err) if err.kind() == ErrorKind::Interrupted => {
        return Err(NetError::ListenerClosed);
      }
      Err(err) => return Err(err.into()),
    };

  let local_addr = tls_stream.local_addr()?;
  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new_tcp(tls_stream.into_split()))
  };

  Ok((rid, IpAddr::from(local_addr), IpAddr::from(remote_addr)))
}

#[op2(async)]
#[serde]
pub async fn op_tls_handshake(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<TlsHandshakeInfo, NetError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TlsStreamResource>(rid)
    .map_err(|_| NetError::ListenerClosed)?;
  resource.handshake().await.map_err(Into::into)
}
