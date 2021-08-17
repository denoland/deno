// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::io::TcpStreamResource;
use crate::io::TlsStreamResource;
use crate::ops::IpAddr;
use crate::ops::OpAddr;
use crate::ops::OpConn;
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
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::parking_lot::Mutex;
use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpPair;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_tls::create_client_config;
use deno_tls::rustls::internal::pemfile::certs;
use deno_tls::rustls::internal::pemfile::pkcs8_private_keys;
use deno_tls::rustls::internal::pemfile::rsa_private_keys;
use deno_tls::rustls::Certificate;
use deno_tls::rustls::ClientConfig;
use deno_tls::rustls::ClientSession;
use deno_tls::rustls::NoClientAuth;
use deno_tls::rustls::PrivateKey;
use deno_tls::rustls::ServerConfig;
use deno_tls::rustls::ServerSession;
use deno_tls::rustls::Session;
use deno_tls::webpki::DNSNameRef;
use io::Error;
use io::Read;
use io::Write;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::ErrorKind;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Weak;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::task::spawn_local;

#[derive(Debug)]
enum TlsSession {
  Client(ClientSession),
  Server(ServerSession),
}

impl Deref for TlsSession {
  type Target = dyn Session;

  fn deref(&self) -> &Self::Target {
    match self {
      TlsSession::Client(client_session) => client_session,
      TlsSession::Server(server_session) => server_session,
    }
  }
}

impl DerefMut for TlsSession {
  fn deref_mut(&mut self) -> &mut Self::Target {
    match self {
      TlsSession::Client(client_session) => client_session,
      TlsSession::Server(server_session) => server_session,
    }
  }
}

impl From<ClientSession> for TlsSession {
  fn from(client_session: ClientSession) -> Self {
    TlsSession::Client(client_session)
  }
}

impl From<ServerSession> for TlsSession {
  fn from(server_session: ServerSession) -> Self {
    TlsSession::Server(server_session)
  }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Flow {
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

#[derive(Debug)]
pub struct TlsStream(Option<TlsStreamInner>);

impl TlsStream {
  fn new(tcp: TcpStream, tls: TlsSession) -> Self {
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
    tls_config: &Arc<ClientConfig>,
    hostname: DNSNameRef,
  ) -> Self {
    let tls = TlsSession::Client(ClientSession::new(tls_config, hostname));
    Self::new(tcp, tls)
  }

  pub fn new_server_side(
    tcp: TcpStream,
    tls_config: &Arc<ServerConfig>,
  ) -> Self {
    let tls = TlsSession::Server(ServerSession::new(tls_config));
    Self::new(tcp, tls)
  }

  pub async fn handshake(&mut self) -> io::Result<()> {
    poll_fn(|cx| self.inner_mut().poll_io(cx, Flow::Write)).await
  }

  fn into_split(self) -> (ReadHalf, WriteHalf) {
    let shared = Shared::new(self);
    let rd = ReadHalf {
      shared: shared.clone(),
    };
    let wr = WriteHalf { shared };
    (rd, wr)
  }

  /// Tokio-rustls compatibility: returns a reference to the underlying TCP
  /// stream, and a reference to the Rustls `Session` object.
  pub fn get_ref(&self) -> (&TcpStream, &dyn Session) {
    let inner = self.0.as_ref().unwrap();
    (&inner.tcp, &*inner.tls)
  }

  fn inner_mut(&mut self) -> &mut TlsStreamInner {
    self.0.as_mut().unwrap()
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

#[derive(Debug)]
pub struct TlsStreamInner {
  tls: TlsSession,
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
            // flusing the data that is already in the queue.
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

        // Poll whether there is space in the socket send buffer so we can flush
        // the remaining outgoing ciphertext.
        if self.tcp.poll_write_ready(cx)?.is_pending() {
          break false;
        }

        // Write ciphertext to the TCP socket.
        let mut wrapped_tcp = ImplementWriteTrait(&mut self.tcp);
        match self.tls.write_tls(&mut wrapped_tcp) {
          Ok(0) => unreachable!(),
          Ok(_) => {}
          Err(err) if err.kind() == ErrorKind::WouldBlock => {}
          Err(err) => return Poll::Ready(Err(err)),
        }
      };

      let rd_ready = loop {
        match self.rd_state {
          State::TcpClosed if self.tls.is_handshaking() => {
            let err = Error::new(ErrorKind::UnexpectedEof, "tls handshake eof");
            return Poll::Ready(Err(err));
          }
          _ if self.tls.is_handshaking() && !self.tls.wants_read() => {
            break true;
          }
          _ if self.tls.is_handshaking() => {}
          State::StreamOpen if !self.tls.wants_read() => break true,
          State::StreamOpen => {}
          State::StreamClosed if !self.tls.wants_read() => {
            // Rustls has more incoming cleartext buffered up, but the TLS
            // session is closing so this data will never be processed by the
            // application layer. Just like what would happen if this were a raw
            // TCP stream, don't gracefully end the TLS session, but abort it.
            return Poll::Ready(Err(Error::from(ErrorKind::ConnectionReset)));
          }
          State::StreamClosed => {}
          State::TlsClosed if self.wr_state == State::TcpClosed => {
            // Wait for the remote end to gracefully close the TCP connection.
            // TODO(piscisaureus): this is unnecessary; remove when stable.
          }
          _ => break true,
        }

        if self.rd_state < State::TlsClosed {
          // Do a zero-length plaintext read so we can detect the arrival of
          // 'CloseNotify' messages, even if only the write half is open.
          // Actually reading data from the socket is done in `poll_read()`.
          match self.tls.read(&mut []) {
            Ok(0) => {}
            Err(err) if err.kind() == ErrorKind::ConnectionAborted => {
              // `Session::read()` returns `ConnectionAborted` when a
              // 'CloseNotify' alert has been received, which indicates that
              // the remote peer wants to gracefully end the TLS session.
              self.rd_state = State::TlsClosed;
              continue;
            }
            Err(err) => return Poll::Ready(Err(err)),
            _ => unreachable!(),
          }
        }

        // Poll whether more ciphertext is available in the socket receive
        // buffer.
        if self.tcp.poll_read_ready(cx)?.is_pending() {
          break false;
        }

        // Receive ciphertext from the socket.
        let mut wrapped_tcp = ImplementReadTrait(&mut self.tcp);
        match self.tls.read_tls(&mut wrapped_tcp) {
          Ok(0) => self.rd_state = State::TcpClosed,
          Ok(_) => self
            .tls
            .process_new_packets()
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?,
          Err(err) if err.kind() == ErrorKind::WouldBlock => {}
          Err(err) => return Poll::Ready(Err(err)),
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
        Flow::Read => rd_ready,
        Flow::Write => wr_ready,
      };
      return match io_ready {
        false => Poll::Pending,
        true => Poll::Ready(Ok(())),
      };
    }
  }

  fn poll_read(
    &mut self,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    ready!(self.poll_io(cx, Flow::Read))?;

    if self.rd_state == State::StreamOpen {
      let buf_slice =
        unsafe { &mut *(buf.unfilled_mut() as *mut [_] as *mut [u8]) };
      let bytes_read = self.tls.read(buf_slice)?;
      assert_ne!(bytes_read, 0);
      unsafe { buf.assume_init(bytes_read) };
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
      let bytes_written = self.tls.write(buf)?;
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

    // Send TLS 'CloseNotify' alert.
    ready!(self.poll_shutdown(cx))?;
    // Wait for 'CloseNotify', shut down TCP stream, wait for TCP FIN packet.
    ready!(self.poll_io(cx, Flow::Read))?;

    assert_eq!(self.rd_state, State::TcpClosed);
    assert_eq!(self.wr_state, State::TcpClosed);

    Poll::Ready(Ok(()))
  }
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct WriteHalf {
  shared: Arc<Shared>,
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

#[derive(Debug)]
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
    unsafe { Waker::from_raw(raw_waker) }
  }

  fn clone_shared_waker(self_ptr: *const ()) -> RawWaker {
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
    let self_weak = unsafe { Weak::from_raw(self_ptr as *const Self) };
    if let Some(self_arc) = Weak::upgrade(&self_weak) {
      self_arc.rd_waker.wake();
      self_arc.wr_waker.wake();
    }
    self_weak.into_raw();
  }

  fn drop_shared_waker(self_ptr: *const ()) {
    let _ = unsafe { Weak::from_raw(self_ptr as *const Self) };
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
    self.0.try_write(buf)
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

pub fn init<P: NetPermissions + 'static>() -> Vec<OpPair> {
  vec![
    ("op_start_tls", op_async(op_start_tls::<P>)),
    ("op_connect_tls", op_async(op_connect_tls::<P>)),
    ("op_listen_tls", op_sync(op_listen_tls::<P>)),
    ("op_accept_tls", op_async(op_accept_tls)),
  ]
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectTlsArgs {
  transport: String,
  hostname: String,
  port: u16,
  cert_file: Option<String>,
  cert_chain: Option<String>,
  private_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartTlsArgs {
  rid: ResourceId,
  cert_file: Option<String>,
  hostname: String,
}

async fn op_start_tls<NP>(
  state: Rc<RefCell<OpState>>,
  args: StartTlsArgs,
  _: (),
) -> Result<OpConn, AnyError>
where
  NP: NetPermissions + 'static,
{
  let rid = args.rid;
  let hostname = match &*args.hostname {
    "" => "localhost",
    n => n,
  };
  let cert_file = args.cert_file.as_deref();
  {
    super::check_unstable2(&state, "Deno.startTls");
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<NP>();
    permissions.check_net(&(hostname, Some(0)))?;
    if let Some(path) = cert_file {
      permissions.check_read(Path::new(path))?;
    }
  }

  let ca_data = match cert_file {
    Some(path) => {
      let mut buf = Vec::new();
      File::open(path)?.read_to_end(&mut buf)?;
      Some(buf)
    }
    _ => None,
  };

  let hostname_dns = DNSNameRef::try_from_ascii_str(hostname)
    .map_err(|_| invalid_hostname(hostname))?;

  let unsafely_ignore_certificate_errors = state
    .borrow()
    .borrow::<UnsafelyIgnoreCertificateErrors>()
    .0
    .clone();

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
  let resource = Rc::try_unwrap(resource_rc)
    .expect("Only a single use of this resource should happen");
  let (read_half, write_half) = resource.into_inner();
  let tcp_stream = read_half.reunite(write_half)?;

  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let tls_config = Arc::new(create_client_config(
    root_cert_store,
    ca_data,
    unsafely_ignore_certificate_errors,
  )?);
  let tls_stream =
    TlsStream::new_client_side(tcp_stream, &tls_config, hostname_dns);

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

async fn op_connect_tls<NP>(
  state: Rc<RefCell<OpState>>,
  args: ConnectTlsArgs,
  _: (),
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
    .borrow::<UnsafelyIgnoreCertificateErrors>()
    .0
    .clone();

  if args.cert_chain.is_some() {
    super::check_unstable2(&state, "ConnectTlsOptions.certChain");
  }
  if args.private_key.is_some() {
    super::check_unstable2(&state, "ConnectTlsOptions.privateKey");
  }

  {
    let mut s = state.borrow_mut();
    let permissions = s.borrow_mut::<NP>();
    permissions.check_net(&(hostname, Some(port)))?;
    if let Some(path) = cert_file {
      permissions.check_read(Path::new(path))?;
    }
  }

  let ca_data = match cert_file {
    Some(path) => {
      let mut buf = Vec::new();
      File::open(path)?.read_to_end(&mut buf)?;
      Some(buf)
    }
    _ => None,
  };

  let root_cert_store = state
    .borrow()
    .borrow::<DefaultTlsOptions>()
    .root_cert_store
    .clone();
  let hostname_dns = DNSNameRef::try_from_ascii_str(hostname)
    .map_err(|_| invalid_hostname(hostname))?;

  let connect_addr = resolve_addr(hostname, port)
    .await?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let tcp_stream = TcpStream::connect(connect_addr).await?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;
  let mut tls_config = create_client_config(
    root_cert_store,
    ca_data,
    unsafely_ignore_certificate_errors,
  )?;

  if args.cert_chain.is_some() || args.private_key.is_some() {
    let cert_chain = args
      .cert_chain
      .ok_or_else(|| type_error("No certificate chain provided"))?;
    let private_key = args
      .private_key
      .ok_or_else(|| type_error("No private key provided"))?;

    // The `remove` is safe because load_private_keys checks that there is at least one key.
    let private_key = load_private_keys(private_key.as_bytes())?.remove(0);

    tls_config.set_single_client_cert(
      load_certs(&mut cert_chain.as_bytes())?,
      private_key,
    )?;
  }

  let tls_config = Arc::new(tls_config);

  let tls_stream =
    TlsStream::new_client_side(tcp_stream, &tls_config, hostname_dns);

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

fn load_certs(reader: &mut dyn BufRead) -> Result<Vec<Certificate>, AnyError> {
  let certs = certs(reader)
    .map_err(|_| custom_error("InvalidData", "Unable to decode certificate"))?;

  if certs.is_empty() {
    let e = custom_error("InvalidData", "No certificates found in cert file");
    return Err(e);
  }

  Ok(certs)
}

fn load_certs_from_file(path: &str) -> Result<Vec<Certificate>, AnyError> {
  let cert_file = File::open(path)?;
  let reader = &mut BufReader::new(cert_file);
  load_certs(reader)
}

fn key_decode_err() -> AnyError {
  custom_error("InvalidData", "Unable to decode key")
}

fn key_not_found_err() -> AnyError {
  custom_error("InvalidData", "No keys found in key file")
}

/// Starts with -----BEGIN RSA PRIVATE KEY-----
fn load_rsa_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = rsa_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(keys)
}

/// Starts with -----BEGIN PRIVATE KEY-----
fn load_pkcs8_keys(mut bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let keys = pkcs8_private_keys(&mut bytes).map_err(|_| key_decode_err())?;
  Ok(keys)
}

fn load_private_keys(bytes: &[u8]) -> Result<Vec<PrivateKey>, AnyError> {
  let mut keys = load_rsa_keys(bytes)?;

  if keys.is_empty() {
    keys = load_pkcs8_keys(bytes)?;
  }

  if keys.is_empty() {
    return Err(key_not_found_err());
  }

  Ok(keys)
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
  cert_file: String,
  key_file: String,
  alpn_protocols: Option<Vec<String>>,
}

fn op_listen_tls<NP>(
  state: &mut OpState,
  args: ListenTlsArgs,
  _: (),
) -> Result<OpConn, AnyError>
where
  NP: NetPermissions + 'static,
{
  assert_eq!(args.transport, "tcp");
  let hostname = &*args.hostname;
  let port = args.port;
  let cert_file = &*args.cert_file;
  let key_file = &*args.key_file;

  {
    let permissions = state.borrow_mut::<NP>();
    permissions.check_net(&(hostname, Some(port)))?;
    permissions.check_read(Path::new(cert_file))?;
    permissions.check_read(Path::new(key_file))?;
  }

  let mut tls_config = ServerConfig::new(NoClientAuth::new());
  if let Some(alpn_protocols) = args.alpn_protocols {
    super::check_unstable(state, "Deno.listenTls#alpn_protocols");
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }
  tls_config
    .set_single_cert(
      load_certs_from_file(cert_file)?,
      load_private_keys_from_file(key_file)?.remove(0),
    )
    .expect("invalid key or certificate");

  let bind_addr = resolve_addr_sync(hostname, port)?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let std_listener = std::net::TcpListener::bind(bind_addr)?;
  std_listener.set_nonblocking(true)?;
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

async fn op_accept_tls(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _: (),
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

  let tls_stream = TlsStream::new_server_side(tcp_stream, &resource.tls_config);

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
