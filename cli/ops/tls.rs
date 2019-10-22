// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::ops::json_op;
use crate::resolve_addr::resolve_addr;
use crate::resources;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use futures::Future;
use std;
use std::convert::From;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
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
  i.register_op("dial_tls", s.core_op(json_op(s.stateful_op(op_dial_tls))));
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
struct DialTLSArgs {
  hostname: String,
  port: u16,
  cert_file: Option<String>,
}

pub fn op_dial_tls(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: DialTLSArgs = serde_json::from_value(args)?;
  let cert_file = args.cert_file;

  state.check_net(&args.hostname, args.port)?;
  if let Some(path) = cert_file.clone() {
    state.check_read(&path)?;
  }

  let mut domain = args.hostname.clone();
  if domain.is_empty() {
    domain.push_str("localhost");
  }

  let op = resolve_addr(&args.hostname, args.port).and_then(move |addr| {
    TcpStream::connect(&addr)
      .and_then(move |tcp_stream| {
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
        Ok((tls_connector, tcp_stream, local_addr, remote_addr))
      })
      .map_err(ErrBox::from)
      .and_then(
        move |(tls_connector, tcp_stream, local_addr, remote_addr)| {
          let dnsname = DNSNameRef::try_from_ascii_str(&domain)
            .expect("Invalid DNS lookup");
          tls_connector
            .connect(dnsname, tcp_stream)
            .map_err(ErrBox::from)
            .and_then(move |tls_stream| {
              let tls_stream_resource = resources::add_tls_stream(tls_stream);
              futures::future::ok(json!({
                "rid": tls_stream_resource.rid,
                "localAddr": local_addr.to_string(),
                "remoteAddr": remote_addr.to_string(),
              }))
            })
        },
      )
  });

  Ok(JsonOp::Async(Box::new(op)))
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
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ListenTlsArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp");

  let cert_file = args.cert_file;
  let key_file = args.key_file;

  state.check_net(&args.hostname, args.port)?;
  state.check_read(&cert_file)?;
  state.check_read(&key_file)?;

  let mut config = ServerConfig::new(NoClientAuth::new());
  config
    .set_single_cert(load_certs(&cert_file)?, load_keys(&key_file)?.remove(0))
    .expect("invalid key or certificate");
  let acceptor = TlsAcceptor::from(Arc::new(config));
  let addr = resolve_addr(&args.hostname, args.port).wait()?;
  let listener = TcpListener::bind(&addr)?;
  let local_addr = listener.local_addr()?;
  let resource = resources::add_tls_listener(listener, acceptor);

  Ok(JsonOp::Sync(json!({
    "rid": resource.rid,
    "localAddr": local_addr.to_string()
  })))
}

#[derive(Deserialize)]
struct AcceptTlsArgs {
  rid: i32,
}

fn op_accept_tls(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: AcceptTlsArgs = serde_json::from_value(args)?;
  let server_rid = args.rid as u32;

  let server_resource = resources::lookup(server_rid)?;
  let op = tokio_util::accept(server_resource)
    .and_then(move |(tcp_stream, _socket_addr)| {
      let local_addr = tcp_stream.local_addr()?;
      let remote_addr = tcp_stream.peer_addr()?;
      Ok((tcp_stream, local_addr, remote_addr))
    })
    .and_then(move |(tcp_stream, local_addr, remote_addr)| {
      let mut server_resource = resources::lookup(server_rid).unwrap();
      server_resource
        .poll_accept_tls(tcp_stream)
        .and_then(move |tls_stream| {
          let tls_stream_resource =
            resources::add_server_tls_stream(tls_stream);
          Ok((tls_stream_resource, local_addr, remote_addr))
        })
    })
    .map_err(ErrBox::from)
    .and_then(move |(tls_stream_resource, local_addr, remote_addr)| {
      futures::future::ok(json!({
        "rid": tls_stream_resource.rid,
        "localAddr": local_addr.to_string(),
        "remoteAddr": remote_addr.to_string(),
      }))
    });

  Ok(JsonOp::Async(Box::new(op)))
}
