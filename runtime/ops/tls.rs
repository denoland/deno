// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::io::TcpStreamResource;
use super::io::TlsClientStreamResource;
use super::io::TlsServerStreamResource;
use crate::permissions::Permissions;
use crate::resolve_addr::resolve_addr;
use crate::resolve_addr::resolve_addr_sync;
use deno_core::error::bad_resource;
use deno_core::error::bad_resource_id;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::AsyncRefCell;
use deno_core::BufVec;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::From;
use std::fs::File;
use std::io::{BufRead, BufReader};
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

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ClientCertArgs {
  chain: String,
  private_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectTLSArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert_file: Option<String>,

  client_cert: Option<ClientCertArgs>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartTLSArgs {
  rid: u32,
  cert_file: Option<String>,
  hostname: String,
}

async fn op_start_tls(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: StartTLSArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  let mut domain = args.hostname.as_str();
  if domain.is_empty() {
    domain = "localhost";
  }
  {
    super::check_unstable2(&state, "Deno.startTls");
    let s = state.borrow();
    let permissions = s.borrow::<Permissions>();
    permissions.check_net(&(&domain, Some(0)))?;
    if let Some(path) = &args.cert_file {
      permissions.check_read(Path::new(&path))?;
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
  let dnsname =
    DNSNameRef::try_from_ascii_str(&domain).expect("Invalid DNS lookup");
  let tls_stream = tls_connector.connect(dnsname, tcp_stream).await?;

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsClientStreamResource::from(tls_stream))
  };
  Ok(json!({
      "rid": rid,
      "localAddr": {
        "hostname": local_addr.ip().to_string(),
        "port": local_addr.port(),
        "transport": "tcp",
      },
      "remoteAddr": {
        "hostname": remote_addr.ip().to_string(),
        "port": remote_addr.port(),
        "transport": "tcp",
      }
  }))
}

async fn op_connect_tls(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: ConnectTLSArgs = serde_json::from_value(args)?;
  {
    let s = state.borrow();
    let permissions = s.borrow::<Permissions>();
    permissions.check_net(&(&args.hostname, Some(args.port)))?;
    if let Some(path) = &args.cert_file {
      permissions.check_read(Path::new(&path))?;
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
  if let Some(client_cert) = args.client_cert {
    let cert_chain = load_certs(&mut client_cert.chain.as_bytes())?;
    let private_key = load_keys(&mut client_cert.private_key.as_bytes())
      .and_then(|keys| {
        if keys.len() != 1 {
          return Err(custom_error(
            "InvalidData",
            "Multiple private keys given",
          ));
        }
        Ok(keys[0].clone())
      })?;

    config.set_single_client_cert(cert_chain, private_key)?;
  }

  let tls_connector = TlsConnector::from(Arc::new(config));
  let dnsname =
    DNSNameRef::try_from_ascii_str(&domain).expect("Invalid DNS lookup");
  let tls_stream = tls_connector.connect(dnsname, tcp_stream).await?;
  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsClientStreamResource::from(tls_stream))
  };
  Ok(json!({
      "rid": rid,
      "localAddr": {
        "hostname": local_addr.ip().to_string(),
        "port": local_addr.port(),
        "transport": args.transport,
      },
      "remoteAddr": {
        "hostname": remote_addr.ip().to_string(),
        "port": remote_addr.port(),
        "transport": args.transport,
      }
  }))
}

fn load_certs(reader: &mut dyn BufRead) -> Result<Vec<Certificate>, AnyError> {
  let certs = certs(reader)
    .map_err(|_| custom_error("InvalidData", "Unable to decode certificate"))?;

  if certs.is_empty() {
    let e = custom_error("InvalidData", "No certificates found in cert file");
    return Err(e);
  }

  Ok(certs)
}

fn load_certs_from_file(path: &str) -> Result<Vec<Certificate>, AnyError> {
  let cert_file = File::open(path)?;
  let reader = &mut BufReader::new(cert_file);
  return load_certs(reader);
}

fn key_decode_err() -> AnyError {
  custom_error("InvalidData", "Unable to decode key")
}

fn key_not_found_err() -> AnyError {
  custom_error("InvalidData", "No keys found in key file")
}

fn load_keys(bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  // Starts with -----BEGIN RSA PRIVATE KEY-----
  let mut keys =
    rsa_private_keys(&mut bytes.clone()).map_err(|_| key_decode_err())?;

  if keys.is_empty() {
    // Starts with -----BEGIN PRIVATE KEY-----
    keys =
      pkcs8_private_keys(&mut bytes.clone()).map_err(|_| key_decode_err())?;
  }

  if keys.is_empty() {
    return Err(key_not_found_err());
  }

  Ok(keys)
}

fn load_keys_from_file(path: &str) -> Result<Vec<PrivateKey>, AnyError> {
  let key_bytes = std::fs::read(path)?;
  return load_keys(&key_bytes);
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
struct ListenTlsArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert_file: String,
  key_file: String,
}

fn op_listen_tls(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ListenTlsArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp");

  let cert_file = args.cert_file;
  let key_file = args.key_file;
  {
    let permissions = state.borrow::<Permissions>();
    permissions.check_net(&(&args.hostname, Some(args.port)))?;
    permissions.check_read(Path::new(&cert_file))?;
    permissions.check_read(Path::new(&key_file))?;
  }
  let mut config = ServerConfig::new(NoClientAuth::new());
  config
    .set_single_cert(
      load_certs_from_file(&cert_file)?,
      load_keys_from_file(&key_file)?.remove(0),
    )
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

  Ok(json!({
    "rid": rid,
    "localAddr": {
      "hostname": local_addr.ip().to_string(),
      "port": local_addr.port(),
      "transport": args.transport,
    },
  }))
}

#[derive(Deserialize)]
struct AcceptTlsArgs {
  rid: i32,
}

async fn op_accept_tls(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: AcceptTlsArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

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

  Ok(json!({
    "rid": rid,
    "localAddr": {
      "transport": "tcp",
      "hostname": local_addr.ip().to_string(),
      "port": local_addr.port()
    },
    "remoteAddr": {
      "transport": "tcp",
      "hostname": remote_addr.ip().to_string(),
      "port": remote_addr.port()
    }
  }))
}
