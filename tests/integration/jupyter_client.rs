// Copyright 2018-2026 the Deno authors. MIT license.

//! Jupyter integration-test client backed by the real `zeromq` crate, a
//! spec-compliant ZMTP implementation. Using a real ZMTP stack (rather than a
//! hand-rolled one) means these tests exercise the kernel the same way libzmq
//! peers do (VSCode/JupyterLab/jupyter_client): a malformed greeting or READY
//! command fails the handshake here, just as it does for those clients.

use anyhow::Result;
use bytes::Bytes;
use zeromq::Socket;
use zeromq::SocketRecv;
use zeromq::SocketSend;
use zeromq::ZmqMessage;

fn endpoint(addr: &str) -> String {
  if addr.contains("://") {
    addr.to_string()
  } else {
    format!("tcp://{addr}")
  }
}

fn frames_to_message(frames: &[Bytes]) -> ZmqMessage {
  // Our Jupyter frames are always non-empty (at minimum the <IDS|MSG>
  // delimiter plus signature/header/...).
  ZmqMessage::try_from(frames.to_vec())
    .expect("ZMTP message must have at least one frame")
}

/// REQ socket (heartbeat channel).
pub struct ReqSocket(zeromq::ReqSocket);

impl ReqSocket {
  pub async fn connect(addr: &str) -> Result<Self> {
    let mut socket = zeromq::ReqSocket::new();
    socket.connect(&endpoint(addr)).await?;
    Ok(Self(socket))
  }

  /// Send a single frame (the REQ envelope is handled by the socket).
  pub async fn send(&mut self, data: Bytes) -> Result<()> {
    self.0.send(ZmqMessage::from(data)).await?;
    Ok(())
  }

  /// Receive a single frame.
  pub async fn recv(&mut self) -> Result<Bytes> {
    let msg = self.0.recv().await?;
    Ok(msg.into_vec().into_iter().next().unwrap_or_default())
  }
}

/// DEALER socket (shell, control, and stdin channels).
///
/// A real Jupyter frontend talks to the kernel's ROUTER sockets — including
/// stdin — with a DEALER, so all three use this type.
pub struct DealerSocket(zeromq::DealerSocket);

impl DealerSocket {
  pub async fn connect(addr: &str) -> Result<Self> {
    let mut socket = zeromq::DealerSocket::new();
    socket.connect(&endpoint(addr)).await?;
    Ok(Self(socket))
  }

  pub async fn send_multipart(&mut self, frames: &[Bytes]) -> Result<()> {
    self.0.send(frames_to_message(frames)).await?;
    Ok(())
  }

  pub async fn recv_multipart(&mut self) -> Result<Vec<Bytes>> {
    Ok(self.0.recv().await?.into_vec())
  }
}

/// SUB socket (iopub channel).
pub struct SubSocket(zeromq::SubSocket);

impl SubSocket {
  pub async fn connect(addr: &str) -> Result<Self> {
    let mut socket = zeromq::SubSocket::new();
    socket.connect(&endpoint(addr)).await?;
    Ok(Self(socket))
  }

  /// Subscribe to a topic prefix (empty = subscribe to all).
  pub async fn subscribe(&mut self, topic: &str) -> Result<()> {
    self.0.subscribe(topic).await?;
    Ok(())
  }

  pub async fn recv_multipart(&mut self) -> Result<Vec<Bytes>> {
    Ok(self.0.recv().await?.into_vec())
  }
}
