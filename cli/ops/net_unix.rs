use super::dispatch_json::{Deserialize, JsonOp};
use super::io::{StreamResource, StreamResourceHolder};
use crate::op_error::OpError;
use crate::state::State;
use futures::future::FutureExt;

use deno_core::*;
use std::fs::remove_file;
use std::os::unix;
pub use std::path::Path;
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
  pub address: String,
}

pub fn accept_unix(
  isolate: &mut deno_core::Isolate,
  state: &State,
  rid: u32,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let resource_table = isolate.resource_table.clone();
  let state_ = state.clone();
  {
    let state = resource_table
      .get::<UnixListenerResource>(rid)
      .ok_or_else(OpError::bad_resource_id)?;
  }
  let op = async move {
    let listener_resource = {
      let resource_table = std::rc::Rc::get_mut(&mut resource_table).unwrap();
      resource_table
        .get_mut::<UnixListenerResource>(rid)
        .ok_or_else(|| {
          OpError::bad_resource("Listener has been closed".to_string())
        })?
    };
    let (unix_stream, _socket_addr) =
      listener_resource.listener.accept().await?;
    let local_addr = unix_stream.local_addr()?;
    let remote_addr = unix_stream.peer_addr()?;
    let resource_table = std::rc::Rc::get_mut(&mut resource_table).unwrap();
    let rid = resource_table.add(
      "unixStream",
      Box::new(StreamResourceHolder::new(StreamResource::UnixStream(
        unix_stream,
      ))),
    );
    Ok(json!({
      "rid": rid,
      "localAddr": {
        "address": local_addr.as_pathname(),
        "transport": "unix",
      },
      "remoteAddr": {
        "address": remote_addr.as_pathname(),
        "transport": "unix",
      }
    }))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

pub fn receive_unix_packet(
  isolate: &mut deno_core::Isolate,
  state: &State,
  rid: u32,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let mut buf = zero_copy.unwrap();
  let resource_table = isolate.resource_table.clone();

  let op = async move {
    let resource_table = std::rc::Rc::get_mut(&mut resource_table).unwrap();
    let resource = resource_table
      .get_mut::<UnixDatagramResource>(rid)
      .ok_or_else(|| {
        OpError::bad_resource("Socket has been closed".to_string())
      })?;
    let (size, remote_addr) = resource.socket.recv_from(&mut buf).await?;
    Ok(json!({
      "size": size,
      "remoteAddr": {
        "address": remote_addr.as_pathname(),
        "transport": "unixpacket",
      }
    }))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

pub fn listen_unix(
  state: &State,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), OpError> {
  let mut state = state.borrow_mut();
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
  state: &State,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), OpError> {
  let mut state = state.borrow_mut();
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
