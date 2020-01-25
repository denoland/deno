// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::StreamResource;
use crate::deno_error::bad_resource;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::ops::json_op;
use crate::resolve_addr::resolve_addr;
use crate::state::ThreadSafeState;
use deno_core::Resource;
use deno_core::*;
use futures::future::FutureExt;
use std;
use std::convert::From;
use std::fs::File;
use std::future::Future;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_rustls::{rustls::ClientConfig, TlsConnector};
use tokio_rustls::{
  rustls::{
    internal::pemfile::{certs, pkcs8_private_keys, rsa_private_keys},
    Certificate, NoClientAuth, PrivateKey, ServerConfig,
  },
  TlsAcceptor,
};
use webpki;
use webpki::DNSNameRef;
use webpki_roots;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "connect_tls",
    s.core_op(json_op(s.stateful_op(op_connect_tls))),
  );
  i.register_op(
    "listen_tls",
    s.core_op(json_op(s.stateful_op(op_listen_tls))),
  );
  i.register_op(
    "accept_tls",
    s.core_op(json_op(s.stateful_op(op_accept_tls))),
  );
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectTLSArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert_file: Option<String>,
}

pub fn op_connect_tls(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ConnectTLSArgs = serde_json::from_value(args)?;
  let cert_file = args.cert_file.clone();
  let state_ = state.clone();
  state.check_net(&args.hostname, args.port)?;
  if let Some(path) = cert_file.clone() {
    state.check_read(Path::new(&path))?;
  }

  let mut domain = args.hostname.clone();
  if domain.is_empty() {
    domain.push_str("localhost");
  }

  let op = async move {
    let addr = resolve_addr(&args.hostname, args.port).await?;
    let tcp_stream = TcpStream::connect(&addr).await?;
    let local_addr = tcp_stream.local_addr()?;
    let remote_addr = tcp_stream.peer_addr()?;
    let mut config = ClientConfig::new();
    config
      .root_store
      .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    if let Some(path) = cert_file {
      let key_file = File::open(path)?;
      let reader = &mut BufReader::new(key_file);
      config.root_store.add_pem_file(reader).unwrap();
    }
    let tls_connector = TlsConnector::from(Arc::new(config));
    let dnsname =
      DNSNameRef::try_from_ascii_str(&domain).expect("Invalid DNS lookup");
    let tls_stream = tls_connector.connect(dnsname, tcp_stream).await?;
    let mut table = state_.lock_resource_table();
    let rid = table.add(
      "clientTlsStream",
      Box::new(StreamResource::ClientTlsStream(Box::new(tls_stream))),
    );
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
  };

  Ok(JsonOp::Async(op.boxed()))
}

fn load_certs(path: &str) -> Result<Vec<Certificate>, ErrBox> {
  let cert_file = File::open(path)?;
  let reader = &mut BufReader::new(cert_file);

  let certs = certs(reader).map_err(|_| {
    DenoError::new(ErrorKind::Other, "Unable to decode certificate".to_string())
  })?;

  if certs.is_empty() {
    let e = DenoError::new(
      ErrorKind::Other,
      "No certificates found in cert file".to_string(),
    );
    return Err(ErrBox::from(e));
  }

  Ok(certs)
}

fn key_decode_err() -> DenoError {
  DenoError::new(ErrorKind::Other, "Unable to decode key".to_string())
}

fn key_not_found_err() -> DenoError {
  DenoError::new(ErrorKind::Other, "No keys found in key file".to_string())
}

/// Starts with -----BEGIN RSA PRIVATE KEY-----
fn load_rsa_keys(path: &str) -> Result<Vec<PrivateKey>, ErrBox> {
  let key_file = File::open(path)?;
  let reader = &mut BufReader::new(key_file);
  let keys = rsa_private_keys(reader).map_err(|_| key_decode_err())?;
  Ok(keys)
}

/// Starts with -----BEGIN PRIVATE KEY-----
fn load_pkcs8_keys(path: &str) -> Result<Vec<PrivateKey>, ErrBox> {
  let key_file = File::open(path)?;
  let reader = &mut BufReader::new(key_file);
  let keys = pkcs8_private_keys(reader).map_err(|_| key_decode_err())?;
  Ok(keys)
}

fn load_keys(path: &str) -> Result<Vec<PrivateKey>, ErrBox> {
  let path = path.to_string();
  let mut keys = load_rsa_keys(&path)?;

  if keys.is_empty() {
    keys = load_pkcs8_keys(&path)?;
  }

  if keys.is_empty() {
    return Err(ErrBox::from(key_not_found_err()));
  }

  Ok(keys)
}

#[allow(dead_code)]
pub struct TlsListenerResource {
  listener: TcpListener,
  tls_acceptor: TlsAcceptor,
  waker: Option<futures::task::AtomicWaker>,
  local_addr: SocketAddr,
}

impl Resource for TlsListenerResource {}

impl Drop for TlsListenerResource {
  fn drop(&mut self) {
    self.wake_task();
  }
}

impl TlsListenerResource {
  /// Track the current task so future awaiting for connection
  /// can be notified when listener is closed.
  ///
  /// Throws an error if another task is already tracked.
  pub fn track_task(&mut self, cx: &Context) -> Result<(), ErrBox> {
    // Currently, we only allow tracking a single accept task for a listener.
    // This might be changed in the future with multiple workers.
    // Caveat: TcpListener by itself also only tracks an accept task at a time.
    // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
    if self.waker.is_some() {
      let e = std::io::Error::new(
        std::io::ErrorKind::Other,
        "Another accept task is ongoing",
      );
      return Err(ErrBox::from(e));
    }

    let waker = futures::task::AtomicWaker::new();
    waker.register(cx.waker());
    self.waker.replace(waker);
    Ok(())
  }

  /// Notifies a task when listener is closed so accept future can resolve.
  pub fn wake_task(&mut self) {
    if let Some(waker) = self.waker.as_ref() {
      waker.wake();
    }
  }

  /// Stop tracking a task.
  /// Happens when the task is done and thus no further tracking is needed.
  pub fn untrack_task(&mut self) {
    if self.waker.is_some() {
      self.waker.take();
    }
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ListenTlsArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp");

  let cert_file = args.cert_file;
  let key_file = args.key_file;

  state.check_net(&args.hostname, args.port)?;
  state.check_read(Path::new(&cert_file))?;
  state.check_read(Path::new(&key_file))?;

  let mut config = ServerConfig::new(NoClientAuth::new());
  config
    .set_single_cert(load_certs(&cert_file)?, load_keys(&key_file)?.remove(0))
    .expect("invalid key or certificate");
  let tls_acceptor = TlsAcceptor::from(Arc::new(config));
  let addr =
    futures::executor::block_on(resolve_addr(&args.hostname, args.port))?;
  let listener = futures::executor::block_on(TcpListener::bind(&addr))?;
  let local_addr = listener.local_addr()?;
  let tls_listener_resource = TlsListenerResource {
    listener,
    tls_acceptor,
    waker: None,
    local_addr,
  };
  let mut table = state.lock_resource_table();
  let rid = table.add("tlsListener", Box::new(tls_listener_resource));

  Ok(JsonOp::Sync(json!({
    "rid": rid,
    "localAddr": {
      "hostname": local_addr.ip().to_string(),
      "port": local_addr.port(),
      "transport": args.transport,
    },
  })))
}

#[derive(Debug, PartialEq)]
enum AcceptTlsState {
  Pending,
  Done,
}

/// Simply accepts a TLS connection.
pub fn accept_tls(state: &ThreadSafeState, rid: ResourceId) -> AcceptTls {
  AcceptTls {
    accept_state: AcceptTlsState::Pending,
    rid,
    state: state.clone(),
  }
}

/// A future representing state of accepting a TLS connection.
pub struct AcceptTls {
  accept_state: AcceptTlsState,
  rid: ResourceId,
  state: ThreadSafeState,
}

impl Future for AcceptTls {
  type Output = Result<(TcpStream, SocketAddr), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    if inner.accept_state == AcceptTlsState::Done {
      panic!("poll AcceptTls after it's done");
    }

    let mut table = inner.state.lock_resource_table();
    let listener_resource = table
      .get_mut::<TlsListenerResource>(inner.rid)
      .ok_or_else(|| {
        let e = std::io::Error::new(
          std::io::ErrorKind::Other,
          "Listener has been closed",
        );
        ErrBox::from(e)
      })?;

    let listener = &mut listener_resource.listener;

    match listener.poll_accept(cx).map_err(ErrBox::from) {
      Poll::Ready(Ok((stream, addr))) => {
        listener_resource.untrack_task();
        inner.accept_state = AcceptTlsState::Done;
        Poll::Ready(Ok((stream, addr)))
      }
      Poll::Pending => {
        listener_resource.track_task(cx)?;
        Poll::Pending
      }
      Poll::Ready(Err(e)) => {
        listener_resource.untrack_task();
        inner.accept_state = AcceptTlsState::Done;
        Poll::Ready(Err(e))
      }
    }
  }
}

#[derive(Deserialize)]
struct AcceptTlsArgs {
  rid: i32,
}

fn op_accept_tls(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: AcceptTlsArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let state = state.clone();
  let op = async move {
    let (tcp_stream, _socket_addr) = accept_tls(&state.clone(), rid).await?;
    let local_addr = tcp_stream.local_addr()?;
    let remote_addr = tcp_stream.peer_addr()?;
    let tls_acceptor = {
      let table = state.lock_resource_table();
      let resource = table
        .get::<TlsListenerResource>(rid)
        .ok_or_else(bad_resource)
        .expect("Can't find tls listener");
      resource.tls_acceptor.clone()
    };
    let tls_stream = tls_acceptor.accept(tcp_stream).await?;
    let rid = {
      let mut table = state.lock_resource_table();
      table.add(
        "serverTlsStream",
        Box::new(StreamResource::ServerTlsStream(Box::new(tls_stream))),
      )
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
  };

  Ok(JsonOp::Async(op.boxed()))
}
