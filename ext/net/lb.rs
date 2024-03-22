use pin_project::pin_project;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;

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
  socket_addr: SocketAddr,
}

impl TcpConnection {
  /// Boot a load-balanced TCP connection
  pub fn start(key: SocketAddr) -> std::io::Result<Self> {
    let listener = std::net::TcpListener::bind(key)?;
    let addr = listener.local_addr()?;
    let socket = socket2::Socket::from(listener);
    socket.set_nonblocking(true)?;

    let sock = socket.into();

    Ok(Self {
      sock,
      key,
      socket_addr: addr,
    })
  }

  fn listener(&self) -> std::io::Result<tokio::net::TcpListener> {
    let listener = std::net::TcpListener::from(self.sock.try_clone()?);
    let listener = tokio::net::TcpListener::from_std(listener)?;
    Ok(listener)
  }
}

pub struct TcpLbListener {
  listener: Option<tokio::net::TcpListener>,
  conn: Option<Arc<TcpConnection>>,
}

impl TcpLbListener {
  pub(crate) fn bind(socket_addr: SocketAddr) -> std::io::Result<Self> {
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

  pub async fn accept(&self) -> std::io::Result<(TcpLbStream, SocketAddr)> {
    let (tcp, addr) = self.listener.as_ref().unwrap().accept().await?;
    Ok((TcpLbStream(tcp), addr))
  }

  pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
    Ok(self.conn.as_ref().unwrap().socket_addr)
  }
}

impl Drop for TcpLbListener {
  fn drop(&mut self) {
    let mut tcp = CONNS.get().unwrap().lock().unwrap();
    let conn = self.conn.take().unwrap();
    if Arc::strong_count(&conn) == 2 {
      tcp.tcp.remove(&conn.key);
      // Close the connection
      debug_assert_eq!(Arc::strong_count(&conn), 1);
      drop(conn);
    }
  }
}

#[pin_project]
pub struct TcpLbStream(#[pin] tokio::net::TcpStream);

impl TcpLbStream {
  pub fn into_inner(self) -> tokio::net::TcpStream {
    self.0
  }
}

impl Deref for TcpLbStream {
  type Target = tokio::net::TcpStream;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for TcpLbStream {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl AsyncRead for TcpLbStream {
  fn poll_read(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    self.project().0.poll_read(cx, buf)
  }
}

impl AsyncWrite for TcpLbStream {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<std::io::Result<usize>> {
    self.project().0.poll_write(cx, buf)
  }

  fn poll_write_vectored(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    bufs: &[std::io::IoSlice<'_>],
  ) -> std::task::Poll<std::io::Result<usize>> {
    self.project().0.poll_write_vectored(cx, bufs)
  }
  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    self.project().0.poll_flush(cx)
  }
  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    self.project().0.poll_shutdown(cx)
  }
  fn is_write_vectored(&self) -> bool {
    self.0.is_write_vectored()
  }
}
