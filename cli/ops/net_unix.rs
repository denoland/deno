use crate::ops::io::StreamResource;
use crate::ops::io::StreamResourceHolder;
use crate::ops::net::AcceptArgs;
use crate::ops::net::ReceiveArgs;
use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use serde_derive::Deserialize;
use serde_json::Value;
use std::fs::remove_file;
use std::os::unix;
pub use std::path::Path;
use std::rc::Rc;
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
  state: Rc<State>,
  args: AcceptArgs,
  _bufs: BufVec,
) -> Result<Value, ErrBox> {
  let rid = args.rid as u32;

  let mut resource_table_ = state.resource_table.borrow_mut();
  let listener_resource = {
    resource_table_
      .get_mut::<UnixListenerResource>(rid)
      .ok_or_else(|| ErrBox::bad_resource("Listener has been closed"))?
  };
  let (unix_stream, _socket_addr) = listener_resource.listener.accept().await?;
  drop(resource_table_);

  let local_addr = unix_stream.local_addr()?;
  let remote_addr = unix_stream.peer_addr()?;
  let mut resource_table_ = state.resource_table.borrow_mut();
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
}

pub(crate) async fn receive_unix_packet(
  state: Rc<State>,
  args: ReceiveArgs,
  bufs: BufVec,
) -> Result<Value, ErrBox> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");

  let rid = args.rid as u32;
  let mut buf = bufs.into_iter().next().unwrap();

  let mut resource_table_ = state.resource_table.borrow_mut();
  let resource = resource_table_
    .get_mut::<UnixDatagramResource>(rid)
    .ok_or_else(|| ErrBox::bad_resource("Socket has been closed"))?;
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
  state: &State,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), ErrBox> {
  if addr.exists() {
    remove_file(&addr).unwrap();
  }
  let listener = UnixListener::bind(&addr)?;
  let local_addr = listener.local_addr()?;
  let listener_resource = UnixListenerResource { listener };
  let rid = state
    .resource_table
    .borrow_mut()
    .add("unixListener", Box::new(listener_resource));

  Ok((rid, local_addr))
}

pub fn listen_unix_packet(
  state: &State,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), ErrBox> {
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
    .borrow_mut()
    .add("unixDatagram", Box::new(datagram_resource));

  Ok((rid, local_addr))
}
