// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use socket2::Domain;
use socket2::Protocol;
use socket2::Type;

/// Our per-process `Connections`. We can use this to find an existent listener for
/// a given local address and clone its socket for us to listen on in our thread.
static CONNS: std::sync::OnceLock<std::sync::Mutex<Connections>> =
  std::sync::OnceLock::new();

/// Maintains a map of listening address to `TcpConnection`.
#[derive(Default)]
struct Connections {
  tcp: HashMap<SocketAddr, Arc<TcpConnection>>,
}

/// Holds an open listener. We clone the underlying file descriptor (unix) or socket handle (Windows)
/// and then listen on our copy of it.
pub struct TcpConnection {
  /// The pristine FD that we'll clone for each LB listener
  #[cfg(unix)]
  sock: std::os::fd::OwnedFd,
  #[cfg(not(unix))]
  sock: std::os::windows::io::OwnedSocket,
  key: SocketAddr,
}

impl TcpConnection {
  /// Boot a load-balanced TCP connection
  pub fn start(key: SocketAddr) -> std::io::Result<Self> {
    let listener = bind_socket_and_listen(key, false)?;
    let sock = listener.into();

    Ok(Self { sock, key })
  }

  fn listener(&self) -> std::io::Result<tokio::net::TcpListener> {
    let listener = std::net::TcpListener::from(self.sock.try_clone()?);
    let listener = tokio::net::TcpListener::from_std(listener)?;
    Ok(listener)
  }
}

/// A TCP socket listener that optionally allows for round-robin load-balancing in-process.
pub struct TcpListener {
  listener: Option<tokio::net::TcpListener>,
  conn: Option<Arc<TcpConnection>>,
}

/// Does this platform implement `SO_REUSEPORT` in a load-balancing manner?
const REUSE_PORT_LOAD_BALANCES: bool =
  cfg!(any(target_os = "android", target_os = "linux"));

impl TcpListener {
  /// Bind to a port. On Linux, or when we don't have `SO_REUSEPORT` set, we just bind the port directly.
  /// On other platforms, we emulate `SO_REUSEPORT` by cloning the socket and having each clone race to
  /// accept every connection.
  ///
  /// ## Why not `SO_REUSEPORT`?
  ///
  /// The `SO_REUSEPORT` socket option allows multiple sockets on the same host to bind to the same port. This is
  /// particularly useful for load balancing or implementing high availability in server applications.
  ///
  /// On Linux, `SO_REUSEPORT` allows multiple sockets to bind to the same port, and the kernel will load
  /// balance incoming connections among those sockets. Each socket can accept connections independently.
  /// This is useful for scenarios where you want to distribute incoming connections among multiple processes
  /// or threads.
  ///
  /// On macOS (which is based on BSD), the behaviour of `SO_REUSEPORT` is slightly different. When `SO_REUSEPORT` is set,
  /// multiple sockets can still bind to the same port, but the kernel does not perform load balancing as it does on Linux.
  /// Instead, it follows a "last bind wins" strategy. This means that the most recently bound socket will receive
  /// incoming connections exclusively, while the previously bound sockets will not receive any connections.
  /// This behaviour is less useful for load balancing compared to Linux, but it can still be valuable in certain scenarios.
  pub fn bind(
    socket_addr: SocketAddr,
    reuse_port: bool,
  ) -> std::io::Result<Self> {
    if REUSE_PORT_LOAD_BALANCES && reuse_port {
      Self::bind_load_balanced(socket_addr)
    } else {
      Self::bind_direct(socket_addr, reuse_port)
    }
  }

  /// Bind directly to the port, passing `reuse_port` directly to the socket. On platforms other
  /// than Linux, `reuse_port` does not do any load balancing.
  pub fn bind_direct(
    socket_addr: SocketAddr,
    reuse_port: bool,
  ) -> std::io::Result<Self> {
    // We ignore `reuse_port` on platforms other than Linux to match the existing behaviour.
    let listener = bind_socket_and_listen(socket_addr, reuse_port)?;
    Ok(Self {
      listener: Some(tokio::net::TcpListener::from_std(listener)?),
      conn: None,
    })
  }

  /// Bind to the port in a load-balanced manner.
  pub fn bind_load_balanced(socket_addr: SocketAddr) -> std::io::Result<Self> {
    let tcp = &mut CONNS.get_or_init(Default::default).lock().unwrap().tcp;
    if let Some(conn) = tcp.get(&socket_addr) {
      let listener = Some(conn.listener()?);
      return Ok(Self {
        listener,
        conn: Some(conn.clone()),
      });
    }
    let conn = Arc::new(TcpConnection::start(socket_addr)?);
    let listener = Some(conn.listener()?);
    tcp.insert(socket_addr, conn.clone());
    Ok(Self {
      listener,
      conn: Some(conn),
    })
  }

  pub async fn accept(
    &self,
  ) -> std::io::Result<(tokio::net::TcpStream, SocketAddr)> {
    let (tcp, addr) = self.listener.as_ref().unwrap().accept().await?;
    Ok((tcp, addr))
  }

  pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
    self.listener.as_ref().unwrap().local_addr()
  }
}

impl Drop for TcpListener {
  fn drop(&mut self) {
    // If we're in load-balancing mode
    if let Some(conn) = self.conn.take() {
      let mut tcp = CONNS.get().unwrap().lock().unwrap();
      if Arc::strong_count(&conn) == 2 {
        tcp.tcp.remove(&conn.key);
        // Close the connection
        debug_assert_eq!(Arc::strong_count(&conn), 1);
        drop(conn);
      }
    }
  }
}

/// Bind a socket to an address and listen with the low-level options we need.
#[allow(unused_variables)]
fn bind_socket_and_listen(
  socket_addr: SocketAddr,
  reuse_port: bool,
) -> Result<std::net::TcpListener, std::io::Error> {
  let socket = if socket_addr.is_ipv4() {
    socket2::Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?
  } else {
    socket2::Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?
  };
  #[cfg(not(windows))]
  if REUSE_PORT_LOAD_BALANCES && reuse_port {
    socket.set_reuse_port(true)?;
  }
  #[cfg(not(windows))]
  // This is required for re-use of a port immediately after closing. There's a small
  // security trade-off here but we err on the side of convenience.
  //
  // https://stackoverflow.com/questions/14388706/how-do-so-reuseaddr-and-so-reuseport-differ
  // https://stackoverflow.com/questions/26772549/is-it-a-good-idea-to-reuse-port-using-option-so-reuseaddr-which-is-already-in-ti
  socket.set_reuse_address(true)?;
  socket.set_nonblocking(true)?;
  socket.bind(&socket_addr.into())?;
  socket.listen(128)?;
  let listener = socket.into();
  Ok(listener)
}
