// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::futures::TryFutureExt;
use deno_error::JsErrorBox;
use deno_tls::SocketUse;
use deno_tls::TlsKeys;
use deno_tls::create_client_config;
use deno_tls::rustls::RootCertStore;
use quinn::ConnectionError;
use quinn::crypto::rustls::QuicClientConfig;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

const VERSION: u32 = 1;

#[derive(thiserror::Error, Debug)]
pub enum Error {
  #[error(transparent)]
  StdIo(#[from] std::io::Error),
  #[error(transparent)]
  SerdeJson(#[from] deno_core::serde_json::Error),
  #[error(transparent)]
  Tls(#[from] deno_tls::TlsError),
  #[error(transparent)]
  QuinnConnect(#[from] quinn::ConnectError),
  #[error(transparent)]
  QuinnConnection(#[from] quinn::ConnectionError),
  #[error(transparent)]
  QuinnRead(#[from] quinn::ReadError),
  #[error(transparent)]
  QuinnReadExact(#[from] quinn::ReadExactError),
  #[error(transparent)]
  QuinnWrite(#[from] quinn::WriteError),

  #[error("Unexpected header")]
  UnexpectedHeader,
  #[error("Unsupported version")]
  UnsupportedVersion,
  #[error("Invalid authorization token")]
  InvalidToken,
}

static TUNNEL: OnceLock<crate::tunnel::TunnelListener> = OnceLock::new();

pub fn set_tunnel(tunnel: crate::tunnel::TunnelListener) {
  if TUNNEL.set(tunnel).is_ok() {
    setup_signal_handlers();
  }
}

pub fn get_tunnel() -> Option<&'static crate::tunnel::TunnelListener> {
  TUNNEL.get()
}

#[cfg(unix)]
fn setup_signal_handlers() {
  use tokio::signal::unix::SignalKind;

  let signals_to_handle = [
    SignalKind::hangup(),
    SignalKind::interrupt(),
    SignalKind::terminate(),
  ];

  for signal_kind in signals_to_handle {
    tokio::spawn(async move {
      let Ok(mut signal_fut) = tokio::signal::unix::signal(signal_kind) else {
        return;
      };

      loop {
        signal_fut.recv().await;
        if let Some(tunnel) = get_tunnel() {
          tunnel.connection.close(1u32.into(), b"");
        }
      }
    });
  }
}

#[cfg(windows)]
fn setup_signal_handlers() {
  macro_rules! handle_signal {
    ($handler:expr) => {
      tokio::spawn(async {
        let Ok(mut signal_fut) = $handler() else {
          return;
        };
        loop {
          signal_fut.recv().await;
          if let Some(tunnel) = get_tunnel() {
            tunnel.connection.close(1u32.into(), b"");
          }
        }
      });
    };
  }

  handle_signal!(tokio::signal::windows::ctrl_break);
  handle_signal!(tokio::signal::windows::ctrl_c);
  handle_signal!(tokio::signal::windows::ctrl_close);
  handle_signal!(tokio::signal::windows::ctrl_logoff);
  handle_signal!(tokio::signal::windows::ctrl_shutdown);
}

/// Essentially a SocketAddr, except we prefer a human
/// readable hostname to identify the remote endpoint.
#[derive(Debug, Clone)]
pub struct TunnelAddr {
  socket: SocketAddr,
  hostname: Option<String>,
}

impl TunnelAddr {
  pub fn hostname(&self) -> String {
    self
      .hostname
      .clone()
      .unwrap_or_else(|| self.socket.ip().to_string())
  }

  pub fn ip(&self) -> IpAddr {
    self.socket.ip()
  }

  pub fn port(&self) -> u16 {
    self.socket.port()
  }
}

#[derive(Debug)]
pub struct Metadata {
  pub hostnames: Vec<String>,
  pub env: HashMap<String, String>,
}

#[derive(Debug)]
pub enum Event {
  Routed,
  Migrate,
}

#[derive(Debug)]
pub struct Events {
  event_rx: tokio::sync::mpsc::Receiver<Event>,
}

impl Events {
  pub async fn next(&mut self) -> Option<Event> {
    self.event_rx.recv().await
  }
}

#[derive(Debug, Clone)]
pub struct TunnelListener {
  endpoint: quinn::Endpoint,
  connection: quinn::Connection,
  local_addr: TunnelAddr,
}

impl TunnelListener {
  pub async fn connect(
    addr: std::net::SocketAddr,
    hostname: &str,
    root_cert_store: Option<RootCertStore>,
    token: String,
    org: String,
    app: String,
  ) -> Result<(Self, Metadata, Events), Error> {
    let config = quinn::EndpointConfig::default();
    let socket = std::net::UdpSocket::bind(("::", 0))?;
    let endpoint = quinn::Endpoint::new(
      config,
      None,
      socket,
      quinn::default_runtime().unwrap(),
    )?;

    let mut tls_config = create_client_config(
      root_cert_store,
      vec![],
      None,
      TlsKeys::Null,
      SocketUse::GeneralSsl,
    )?;

    tls_config.alpn_protocols = vec!["🦕🕳️".into()];
    tls_config.enable_early_data = true;

    let mut transport_config = quinn::TransportConfig::default();
    transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
    transport_config
      .max_idle_timeout(Some(Duration::from_secs(15).try_into().unwrap()));

    let client_config =
      QuicClientConfig::try_from(tls_config).expect("TLS13 supported");
    let mut client_config = quinn::ClientConfig::new(Arc::new(client_config));
    client_config.transport_config(Arc::new(transport_config));

    let connecting = endpoint.connect_with(client_config, addr, hostname)?;

    let connection = connecting.await?;

    let mut control = connection.open_bi().await?;
    control.0.write_u32_le(VERSION).await?;
    if control.1.read_u32_le().await? != VERSION {
      return Err(Error::UnsupportedVersion);
    }

    write_message(&mut control.0, StreamHeader::Control { token, org, app })
      .await?;

    let ControlMessage::Authenticated {
      addr,
      hostnames,
      env,
      metadata,
    } = read_message(&mut control.1).await?
    else {
      return Err(Error::UnexpectedHeader);
    };

    let (event_tx, event_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
      while let Ok(message) = read_message(&mut control.1).await {
        let event = match message {
          ControlMessage::Routed {} => Event::Routed,
          ControlMessage::Migrate {} => Event::Migrate,
          _ => {
            continue;
          }
        };
        if event_tx.send(event).await.is_err() {
          break;
        }
      }
    });

    log::debug!("tunnel connected: {metadata:?}");

    let local_addr = TunnelAddr {
      socket: addr,
      hostname: hostnames.first().cloned(),
    };

    let metadata = Metadata { hostnames, env };
    let routed = Events { event_rx };

    Ok((
      Self {
        endpoint,
        connection,
        local_addr,
      },
      metadata,
      routed,
    ))
  }
}

impl TunnelListener {
  pub fn local_addr(&self) -> Result<TunnelAddr, std::io::Error> {
    Ok(self.local_addr.clone())
  }

  pub async fn accept(
    &self,
  ) -> Result<(TunnelStream, TunnelAddr), std::io::Error> {
    let (tx, mut rx) = self.connection.accept_bi().await?;

    let StreamHeader::Stream {
      remote_addr,
      local_addr,
    } = read_message(&mut rx).await.map_err(std::io::Error::other)?
    else {
      return Err(std::io::Error::other(Error::UnexpectedHeader));
    };

    Ok((
      TunnelStream {
        tx,
        rx,
        local_addr,
        remote_addr,
      },
      TunnelAddr {
        hostname: None,
        socket: remote_addr,
      },
    ))
  }

  pub async fn create_agent_stream(&self) -> Result<TunnelStream, Error> {
    let (mut tx, rx) = self.connection.open_bi().await?;
    write_message(&mut tx, StreamHeader::Agent {}).await?;
    Ok(TunnelStream {
      tx,
      rx,
      local_addr: self.endpoint.local_addr()?,
      remote_addr: self.connection.remote_address(),
    })
  }
}

#[derive(Debug)]
#[pin_project::pin_project]
pub struct TunnelStream {
  #[pin]
  tx: quinn::SendStream,
  #[pin]
  rx: quinn::RecvStream,

  local_addr: SocketAddr,
  remote_addr: SocketAddr,
}

impl TunnelStream {
  pub fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
    Ok(self.local_addr)
  }

  pub fn peer_addr(&self) -> Result<SocketAddr, std::io::Error> {
    Ok(self.remote_addr)
  }
}

impl AsyncRead for TunnelStream {
  fn poll_read(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    self.project().rx.poll_read(cx, buf)
  }
}

impl AsyncWrite for TunnelStream {
  fn poll_write(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<Result<usize, std::io::Error>> {
    AsyncWrite::poll_write(self.project().tx, cx, buf)
  }

  fn poll_flush(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    self.project().tx.poll_flush(cx)
  }

  fn poll_shutdown(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    self.project().tx.poll_shutdown(cx)
  }
}

#[derive(Debug)]
pub struct TunnelStreamResource {
  tx: AsyncRefCell<quinn::SendStream>,
  rx: AsyncRefCell<quinn::RecvStream>,
  local_addr: SocketAddr,
  remote_addr: SocketAddr,
  cancel_handle: CancelHandle,
}

impl TunnelStreamResource {
  pub fn new(stream: TunnelStream) -> Self {
    Self {
      tx: AsyncRefCell::new(stream.tx),
      rx: AsyncRefCell::new(stream.rx),
      local_addr: stream.local_addr,
      remote_addr: stream.remote_addr,
      cancel_handle: Default::default(),
    }
  }

  pub fn into_inner(self) -> TunnelStream {
    let tx = self.tx.into_inner();
    let rx = self.rx.into_inner();
    TunnelStream {
      tx,
      rx,
      local_addr: self.local_addr,
      remote_addr: self.remote_addr,
    }
  }

  fn rd_borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<quinn::RecvStream> {
    RcRef::map(self, |r| &r.rx).borrow_mut()
  }

  fn wr_borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<quinn::SendStream> {
    RcRef::map(self, |r| &r.tx).borrow_mut()
  }

  pub fn cancel_handle(self: &Rc<Self>) -> RcRef<CancelHandle> {
    RcRef::map(self, |r| &r.cancel_handle)
  }
}

impl Resource for TunnelStreamResource {
  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<deno_core::BufView> {
    Box::pin(async move {
      let mut vec = vec![0; limit];
      let nread = self
        .rd_borrow_mut()
        .await
        .read(&mut vec)
        .map_err(|e| JsErrorBox::generic(format!("{e}")))
        .try_or_cancel(self.cancel_handle())
        .await?
        .unwrap_or(0);
      if nread != vec.len() {
        vec.truncate(nread);
      }
      Ok(vec.into())
    })
  }

  fn read_byob(
    self: Rc<Self>,
    mut buf: deno_core::BufMutView,
  ) -> AsyncResult<(usize, deno_core::BufMutView)> {
    Box::pin(async move {
      let nread = self
        .rd_borrow_mut()
        .await
        .read(&mut buf)
        .map_err(|e| JsErrorBox::generic(format!("{e}")))
        .try_or_cancel(self.cancel_handle())
        .await?
        .unwrap_or(0);
      Ok((nread, buf))
    })
  }

  fn write(
    self: Rc<Self>,
    buf: deno_core::BufView,
  ) -> AsyncResult<deno_core::WriteOutcome> {
    Box::pin(async move {
      let nwritten = self
        .wr_borrow_mut()
        .await
        .write(&buf)
        .await
        .map_err(|e| JsErrorBox::generic(format!("{e}")))?;
      Ok(deno_core::WriteOutcome::Partial {
        nwritten,
        view: buf,
      })
    })
  }

  fn name(&self) -> std::borrow::Cow<str> {
    "tunnelStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(async move {
      let mut wr = self.wr_borrow_mut().await;
      wr.reset(quinn::VarInt::from_u32(0))
        .map_err(|e| JsErrorBox::generic(format!("{e}")))?;
      Ok(())
    })
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel()
  }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum StreamHeader {
  Control {
    token: String,
    org: String,
    app: String,
  },
  Stream {
    local_addr: SocketAddr,
    remote_addr: SocketAddr,
  },
  Agent {},
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum ControlMessage {
  Authenticated {
    metadata: HashMap<String, String>,
    addr: SocketAddr,
    hostnames: Vec<String>,
    env: HashMap<String, String>,
  },
  Routed {},
  Migrate {},
}

async fn write_message<T: serde::Serialize>(
  tx: &mut quinn::SendStream,
  message: T,
) -> Result<(), Error> {
  let data = deno_core::serde_json::to_vec(&message)?;
  tx.write_u32_le(data.len() as _).await?;
  tx.write_all(&data).await?;
  Ok(())
}

async fn read_message<T: serde::de::DeserializeOwned>(
  rx: &mut quinn::RecvStream,
) -> Result<T, Error> {
  let length = rx.read_u32_le().await.map_err(|e| {
    if let Some(custom_error) = e.get_ref() {
      if let Some(quinn::ReadError::ConnectionLost(
        ConnectionError::ApplicationClosed(err),
      )) = custom_error.downcast_ref::<quinn::ReadError>()
      {
        if err.reason == b"invalid token".as_slice() {
          return Error::InvalidToken;
        }
      }
    }

    e.into()
  })?;
  let mut data = vec![0; length as usize];
  rx.read_exact(&mut data).await?;

  let message = deno_core::serde_json::from_slice(&data)?;

  Ok(message)
}
