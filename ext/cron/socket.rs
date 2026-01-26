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
use crate::CronSpec;

pub struct SocketCronHandler {
  socket_task_tx: mpsc::Sender<SocketTaskCommand>,
  task_handle: deno_core::unsync::JoinHandle<()>,
  socket_task_exit_error: Rc<OnceCell<CronError>>,
}

// Commands sent to the socket task
pub(crate) enum SocketTaskCommand {
  RegisterCron {
    spec: CronSpec,
    invocation_tx: mpsc::Sender<()>,
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
#[serde(tag = "kind", rename_all = "lowercase")]
enum InboundMessage {
  Invoke { name: String },
}

impl SocketCronHandler {
  pub fn new(socket_addr: String) -> Self {
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    let exit_error = Rc::new(OnceCell::new());
    let task =
      spawn(Self::socket_task(socket_addr, cmd_rx, exit_error.clone()));

    Self {
      socket_task_tx: cmd_tx,
      task_handle: task,
      socket_task_exit_error: exit_error,
    }
  }

  async fn socket_task(
    socket_addr: String,
    mut cmd_rx: mpsc::Receiver<SocketTaskCommand>,
    exit_error: Rc<OnceCell<CronError>>,
  ) {
    let mut invocation_senders = HashMap::new();
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
    let mut reader = BufReader::new(reader).lines();
    let mut writer = BufWriter::new(writer);

    loop {
      tokio::select! {
        Some(cmd) = cmd_rx.recv() => {
          match cmd {
            SocketTaskCommand::RegisterCron { spec, invocation_tx } => {
              invocation_senders.insert(spec.name.clone(), invocation_tx);
              if let Err(e) = register_cron(&mut writer, &spec).await {
                let _ = exit_error.set(CronError::SocketError(format!(
                  "Failed to send cron registration: {}",
                  e
                )));
                return;
              }
            }
            SocketTaskCommand::SendResult { name, success } => {
              if let Err(e) = send_execution_result(&mut writer, &name, success).await {
                let _ = exit_error.set(CronError::SocketError(format!(
                  "Failed to send execution result: {}",
                  e
                )));
                return;
              }
            }
          }
        }

        result = reader.next_line() => {
          match result {
            Ok(Some(line)) => {
              let _ = handle_invocation_message(&line, &invocation_senders);
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
    self.task_handle.abort();
  }
}

pub struct SocketCronHandle {
  spec: CronSpec,
  invocation_rx: std::cell::RefCell<Option<mpsc::Receiver<()>>>,
  socket_task_tx: mpsc::Sender<SocketTaskCommand>,
  closed: std::cell::Cell<bool>,
  first_call: std::cell::Cell<bool>,
}

impl SocketCronHandle {
  pub(crate) fn new(
    spec: CronSpec,
    invocation_rx: mpsc::Receiver<()>,
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
  async fn next(&self, prev_success: bool) -> Result<bool, CronError> {
    if self.closed.get() {
      return Ok(false);
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
      Some(()) => Ok(true),
      None => {
        self.closed.set(true);
        Ok(false)
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
  writer: &mut BufWriter<impl tokio::io::AsyncWrite + Unpin>,
  spec: &CronSpec,
) -> Result<(), std::io::Error> {
  let cron = CronRegistration {
    name: &spec.name,
    schedule: &spec.cron_schedule,
    backoff_schedule: spec.backoff_schedule.as_deref(),
  };

  let msg = OutboundMessage::Register { crons: &[cron] };

  let mut json = serde_json::to_string(&msg).map_err(std::io::Error::other)?;
  json.push('\n');
  writer.write_all(json.as_bytes()).await?;
  writer.flush().await?;

  Ok(())
}

async fn send_execution_result(
  writer: &mut BufWriter<impl tokio::io::AsyncWrite + Unpin>,
  name: &str,
  success: bool,
) -> Result<(), std::io::Error> {
  let msg = OutboundMessage::Result { name, success };

  let mut json = serde_json::to_string(&msg).map_err(std::io::Error::other)?;
  json.push('\n');
  writer.write_all(json.as_bytes()).await?;
  writer.flush().await?;

  Ok(())
}

fn handle_invocation_message(
  line: &str,
  invocation_senders: &HashMap<String, mpsc::Sender<()>>,
) -> Result<(), Box<dyn std::error::Error>> {
  let msg: InboundMessage = serde_json::from_str(line)?;

  match msg {
    InboundMessage::Invoke { name } => {
      if let Some(tx) = invocation_senders.get(&name) {
        let _ = tx.try_send(());
      }
    }
  }

  Ok(())
}

impl CronHandler for SocketCronHandler {
  type EH = SocketCronHandle;

  fn create(&self, spec: CronSpec) -> Result<Self::EH, CronError> {
    let (tx, rx) = mpsc::channel(1);
    let socket_tx = self.socket_task_tx.clone();
    let exit_error = self.socket_task_exit_error.clone();

    socket_tx
      .try_send(SocketTaskCommand::RegisterCron {
        spec: spec.clone(),
        invocation_tx: tx,
      })
      .map_err(|_| {
        if let Some(err) = exit_error.get() {
          CronError::SocketError(err.to_string())
        } else {
          CronError::SocketError("Socket task closed".into())
        }
      })?;

    Ok(SocketCronHandle::new(spec, rx, socket_tx))
  }
}
