// Copyright 2018-2026 the Deno authors. MIT license.

//! Minimal ZMTP 3.1 client for Jupyter integration tests.
//! Implements the client side of REQ, DEALER, SUB, and ROUTER socket types.

use std::io;

use bytes::Bytes;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

// ─── Low-level ZMTP framing ────────────────────────────────────────────────────

pub struct ZmtpConn {
  stream: TcpStream,
}

impl ZmtpConn {
  pub async fn connect(addr: &str) -> io::Result<Self> {
    let stream = TcpStream::connect(addr).await?;
    Ok(Self { stream })
  }

  pub async fn read_exact_bytes(&mut self, n: usize) -> io::Result<Bytes> {
    let mut buf = vec![0u8; n];
    self.stream.read_exact(&mut buf).await?;
    Ok(Bytes::from(buf))
  }

  pub async fn read_frame(&mut self) -> io::Result<(Bytes, bool)> {
    let flag = self.read_exact_bytes(1).await?[0];
    let _is_command = (flag & 0x04) != 0;
    let is_long = (flag & 0x02) != 0;
    let has_more = (flag & 0x01) != 0;

    let size = if is_long {
      let b = self.read_exact_bytes(8).await?;
      u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as usize
    } else {
      self.read_exact_bytes(1).await?[0] as usize
    };

    let data = self.read_exact_bytes(size).await?;
    Ok((data, has_more))
  }

  pub async fn write_frame(
    &mut self,
    data: &[u8],
    more: bool,
    is_command: bool,
  ) -> io::Result<()> {
    let flag = if more { 0x01u8 } else { 0x00 }
      | if data.len() > 255 { 0x02 } else { 0x00 }
      | if is_command { 0x04 } else { 0x00 };

    if data.len() > 255 {
      let mut header = vec![flag, 0, 0, 0, 0];
      let len_bytes = (data.len() as u32).to_be_bytes();
      header.extend_from_slice(&len_bytes);
      self.stream.write_all(&header).await?;
    } else {
      self.stream.write_all(&[flag, data.len() as u8]).await?;
    }
    self.stream.write_all(data).await?;
    Ok(())
  }

  pub async fn send_multipart(&mut self, frames: &[Bytes]) -> io::Result<()> {
    for (i, frame) in frames.iter().enumerate() {
      let more = i < frames.len() - 1;
      self.write_frame(frame, more, false).await?;
    }
    Ok(())
  }

  pub async fn recv_multipart(&mut self) -> io::Result<Vec<Bytes>> {
    let mut parts = Vec::new();
    loop {
      let (data, has_more) = self.read_frame().await?;
      parts.push(data);
      if !has_more {
        break;
      }
    }
    Ok(parts)
  }

  /// ZMTP 3.1 NULL handshake (client side).
  pub async fn handshake(&mut self, socket_type: &str) -> io::Result<()> {
    // Send greeting
    let greeting = make_greeting(socket_type, false);
    self.stream.write_all(&greeting).await?;

    // Read server's greeting (64 bytes)
    self.read_exact_bytes(64).await?;

    // Send READY command
    let ready = make_ready_command(socket_type);
    self.stream.write_all(&ready).await?;

    // Read server's READY command (skip it)
    self.read_frame().await?;

    Ok(())
  }
}

fn make_greeting(_socket_type: &str, as_server: bool) -> Vec<u8> {
  let mut buf = vec![0u8; 64];
  buf[0] = 0xff;
  buf[8] = 0x01;
  buf[9] = 0x7f;
  buf[10] = 0x03; // major
  buf[11] = 0x01; // minor
  let mech = b"NULL";
  buf[12..12 + mech.len()].copy_from_slice(mech);
  buf[32] = if as_server { 1 } else { 0 };
  buf
}

fn make_ready_command(socket_type: &str) -> Vec<u8> {
  let name = b"READY";
  let prop_name = b"Socket-Type";
  let prop_value = socket_type.as_bytes();

  // Build body: name + properties
  // properties: len(1) propName + len(4) propValue
  let mut body = Vec::new();
  body.extend_from_slice(name);
  body.push(prop_name.len() as u8);
  body.extend_from_slice(prop_name);
  body.extend_from_slice(&(prop_value.len() as u32).to_be_bytes());
  body.extend_from_slice(prop_value);

  // Command frame: flag=0x04 | 0x00, size(1), body
  let mut frame = Vec::new();
  let flag = 0x04u8; // command, not long, no more
  frame.push(flag);
  frame.push(body.len() as u8);
  frame.extend_from_slice(&body);
  frame
}

// ─── High-level socket types ──────────────────────────────────────────────────

/// REQ socket (heartbeat channel).
pub struct ReqSocket(ZmtpConn);

impl ReqSocket {
  pub async fn connect(addr: &str) -> io::Result<Self> {
    let mut conn = ZmtpConn::connect(addr).await?;
    conn.handshake("REQ").await?;
    Ok(Self(conn))
  }

  /// Send a single frame.
  pub async fn send(&mut self, data: Bytes) -> io::Result<()> {
    // REQ socket wraps in envelope: [empty, data]
    self.0.send_multipart(&[Bytes::new(), data]).await
  }

  /// Receive a single frame (strips the empty envelope).
  pub async fn recv(&mut self) -> io::Result<Bytes> {
    let mut parts = self.0.recv_multipart().await?;
    // REP reply is [empty, data]; strip empty
    if parts.len() >= 2 && parts[0].is_empty() {
      parts.remove(0);
    }
    Ok(parts.into_iter().next().unwrap_or_default())
  }
}

/// DEALER socket (shell/control channels).
pub struct DealerSocket(ZmtpConn);

impl DealerSocket {
  pub async fn connect(addr: &str) -> io::Result<Self> {
    let mut conn = ZmtpConn::connect(addr).await?;
    conn.handshake("DEALER").await?;
    Ok(Self(conn))
  }

  pub async fn send_multipart(&mut self, frames: &[Bytes]) -> io::Result<()> {
    self.0.send_multipart(frames).await
  }

  pub async fn recv_multipart(&mut self) -> io::Result<Vec<Bytes>> {
    self.0.recv_multipart().await
  }
}

/// SUB socket (iopub channel).
pub struct SubSocket(ZmtpConn);

impl SubSocket {
  pub async fn connect(addr: &str) -> io::Result<Self> {
    let mut conn = ZmtpConn::connect(addr).await?;
    conn.handshake("SUB").await?;
    Ok(Self(conn))
  }

  /// Subscribe to a topic prefix (empty = subscribe to all).
  pub async fn subscribe(&mut self, topic: &str) -> io::Result<()> {
    // SUB subscribe: send a SUBSCRIBE command
    // In ZMTP, SUB sends a command with the subscription prefix
    // Format: command frame with body = b"\x01" + topic bytes
    let mut body = Vec::new();
    body.push(b'\x01'); // subscribe action
    body.extend_from_slice(topic.as_bytes());
    self.0.write_frame(&body, false, true).await
  }

  pub async fn recv_multipart(&mut self) -> io::Result<Vec<Bytes>> {
    self.0.recv_multipart().await
  }
}

/// ROUTER socket (stdin channel in test client, mimics ZMQ ROUTER connecting to DEALER).
pub struct RouterSocket(ZmtpConn);

impl RouterSocket {
  pub async fn connect(addr: &str) -> io::Result<Self> {
    let mut conn = ZmtpConn::connect(addr).await?;
    conn.handshake("ROUTER").await?;
    Ok(Self(conn))
  }

  pub async fn send_multipart(&mut self, frames: &[Bytes]) -> io::Result<()> {
    self.0.send_multipart(frames).await
  }

  pub async fn recv_multipart(&mut self) -> io::Result<Vec<Bytes>> {
    self.0.recv_multipart().await
  }
}
