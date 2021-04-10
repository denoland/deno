// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::io::TcpStreamResource;
use super::io::TlsClientStreamResource;
use super::io::TlsServerStreamResource;
use super::net::IpAddr;
use super::net::OpAddr;
use super::net::OpConn;
use crate::permissions::Permissions;
use crate::resolve_addr::resolve_addr;
use crate::resolve_addr::resolve_addr_sync;
use deno_core::error::bad_resource;
use deno_core::error::bad_resource_id;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::From;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_rustls::{rustls::ClientConfig, TlsConnector};
use tokio_rustls::{
  rustls::{
    internal::pemfile::{certs, pkcs8_private_keys, rsa_private_keys},
    Certificate, NoClientAuth, PrivateKey, ServerConfig, StoresClientSessions,
  },
  TlsAcceptor,
};
use webpki::DNSNameRef;

lazy_static::lazy_static! {
  static ref CLIENT_SESSION_MEMORY_CACHE: Arc<ClientSessionMemoryCache> =
    Arc::new(ClientSessionMemoryCache::default());
}

#[derive(Default)]
struct ClientSessionMemoryCache(Mutex<HashMap<Vec<u8>, Vec<u8>>>);

impl StoresClientSessions for ClientSessionMemoryCache {
  fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
    self.0.lock().unwrap().get(key).cloned()
  }

  fn put(&self, key: Vec<u8>, value: Vec<u8>) -> bool {
    let mut sessions = self.0.lock().unwrap();
    // TODO(bnoordhuis) Evict sessions LRU-style instead of arbitrarily.
    while sessions.len() >= 1024 {
      let key = sessions.keys().next().unwrap().clone();
      sessions.remove(&key);
    }
    sessions.insert(key, value);
    true
  }
}

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_async(rt, "op_start_tls", op_start_tls);
  super::reg_json_async(rt, "op_connect_tls", op_connect_tls);
  super::reg_json_sync(rt, "op_listen_tls", op_listen_tls);
  super::reg_json_async(rt, "op_accept_tls", op_accept_tls);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectTlsArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert_file: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartTlsArgs {
  rid: ResourceId,
  cert_file: Option<String>,
  hostname: String,
}

async fn op_start_tls(
  state: Rc<RefCell<OpState>>,
  args: StartTlsArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<OpConn, AnyError> {
  let rid = args.rid;

  let mut domain = args.hostname.as_str();
  if domain.is_empty() {
    domain = "localhost";
  }
  {
    super::check_unstable2(&state, "Deno.startTls");
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<Permissions>();
    permissions.net.check(&(&domain, Some(0)))?;
    if let Some(path) = &args.cert_file {
      permissions.read.check(Path::new(&path))?;
    }
  }

  let resource_rc = state
    .borrow_mut()
    .resource_table
    .take::<TcpStreamResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let resource = Rc::try_unwrap(resource_rc)
    .expect("Only a single use of this resource should happen");
  let (read_half, write_half) = resource.into_inner();
  let tcp_stream = read_half.reunite(write_half)?;

  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;
  let mut config = ClientConfig::new();
  config.set_persistence(CLIENT_SESSION_MEMORY_CACHE.clone());
  config
    .root_store
    .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
  if let Some(path) = args.cert_file {
    let key_file = File::open(path)?;
    let reader = &mut BufReader::new(key_file);
    config.root_store.add_pem_file(reader).unwrap();
  }

  let tls_connector = TlsConnector::from(Arc::new(config));
  let dnsname = DNSNameRef::try_from_ascii_str(&domain)
    .map_err(|_| generic_error("Invalid DNS lookup"))?;
  let tls_stream = tls_connector.connect(dnsname, tcp_stream).await?;

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsClientStreamResource::from(tls_stream))
  };
  Ok(OpConn {
    rid,
    local_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: local_addr.ip().to_string(),
      port: local_addr.port(),
    })),
    remote_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: remote_addr.ip().to_string(),
      port: remote_addr.port(),
    })),
  })
}

async fn op_connect_tls(
  state: Rc<RefCell<OpState>>,
  args: ConnectTlsArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<OpConn, AnyError> {
  assert_eq!(args.transport, "tcp");

  {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<Permissions>();
    permissions.net.check(&(&args.hostname, Some(args.port)))?;
    if let Some(path) = &args.cert_file {
      permissions.read.check(Path::new(&path))?;
    }
  }
  let mut domain = args.hostname.as_str();
  if domain.is_empty() {
    domain = "localhost";
  }

  let addr = resolve_addr(&args.hostname, args.port)
    .await?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let tcp_stream = TcpStream::connect(&addr).await?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;
  let mut config = ClientConfig::new();
  config.set_persistence(CLIENT_SESSION_MEMORY_CACHE.clone());
  config
    .root_store
    .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
  if let Some(path) = args.cert_file {
    let key_file = File::open(path)?;
    let reader = &mut BufReader::new(key_file);
    config.root_store.add_pem_file(reader).unwrap();
  }
  let tls_connector = TlsConnector::from(Arc::new(config));
  let dnsname = DNSNameRef::try_from_ascii_str(&domain)
    .map_err(|_| generic_error("Invalid DNS lookup"))?;
  let tls_stream = tls_connector.connect(dnsname, tcp_stream).await?;
  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsClientStreamResource::from(tls_stream))
  };
  Ok(OpConn {
    rid,
    local_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: local_addr.ip().to_string(),
      port: local_addr.port(),
    })),
    remote_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: remote_addr.ip().to_string(),
      port: remote_addr.port(),
    })),
  })
}

fn load_certs(path: &str) -> Result<Vec<Certificate>, AnyError> {
  let cert_file = File::open(path)?;
  let reader = &mut BufReader::new(cert_file);

  let certs = certs(reader)
    .map_err(|_| custom_error("InvalidData", "Unable to decode certificate"))?;

  if certs.is_empty() {
    let e = custom_error("InvalidData", "No certificates found in cert file");
    return Err(e);
  }

  Ok(certs)
}

fn key_decode_err() -> AnyError {
  custom_error("InvalidData", "Unable to decode key")
}

fn key_not_found_err() -> AnyError {
  custom_error("InvalidData", "No keys found in key file")
}

/// Starts with -----BEGIN RSA PRIVATE KEY-----
fn load_rsa_keys(path: &str) -> Result<Vec<PrivateKey>, AnyError> {
  let key_file = File::open(path)?;
  let reader = &mut BufReader::new(key_file);
  let keys = rsa_private_keys(reader).map_err(|_| key_decode_err())?;
  Ok(keys)
}

/// Starts with -----BEGIN PRIVATE KEY-----
fn load_pkcs8_keys(path: &str) -> Result<Vec<PrivateKey>, AnyError> {
  let key_file = File::open(path)?;
  let reader = &mut BufReader::new(key_file);
  let keys = pkcs8_private_keys(reader).map_err(|_| key_decode_err())?;
  Ok(keys)
}

fn load_keys(path: &str) -> Result<Vec<PrivateKey>, AnyError> {
  let path = path.to_string();
  let mut keys = load_rsa_keys(&path)?;

  if keys.is_empty() {
    keys = load_pkcs8_keys(&path)?;
  }

  if keys.is_empty() {
    return Err(key_not_found_err());
  }

  Ok(keys)
}

pub struct TlsListenerResource {
  listener: AsyncRefCell<TcpListener>,
  tls_acceptor: TlsAcceptor,
  cancel: CancelHandle,
}

impl Resource for TlsListenerResource {
  fn name(&self) -> Cow<str> {
    "tlsListener".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenTlsArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert_file: String,
  key_file: String,
  alpn_protocols: Option<Vec<String>>,
}

fn op_listen_tls(
  state: &mut OpState,
  args: ListenTlsArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<OpConn, AnyError> {
  assert_eq!(args.transport, "tcp");

  let cert_file = args.cert_file;
  let key_file = args.key_file;
  {
    let permissions = state.borrow_mut::<Permissions>();
    permissions.net.check(&(&args.hostname, Some(args.port)))?;
    permissions.read.check(Path::new(&cert_file))?;
    permissions.read.check(Path::new(&key_file))?;
  }
  let mut config = ServerConfig::new(NoClientAuth::new());
  if let Some(alpn_protocols) = args.alpn_protocols {
    super::check_unstable(state, "Deno.listenTls#alpn_protocols");
    config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }
  config
    .set_single_cert(load_certs(&cert_file)?, load_keys(&key_file)?.remove(0))
    .expect("invalid key or certificate");
  let tls_acceptor = TlsAcceptor::from(Arc::new(config));
  let addr = resolve_addr_sync(&args.hostname, args.port)?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let std_listener = std::net::TcpListener::bind(&addr)?;
  std_listener.set_nonblocking(true)?;
  let listener = TcpListener::from_std(std_listener)?;
  let local_addr = listener.local_addr()?;
  let tls_listener_resource = TlsListenerResource {
    listener: AsyncRefCell::new(listener),
    tls_acceptor,
    cancel: Default::default(),
  };

  let rid = state.resource_table.add(tls_listener_resource);

  Ok(OpConn {
    rid,
    local_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: local_addr.ip().to_string(),
      port: local_addr.port(),
    })),
    remote_addr: None,
  })
}

async fn op_accept_tls(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<OpConn, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TlsListenerResource>(rid)
    .ok_or_else(|| bad_resource("Listener has been closed"))?;
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Another accept task is ongoing"))?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (tcp_stream, _socket_addr) =
    listener.accept().try_or_cancel(cancel).await.map_err(|e| {
      // FIXME(bartlomieju): compatibility with current JS implementation
      if let std::io::ErrorKind::Interrupted = e.kind() {
        bad_resource("Listener has been closed")
      } else {
        e.into()
      }
    })?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;
  let resource = state
    .borrow()
    .resource_table
    .get::<TlsListenerResource>(rid)
    .ok_or_else(|| bad_resource("Listener has been closed"))?;
  let cancel = RcRef::map(&resource, |r| &r.cancel);
  let tls_acceptor = resource.tls_acceptor.clone();
  let tls_stream = tls_acceptor
    .accept(tcp_stream)
    .try_or_cancel(cancel)
    .await?;

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsServerStreamResource::from(tls_stream))
  };

  Ok(OpConn {
    rid,
    local_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: local_addr.ip().to_string(),
      port: local_addr.port(),
    })),
    remote_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: remote_addr.ip().to_string(),
      port: remote_addr.port(),
    })),
  })
}
