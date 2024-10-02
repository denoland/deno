// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::io::TcpStreamResource;
use crate::ops::IpAddr;
use crate::ops::TlsHandshakeInfo;
use crate::raw::NetworkListenerResource;
use crate::resolve_addr::resolve_addr;
use crate::resolve_addr::resolve_addr_sync;
use crate::tcp::TcpListener;
use crate::DefaultTlsOptions;
use crate::NetPermissions;
use crate::UnsafelyIgnoreCertificateErrors;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::bad_resource;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::invalid_hostname;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::v8;
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
use deno_tls::new_resolver;
use deno_tls::rustls::pki_types::ServerName;
use deno_tls::rustls::ClientConnection;
use deno_tls::rustls::ServerConfig;
use deno_tls::ServerConfigProvider;
use deno_tls::SocketUse;
use deno_tls::TlsKey;
use deno_tls::TlsKeyLookup;
use deno_tls::TlsKeys;
use deno_tls::TlsKeysHolder;
use rustls_tokio_stream::TlsStreamRead;
use rustls_tokio_stream::TlsStreamWrite;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::fs::File;
use std::io::BufReader;
use std::io::ErrorKind;
use std::io::Read;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub use rustls_tokio_stream::TlsStream;

pub(crate) const TLS_BUFFER_SIZE: Option<NonZeroUsize> =
  NonZeroUsize::new(65536);

pub struct TlsListener {
  pub(crate) tcp_listener: TcpListener,
  pub(crate) tls_config: Option<Arc<ServerConfig>>,
  pub(crate) server_config_provider: Option<ServerConfigProvider>,
}

impl TlsListener {
  pub async fn accept(&self) -> std::io::Result<(TlsStream, SocketAddr)> {
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
  alpn_protocols: Option<Vec<String>>,
  server_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTlsArgs {
  rid: ResourceId,
  ca_certs: Vec<String>,
  hostname: String,
  alpn_protocols: Option<Vec<String>>,
}

#[op2]
#[cppgc]
pub fn op_tls_key_null() -> TlsKeysHolder {
  TlsKeysHolder::from(TlsKeys::Null)
}

#[op2]
#[cppgc]
pub fn op_tls_key_static(
  #[string] cert: &str,
  #[string] key: &str,
) -> Result<TlsKeysHolder, AnyError> {
  let cert = load_certs(&mut BufReader::new(cert.as_bytes()))?;
  let key = load_private_keys(key.as_bytes())?
    .into_iter()
    .next()
    .unwrap();
  Ok(TlsKeysHolder::from(TlsKeys::Static(TlsKey(cert, key))))
}

#[op2]
pub fn op_tls_cert_resolver_create<'s>(
  scope: &mut v8::HandleScope<'s>,
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
) -> Result<(), AnyError> {
  let TlsKeys::Static(key) = key.take() else {
    bail!("unexpected key type");
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
  lookup.resolve(sni, Err(anyhow!(error)))
}

#[op2]
#[serde]
pub fn op_tls_start<NP>(
  state: Rc<RefCell<OpState>>,
  #[serde] args: StartTlsArgs,
) -> Result<(ResourceId, IpAddr, IpAddr), AnyError>
where
  NP: NetPermissions + 'static,
{
  let rid = args.rid;
  let hostname = match &*args.hostname {
    "" => "localhost".to_string(),
    n => n.to_string(),
  };

  {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<NP>();
    permissions.check_net(&(&hostname, Some(0)), "Deno.startTls()")?;
  }

  let ca_certs = args
    .ca_certs
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  let hostname_dns = ServerName::try_from(hostname.to_string())
    .map_err(|_| invalid_hostname(&hostname))?;

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
  // process of starting a TLS connection on top of this TCP connection, so we just return a Busy error.
  // See also: https://github.com/denoland/deno/pull/16242
  let resource = Rc::try_unwrap(resource_rc)
    .map_err(|_| custom_error("Busy", "TCP stream is currently in use"))?;
  let (read_half, write_half) = resource.into_inner();
  let tcp_stream = read_half.reunite(write_half)?;

  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let mut tls_config = create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    TlsKeys::Null,
    SocketUse::GeneralSsl,
  )?;

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
  #[cppgc] key_pair: &TlsKeysHolder,
) -> Result<(ResourceId, IpAddr, IpAddr), AnyError>
where
  NP: NetPermissions + 'static,
{
  let cert_file = args.cert_file.as_deref();
  let unsafely_ignore_certificate_errors = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());

  let cert_file = {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<NP>();
    permissions
      .check_net(&(&addr.hostname, Some(addr.port)), "Deno.connectTls()")?;
    if let Some(path) = cert_file {
      Some(permissions.check_read(path, "Deno.connectTls()")?)
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
    .root_cert_store()?;
  let hostname_dns = if let Some(server_name) = args.server_name {
    ServerName::try_from(server_name)
  } else {
    ServerName::try_from(addr.hostname.clone())
  }
  .map_err(|_| invalid_hostname(&addr.hostname))?;
  let connect_addr = resolve_addr(&addr.hostname, addr.port)
    .await?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let tcp_stream = TcpStream::connect(connect_addr).await?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

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
      .add(TlsStreamResource::new(tls_stream.into_split()))
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
}

#[op2]
#[serde]
pub fn op_net_listen_tls<NP>(
  state: &mut OpState,
  #[serde] addr: IpAddr,
  #[serde] args: ListenTlsArgs,
  #[cppgc] keys: &TlsKeysHolder,
) -> Result<(ResourceId, IpAddr), AnyError>
where
  NP: NetPermissions + 'static,
{
  if args.reuse_port {
    super::check_unstable(state, "Deno.listenTls({ reusePort: true })");
  }

  {
    let permissions = state.borrow_mut::<NP>();
    permissions
      .check_net(&(&addr.hostname, Some(addr.port)), "Deno.listenTls()")?;
  }

  let bind_addr = resolve_addr_sync(&addr.hostname, addr.port)?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;

  let tcp_listener = if args.load_balanced {
    TcpListener::bind_load_balanced(bind_addr)
  } else {
    TcpListener::bind_direct(bind_addr, args.reuse_port)
  }?;
  let local_addr = tcp_listener.local_addr()?;
  let alpn = args
    .alpn_protocols
    .unwrap_or_default()
    .into_iter()
    .map(|s| s.into_bytes())
    .collect();
  let listener = match keys.take() {
    TlsKeys::Null => Err(anyhow!("Deno.listenTls requires a key")),
    TlsKeys::Static(TlsKey(cert, key)) => {
      let mut tls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .map_err(|e| anyhow!(e))?;
      tls_config.alpn_protocols = alpn;
      Ok(TlsListener {
        tcp_listener,
        tls_config: Some(tls_config.into()),
        server_config_provider: None,
      })
    }
    TlsKeys::Resolver(resolver) => Ok(TlsListener {
      tcp_listener,
      tls_config: None,
      server_config_provider: Some(resolver.into_server_config_provider(alpn)),
    }),
  }
  .map_err(|e| {
    custom_error("InvalidData", "Error creating TLS certificate").context(e)
  })?;

  let tls_listener_resource = NetworkListenerResource::new(listener);

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
    .get::<NetworkListenerResource<TlsListener>>(rid)
    .map_err(|_| bad_resource("Listener has been closed"))?;

  let cancel_handle = RcRef::map(&resource, |r| &r.cancel);
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Another accept task is ongoing"))?;

  let (tls_stream, remote_addr) =
    match listener.accept().try_or_cancel(&cancel_handle).await {
      Ok(tuple) => tuple,
      Err(err) if err.kind() == ErrorKind::Interrupted => {
        return Err(bad_resource("Listener has been closed"));
      }
      Err(err) => return Err(err.into()),
    };

  let local_addr = tls_stream.local_addr()?;
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
    .get::<TlsStreamResource>(rid)
    .map_err(|_| bad_resource("Listener has been closed"))?;
  resource.handshake().await
}
