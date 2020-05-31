use super::dispatch_json::{Deserialize, JsonOp};
use super::io::{StreamResource, StreamResourceHolder};
use crate::op_error::OpError;
use deno_core::CoreIsolateState;
use deno_core::ResourceTable;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
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
  pub path: String,
}

pub fn accept_unix(
  isolate_state: &mut CoreIsolateState,
  rid: u32,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let resource_table = isolate_state.resource_table.clone();
  {
    let _ = resource_table
      .borrow()
      .get::<UnixListenerResource>(rid)
      .ok_or_else(OpError::bad_resource_id)?;
  }
  let op = async move {
    let mut resource_table_ = resource_table.borrow_mut();
    let listener_resource = {
      resource_table_
        .get_mut::<UnixListenerResource>(rid)
        .ok_or_else(|| {
          OpError::bad_resource("Listener has been closed".to_string())
        })?
    };

    let (unix_stream, _socket_addr) =
      listener_resource.listener.accept().await?;
    drop(resource_table_);

    let local_addr = unix_stream.local_addr()?;
    let remote_addr = unix_stream.peer_addr()?;
    let mut resource_table_ = resource_table.borrow_mut();
    let rid = resource_table_.add(
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
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

pub fn receive_unix_packet(
  isolate_state: &mut CoreIsolateState,
  rid: u32,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");
  let mut zero_copy = zero_copy[0].clone();
  let resource_table = isolate_state.resource_table.clone();

  let op = async move {
    let mut resource_table_ = resource_table.borrow_mut();
    let resource = resource_table_
      .get_mut::<UnixDatagramResource>(rid)
      .ok_or_else(|| {
        OpError::bad_resource("Socket has been closed".to_string())
      })?;
    let (size, remote_addr) = resource.socket.recv_from(&mut zero_copy).await?;
    Ok(json!({
      "size": size,
      "remoteAddr": {
        "path": remote_addr.as_pathname(),
        "transport": "unixpacket",
      }
    }))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

pub fn listen_unix(
  resource_table: &mut ResourceTable,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), OpError> {
  if addr.exists() {
    remove_file(&addr).unwrap();
  }
  let listener = UnixListener::bind(&addr)?;
  let local_addr = listener.local_addr()?;
  let listener_resource = UnixListenerResource { listener };
  let rid = resource_table.add("unixListener", Box::new(listener_resource));

  Ok((rid, local_addr))
}

pub fn listen_unix_packet(
  resource_table: &mut ResourceTable,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), OpError> {
  if addr.exists() {
    remove_file(&addr).unwrap();
  }
  let socket = UnixDatagram::bind(&addr)?;
  let local_addr = socket.local_addr()?;
  let datagram_resource = UnixDatagramResource {
    socket,
    local_addr: local_addr.clone(),
  };
  let rid = resource_table.add("unixDatagram", Box::new(datagram_resource));

  Ok((rid, local_addr))
}
