// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ops::io::StreamResource;
use crate::ops::io::StreamResourceHolder;
use crate::ops::net::AcceptArgs;
use crate::ops::net::ReceiveArgs;
use deno_core::error::bad_resource;
use deno_core::error::AnyError;
use deno_core::futures::future::poll_fn;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use serde::Deserialize;
use std::cell::RefCell;
use std::fs::remove_file;
use std::os::unix;
use std::path::Path;
use std::rc::Rc;
use std::task::Poll;
use tokio::net::UnixDatagram;
use tokio::net::UnixListener;
pub use tokio::net::UnixStream;

struct UnixListenerResource {
  listener: UnixListener,
}

pub struct UnixDatagramResource {
  pub socket: UnixDatagram,
  pub local_addr: unix::net::SocketAddr,
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

  let accept_fut = poll_fn(|cx| {
    let mut state = state.borrow_mut();
    let listener_resource = state
      .resource_table
      .get_mut::<UnixListenerResource>(rid)
      .ok_or_else(|| bad_resource("Listener has been closed"))?;
    let listener = &mut listener_resource.listener;
    use deno_core::futures::StreamExt;
    match listener.poll_next_unpin(cx) {
      Poll::Ready(Some(stream)) => {
        //listener_resource.untrack_task();
        Poll::Ready(stream)
      }
      Poll::Ready(None) => todo!(),
      Poll::Pending => {
        //listener_resource.track_task(cx)?;
        Poll::Pending
      }
    }
    .map_err(AnyError::from)
  });
  let unix_stream = accept_fut.await?;

  let local_addr = unix_stream.local_addr()?;
  let remote_addr = unix_stream.peer_addr()?;
  let mut state = state.borrow_mut();
  let rid = state.resource_table.add(
    "unixStream",
    Box::new(StreamResourceHolder::new(StreamResource::UnixStream(
      unix_stream,
    ))),
  );
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

  let mut state = state.borrow_mut();
  let resource = state
    .resource_table
    .get_mut::<UnixDatagramResource>(rid)
    .ok_or_else(|| bad_resource("Socket has been closed"))?;
  let (size, remote_addr) = resource.socket.recv_from(&mut buf).await?;
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
  let listener_resource = UnixListenerResource { listener };
  let rid = state
    .resource_table
    .add("unixListener", Box::new(listener_resource));

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
    socket,
    local_addr: local_addr.clone(),
  };
  let rid = state
    .resource_table
    .add("unixDatagram", Box::new(datagram_resource));

  Ok((rid, local_addr))
}
