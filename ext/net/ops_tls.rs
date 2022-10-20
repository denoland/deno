// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::io::TcpStreamResource;
use crate::ops::IpAddr;
use crate::ops::OpAddr;
use crate::ops::OpConn;
use crate::ops::TlsHandshakeInfo;
use crate::resolve_addr::resolve_addr;
use crate::resolve_addr::resolve_addr_sync;
use crate::DefaultTlsOptions;
use crate::NetPermissions;
use crate::UnsafelyIgnoreCertificateErrors;
use deno_core::error::bad_resource;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::invalid_hostname;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::future::poll_fn;
use deno_core::futures::ready;
use deno_core::futures::task::noop_waker_ref;
use deno_core::futures::task::AtomicWaker;
use deno_core::futures::task::Context;
use deno_core::futures::task::Poll;
use deno_core::futures::task::RawWaker;
use deno_core::futures::task::RawWakerVTable;
use deno_core::futures::task::Waker;
use deno_core::op;

use deno_core::parking_lot::Mutex;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::ByteString;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpDecl;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_tls::create_client_config;
use deno_tls::load_certs;
use deno_tls::load_private_keys;
use deno_tls::rustls::Certificate;
use deno_tls::rustls::ClientConfig;
use deno_tls::rustls::ClientConnection;
use deno_tls::rustls::Connection;
use deno_tls::rustls::PrivateKey;
use deno_tls::rustls::ServerConfig;
use deno_tls::rustls::ServerConnection;
use deno_tls::rustls::ServerName;
use io::Error;
use io::Read;
use io::Write;
use serde::Deserialize;
use socket2::Domain;
use socket2::Socket;
use socket2::Type;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::convert::TryFrom;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::io::ErrorKind;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Weak;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::ReadBuf;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::task::spawn_local;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Flow {
  Handshake,
  Read,
  Write,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum State {
  StreamOpen,
  StreamClosed,
  TlsClosing,
  TlsClosed,
  TcpClosed,
}

pub struct TlsStream(Option<TlsStreamInner>);

impl TlsStream {
  fn new(tcp: TcpStream, mut tls: Connection) -> Self {
    tls.set_buffer_limit(None);

    let inner = TlsStreamInner {
      tcp,
      tls,
      rd_state: State::StreamOpen,
      wr_state: State::StreamOpen,
    };
    Self(Some(inner))
  }

  pub fn new_client_side(
    tcp: TcpStream,
    tls_config: Arc<ClientConfig>,
    server_name: ServerName,
  ) -> Self {
    let tls = ClientConnection::new(tls_config, server_name).unwrap();
    Self::new(tcp, Connection::Client(tls))
  }

  pub fn new_server_side(
    tcp: TcpStream,
    tls_config: Arc<ServerConfig>,
  ) -> Self {
    let tls = ServerConnection::new(tls_config).unwrap();
    Self::new(tcp, Connection::Server(tls))
  }

  pub fn into_split(self) -> (ReadHalf, WriteHalf) {
    let shared = Shared::new(self);
    let rd = ReadHalf {
      shared: shared.clone(),
    };
    let wr = WriteHalf { shared };
    (rd, wr)
  }

  /// Tokio-rustls compatibility: returns a reference to the underlying TCP
  /// stream, and a reference to the Rustls `Connection` object.
  pub fn get_ref(&self) -> (&TcpStream, &Connection) {
    let inner = self.0.as_ref().unwrap();
    (&inner.tcp, &inner.tls)
  }

  fn inner_mut(&mut self) -> &mut TlsStreamInner {
    self.0.as_mut().unwrap()
  }

  pub async fn handshake(&mut self) -> io::Result<()> {
    poll_fn(|cx| self.inner_mut().poll_handshake(cx)).await
  }

  fn poll_handshake(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    self.inner_mut().poll_handshake(cx)
  }

  fn get_alpn_protocol(&mut self) -> Option<ByteString> {
    self.inner_mut().tls.alpn_protocol().map(|s| s.into())
  }
}

impl AsyncRead for TlsStream {
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    self.inner_mut().poll_read(cx, buf)
  }
}

impl AsyncWrite for TlsStream {
  fn poll_write(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    self.inner_mut().poll_write(cx, buf)
  }

  fn poll_flush(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    self.inner_mut().poll_io(cx, Flow::Write)
    // The underlying TCP stream does not need to be flushed.
  }

  fn poll_shutdown(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    self.inner_mut().poll_shutdown(cx)
  }
}

impl Drop for TlsStream {
  fn drop(&mut self) {
    let mut inner = self.0.take().unwrap();

    let mut cx = Context::from_waker(noop_waker_ref());
    let use_linger_task = inner.poll_close(&mut cx).is_pending();

    if use_linger_task {
      spawn_local(poll_fn(move |cx| inner.poll_close(cx)));
    } else if cfg!(debug_assertions) {
      spawn_local(async {}); // Spawn dummy task to detect missing LocalSet.
    }
  }
}

pub struct TlsStreamInner {
  tls: Connection,
  tcp: TcpStream,
  rd_state: State,
  wr_state: State,
}

impl TlsStreamInner {
  fn poll_io(
    &mut self,
    cx: &mut Context<'_>,
    flow: Flow,
  ) -> Poll<io::Result<()>> {
    loop {
      let wr_ready = loop {
        match self.wr_state {
          _ if self.tls.is_handshaking() && !self.tls.wants_write() => {
            break true;
          }
          _ if self.tls.is_handshaking() => {}
          State::StreamOpen if !self.tls.wants_write() => break true,
          State::StreamClosed => {
            // Rustls will enqueue the 'CloseNotify' alert and send it after
            // flushing the data that is already in the queue.
            self.tls.send_close_notify();
            self.wr_state = State::TlsClosing;
            continue;
          }
          State::TlsClosing if !self.tls.wants_write() => {
            self.wr_state = State::TlsClosed;
            continue;
          }
          // If a 'CloseNotify' alert sent by the remote end has been received,
          // shut down the underlying TCP socket. Otherwise, consider polling
          // done for the moment.
          State::TlsClosed if self.rd_state < State::TlsClosed => break true,
          State::TlsClosed
            if Pin::new(&mut self.tcp).poll_shutdown(cx)?.is_pending() =>
          {
            break false;
          }
          State::TlsClosed => {
            self.wr_state = State::TcpClosed;
            continue;
          }
          State::TcpClosed => break true,
          _ => {}
        }

        // Write ciphertext to the TCP socket.
        let mut wrapped_tcp = ImplementWriteTrait(&mut self.tcp);
        match self.tls.write_tls(&mut wrapped_tcp) {
          Ok(0) => {}        // Wait until the socket has enough buffer space.
          Ok(_) => continue, // Try to send more more data immediately.
          Err(err) if err.kind() == ErrorKind::WouldBlock => unreachable!(),
          Err(err) => return Poll::Ready(Err(err)),
        }

        // Poll whether there is space in the socket send buffer so we can flush
        // the remaining outgoing ciphertext.
        if self.tcp.poll_write_ready(cx)?.is_pending() {
          break false;
        }
      };

      let rd_ready = loop {
        // Interpret and decrypt unprocessed TLS protocol data.
        let tls_state = self
          .tls
          .process_new_packets()
          .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

        match self.rd_state {
          State::TcpClosed if self.tls.is_handshaking() => {
            let err = Error::new(ErrorKind::UnexpectedEof, "tls handshake eof");
            return Poll::Ready(Err(err));
          }
          _ if self.tls.is_handshaking() && !self.tls.wants_read() => {
            break true;
          }
          _ if self.tls.is_handshaking() => {}
          State::StreamOpen if tls_state.plaintext_bytes_to_read() > 0 => {
            break true;
          }
          State::StreamOpen if tls_state.peer_has_closed() => {
            self.rd_state = State::TlsClosed;
            continue;
          }
          State::StreamOpen => {}
          State::StreamClosed if tls_state.plaintext_bytes_to_read() > 0 => {
            // Rustls has more incoming cleartext buffered up, but the TLS
            // session is closing so this data will never be processed by the
            // application layer. Just like what would happen if this were a raw
            // TCP stream, don't gracefully end the TLS session, but abort it.
            return Poll::Ready(Err(Error::from(ErrorKind::ConnectionReset)));
          }
          State::StreamClosed => {}
          State::TlsClosed if self.wr_state == State::TcpClosed => {
            // Keep trying to read from the TCP connection until the remote end
            // closes it gracefully.
          }
          State::TlsClosed => break true,
          State::TcpClosed => break true,
          _ => unreachable!(),
        }

        // Try to read more TLS protocol data from the TCP socket.
        let mut wrapped_tcp = ImplementReadTrait(&mut self.tcp);
        match self.tls.read_tls(&mut wrapped_tcp) {
          Ok(0) => {
            self.rd_state = State::TcpClosed;
            continue;
          }
          Ok(_) => continue,
          Err(err) if err.kind() == ErrorKind::WouldBlock => {}
          Err(err) => return Poll::Ready(Err(err)),
        }

        // Get notified when more ciphertext becomes available to read from the
        // TCP socket.
        if self.tcp.poll_read_ready(cx)?.is_pending() {
          break false;
        }
      };

      if wr_ready {
        if self.rd_state >= State::TlsClosed
          && self.wr_state >= State::TlsClosed
          && self.wr_state < State::TcpClosed
        {
          continue;
        }
        if self.tls.wants_write() {
          continue;
        }
      }

      let io_ready = match flow {
        _ if self.tls.is_handshaking() => false,
        Flow::Handshake => true,
        Flow::Read => rd_ready,
        Flow::Write => wr_ready,
      };
      return match io_ready {
        false => Poll::Pending,
        true => Poll::Ready(Ok(())),
      };
    }
  }

  fn poll_handshake(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    if self.tls.is_handshaking() {
      ready!(self.poll_io(cx, Flow::Handshake))?;
    }
    Poll::Ready(Ok(()))
  }

  fn poll_read(
    &mut self,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    ready!(self.poll_io(cx, Flow::Read))?;

    if self.rd_state == State::StreamOpen {
      // TODO(bartlomieju):
      #[allow(clippy::undocumented_unsafe_blocks)]
      let buf_slice =
        unsafe { &mut *(buf.unfilled_mut() as *mut [_] as *mut [u8]) };
      let bytes_read = self.tls.reader().read(buf_slice)?;
      assert_ne!(bytes_read, 0);
      // TODO(bartlomieju):
      #[allow(clippy::undocumented_unsafe_blocks)]
      unsafe {
        buf.assume_init(bytes_read)
      };
      buf.advance(bytes_read);
    }

    Poll::Ready(Ok(()))
  }

  fn poll_write(
    &mut self,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    if buf.is_empty() {
      // Tokio-rustls compatibility: a zero byte write always succeeds.
      Poll::Ready(Ok(0))
    } else if self.wr_state == State::StreamOpen {
      // Flush Rustls' ciphertext send queue.
      ready!(self.poll_io(cx, Flow::Write))?;

      // Copy data from `buf` to the Rustls cleartext send queue.
      let bytes_written = self.tls.writer().write(buf)?;
      assert_ne!(bytes_written, 0);

      // Try to flush as much ciphertext as possible. However, since we just
      // handed off at least some bytes to rustls, so we can't return
      // `Poll::Pending()` any more: this would tell the caller that it should
      // try to send those bytes again.
      let _ = self.poll_io(cx, Flow::Write)?;

      Poll::Ready(Ok(bytes_written))
    } else {
      // Return error if stream has been shut down for writing.
      Poll::Ready(Err(ErrorKind::BrokenPipe.into()))
    }
  }

  fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    if self.wr_state == State::StreamOpen {
      self.wr_state = State::StreamClosed;
    }

    ready!(self.poll_io(cx, Flow::Write))?;

    // At minimum, a TLS 'CloseNotify' alert should have been sent.
    assert!(self.wr_state >= State::TlsClosed);
    // If we received a TLS 'CloseNotify' alert from the remote end
    // already, the TCP socket should be shut down at this point.
    assert!(
      self.rd_state < State::TlsClosed || self.wr_state == State::TcpClosed
    );

    Poll::Ready(Ok(()))
  }

  fn poll_close(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    if self.rd_state == State::StreamOpen {
      self.rd_state = State::StreamClosed;
    }

    // Wait for the handshake to complete.
    ready!(self.poll_io(cx, Flow::Handshake))?;
    // Send TLS 'CloseNotify' alert.
    ready!(self.poll_shutdown(cx))?;
    // Wait for 'CloseNotify', shut down TCP stream, wait for TCP FIN packet.
    ready!(self.poll_io(cx, Flow::Read))?;

    assert_eq!(self.rd_state, State::TcpClosed);
    assert_eq!(self.wr_state, State::TcpClosed);

    Poll::Ready(Ok(()))
  }
}

pub struct ReadHalf {
  shared: Arc<Shared>,
}

impl ReadHalf {
  pub fn reunite(self, wr: WriteHalf) -> TlsStream {
    assert!(Arc::ptr_eq(&self.shared, &wr.shared));
    drop(wr); // Drop `wr`, so only one strong reference to `shared` remains.

    Arc::try_unwrap(self.shared)
      .unwrap_or_else(|_| panic!("Arc::<Shared>::try_unwrap() failed"))
      .tls_stream
      .into_inner()
  }
}

impl AsyncRead for ReadHalf {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    self
      .shared
      .poll_with_shared_waker(cx, Flow::Read, move |tls, cx| {
        tls.poll_read(cx, buf)
      })
  }
}

pub struct WriteHalf {
  shared: Arc<Shared>,
}

impl WriteHalf {
  pub async fn handshake(&mut self) -> io::Result<()> {
    poll_fn(|cx| {
      self
        .shared
        .poll_with_shared_waker(cx, Flow::Write, |mut tls, cx| {
          tls.poll_handshake(cx)
        })
    })
    .await
  }

  fn get_alpn_protocol(&mut self) -> Option<ByteString> {
    self.shared.get_alpn_protocol()
  }
}

impl AsyncWrite for WriteHalf {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    self
      .shared
      .poll_with_shared_waker(cx, Flow::Write, move |tls, cx| {
        tls.poll_write(cx, buf)
      })
  }

  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    self
      .shared
      .poll_with_shared_waker(cx, Flow::Write, |tls, cx| tls.poll_flush(cx))
  }

  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    self
      .shared
      .poll_with_shared_waker(cx, Flow::Write, |tls, cx| tls.poll_shutdown(cx))
  }
}

struct Shared {
  tls_stream: Mutex<TlsStream>,
  rd_waker: AtomicWaker,
  wr_waker: AtomicWaker,
}

impl Shared {
  fn new(tls_stream: TlsStream) -> Arc<Self> {
    let self_ = Self {
      tls_stream: Mutex::new(tls_stream),
      rd_waker: AtomicWaker::new(),
      wr_waker: AtomicWaker::new(),
    };
    Arc::new(self_)
  }

  fn poll_with_shared_waker<R>(
    self: &Arc<Self>,
    cx: &mut Context<'_>,
    flow: Flow,
    mut f: impl FnMut(Pin<&mut TlsStream>, &mut Context<'_>) -> R,
  ) -> R {
    match flow {
      Flow::Handshake => unreachable!(),
      Flow::Read => self.rd_waker.register(cx.waker()),
      Flow::Write => self.wr_waker.register(cx.waker()),
    }

    let shared_waker = self.new_shared_waker();
    let mut cx = Context::from_waker(&shared_waker);

    let mut tls_stream = self.tls_stream.lock();
    f(Pin::new(&mut tls_stream), &mut cx)
  }

  const SHARED_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    Self::clone_shared_waker,
    Self::wake_shared_waker,
    Self::wake_shared_waker_by_ref,
    Self::drop_shared_waker,
  );

  fn new_shared_waker(self: &Arc<Self>) -> Waker {
    let self_weak = Arc::downgrade(self);
    let self_ptr = self_weak.into_raw() as *const ();
    let raw_waker = RawWaker::new(self_ptr, &Self::SHARED_WAKER_VTABLE);
    // TODO(bartlomieju):
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
      Waker::from_raw(raw_waker)
    }
  }

  fn clone_shared_waker(self_ptr: *const ()) -> RawWaker {
    // TODO(bartlomieju):
    #[allow(clippy::undocumented_unsafe_blocks)]
    let self_weak = unsafe { Weak::from_raw(self_ptr as *const Self) };
    let ptr1 = self_weak.clone().into_raw();
    let ptr2 = self_weak.into_raw();
    assert!(ptr1 == ptr2);
    RawWaker::new(self_ptr, &Self::SHARED_WAKER_VTABLE)
  }

  fn wake_shared_waker(self_ptr: *const ()) {
    Self::wake_shared_waker_by_ref(self_ptr);
    Self::drop_shared_waker(self_ptr);
  }

  fn wake_shared_waker_by_ref(self_ptr: *const ()) {
    // TODO(bartlomieju):
    #[allow(clippy::undocumented_unsafe_blocks)]
    let self_weak = unsafe { Weak::from_raw(self_ptr as *const Self) };
    if let Some(self_arc) = Weak::upgrade(&self_weak) {
      self_arc.rd_waker.wake();
      self_arc.wr_waker.wake();
    }
    let _ = self_weak.into_raw();
  }

  fn drop_shared_waker(self_ptr: *const ()) {
    // TODO(bartlomieju):
    #[allow(clippy::undocumented_unsafe_blocks)]
    let _ = unsafe { Weak::from_raw(self_ptr as *const Self) };
  }

  fn get_alpn_protocol(self: &Arc<Self>) -> Option<ByteString> {
    let mut tls_stream = self.tls_stream.lock();
    tls_stream.get_alpn_protocol()
  }
}

struct ImplementReadTrait<'a, T>(&'a mut T);

impl Read for ImplementReadTrait<'_, TcpStream> {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    self.0.try_read(buf)
  }
}

struct ImplementWriteTrait<'a, T>(&'a mut T);

impl Write for ImplementWriteTrait<'_, TcpStream> {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    match self.0.try_write(buf) {
      Ok(n) => Ok(n),
      Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(0),
      Err(err) => Err(err),
    }
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

pub fn init<P: NetPermissions + 'static>() -> Vec<OpDecl> {
  vec![
    op_tls_start::decl::<P>(),
    op_tls_connect::decl::<P>(),
    op_tls_listen::decl::<P>(),
    op_tls_accept::decl(),
    op_tls_handshake::decl(),
  ]
}

#[derive(Debug)]
pub struct TlsStreamResource {
  rd: AsyncRefCell<ReadHalf>,
  wr: AsyncRefCell<WriteHalf>,
  // `None` when a TLS handshake hasn't been done.
  handshake_info: RefCell<Option<TlsHandshakeInfo>>,
  cancel_handle: CancelHandle, // Only read and handshake ops get canceled.
}

impl TlsStreamResource {
  pub fn new((rd, wr): (ReadHalf, WriteHalf)) -> Self {
    Self {
      rd: rd.into(),
      wr: wr.into(),
      handshake_info: RefCell::new(None),
      cancel_handle: Default::default(),
    }
  }

  pub fn into_inner(self) -> (ReadHalf, WriteHalf) {
    (self.rd.into_inner(), self.wr.into_inner())
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, AnyError> {
    let mut rd = RcRef::map(&self, |r| &r.rd).borrow_mut().await;
    let cancel_handle = RcRef::map(&self, |r| &r.cancel_handle);
    let nread = rd.read(data).try_or_cancel(cancel_handle).await?;
    Ok(nread)
  }

  pub async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, AnyError> {
    self.handshake().await?;
    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    let nwritten = wr.write(data).await?;
    wr.flush().await?;
    Ok(nwritten)
  }

  pub async fn shutdown(self: Rc<Self>) -> Result<(), AnyError> {
    self.handshake().await?;
    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    wr.shutdown().await?;
    Ok(())
  }

  pub async fn handshake(
    self: &Rc<Self>,
  ) -> Result<TlsHandshakeInfo, AnyError> {
    if let Some(tls_info) = &*self.handshake_info.borrow() {
      return Ok(tls_info.clone());
    }

    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    let cancel_handle = RcRef::map(self, |r| &r.cancel_handle);
    wr.handshake().try_or_cancel(cancel_handle).await?;

    let alpn_protocol = wr.get_alpn_protocol();
    let tls_info = TlsHandshakeInfo { alpn_protocol };
    self.handshake_info.replace(Some(tls_info.clone()));
    Ok(tls_info)
  }
}

impl Resource for TlsStreamResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<str> {
    "tlsStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown())
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectTlsArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert_file: Option<String>,
  ca_certs: Vec<String>,
  cert_chain: Option<String>,
  private_key: Option<String>,
  alpn_protocols: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTlsArgs {
  rid: ResourceId,
  ca_certs: Vec<String>,
  hostname: String,
  alpn_protocols: Option<Vec<String>>,
}

#[op]
pub async fn op_tls_start<NP>(
  state: Rc<RefCell<OpState>>,
  args: StartTlsArgs,
) -> Result<OpConn, AnyError>
where
  NP: NetPermissions + 'static,
{
  let rid = args.rid;
  let hostname = match &*args.hostname {
    "" => "localhost",
    n => n,
  };

  {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<NP>();
    permissions.check_net(&(hostname, Some(0)), "Deno.startTls()")?;
  }

  let ca_certs = args
    .ca_certs
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  let hostname_dns =
    ServerName::try_from(hostname).map_err(|_| invalid_hostname(hostname))?;

  let unsafely_ignore_certificate_errors = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());

  // TODO(@justinmchase): Ideally the certificate store is created once
  // and not cloned. The store should be wrapped in Arc<T> to reduce
  // copying memory unnecessarily.
  let root_cert_store = state
    .borrow()
    .borrow::<DefaultTlsOptions>()
    .root_cert_store
    .clone();

  let resource_rc = state
    .borrow_mut()
    .resource_table
    .take::<TcpStreamResource>(rid)?;
  // This TCP connection might be used somewhere else. If it's the case, we cannot proceed with the
  // process of starting a TLS connection on top of this TCP connection, so we just return a bad
  // resource error. See also: https://github.com/denoland/deno/pull/16242
  let resource = Rc::try_unwrap(resource_rc)
    .map_err(|_| bad_resource("TCP stream is currently in use"))?;
  let (read_half, write_half) = resource.into_inner();
  let tcp_stream = read_half.reunite(write_half)?;

  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let mut tls_config = create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    None,
  )?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    super::check_unstable2(&state, "Deno.startTls#alpnProtocols");
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let tls_config = Arc::new(tls_config);

  let tls_stream =
    TlsStream::new_client_side(tcp_stream, tls_config, hostname_dns);

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new(tls_stream.into_split()))
  };

  Ok(OpConn {
    rid,
    local_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: local_addr.ip().to_string(),
      port: local_addr.port(),
    })),
    remote_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: remote_addr.ip().to_string(),
      port: remote_addr.port(),
    })),
  })
}

#[op]
pub async fn op_tls_connect<NP>(
  state: Rc<RefCell<OpState>>,
  args: ConnectTlsArgs,
) -> Result<OpConn, AnyError>
where
  NP: NetPermissions + 'static,
{
  assert_eq!(args.transport, "tcp");
  let hostname = match &*args.hostname {
    "" => "localhost",
    n => n,
  };
  let port = args.port;
  let cert_file = args.cert_file.as_deref();
  let unsafely_ignore_certificate_errors = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());

  if args.cert_chain.is_some() {
    super::check_unstable2(&state, "ConnectTlsOptions.certChain");
  }
  if args.private_key.is_some() {
    super::check_unstable2(&state, "ConnectTlsOptions.privateKey");
  }

  {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<NP>();
    permissions.check_net(&(hostname, Some(port)), "Deno.connectTls()")?;
    if let Some(path) = cert_file {
      permissions.check_read(Path::new(path), "Deno.connectTls()")?;
    }
  }

  let mut ca_certs = args
    .ca_certs
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  if let Some(path) = cert_file {
    let mut buf = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    ca_certs.push(buf);
  };

  let root_cert_store = state
    .borrow()
    .borrow::<DefaultTlsOptions>()
    .root_cert_store
    .clone();
  let hostname_dns =
    ServerName::try_from(hostname).map_err(|_| invalid_hostname(hostname))?;

  let connect_addr = resolve_addr(hostname, port)
    .await?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let tcp_stream = TcpStream::connect(connect_addr).await?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let cert_chain_and_key =
    if args.cert_chain.is_some() || args.private_key.is_some() {
      let cert_chain = args
        .cert_chain
        .ok_or_else(|| type_error("No certificate chain provided"))?;
      let private_key = args
        .private_key
        .ok_or_else(|| type_error("No private key provided"))?;
      Some((cert_chain, private_key))
    } else {
      None
    };

  let mut tls_config = create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    cert_chain_and_key,
  )?;

  if let Some(alpn_protocols) = args.alpn_protocols {
    super::check_unstable2(&state, "Deno.connectTls#alpnProtocols");
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let tls_config = Arc::new(tls_config);

  let tls_stream =
    TlsStream::new_client_side(tcp_stream, tls_config, hostname_dns);

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new(tls_stream.into_split()))
  };

  Ok(OpConn {
    rid,
    local_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: local_addr.ip().to_string(),
      port: local_addr.port(),
    })),
    remote_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: remote_addr.ip().to_string(),
      port: remote_addr.port(),
    })),
  })
}

fn load_certs_from_file(path: &str) -> Result<Vec<Certificate>, AnyError> {
  let cert_file = File::open(path)?;
  let reader = &mut BufReader::new(cert_file);
  load_certs(reader)
}

fn load_private_keys_from_file(
  path: &str,
) -> Result<Vec<PrivateKey>, AnyError> {
  let key_bytes = std::fs::read(path)?;
  load_private_keys(&key_bytes)
}

pub struct TlsListenerResource {
  tcp_listener: AsyncRefCell<TcpListener>,
  tls_config: Arc<ServerConfig>,
  cancel_handle: CancelHandle,
}

impl Resource for TlsListenerResource {
  fn name(&self) -> Cow<str> {
    "tlsListener".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenTlsArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert: Option<String>,
  // TODO(kt3k): Remove this option at v2.0.
  cert_file: Option<String>,
  key: Option<String>,
  // TODO(kt3k): Remove this option at v2.0.
  key_file: Option<String>,
  alpn_protocols: Option<Vec<String>>,
}

#[op]
pub fn op_tls_listen<NP>(
  state: &mut OpState,
  args: ListenTlsArgs,
) -> Result<OpConn, AnyError>
where
  NP: NetPermissions + 'static,
{
  assert_eq!(args.transport, "tcp");
  let hostname = &*args.hostname;
  let port = args.port;
  let cert_file = args.cert_file.as_deref();
  let key_file = args.key_file.as_deref();
  let cert = args.cert.as_deref();
  let key = args.key.as_deref();

  {
    let permissions = state.borrow_mut::<NP>();
    permissions.check_net(&(hostname, Some(port)), "Deno.listenTls()")?;
    if let Some(path) = cert_file {
      permissions.check_read(Path::new(path), "Deno.listenTls()")?;
    }
    if let Some(path) = key_file {
      permissions.check_read(Path::new(path), "Deno.listenTls()")?;
    }
  }

  let cert_chain = if cert_file.is_some() && cert.is_some() {
    return Err(generic_error("Both cert and certFile is specified. You can specify either one of them."));
  } else if let Some(path) = cert_file {
    load_certs_from_file(path)?
  } else if let Some(cert) = cert {
    load_certs(&mut BufReader::new(cert.as_bytes()))?
  } else {
    return Err(generic_error("`cert` is not specified."));
  };
  let key_der = if key_file.is_some() && key.is_some() {
    return Err(generic_error(
      "Both key and keyFile is specified. You can specify either one of them.",
    ));
  } else if let Some(path) = key_file {
    load_private_keys_from_file(path)?.remove(0)
  } else if let Some(key) = key {
    load_private_keys(key.as_bytes())?.remove(0)
  } else {
    return Err(generic_error("`key` is not specified."));
  };

  let mut tls_config = ServerConfig::builder()
    .with_safe_defaults()
    .with_no_client_auth()
    .with_single_cert(cert_chain, key_der)
    .expect("invalid key or certificate");
  if let Some(alpn_protocols) = args.alpn_protocols {
    super::check_unstable(state, "Deno.listenTls#alpn_protocols");
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let bind_addr = resolve_addr_sync(hostname, port)?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let domain = if bind_addr.is_ipv4() {
    Domain::IPV4
  } else {
    Domain::IPV6
  };
  let socket = Socket::new(domain, Type::STREAM, None)?;
  #[cfg(not(windows))]
  socket.set_reuse_address(true)?;
  let socket_addr = socket2::SockAddr::from(bind_addr);
  socket.bind(&socket_addr)?;
  socket.listen(128)?;
  socket.set_nonblocking(true)?;
  let std_listener: std::net::TcpListener = socket.into();
  let tcp_listener = TcpListener::from_std(std_listener)?;
  let local_addr = tcp_listener.local_addr()?;

  let tls_listener_resource = TlsListenerResource {
    tcp_listener: AsyncRefCell::new(tcp_listener),
    tls_config: Arc::new(tls_config),
    cancel_handle: Default::default(),
  };

  let rid = state.resource_table.add(tls_listener_resource);

  Ok(OpConn {
    rid,
    local_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: local_addr.ip().to_string(),
      port: local_addr.port(),
    })),
    remote_addr: None,
  })
}

#[op]
pub async fn op_tls_accept(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<OpConn, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TlsListenerResource>(rid)
    .map_err(|_| bad_resource("Listener has been closed"))?;

  let cancel_handle = RcRef::map(&resource, |r| &r.cancel_handle);
  let tcp_listener = RcRef::map(&resource, |r| &r.tcp_listener)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Another accept task is ongoing"))?;

  let (tcp_stream, remote_addr) =
    match tcp_listener.accept().try_or_cancel(&cancel_handle).await {
      Ok(tuple) => tuple,
      Err(err) if err.kind() == ErrorKind::Interrupted => {
        // FIXME(bartlomieju): compatibility with current JS implementation.
        return Err(bad_resource("Listener has been closed"));
      }
      Err(err) => return Err(err.into()),
    };

  let local_addr = tcp_stream.local_addr()?;

  let tls_stream =
    TlsStream::new_server_side(tcp_stream, resource.tls_config.clone());

  let rid = {
    let mut state_ = state.borrow_mut();
    state_
      .resource_table
      .add(TlsStreamResource::new(tls_stream.into_split()))
  };

  Ok(OpConn {
    rid,
    local_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: local_addr.ip().to_string(),
      port: local_addr.port(),
    })),
    remote_addr: Some(OpAddr::Tcp(IpAddr {
      hostname: remote_addr.ip().to_string(),
      port: remote_addr.port(),
    })),
  })
}

#[op]
pub async fn op_tls_handshake(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<TlsHandshakeInfo, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TlsStreamResource>(rid)?;
  resource.handshake().await
}
