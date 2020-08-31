use super::dispatch_json::{Deserialize, Value};
use super::io::{StreamResource, StreamResourceHolder};
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::ResourceTable;
use std::cell::RefCell;
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

pub async fn accept_unix(
  resource_table: Rc<RefCell<ResourceTable>>,
  rid: u32,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  let mut resource_table_ = resource_table.borrow_mut();
  let listener_resource = {
    resource_table_
      .get_mut::<UnixListenerResource>(rid)
      .ok_or_else(|| ErrBox::bad_resource("Listener has been closed"))?
  };

  let (unix_stream, _socket_addr) = listener_resource.listener.accept().await?;
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
}

pub async fn receive_unix_packet(
  resource_table: Rc<RefCell<ResourceTable>>,
  rid: u32,
  zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");
  let mut zero_copy = zero_copy[0].clone();

  let mut resource_table_ = resource_table.borrow_mut();
  let resource = resource_table_
    .get_mut::<UnixDatagramResource>(rid)
    .ok_or_else(|| ErrBox::bad_resource("Socket has been closed"))?;
  let (size, remote_addr) = resource.socket.recv_from(&mut zero_copy).await?;
  Ok(json!({
    "size": size,
    "remoteAddr": {
      "path": remote_addr.as_pathname(),
      "transport": "unixpacket",
    }
  }))
}

pub fn listen_unix(
  resource_table: &mut ResourceTable,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), ErrBox> {
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
  let rid = resource_table.add("unixDatagram", Box::new(datagram_resource));

  Ok((rid, local_addr))
}
