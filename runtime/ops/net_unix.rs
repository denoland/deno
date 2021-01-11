// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ops::io::StreamResource;
use crate::ops::net::AcceptArgs;
use crate::ops::net::ReceiveArgs;
use deno_core::error::bad_resource;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::AsyncRefCell;
use deno_core::BufVec;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fs::remove_file;
use std::os::unix;
use std::path::Path;
use std::rc::Rc;
use tokio::net::UnixDatagram;
use tokio::net::UnixListener;
pub use tokio::net::UnixStream;

struct UnixListenerResource {
  listener: AsyncRefCell<UnixListener>,
  cancel: CancelHandle,
}

impl Resource for UnixListenerResource {
  fn name(&self) -> Cow<str> {
    "unixListener".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

pub struct UnixDatagramResource {
  pub socket: AsyncRefCell<UnixDatagram>,
  pub cancel: CancelHandle,
}

impl Resource for UnixDatagramResource {
  fn name(&self) -> Cow<str> {
    "unixDatagram".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

#[derive(Deserialize)]
pub struct UnixListenArgs {
  pub path: String,
}

pub(crate) async fn accept_unix(
  state: Rc<RefCell<OpState>>,
  args: AcceptArgs,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let rid = args.rid as u32;

  let resource = state
    .borrow()
    .resource_table
    .get::<UnixListenerResource>(rid)
    .ok_or_else(|| bad_resource("Listener has been closed"))?;
  let mut listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Listener already in use"))?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (unix_stream, _socket_addr) =
    listener.accept().try_or_cancel(cancel).await?;

  let local_addr = unix_stream.local_addr()?;
  let remote_addr = unix_stream.peer_addr()?;
  let resource = StreamResource::unix_stream(unix_stream);
  let mut state = state.borrow_mut();
  let rid = state.resource_table.add(resource);
  Ok(json!({
    "rid": rid,
    "localAddr": {
      "path": local_addr.as_pathname(),
      "transport": "unix",
    },
    "remoteAddr": {
      "path": remote_addr.as_pathname(),
      "transport": "unix",
    }
  }))
}

pub(crate) async fn receive_unix_packet(
  state: Rc<RefCell<OpState>>,
  args: ReceiveArgs,
  bufs: BufVec,
) -> Result<Value, AnyError> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");

  let rid = args.rid as u32;
  let mut buf = bufs.into_iter().next().unwrap();

  let resource = state
    .borrow()
    .resource_table
    .get::<UnixDatagramResource>(rid)
    .ok_or_else(|| bad_resource("Socket has been closed"))?;
  let mut socket = RcRef::map(&resource, |r| &r.socket)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Socket already in use"))?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (size, remote_addr) =
    socket.recv_from(&mut buf).try_or_cancel(cancel).await?;
  Ok(json!({
    "size": size,
    "remoteAddr": {
      "path": remote_addr.as_pathname(),
      "transport": "unixpacket",
    }
  }))
}

pub fn listen_unix(
  state: &mut OpState,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), AnyError> {
  if addr.exists() {
    remove_file(&addr).unwrap();
  }
  let listener = UnixListener::bind(&addr)?;
  let local_addr = listener.local_addr()?;
  let listener_resource = UnixListenerResource {
    listener: AsyncRefCell::new(listener),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(listener_resource);

  Ok((rid, local_addr))
}

pub fn listen_unix_packet(
  state: &mut OpState,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), AnyError> {
  if addr.exists() {
    remove_file(&addr).unwrap();
  }
  let socket = UnixDatagram::bind(&addr)?;
  let local_addr = socket.local_addr()?;
  let datagram_resource = UnixDatagramResource {
    socket: AsyncRefCell::new(socket),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(datagram_resource);

  Ok((rid, local_addr))
}
