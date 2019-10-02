// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::resolve_addr::resolve_addr;
use crate::resources;
use crate::state::ThreadSafeState;
use deno::*;
use futures::Future;
use std;
use std::convert::From;
use std::sync::Arc;
use tokio;
use tokio::net::TcpStream;
use tokio_rustls::{rustls::ClientConfig, TlsConnector};
use webpki;
use webpki::DNSNameRef;
use webpki_roots;

#[derive(Deserialize)]
struct DialTLSArgs {
  hostname: String,
  port: u16,
}

pub fn op_dial_tls(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: DialTLSArgs = serde_json::from_value(args)?;

  // TODO(ry) Using format! is suboptimal here. Better would be if
  // state.check_net and resolve_addr() took hostname and port directly.
  let address = format!("{}:{}", args.hostname, args.port);

  state.check_net(&address)?;

  let mut domain = args.hostname;
  if domain.is_empty() {
    domain.push_str("localhost");
  }

  let op = resolve_addr(&address).and_then(move |addr| {
    TcpStream::connect(&addr)
      .and_then(move |tcp_stream| {
        let local_addr = tcp_stream.local_addr()?;
        let remote_addr = tcp_stream.peer_addr()?;
        let mut config = ClientConfig::new();
        config
          .root_store
          .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

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
