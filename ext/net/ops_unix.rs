// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::mem;
#[cfg(target_os = "android")]
use std::os::android::net::SocketAddrExt;
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::os::linux::net::SocketAddrExt;
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::os::unix::ffi::OsStrExt;
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::ptr;
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

use crate::check_unix_socket_path;
use crate::io::UnixStreamResource;
use crate::is_unix_socket_abstract_path;
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

/// A wrapper around `UnixListener` that holds the socket path and removes it on drop.
pub struct UnixListenerWithPath {
  listener: UnixListener,
  path: Option<PathBuf>,
}

impl UnixListenerWithPath {
  pub fn new(listener: UnixListener, path: PathBuf) -> Self {
    let path = if is_unix_socket_abstract_path(&path) {
      None
    } else {
      Some(path)
    };
    Self { listener, path }
  }

  pub async fn accept(
    &self,
  ) -> std::io::Result<(UnixStream, tokio::net::unix::SocketAddr)> {
    self.listener.accept().await
  }

  pub fn local_addr(&self) -> std::io::Result<tokio::net::unix::SocketAddr> {
    self.listener.local_addr()
  }
}

impl Drop for UnixListenerWithPath {
  fn drop(&mut self) {
    if let Some(path) = &self.path {
      #[allow(clippy::disallowed_methods, reason = "requires real fs")]
      let _ = std::fs::remove_file(path);
    }
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

#[op2]
pub async fn op_net_accept_unix(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(ResourceId, Option<String>, Option<String>), NetError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NetworkListenerResource<UnixListenerWithPath>>(rid)
    .map_err(|_| NetError::ListenerClosed)?;
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or(NetError::ListenerBusy)?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (unix_stream, remote_addr) = listener
    .accept()
    .try_or_cancel(cancel)
    .await
    .map_err(crate::ops::accept_err)?;

  let local_addr = unix_stream.local_addr()?;
  let local_addr_path = unix_socket_addr_path(&local_addr)?;
  let remote_addr_path = unix_socket_addr_path(&remote_addr)?;
  let resource = UnixStreamResource::new(unix_stream.into_split());
  let mut state = state.borrow_mut();
  let rid = state.resource_table.add(resource);
  Ok((rid, local_addr_path, remote_addr_path))
}

#[op2(stack_trace)]
pub async fn op_net_connect_unix(
  state: Rc<RefCell<OpState>>,
  #[string] address_path: String,
) -> Result<(ResourceId, Option<String>, Option<String>), NetError> {
  let address_path = {
    let mut state = state.borrow_mut();
    check_unix_socket_path(
      state.borrow_mut::<PermissionsContainer>(),
      Cow::Owned(PathBuf::from(address_path)),
      OpenAccessKind::ReadWriteNoFollow,
      Some("Deno.connect()"),
    )?
  };
  let unix_stream = UnixStream::connect(address_path).await?;
  let local_addr = unix_stream.local_addr()?;
  let remote_addr = unix_stream.peer_addr()?;
  let local_addr_path = unix_socket_addr_path(&local_addr)?;
  let remote_addr_path = unix_socket_addr_path(&remote_addr)?;
  let mut state_ = state.borrow_mut();
  let resource = UnixStreamResource::new(unix_stream.into_split());
  let rid = state_.resource_table.add(resource);
  Ok((rid, local_addr_path, remote_addr_path))
}

#[op2(stack_trace)]
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
  let path = unix_socket_addr_path(&remote_addr)?;
  Ok((nread, path))
}

#[op2(stack_trace)]
#[number]
pub async fn op_net_send_unixpacket(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] address_path: String,
  #[buffer] zero_copy: JsBuffer,
) -> Result<usize, NetError> {
  let address_path = {
    let mut s = state.borrow_mut();
    check_unix_socket_path(
      s.borrow_mut::<PermissionsContainer>(),
      Cow::Owned(PathBuf::from(address_path)),
      OpenAccessKind::WriteNoFollow,
      Some("Deno.DatagramConn.send()"),
    )?
  };

  let resource = state
    .borrow()
    .resource_table
    .get::<UnixDatagramResource>(rid)
    .map_err(|_| NetError::SocketClosedNotConnected)?;
  let socket = RcRef::map(&resource, |r| &r.socket)
    .try_borrow_mut()
    .ok_or(NetError::SocketBusy)?;
  let nwritten =
    send_to_unix_datagram(&socket, &zero_copy, address_path.as_ref()).await?;

  Ok(nwritten)
}

#[op2(stack_trace)]
pub fn op_net_listen_unix(
  state: &mut OpState,
  #[string] address_path: &str,
  #[string] api_name: &str,
) -> Result<(ResourceId, Option<String>), NetError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  let api_call_expr = format!("{}()", api_name);
  let address_path = check_unix_socket_path(
    permissions,
    Cow::Borrowed(Path::new(address_path)),
    OpenAccessKind::ReadWriteNoFollow,
    Some(&api_call_expr),
  )?;
  let listener = UnixListener::bind(&address_path)?;
  let local_addr = listener.local_addr()?;
  let pathname = unix_socket_addr_path(&local_addr)?;
  let listener_with_path =
    UnixListenerWithPath::new(listener, address_path.to_path_buf());
  let listener_resource = NetworkListenerResource::new(listener_with_path);
  let rid = state.resource_table.add(listener_resource);
  Ok((rid, pathname))
}

pub fn net_listen_unixpacket(
  state: &mut OpState,
  address_path: Option<&str>,
) -> Result<(ResourceId, Option<String>), NetError> {
  let socket = match address_path {
    // Bind to the given path
    Some(address_path) => {
      let permissions = state.borrow_mut::<PermissionsContainer>();
      let address_path = check_unix_socket_path(
        permissions,
        Cow::Borrowed(Path::new(address_path)),
        OpenAccessKind::ReadWriteNoFollow,
        Some("Deno.listenDatagram()"),
      )?;
      bind_unix_datagram(address_path.as_ref())?
    }

    // Leave the socket unbound: it can send messages, but not receive them
    None => UnixDatagram::unbound()?,
  };
  let local_addr = socket.local_addr()?;
  let pathname = unix_socket_addr_path(&local_addr)?;
  let datagram_resource = UnixDatagramResource {
    socket: AsyncRefCell::new(socket),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(datagram_resource);
  Ok((rid, pathname))
}

#[op2(stack_trace)]
pub fn op_net_listen_unixpacket(
  state: &mut OpState,
  #[string] path: Option<String>, // todo: Option<&str> not supported in ops yet
) -> Result<(ResourceId, Option<String>), NetError> {
  super::check_unstable(state, "Deno.listenDatagram");
  net_listen_unixpacket(state, path.as_deref())
}

#[op2(stack_trace)]
pub fn op_node_unstable_net_listen_unixpacket(
  state: &mut OpState,
  #[string] path: Option<String>, // todo: Option<&str> not supported in ops yet
) -> Result<(ResourceId, Option<String>), NetError> {
  net_listen_unixpacket(state, path.as_deref())
}

pub fn pathstring(pathname: &Path) -> Result<String, NetError> {
  into_string(pathname.into())
}

fn bind_unix_datagram(address_path: &Path) -> Result<UnixDatagram, NetError> {
  #[cfg(any(target_os = "android", target_os = "linux"))]
  if let Some((addr, addrlen)) = unix_sockaddr_from_abstract_path(address_path)?
  {
    let socket =
      socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::DGRAM, None)?;
    socket.set_nonblocking(true)?;
    // SAFETY: `addr` is initialized as a valid `sockaddr_un`, and `addrlen`
    // is the initialized byte length for this abstract Unix socket address.
    let result = unsafe {
      libc::bind(
        socket.as_raw_fd(),
        (&addr as *const libc::sockaddr_un).cast::<libc::sockaddr>(),
        addrlen,
      )
    };
    if result == -1 {
      return Err(std::io::Error::last_os_error().into());
    }
    let socket = std::os::unix::net::UnixDatagram::from(socket);
    return Ok(UnixDatagram::from_std(socket)?);
  }

  Ok(UnixDatagram::bind(address_path)?)
}

async fn send_to_unix_datagram(
  socket: &UnixDatagram,
  buf: &[u8],
  address_path: &Path,
) -> Result<usize, NetError> {
  #[cfg(any(target_os = "android", target_os = "linux"))]
  if let Some((addr, addrlen)) = unix_sockaddr_from_abstract_path(address_path)?
  {
    return Ok(
      socket
        .async_io(tokio::io::Interest::WRITABLE, || {
          // SAFETY: `socket.as_raw_fd()` is a valid Unix datagram socket,
          // `buf` is a valid byte slice, and `addr`/`addrlen` describe a valid
          // abstract Unix socket address for the duration of this call.
          let result = unsafe {
            libc::sendto(
              socket.as_raw_fd(),
              buf.as_ptr().cast(),
              buf.len(),
              0,
              (&addr as *const libc::sockaddr_un).cast::<libc::sockaddr>(),
              addrlen,
            )
          };
          if result == -1 {
            Err(std::io::Error::last_os_error())
          } else {
            Ok(result as usize)
          }
        })
        .await?,
    );
  }

  Ok(socket.send_to(buf, address_path).await?)
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn unix_sockaddr_from_abstract_path(
  path: &Path,
) -> std::io::Result<Option<(libc::sockaddr_un, libc::socklen_t)>> {
  let bytes = path.as_os_str().as_bytes();
  if bytes.first() != Some(&0) {
    return Ok(None);
  }

  // SAFETY: zeroed `sockaddr_un` is valid. The address family is set below, and
  // the zero-filled `sun_path` is exactly how Linux abstract addresses start.
  let mut addr = unsafe { mem::zeroed::<libc::sockaddr_un>() };
  if bytes.len() > addr.sun_path.len() {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "path must be shorter than SUN_LEN",
    ));
  }

  addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
  // SAFETY: `bytes` and `addr.sun_path` do not overlap, and the bounds check
  // above ensures the copy stays within `sun_path`.
  unsafe {
    ptr::copy_nonoverlapping(
      bytes.as_ptr(),
      addr.sun_path.as_mut_ptr().cast(),
      bytes.len(),
    );
  }

  let base = &addr as *const libc::sockaddr_un as usize;
  let path = &addr.sun_path as *const _ as usize;
  let addrlen = (path - base + bytes.len()) as libc::socklen_t;
  Ok(Some((addr, addrlen)))
}

pub fn unix_socket_addr_path(
  addr: &tokio::net::unix::SocketAddr,
) -> Result<Option<String>, NetError> {
  if let Some(pathname) = addr.as_pathname() {
    return pathstring(pathname).map(Some);
  }

  #[cfg(any(target_os = "android", target_os = "linux"))]
  if let Some(name) =
    std::os::unix::net::SocketAddr::from(addr.clone()).as_abstract_name()
  {
    let mut path = Vec::with_capacity(name.len() + 1);
    path.push(0);
    path.extend_from_slice(name);
    return into_string(std::ffi::OsString::from_vec(path)).map(Some);
  }

  Ok(None)
}
