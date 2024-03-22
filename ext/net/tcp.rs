// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

static CONNS: std::sync::OnceLock<std::sync::Mutex<Connections>> =
  std::sync::OnceLock::new();

#[derive(Default)]
struct Connections {
  tcp: HashMap<SocketAddr, Arc<TcpConnection>>,
}

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
    let listener = std::net::TcpListener::bind(key)?;
    let socket = socket2::Socket::from(listener);
    socket.set_nonblocking(true)?;

    let sock = socket.into();

    Ok(Self { sock, key })
  }

  fn listener(&self) -> std::io::Result<tokio::net::TcpListener> {
    let listener = std::net::TcpListener::from(self.sock.try_clone()?);
    let listener = tokio::net::TcpListener::from_std(listener)?;
    Ok(listener)
  }
}

/// A TCP socket listener that will allow for round-robin load-balancing in-process.
///
/// On Linux, or when we don't have `SO_REUSEPORT` set, we just bind the port directly. On other platforms,
/// we emulate `SO_REUSEPORT` by cloning the socket and having each clone race to accept every connection.
///
/// ## Why not `SO_REUSEPORT`?
///
/// The `SO_REUSEPORT` socket option allows multiple sockets on the same host to bind to the same port. This is
/// particularly useful for load balancing or implementing high availability in server applications.
///
/// On Linux, `SO_REUSEPORT` was introduced in kernel version 3.9. It allows multiple sockets to bind to the
/// same port, and the kernel will load balance incoming connections among those sockets. Each socket can accept
/// connections independently. This is useful for scenarios where you want to distribute incoming connections
/// among multiple processes or threads.
///
/// On macOS (which is based on BSD), the behaviour of `SO_REUSEPORT` is slightly different. When `SO_REUSEPORT` is set,
/// multiple sockets can still bind to the same port, but the kernel does not perform load balancing as it does on Linux.
/// Instead, it follows a "last bind wins" strategy. This means that the most recently bound socket will receive
/// incoming connections exclusively, while the previously bound sockets will not receive any connections.
/// This behaviour is less useful for load balancing compared to Linux, but it can still be valuable in certain scenarios.
///
/// In summary, while both Linux and macOS support the `SO_REUSEPORT` socket option, their behaviour differs: Linux performs
/// load balancing among the sockets, whereas macOS follows a "last bind wins" strategy.
pub struct TcpLbListener {
  listener: Option<tokio::net::TcpListener>,
  conn: Option<Arc<TcpConnection>>,
}

impl TcpLbListener {
  /// Bind to a port. If `reuse_port` is specified and this is not Linux, we use a shared
  /// process-wide port.
  pub fn bind(
    socket_addr: SocketAddr,
    reuse_port: bool,
  ) -> std::io::Result<Self> {
    if cfg!(not(target_os = "linux")) && reuse_port {
      Self::bind_load_balanced(socket_addr)
    } else if reuse_port {
      let this = Self::bind_direct(socket_addr)?;
      socket2::SockRef::from(&this.listener.as_ref().unwrap())
        .set_reuse_port(true)?;
      Ok(this)
    } else {
      Self::bind_direct(socket_addr)
    }
  }

  /// Bind directly to the port.
  fn bind_direct(socket_addr: SocketAddr) -> std::io::Result<Self> {
    let listener = std::net::TcpListener::bind(socket_addr)?;
    let socket = socket2::SockRef::from(&listener);
    socket.set_nonblocking(true)?;
    Ok(Self {
      listener: Some(tokio::net::TcpListener::from_std(listener)?),
      conn: None,
    })
  }

  /// Bind to the port in a load-balanced manner.
  fn bind_load_balanced(socket_addr: SocketAddr) -> std::io::Result<Self> {
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

  pub fn set_reuse_address(&self, flag: bool) -> std::io::Result<()> {
    socket2::SockRef::from(&self.listener.as_ref().unwrap())
      .set_reuse_address(flag)
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

impl Drop for TcpLbListener {
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
