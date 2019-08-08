// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::msg;
use crate::ops::empty_buf;
use crate::ops::ok_buf;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::resources;
use crate::resources::Resource;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use futures::Future;
use std;
use std::net::Shutdown;
use tokio;

use crate::resolve_addr::resolve_addr;
use std::convert::From;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

use crate::ops::blocking;

pub fn op_accept(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_accept().unwrap();
  let server_rid = inner.rid();

  match resources::lookup(server_rid) {
    None => Err(deno_error::bad_resource()),
    Some(server_resource) => {
      let op = tokio_util::accept(server_resource)
        .map_err(ErrBox::from)
        .and_then(move |(tcp_stream, _socket_addr)| {
          new_conn(cmd_id, tcp_stream)
        });
      if base.sync() {
        let buf = op.wait()?;
        Ok(Op::Sync(buf))
      } else {
        Ok(Op::Async(Box::new(op)))
      }
    }
  }
}

pub fn op_dial(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_dial().unwrap();
  let network = inner.network().unwrap();
  assert_eq!(network, "tcp"); // TODO Support others.
  let address = inner.address().unwrap();

  state.check_net(&address)?;

  let op = resolve_addr(address).and_then(move |addr| {
    TcpStream::connect(&addr)
      .map_err(ErrBox::from)
      .and_then(move |tcp_stream| new_conn(cmd_id, tcp_stream))
  });
  if base.sync() {
    let buf = op.wait()?;
    Ok(Op::Sync(buf))
  } else {
    Ok(Op::Async(Box::new(op)))
  }
}

pub fn op_shutdown(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_shutdown().unwrap();
  let rid = inner.rid();
  let how = inner.how();
  match resources::lookup(rid) {
    None => Err(deno_error::bad_resource()),
    Some(mut resource) => {
      let shutdown_mode = match how {
        0 => Shutdown::Read,
        1 => Shutdown::Write,
        _ => unimplemented!(),
      };
      blocking(base.sync(), move || {
        // Use UFCS for disambiguation
        Resource::shutdown(&mut resource, shutdown_mode)?;
        Ok(empty_buf())
      })
    }
  }
}

pub fn op_listen(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_listen().unwrap();
  let network = inner.network().unwrap();
  assert_eq!(network, "tcp");
  let address = inner.address().unwrap();

  state.check_net(&address)?;

  let addr = resolve_addr(address).wait()?;
  let listener = TcpListener::bind(&addr)?;
  let resource = resources::add_tcp_listener(listener);

  let builder = &mut FlatBufferBuilder::new();
  let inner =
    msg::ListenRes::create(builder, &msg::ListenResArgs { rid: resource.rid });
  let response_buf = serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::ListenRes,
      ..Default::default()
    },
  );
  ok_buf(response_buf)
}

fn new_conn(cmd_id: u32, tcp_stream: TcpStream) -> Result<Buf, ErrBox> {
  let tcp_stream_resource = resources::add_tcp_stream(tcp_stream);
  // TODO forward socket_addr to client.

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::NewConn::create(
    builder,
    &msg::NewConnArgs {
      rid: tcp_stream_resource.rid,
      ..Default::default()
    },
  );
  Ok(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::NewConn,
      ..Default::default()
    },
  ))
}
