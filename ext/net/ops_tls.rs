// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::io::TcpStreamResource;
use crate::ops::IpAddr;
use crate::ops::TlsHandshakeInfo;
use crate::resolve_addr::resolve_addr;
use crate::resolve_addr::resolve_addr_sync;
use crate::DefaultTlsOptions;
use crate::NetPermissions;
use crate::UnsafelyIgnoreCertificateErrors;
use deno_core::error::bad_resource;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::invalid_hostname;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_tls::create_client_config;
use deno_tls::load_certs;
use deno_tls::load_private_keys;
use deno_tls::rustls::Certificate;
use deno_tls::rustls::PrivateKey;
use deno_tls::rustls::ServerConfig;
use deno_tls::rustls::ServerName;
use deno_tls::SocketUse;
use io::Read;
use rustls_tokio_stream::TlsStreamRead;
use rustls_tokio_stream::TlsStreamWrite;
use serde::Deserialize;
use socket2::Domain;
use socket2::Socket;
use socket2::Type;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::convert::TryFrom;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::io::ErrorKind;
use std::num::NonZeroUsize;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

pub use rustls_tokio_stream::TlsStream;

pub(crate) const TLS_BUFFER_SIZE: Option<NonZeroUsize> =
  NonZeroUsize::new(65536);

#[derive(Debug)]
pub struct TlsStreamResource {
  rd: AsyncRefCell<TlsStreamRead>,
  wr: AsyncRefCell<TlsStreamWrite>,
  // `None` when a TLS handshake hasn't been done.
  handshake_info: RefCell<Option<TlsHandshakeInfo>>,
  cancel_handle: CancelHandle, // Only read and handshake ops get canceled.
}

impl TlsStreamResource {
  pub fn new((rd, wr): (TlsStreamRead, TlsStreamWrite)) -> Self {
    Self {
      rd: rd.into(),
      wr: wr.into(),
      handshake_info: RefCell::new(None),
      cancel_handle: Default::default(),
    }
  }

  pub fn into_inner(self) -> (TlsStreamRead, TlsStreamWrite) {
    (self.rd.into_inner(), self.wr.into_inner())
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, AnyError> {
    let mut rd = RcRef::map(&self, |r| &r.rd).borrow_mut().await;
    let cancel_handle = RcRef::map(&self, |r| &r.cancel_handle);
    Ok(rd.read(data).try_or_cancel(cancel_handle).await?)
  }

  pub async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, AnyError> {
    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    let nwritten = wr.write(data).await?;
    wr.flush().await?;
    Ok(nwritten)
  }

  pub async fn shutdown(self: Rc<Self>) -> Result<(), AnyError> {
    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    wr.shutdown().await?;
    Ok(())
  }

  pub async fn handshake(
    self: &Rc<Self>,
  ) -> Result<TlsHandshakeInfo, AnyError> {
    if let Some(tls_info) = &*self.handshake_info.borrow() {
      return Ok(tls_info.clone());
    }

    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    let cancel_handle = RcRef::map(self, |r| &r.cancel_handle);
    let handshake = wr.handshake().try_or_cancel(cancel_handle).await?;

    let alpn_protocol = handshake.alpn.map(|alpn| alpn.into());
    let tls_info = TlsHandshakeInfo { alpn_protocol };
    self.handshake_info.replace(Some(tls_info.clone()));
    Ok(tls_info)
  }
}

impl Resource for TlsStreamResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<str> {
    "tlsStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown())
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
  cert_chain: Option<String>,
  private_key: Option<String>,
  alpn_protocols: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTlsArgs {
  rid: ResourceId,
  ca_certs: Vec<String>,
  hostname: String,
  alpn_protocols: Option<Vec<String>>,
}

#[op2(async)]
#[serde]
pub async fn op_tls_start<NP>(
  state: Rc<RefCell<OpState>>,
  #[serde] args: StartTlsArgs,
) -> Result<(ResourceId, IpAddr, IpAddr), AnyError>
where
  NP: NetPermissions + 'static,
{
  let rid = args.rid;
  let hostname = match &*args.hostname {
    "" => "localhost",
    n => n,
  };

  {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<NP>();
    permissions.check_net(&(hostname, Some(0)), "Deno.startTls()")?;
  }

  let ca_certs = args
    .ca_certs
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  let hostname_dns =
    ServerName::try_from(hostname).map_err(|_| invalid_hostname(hostname))?;

  let unsafely_ignore_certificate_errors = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());

  let root_cert_store = state
    .borrow()
    .borrow::<DefaultTlsOptions>()
    .root_cert_store()?;

  let resource_rc = state
    .borrow_mut()
    .resource_table
    .take::<TcpStreamResource>(rid)?;
  // This TCP connection might be used somewhere else. If it's the case, we cannot proceed with the
  // process of starting a TLS connection on top of this TCP connection, so we just return a bad
  // resource error. See also: https://github.com/denoland/deno/pull/16242
  let resource = Rc::try_unwrap(resource_rc)
    .map_err(|_| bad_resource("TCP stream is currently in use"))?;
  let (read_half, write_half) = resource.into_inner();
  let tcp_stream = read_half.reunite(write_half)?;

  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let mut tls_config = create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    None,
    SocketUse::GeneralSsl,
  )?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let tls_config = Arc::new(tls_config);
  let tls_stream = TlsStream::new_client_side(
    tcp_stream,
    tls_config,
    hostname_dns,
    TLS_BUFFER_SIZE,
  );

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new(tls_stream.into_split()))
  };

  Ok((rid, IpAddr::from(local_addr), IpAddr::from(remote_addr)))
}

#[op2(async)]
#[serde]
pub async fn op_net_connect_tls<NP>(
  state: Rc<RefCell<OpState>>,
  #[serde] addr: IpAddr,
  #[serde] args: ConnectTlsArgs,
) -> Result<(ResourceId, IpAddr, IpAddr), AnyError>
where
  NP: NetPermissions + 'static,
{
  let cert_file = args.cert_file.as_deref();
  let unsafely_ignore_certificate_errors = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());

  if args.cert_chain.is_some() {
    super::check_unstable(&state.borrow(), "ConnectTlsOptions.certChain");
  }
  if args.private_key.is_some() {
    super::check_unstable(&state.borrow(), "ConnectTlsOptions.privateKey");
  }

  {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<NP>();
    permissions
      .check_net(&(&addr.hostname, Some(addr.port)), "Deno.connectTls()")?;
    if let Some(path) = cert_file {
      permissions.check_read(Path::new(path), "Deno.connectTls()")?;
    }
  }

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
    .root_cert_store()?;
  let hostname_dns = ServerName::try_from(&*addr.hostname)
    .map_err(|_| invalid_hostname(&addr.hostname))?;
  let connect_addr = resolve_addr(&addr.hostname, addr.port)
    .await?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let tcp_stream = TcpStream::connect(connect_addr).await?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let cert_chain_and_key =
    if args.cert_chain.is_some() || args.private_key.is_some() {
      let cert_chain = args
        .cert_chain
        .ok_or_else(|| type_error("No certificate chain provided"))?;
      let private_key = args
        .private_key
        .ok_or_else(|| type_error("No private key provided"))?;
      Some((cert_chain, private_key))
    } else {
      None
    };

  let mut tls_config = create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    cert_chain_and_key,
    SocketUse::GeneralSsl,
  )?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let tls_config = Arc::new(tls_config);

  let tls_stream = TlsStream::new_client_side(
    tcp_stream,
    tls_config,
    hostname_dns,
    TLS_BUFFER_SIZE,
  );

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new(tls_stream.into_split()))
  };

  Ok((rid, IpAddr::from(local_addr), IpAddr::from(remote_addr)))
}

fn load_certs_from_file(path: &str) -> Result<Vec<Certificate>, AnyError> {
  let cert_file = File::open(path)?;
  let reader = &mut BufReader::new(cert_file);
  load_certs(reader)
}

fn load_private_keys_from_file(
  path: &str,
) -> Result<Vec<PrivateKey>, AnyError> {
  let key_bytes = std::fs::read(path)?;
  load_private_keys(&key_bytes)
}

pub struct TlsListenerResource {
  pub(crate) tcp_listener: AsyncRefCell<TcpListener>,
  pub(crate) tls_config: Arc<ServerConfig>,
  cancel_handle: CancelHandle,
}

impl Resource for TlsListenerResource {
  fn name(&self) -> Cow<str> {
    "tlsListener".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenTlsArgs {
  cert: Option<String>,
  // TODO(kt3k): Remove this option at v2.0.
  cert_file: Option<String>,
  key: Option<String>,
  // TODO(kt3k): Remove this option at v2.0.
  key_file: Option<String>,
  alpn_protocols: Option<Vec<String>>,
  reuse_port: bool,
}

#[op2]
#[serde]
pub fn op_net_listen_tls<NP>(
  state: &mut OpState,
  #[serde] addr: IpAddr,
  #[serde] args: ListenTlsArgs,
) -> Result<(ResourceId, IpAddr), AnyError>
where
  NP: NetPermissions + 'static,
{
  if args.reuse_port {
    super::check_unstable(state, "Deno.listenTls({ reusePort: true })");
  }

  let cert_file = args.cert_file.as_deref();
  let key_file = args.key_file.as_deref();
  let cert = args.cert.as_deref();
  let key = args.key.as_deref();

  {
    let permissions = state.borrow_mut::<NP>();
    permissions
      .check_net(&(&addr.hostname, Some(addr.port)), "Deno.listenTls()")?;
    if let Some(path) = cert_file {
      permissions.check_read(Path::new(path), "Deno.listenTls()")?;
    }
    if let Some(path) = key_file {
      permissions.check_read(Path::new(path), "Deno.listenTls()")?;
    }
  }

  let cert_chain = if cert_file.is_some() && cert.is_some() {
    return Err(generic_error("Both cert and certFile is specified. You can specify either one of them."));
  } else if let Some(path) = cert_file {
    load_certs_from_file(path)?
  } else if let Some(cert) = cert {
    load_certs(&mut BufReader::new(cert.as_bytes()))?
  } else {
    return Err(generic_error("`cert` is not specified."));
  };
  let key_der = if key_file.is_some() && key.is_some() {
    return Err(generic_error(
      "Both key and keyFile is specified. You can specify either one of them.",
    ));
  } else if let Some(path) = key_file {
    load_private_keys_from_file(path)?.remove(0)
  } else if let Some(key) = key {
    load_private_keys(key.as_bytes())?.remove(0)
  } else {
    return Err(generic_error("`key` is not specified."));
  };

  let mut tls_config = ServerConfig::builder()
    .with_safe_defaults()
    .with_no_client_auth()
    .with_single_cert(cert_chain, key_der)
    .map_err(|e| {
      custom_error(
        "InvalidData",
        format!("Error creating TLS certificate: {:?}", e),
      )
    })?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let bind_addr = resolve_addr_sync(&addr.hostname, addr.port)?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let domain = if bind_addr.is_ipv4() {
    Domain::IPV4
  } else {
    Domain::IPV6
  };
  let socket = Socket::new(domain, Type::STREAM, None)?;
  #[cfg(not(windows))]
  socket.set_reuse_address(true)?;
  if args.reuse_port {
    #[cfg(target_os = "linux")]
    socket.set_reuse_port(true)?;
  }
  let socket_addr = socket2::SockAddr::from(bind_addr);
  socket.bind(&socket_addr)?;
  socket.listen(128)?;
  socket.set_nonblocking(true)?;
  let std_listener: std::net::TcpListener = socket.into();
  let tcp_listener = TcpListener::from_std(std_listener)?;
  let local_addr = tcp_listener.local_addr()?;

  let tls_listener_resource = TlsListenerResource {
    tcp_listener: AsyncRefCell::new(tcp_listener),
    tls_config: Arc::new(tls_config),
    cancel_handle: Default::default(),
  };

  let rid = state.resource_table.add(tls_listener_resource);

  Ok((rid, IpAddr::from(local_addr)))
}

#[op2(async)]
#[serde]
pub async fn op_net_accept_tls(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(ResourceId, IpAddr, IpAddr), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TlsListenerResource>(rid)
    .map_err(|_| bad_resource("Listener has been closed"))?;

  let cancel_handle = RcRef::map(&resource, |r| &r.cancel_handle);
  let tcp_listener = RcRef::map(&resource, |r| &r.tcp_listener)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Another accept task is ongoing"))?;

  let (tcp_stream, remote_addr) =
    match tcp_listener.accept().try_or_cancel(&cancel_handle).await {
      Ok(tuple) => tuple,
      Err(err) if err.kind() == ErrorKind::Interrupted => {
        // FIXME(bartlomieju): compatibility with current JS implementation.
        return Err(bad_resource("Listener has been closed"));
      }
      Err(err) => return Err(err.into()),
    };

  let local_addr = tcp_stream.local_addr()?;

  let tls_stream = TlsStream::new_server_side(
    tcp_stream,
    resource.tls_config.clone(),
    TLS_BUFFER_SIZE,
  );

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new(tls_stream.into_split()))
  };

  Ok((rid, IpAddr::from(local_addr), IpAddr::from(remote_addr)))
}

#[op2(async)]
#[serde]
pub async fn op_tls_handshake(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<TlsHandshakeInfo, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TlsStreamResource>(rid)?;
  resource.handshake().await
}
