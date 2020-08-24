// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::{StreamResource, StreamResourceHolder};
use crate::op_error::OpError;
use crate::resolve_addr::resolve_addr;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::FutureExt;
use std::convert::From;
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
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
use webpki::DNSNameRef;

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  i.register_op("op_start_tls", s.stateful_json_op2(op_start_tls));
  i.register_op("op_connect_tls", s.stateful_json_op2(op_connect_tls));
  i.register_op("op_listen_tls", s.stateful_json_op2(op_listen_tls));
  i.register_op("op_accept_tls", s.stateful_json_op2(op_accept_tls));
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectTLSArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert_file: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartTLSArgs {
  rid: u32,
  cert_file: Option<String>,
  hostname: String,
}

pub fn op_start_tls(
  isolate_state: &mut CoreIsolateState,
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.startTls");
  let args: StartTLSArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let cert_file = args.cert_file.clone();
  let resource_table = isolate_state.resource_table.clone();

  let mut domain = args.hostname;
  if domain.is_empty() {
    domain.push_str("localhost");
  }

  state.check_net(&domain, 0)?;
  if let Some(path) = cert_file.clone() {
    state.check_read(Path::new(&path))?;
  }

  let op = async move {
    let mut resource_holder = {
      let mut resource_table_ = resource_table.borrow_mut();
      match resource_table_.remove::<StreamResourceHolder>(rid) {
        Some(resource) => *resource,
        None => return Err(OpError::bad_resource_id()),
      }
    };

    if let StreamResource::TcpStream(ref mut tcp_stream) =
      resource_holder.resource
    {
      let tcp_stream = tcp_stream.take().unwrap();
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

      let mut resource_table_ = resource_table.borrow_mut();
      let rid = resource_table_.add(
        "clientTlsStream",
        Box::new(StreamResourceHolder::new(StreamResource::ClientTlsStream(
          Box::new(tls_stream),
        ))),
      );
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
    } else {
      Err(OpError::bad_resource_id())
    }
  };
  Ok(JsonOp::Async(op.boxed_local()))
}

pub fn op_connect_tls(
  isolate_state: &mut CoreIsolateState,
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: ConnectTLSArgs = serde_json::from_value(args)?;
  let cert_file = args.cert_file.clone();
  let resource_table = isolate_state.resource_table.clone();
  state.check_net(&args.hostname, args.port)?;
  if let Some(path) = cert_file.clone() {
    state.check_read(Path::new(&path))?;
  }

  let mut domain = args.hostname.clone();
  if domain.is_empty() {
    domain.push_str("localhost");
  }

  let op = async move {
    let addr = resolve_addr(&args.hostname, args.port)?;
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
    let mut resource_table_ = resource_table.borrow_mut();
    let rid = resource_table_.add(
      "clientTlsStream",
      Box::new(StreamResourceHolder::new(StreamResource::ClientTlsStream(
        Box::new(tls_stream),
      ))),
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

  Ok(JsonOp::Async(op.boxed_local()))
}

fn load_certs(path: &str) -> Result<Vec<Certificate>, OpError> {
  let cert_file = File::open(path)?;
  let reader = &mut BufReader::new(cert_file);

  let certs = certs(reader)
    .map_err(|_| OpError::other("Unable to decode certificate".to_string()))?;

  if certs.is_empty() {
    let e = OpError::other("No certificates found in cert file".to_string());
    return Err(e);
  }

  Ok(certs)
}

fn key_decode_err() -> OpError {
  OpError::other("Unable to decode key".to_string())
}

fn key_not_found_err() -> OpError {
  OpError::other("No keys found in key file".to_string())
}

/// Starts with -----BEGIN RSA PRIVATE KEY-----
fn load_rsa_keys(path: &str) -> Result<Vec<PrivateKey>, OpError> {
  let key_file = File::open(path)?;
  let reader = &mut BufReader::new(key_file);
  let keys = rsa_private_keys(reader).map_err(|_| key_decode_err())?;
  Ok(keys)
}

/// Starts with -----BEGIN PRIVATE KEY-----
fn load_pkcs8_keys(path: &str) -> Result<Vec<PrivateKey>, OpError> {
  let key_file = File::open(path)?;
  let reader = &mut BufReader::new(key_file);
  let keys = pkcs8_private_keys(reader).map_err(|_| key_decode_err())?;
  Ok(keys)
}

fn load_keys(path: &str) -> Result<Vec<PrivateKey>, OpError> {
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

#[allow(dead_code)]
pub struct TlsListenerResource {
  listener: TcpListener,
  tls_acceptor: TlsAcceptor,
  waker: Option<futures::task::AtomicWaker>,
  local_addr: SocketAddr,
}

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
  pub fn track_task(&mut self, cx: &Context) -> Result<(), OpError> {
    // Currently, we only allow tracking a single accept task for a listener.
    // This might be changed in the future with multiple workers.
    // Caveat: TcpListener by itself also only tracks an accept task at a time.
    // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
    if self.waker.is_some() {
      return Err(OpError::other("Another accept task is ongoing".to_string()));
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
    self.waker.take();
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
  isolate_state: &mut CoreIsolateState,
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
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
  let addr = resolve_addr(&args.hostname, args.port)?;
  let std_listener = std::net::TcpListener::bind(&addr)?;
  let listener = TcpListener::from_std(std_listener)?;
  let local_addr = listener.local_addr()?;
  let tls_listener_resource = TlsListenerResource {
    listener,
    tls_acceptor,
    waker: None,
    local_addr,
  };

  let mut resource_table = isolate_state.resource_table.borrow_mut();
  let rid = resource_table.add("tlsListener", Box::new(tls_listener_resource));

  Ok(JsonOp::Sync(json!({
    "rid": rid,
    "localAddr": {
      "hostname": local_addr.ip().to_string(),
      "port": local_addr.port(),
      "transport": args.transport,
    },
  })))
}

#[derive(Deserialize)]
struct AcceptTlsArgs {
  rid: i32,
}

fn op_accept_tls(
  isolate_state: &mut CoreIsolateState,
  _state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: AcceptTlsArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let resource_table = isolate_state.resource_table.clone();
  let op = async move {
    let accept_fut = poll_fn(|cx| {
      let mut resource_table = resource_table.borrow_mut();
      let listener_resource = resource_table
        .get_mut::<TlsListenerResource>(rid)
        .ok_or_else(|| {
          OpError::bad_resource("Listener has been closed".to_string())
        })?;
      let listener = &mut listener_resource.listener;
      match listener.poll_accept(cx).map_err(OpError::from) {
        Poll::Ready(Ok((stream, addr))) => {
          listener_resource.untrack_task();
          Poll::Ready(Ok((stream, addr)))
        }
        Poll::Pending => {
          listener_resource.track_task(cx)?;
          Poll::Pending
        }
        Poll::Ready(Err(e)) => {
          listener_resource.untrack_task();
          Poll::Ready(Err(e))
        }
      }
    });
    let (tcp_stream, _socket_addr) = accept_fut.await?;
    let local_addr = tcp_stream.local_addr()?;
    let remote_addr = tcp_stream.peer_addr()?;
    let tls_acceptor = {
      let resource_table = resource_table.borrow();
      let resource = resource_table
        .get::<TlsListenerResource>(rid)
        .ok_or_else(OpError::bad_resource_id)
        .expect("Can't find tls listener");
      resource.tls_acceptor.clone()
    };
    let tls_stream = tls_acceptor.accept(tcp_stream).await?;
    let rid = {
      let mut resource_table = resource_table.borrow_mut();
      resource_table.add(
        "serverTlsStream",
        Box::new(StreamResourceHolder::new(StreamResource::ServerTlsStream(
          Box::new(tls_stream),
        ))),
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

  Ok(JsonOp::Async(op.boxed_local()))
}
