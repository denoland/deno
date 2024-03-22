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

  pub fn set_reuse_port(&self, flag: bool) -> std::io::Result<()> {
    socket2::SockRef::from(&self.listener.as_ref().unwrap())
      .set_reuse_port(flag)
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
