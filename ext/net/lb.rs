use pin_project::pin_project;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::Mutex;

static CONNS: std::sync::OnceLock<std::sync::Mutex<Connections>> =
  std::sync::OnceLock::new();

#[derive(Default)]
struct Connections {
  tcp: HashMap<SocketAddr, (Arc<Mutex<TcpConnection>>, SocketAddr)>,
}

pub struct TcpConnection {
  streams: tokio::sync::mpsc::Receiver<
    std::io::Result<(std::net::TcpStream, std::net::SocketAddr)>,
  >,
}

impl TcpConnection {
  pub fn start(addr: SocketAddr) -> std::io::Result<(Self, SocketAddr)> {
    let listener = std::net::TcpListener::bind(addr)?;
    let addr = listener.local_addr()?;
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    std::thread::spawn(move || loop {
      let res = listener.accept();
      let err = res.is_err();
      if tx.blocking_send(res).is_err() || err {
        break;
      }
    });
    Ok((Self { streams: rx }, addr))
  }
}

pub struct TcpLbListener {
  listener: Arc<Mutex<TcpConnection>>,
  socket_addr: SocketAddr,
}

impl TcpLbListener {
  pub(crate) fn bind(socket_addr: SocketAddr) -> std::io::Result<Self> {
    let tcp = &mut CONNS.get_or_init(|| {
      Default::default()
    }).lock().unwrap().tcp;
    if let Some(tcp) = tcp.get(&socket_addr) {
      return Ok(Self { listener: tcp.0.clone(), socket_addr: tcp.1 });
    }
    let (conn, addr) = TcpConnection::start(socket_addr)?;
    let conn = Arc::new(Mutex::new(conn));
    tcp.insert(socket_addr, (conn.clone(), socket_addr));
    return Ok(Self { listener: conn.clone(), socket_addr: addr });
  } 
  pub async fn accept(&self) -> std::io::Result<(TcpLbStream, SocketAddr)> {
    let Some(res) = self.listener.lock().await.streams.recv().await else {
      return Err(std::io::ErrorKind::NotConnected.into());
    };

    let (tcp, addr) = res?;
    let tcp = tokio::net::TcpStream::from_std(tcp)?;
    Ok((TcpLbStream(tcp), addr))
  }
  pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
    Ok(self.socket_addr)
  }
  
}

#[pin_project]
pub struct TcpLbStream(#[pin] tokio::net::TcpStream);

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
