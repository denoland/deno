// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::io::UnixStreamResource;
use crate::raw::NetworkListenerResource;
use crate::NetPermissions;
use deno_core::error::bad_resource;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use tokio::net::UnixDatagram;
use tokio::net::UnixListener;
pub use tokio::net::UnixStream;

/// A utility function to map OsStrings to Strings
pub fn into_string(s: std::ffi::OsString) -> Result<String, AnyError> {
  s.into_string().map_err(|s| {
    let message = format!("File name or path {s:?} is not valid UTF-8");
    custom_error("InvalidData", message)
  })
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
) -> Result<(ResourceId, Option<String>, Option<String>), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NetworkListenerResource<UnixListener>>(rid)
    .map_err(|_| bad_resource("Listener has been closed"))?;
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Listener already in use"))?;
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

#[op2(async)]
#[serde]
pub async fn op_net_connect_unix<NP>(
  state: Rc<RefCell<OpState>>,
  #[string] address_path: String,
) -> Result<(ResourceId, Option<String>, Option<String>), AnyError>
where
  NP: NetPermissions + 'static,
{
  let address_path = {
    let mut state_ = state.borrow_mut();
    let address_path = state_
      .borrow_mut::<NP>()
      .check_read(&address_path, "Deno.connect()")?;
    _ = state_
      .borrow_mut::<NP>()
      .check_write_path(&address_path, "Deno.connect()")?;
    address_path
  };
  let unix_stream = UnixStream::connect(&address_path).await?;
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

#[op2(async)]
#[serde]
pub async fn op_net_recv_unixpacket(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] mut buf: JsBuffer,
) -> Result<(usize, Option<String>), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<UnixDatagramResource>(rid)
    .map_err(|_| bad_resource("Socket has been closed"))?;
  let socket = RcRef::map(&resource, |r| &r.socket)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Socket already in use"))?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (nread, remote_addr) =
    socket.recv_from(&mut buf).try_or_cancel(cancel).await?;
  let path = remote_addr.as_pathname().map(pathstring).transpose()?;
  Ok((nread, path))
}

#[op2(async)]
#[number]
pub async fn op_net_send_unixpacket<NP>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] address_path: String,
  #[buffer] zero_copy: JsBuffer,
) -> Result<usize, AnyError>
where
  NP: NetPermissions + 'static,
{
  let address_path = {
    let mut s = state.borrow_mut();
    s.borrow_mut::<NP>()
      .check_write(&address_path, "Deno.DatagramConn.send()")?
  };

  let resource = state
    .borrow()
    .resource_table
    .get::<UnixDatagramResource>(rid)
    .map_err(|_| custom_error("NotConnected", "Socket has been closed"))?;
  let socket = RcRef::map(&resource, |r| &r.socket)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Socket already in use"))?;
  let nwritten = socket.send_to(&zero_copy, address_path).await?;

  Ok(nwritten)
}

#[op2]
#[serde]
pub fn op_net_listen_unix<NP>(
  state: &mut OpState,
  #[string] address_path: String,
  #[string] api_name: String,
) -> Result<(ResourceId, Option<String>), AnyError>
where
  NP: NetPermissions + 'static,
{
  let permissions = state.borrow_mut::<NP>();
  let api_call_expr = format!("{}()", api_name);
  let address_path = permissions.check_read(&address_path, &api_call_expr)?;
  _ = permissions.check_write_path(&address_path, &api_call_expr)?;
  let listener = UnixListener::bind(address_path)?;
  let local_addr = listener.local_addr()?;
  let pathname = local_addr.as_pathname().map(pathstring).transpose()?;
  let listener_resource = NetworkListenerResource::new(listener);
  let rid = state.resource_table.add(listener_resource);
  Ok((rid, pathname))
}

pub fn net_listen_unixpacket<NP>(
  state: &mut OpState,
  address_path: String,
) -> Result<(ResourceId, Option<String>), AnyError>
where
  NP: NetPermissions + 'static,
{
  let permissions = state.borrow_mut::<NP>();
  let address_path =
    permissions.check_read(&address_path, "Deno.listenDatagram()")?;
  _ = permissions.check_write_path(&address_path, "Deno.listenDatagram()")?;
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

#[op2]
#[serde]
pub fn op_net_listen_unixpacket<NP>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<(ResourceId, Option<String>), AnyError>
where
  NP: NetPermissions + 'static,
{
  super::check_unstable(state, "Deno.listenDatagram");
  net_listen_unixpacket::<NP>(state, path)
}

#[op2]
#[serde]
pub fn op_node_unstable_net_listen_unixpacket<NP>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<(ResourceId, Option<String>), AnyError>
where
  NP: NetPermissions + 'static,
{
  net_listen_unixpacket::<NP>(state, path)
}

pub fn pathstring(pathname: &Path) -> Result<String, AnyError> {
  into_string(pathname.into())
}
