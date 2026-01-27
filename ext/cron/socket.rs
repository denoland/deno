// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::rc::Rc;

use deno_core::serde_json;
use deno_core::unsync::spawn;
use once_cell::unsync::OnceCell;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;
use tokio::sync::mpsc;

use crate::CronError;
use crate::CronHandle;
use crate::CronHandler;
use crate::CronNextResult;
use crate::CronSpec;
use crate::Traceparent;

pub struct SocketCronHandler {
  socket_task_tx: mpsc::Sender<SocketTaskCommand>,
  socket_task_handle: deno_core::unsync::JoinHandle<()>,
  socket_task_exit_error: Rc<OnceCell<CronError>>,
  reject_reason: Rc<OnceCell<String>>,
}

// Commands sent to the socket task
pub(crate) enum SocketTaskCommand {
  RegisterCron {
    spec: CronSpec,
    invocation_tx: mpsc::Sender<Traceparent>,
  },
  SendResult {
    name: String,
    success: bool,
  },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum OutboundMessage<'a> {
  Register { crons: &'a [CronRegistration<'a>] },
  Result { name: &'a str, success: bool },
}

#[derive(Serialize)]
struct CronRegistration<'a> {
  name: &'a str,
  schedule: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  backoff_schedule: Option<&'a [u32]>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum InboundMessage {
  Invoke {
    name: String,
    traceparent: Option<String>,
  },
  RejectNewCrons {
    reason: String,
  },
}

impl SocketCronHandler {
  pub fn new(socket_addr: String) -> Self {
    let (socket_task_tx, socket_task_rx) = mpsc::channel(32);
    let socket_task_exit_error = Rc::new(OnceCell::new());
    let reject_reason = Rc::new(OnceCell::new());
    let socket_task_handle = spawn(Self::socket_task(
      socket_addr,
      socket_task_rx,
      socket_task_exit_error.clone(),
      reject_reason.clone(),
    ));

    Self {
      socket_task_tx,
      socket_task_handle,
      socket_task_exit_error,
      reject_reason,
    }
  }

  async fn socket_task(
    socket_addr: String,
    mut socket_task_rx: mpsc::Receiver<SocketTaskCommand>,
    exit_error: Rc<OnceCell<CronError>>,
    reject_reason: Rc<OnceCell<String>>,
  ) {
    let mut invocation_txs: HashMap<String, mpsc::Sender<Traceparent>> =
      HashMap::new();
    let stream = match connect_to_socket(&socket_addr).await {
      Ok(s) => s,
      Err(e) => {
        let _ = exit_error.set(CronError::SocketError(format!(
          "Failed to connect to cron socket {}: {}",
          socket_addr, e
        )));
        return;
      }
    };

    let (reader, writer) = tokio::io::split(stream);
    let mut socket_reader = BufReader::new(reader).lines();
    let mut socket_writer = BufWriter::new(writer);

    loop {
      tokio::select! {
        Some(cmd) = socket_task_rx.recv() => {
          match cmd {
            SocketTaskCommand::RegisterCron { spec, invocation_tx } => {
              invocation_txs.insert(spec.name.clone(), invocation_tx);
              if let Err(e) = register_cron(&mut socket_writer, &spec).await {
                let _ = exit_error.set(CronError::SocketError(format!(
                  "Failed to send cron registration: {}",
                  e
                )));
                return;
              }
            }
            SocketTaskCommand::SendResult { name, success } => {
              if let Err(e) = send_execution_result(&mut socket_writer, &name, success).await {
                let _ = exit_error.set(CronError::SocketError(format!(
                  "Failed to send execution result: {}",
                  e
                )));
                return;
              }
            }
          }
        }

        result = socket_reader.next_line() => {
          match result {
            Ok(Some(line)) => {
              let _ = handle_inbound_messages(&line, &invocation_txs, &reject_reason);
            }
            Ok(None) => {
              let _ = exit_error.set(CronError::SocketError(
                "Cron socket connection closed".into()
              ));
              return;
            }
            Err(e) => {
              let _ = exit_error.set(CronError::SocketError(format!(
                "Cron socket read error: {}",
                e
              )));
              return;
            }
          }
        }
      }
    }
  }
}

impl Drop for SocketCronHandler {
  fn drop(&mut self) {
    self.socket_task_handle.abort();
  }
}

pub struct SocketCronHandle {
  spec: CronSpec,
  invocation_rx: std::cell::RefCell<Option<mpsc::Receiver<Traceparent>>>,
  socket_task_tx: mpsc::Sender<SocketTaskCommand>,
  closed: std::cell::Cell<bool>,
  first_call: std::cell::Cell<bool>,
}

impl SocketCronHandle {
  pub(crate) fn new(
    spec: CronSpec,
    invocation_rx: mpsc::Receiver<Traceparent>,
    socket_task_tx: mpsc::Sender<SocketTaskCommand>,
  ) -> Self {
    Self {
      spec,
      invocation_rx: std::cell::RefCell::new(Some(invocation_rx)),
      socket_task_tx,
      closed: std::cell::Cell::new(false),
      first_call: std::cell::Cell::new(true),
    }
  }
}

#[async_trait::async_trait(?Send)]
impl CronHandle for SocketCronHandle {
  async fn next(
    &self,
    prev_success: bool,
  ) -> Result<CronNextResult, CronError> {
    if self.closed.get() {
      return Ok(CronNextResult {
        active: false,
        traceparent: None,
      });
    }

    if !self.first_call.replace(false) {
      let _ = self
        .socket_task_tx
        .send(SocketTaskCommand::SendResult {
          name: self.spec.name.clone(),
          success: prev_success,
        })
        .await;
    }

    let mut invocation_rx = self
      .invocation_rx
      .take()
      .expect("calls to CronHandle::next should be serialized");
    let r = match invocation_rx.recv().await {
      Some(traceparent) => Ok(CronNextResult {
        active: true,
        traceparent,
      }),
      None => {
        self.closed.set(true);
        Ok(CronNextResult {
          active: false,
          traceparent: None,
        })
      }
    };
    self.invocation_rx.replace(Some(invocation_rx));
    r
  }

  fn close(&self) {
    self.closed.set(true);
  }
}

enum SocketStream {
  Tcp(tokio::net::TcpStream),
  Unix(tokio::net::UnixStream),
  #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
  Vsock(tokio_vsock::VsockStream),
}

impl tokio::io::AsyncRead for SocketStream {
  fn poll_read(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    match &mut *self {
      Self::Tcp(s) => std::pin::Pin::new(s).poll_read(cx, buf),
      Self::Unix(s) => std::pin::Pin::new(s).poll_read(cx, buf),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Self::Vsock(s) => std::pin::Pin::new(s).poll_read(cx, buf),
    }
  }
}

impl tokio::io::AsyncWrite for SocketStream {
  fn poll_write(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<std::io::Result<usize>> {
    match &mut *self {
      Self::Tcp(s) => std::pin::Pin::new(s).poll_write(cx, buf),
      Self::Unix(s) => std::pin::Pin::new(s).poll_write(cx, buf),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Self::Vsock(s) => std::pin::Pin::new(s).poll_write(cx, buf),
    }
  }

  fn poll_flush(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    match &mut *self {
      Self::Tcp(s) => std::pin::Pin::new(s).poll_flush(cx),
      Self::Unix(s) => std::pin::Pin::new(s).poll_flush(cx),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Self::Vsock(s) => std::pin::Pin::new(s).poll_flush(cx),
    }
  }

  fn poll_shutdown(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    match &mut *self {
      Self::Tcp(s) => std::pin::Pin::new(s).poll_shutdown(cx),
      Self::Unix(s) => std::pin::Pin::new(s).poll_shutdown(cx),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Self::Vsock(s) => std::pin::Pin::new(s).poll_shutdown(cx),
    }
  }
}

async fn connect_to_socket(
  socket_addr: &str,
) -> Result<SocketStream, std::io::Error> {
  use tokio::net::TcpStream;
  use tokio::net::UnixStream;

  match socket_addr.split_once(':') {
    Some(("tcp", addr)) => {
      let stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect(addr),
      )
      .await??;
      Ok(SocketStream::Tcp(stream))
    }
    Some(("unix", path)) => {
      let stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        UnixStream::connect(path),
      )
      .await??;
      Ok(SocketStream::Unix(stream))
    }
    #[cfg(any(
      target_os = "android",
      target_os = "linux",
      target_os = "macos"
    ))]
    Some(("vsock", addr)) => {
      use tokio_vsock::VsockAddr;
      use tokio_vsock::VsockStream;
      let (cid, port) = addr.split_once(':').ok_or_else(|| {
        std::io::Error::new(
          std::io::ErrorKind::InvalidInput,
          "invalid vsock addr",
        )
      })?;
      let cid = if cid == "-1" {
        u32::MAX
      } else {
        cid.parse().map_err(|_| {
          std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid vsock cid",
          )
        })?
      };
      let port = port.parse().map_err(|_| {
        std::io::Error::new(
          std::io::ErrorKind::InvalidInput,
          "invalid vsock port",
        )
      })?;
      let stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        VsockStream::connect(VsockAddr::new(cid, port)),
      )
      .await??;
      Ok(SocketStream::Vsock(stream))
    }
    _ => Err(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "invalid socket address",
    )),
  }
}

async fn register_cron(
  socket_writer: &mut BufWriter<impl tokio::io::AsyncWrite + Unpin>,
  spec: &CronSpec,
) -> Result<(), std::io::Error> {
  let cron = CronRegistration {
    name: &spec.name,
    schedule: &spec.cron_schedule,
    backoff_schedule: spec.backoff_schedule.as_deref(),
  };

  let msg = OutboundMessage::Register { crons: &[cron] };

  let json = serde_json::to_string(&msg).map_err(std::io::Error::other)?;
  socket_writer.write_all(json.as_bytes()).await?;
  socket_writer.write_all(b"\n").await?;
  socket_writer.flush().await?;

  Ok(())
}

async fn send_execution_result(
  socket_writer: &mut BufWriter<impl tokio::io::AsyncWrite + Unpin>,
  name: &str,
  success: bool,
) -> Result<(), std::io::Error> {
  let msg = OutboundMessage::Result { name, success };

  let json = serde_json::to_string(&msg).map_err(std::io::Error::other)?;
  socket_writer.write_all(json.as_bytes()).await?;
  socket_writer.write_all(b"\n").await?;
  socket_writer.flush().await?;

  Ok(())
}

fn handle_inbound_messages(
  line: &str,
  invocation_txs: &HashMap<String, mpsc::Sender<Traceparent>>,
  reject_reason: &Rc<OnceCell<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
  let msg: InboundMessage = serde_json::from_str(line)?;

  match msg {
    InboundMessage::Invoke { name, traceparent } => {
      if let Some(tx) = invocation_txs.get(&name) {
        let _ = tx.try_send(traceparent);
      }
    }
    InboundMessage::RejectNewCrons { reason } => {
      let _ = reject_reason.set(reason);
    }
  }

  Ok(())
}

impl CronHandler for SocketCronHandler {
  type EH = SocketCronHandle;

  fn create(&self, spec: CronSpec) -> Result<Self::EH, CronError> {
    if let Some(reason) = self.reject_reason.get() {
      return Err(CronError::RejectedError(reason.clone()));
    }

    let (invocation_tx, invocation_rx) = mpsc::channel::<Traceparent>(1);
    let socket_task_tx = self.socket_task_tx.clone();
    let socket_task_exit_error = self.socket_task_exit_error.clone();

    socket_task_tx
      .try_send(SocketTaskCommand::RegisterCron {
        spec: spec.clone(),
        invocation_tx,
      })
      .map_err(|_| {
        if let Some(err) = socket_task_exit_error.get() {
          CronError::SocketError(err.to_string())
        } else {
          CronError::SocketError("Socket task closed".into())
        }
      })?;

    Ok(SocketCronHandle::new(spec, invocation_rx, socket_task_tx))
  }
}
