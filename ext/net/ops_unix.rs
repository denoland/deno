// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use serde::Deserialize;
use serde::Serialize;
use tokio::net::UnixDatagram;
use tokio::net::UnixListener;
pub use tokio::net::UnixStream;

use crate::io::UnixStreamResource;
use crate::ops::NetError;
use crate::raw::NetworkListenerResource;

/// A utility function to map OsStrings to Strings
pub fn into_string(s: std::ffi::OsString) -> Result<String, NetError> {
  s.into_string().map_err(NetError::InvalidUtf8)
}

pub struct UnixDatagramResource {
  pub socket: AsyncRefCell<UnixDatagram>,
  pub cancel: CancelHandle,
}

impl Resource for UnixDatagramResource {
  fn name(&self) -> Cow<'_, str> {
    "unixDatagram".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

#[derive(Serialize)]
pub struct UnixAddr {
  pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct UnixListenArgs {
  pub path: String,
}

#[op2(async)]
#[serde]
pub async fn op_net_accept_unix(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(ResourceId, Option<String>, Option<String>), NetError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NetworkListenerResource<UnixListener>>(rid)
    .map_err(|_| NetError::ListenerClosed)?;
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or(NetError::ListenerBusy)?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (unix_stream, _socket_addr) = listener
    .accept()
    .try_or_cancel(cancel)
    .await
    .map_err(crate::ops::accept_err)?;

  let local_addr = unix_stream.local_addr()?;
  let remote_addr = unix_stream.peer_addr()?;
  let local_addr_path = local_addr.as_pathname().map(pathstring).transpose()?;
  let remote_addr_path =
    remote_addr.as_pathname().map(pathstring).transpose()?;
  let resource = UnixStreamResource::new(unix_stream.into_split());
  let mut state = state.borrow_mut();
  let rid = state.resource_table.add(resource);
  Ok((rid, local_addr_path, remote_addr_path))
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_net_connect_unix(
  state: Rc<RefCell<OpState>>,
  #[string] address_path: String,
) -> Result<(ResourceId, Option<String>, Option<String>), NetError> {
  let address_path = {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<PermissionsContainer>()
      .check_open(
        Cow::Owned(PathBuf::from(address_path)),
        OpenAccessKind::ReadWriteNoFollow,
        Some("Deno.connect()"),
      )
      .map_err(NetError::Permission)?
  };
  let unix_stream = UnixStream::connect(address_path).await?;
  let local_addr = unix_stream.local_addr()?;
  let remote_addr = unix_stream.peer_addr()?;
  let local_addr_path = local_addr.as_pathname().map(pathstring).transpose()?;
  let remote_addr_path =
    remote_addr.as_pathname().map(pathstring).transpose()?;
  let mut state_ = state.borrow_mut();
  let resource = UnixStreamResource::new(unix_stream.into_split());
  let rid = state_.resource_table.add(resource);
  Ok((rid, local_addr_path, remote_addr_path))
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_net_recv_unixpacket(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] mut buf: JsBuffer,
) -> Result<(usize, Option<String>), NetError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<UnixDatagramResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket)
    .try_borrow_mut()
    .ok_or(NetError::SocketBusy)?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (nread, remote_addr) =
    socket.recv_from(&mut buf).try_or_cancel(cancel).await?;
  let path = remote_addr.as_pathname().map(pathstring).transpose()?;
  Ok((nread, path))
}

#[op2(async, stack_trace)]
#[number]
pub async fn op_net_send_unixpacket(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] address_path: String,
  #[buffer] zero_copy: JsBuffer,
) -> Result<usize, NetError> {
  let address_path = {
    let mut s = state.borrow_mut();
    s.borrow_mut::<PermissionsContainer>()
      .check_open(
        Cow::Owned(PathBuf::from(address_path)),
        OpenAccessKind::WriteNoFollow,
        Some("Deno.DatagramConn.send()"),
      )
      .map_err(NetError::Permission)?
  };

  let resource = state
    .borrow()
    .resource_table
    .get::<UnixDatagramResource>(rid)
    .map_err(|_| NetError::SocketClosedNotConnected)?;
  let socket = RcRef::map(&resource, |r| &r.socket)
    .try_borrow_mut()
    .ok_or(NetError::SocketBusy)?;
  let nwritten = socket.send_to(&zero_copy, address_path).await?;

  Ok(nwritten)
}

#[op2(stack_trace)]
#[serde]
pub fn op_net_listen_unix(
  state: &mut OpState,
  #[string] address_path: &str,
  #[string] api_name: &str,
) -> Result<(ResourceId, Option<String>), NetError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  let api_call_expr = format!("{}()", api_name);
  let address_path = permissions
    .check_open(
      Cow::Borrowed(Path::new(address_path)),
      OpenAccessKind::ReadWriteNoFollow,
      Some(&api_call_expr),
    )
    .map_err(NetError::Permission)?;
  let listener = UnixListener::bind(address_path)?;
  let local_addr = listener.local_addr()?;
  let pathname = local_addr.as_pathname().map(pathstring).transpose()?;
  let listener_resource = NetworkListenerResource::new(listener);
  let rid = state.resource_table.add(listener_resource);
  Ok((rid, pathname))
}

pub fn net_listen_unixpacket(
  state: &mut OpState,
  address_path: &str,
) -> Result<(ResourceId, Option<String>), NetError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  let address_path = permissions
    .check_open(
      Cow::Borrowed(Path::new(address_path)),
      OpenAccessKind::ReadWriteNoFollow,
      Some("Deno.listenDatagram()"),
    )
    .map_err(NetError::Permission)?;
  let socket = UnixDatagram::bind(address_path)?;
  let local_addr = socket.local_addr()?;
  let pathname = local_addr.as_pathname().map(pathstring).transpose()?;
  let datagram_resource = UnixDatagramResource {
    socket: AsyncRefCell::new(socket),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(datagram_resource);
  Ok((rid, pathname))
}

#[op2(stack_trace)]
#[serde]
pub fn op_net_listen_unixpacket(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<(ResourceId, Option<String>), NetError> {
  super::check_unstable(state, "Deno.listenDatagram");
  net_listen_unixpacket(state, path)
}

#[op2(stack_trace)]
#[serde]
pub fn op_node_unstable_net_listen_unixpacket(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<(ResourceId, Option<String>), NetError> {
  net_listen_unixpacket(state, path)
}

pub fn pathstring(pathname: &Path) -> Result<String, NetError> {
  into_string(pathname.into())
}

/// Check if fd is a socket using fstat
fn is_socket_fd(fd: i32) -> bool {
  // SAFETY: It is safe to zero-initialize a libc::stat struct
  let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
  // SAFETY: fd is a valid file descriptor, stat_buf is a valid pointer
  let result = unsafe { libc::fstat(fd, &mut stat_buf) };
  if result != 0 {
    return false;
  }
  // S_IFSOCK = 0o140000 on most Unix systems
  (stat_buf.st_mode & libc::S_IFMT) == libc::S_IFSOCK
}

#[op2(fast)]
#[smi]
pub fn op_net_unix_stream_from_fd(
  state: &mut OpState,
  fd: i32,
) -> Result<ResourceId, NetError> {
  use std::os::unix::io::FromRawFd;

  // Validate fd is non-negative
  if fd < 0 {
    return Err(NetError::Io(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "Invalid file descriptor",
    )));
  }

  // Check if fd is a socket - if not, we can't use UnixStream
  if !is_socket_fd(fd) {
    return Err(NetError::Io(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "File descriptor is not a socket",
    )));
  }

  // SAFETY: The caller is responsible for passing a valid fd that they own.
  // The fd will be owned by the created UnixStream from this point on.
  let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(fd) };
  std_stream.set_nonblocking(true)?;
  let unix_stream = UnixStream::from_std(std_stream)?;
  let resource = UnixStreamResource::new(unix_stream.into_split());
  let rid = state.resource_table.add(resource);
  Ok(rid)
}
