// Copyright 2018-2026 the Deno authors. MIT license.

//! In-process "memory" transport.
//!
//! A memory listener accepts connections that are backed by an in-memory
//! [`tokio::io::DuplexStream`] byte pipe rather than a kernel socket. Listeners
//! are registered in a process-global registry keyed by a caller-chosen name;
//! the peer side connects by that same name via [`connect_memory`]. This lets
//! an embedder (e.g. the `deno desktop` runtime) serve an embedded browser over
//! a raw byte channel with no TCP/loopback, port allocation, or localhost
//! exposure.

use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::DuplexStream;
use tokio::io::ReadBuf;
use tokio::io::ReadHalf;
use tokio::io::WriteHalf;
use tokio::sync::mpsc;

/// Size of each direction's in-memory buffer before writes apply backpressure.
const DUPLEX_BUF_SIZE: usize = 64 * 1024;

/// Address of a memory connection. The `name` identifies the listener; `id` is
/// a per-listener monotonic connection counter. Both ends of a connection share
/// the same address (the channel is symmetric).
#[derive(Debug, Clone)]
pub struct MemoryAddr {
  pub name: String,
  pub id: u32,
}

/// A connection backed by an in-memory duplex byte pipe.
#[derive(Debug)]
pub struct MemoryStream {
  inner: DuplexStream,
  addr: MemoryAddr,
}

impl MemoryStream {
  pub fn local_addr(&self) -> io::Result<MemoryAddr> {
    Ok(self.addr.clone())
  }

  pub fn peer_addr(&self) -> io::Result<MemoryAddr> {
    Ok(self.addr.clone())
  }

  pub fn into_split(self) -> (ReadHalf<MemoryStream>, WriteHalf<MemoryStream>) {
    tokio::io::split(self)
  }
}

impl AsyncRead for MemoryStream {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
  }
}

impl AsyncWrite for MemoryStream {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
  }

  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().inner).poll_flush(cx)
  }

  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
  }
}

/// A listener that yields in-memory duplex connections delivered via
/// [`connect_memory`].
#[derive(Debug)]
pub struct MemoryListener {
  name: String,
  rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<MemoryStream>>,
}

impl MemoryListener {
  pub async fn accept(&self) -> io::Result<(MemoryStream, MemoryAddr)> {
    let mut rx = self.rx.lock().await;
    match rx.recv().await {
      Some(stream) => {
        let addr = stream.addr.clone();
        Ok((stream, addr))
      }
      None => Err(io::Error::new(
        io::ErrorKind::ConnectionAborted,
        "memory listener closed",
      )),
    }
  }

  pub fn local_addr(&self) -> io::Result<MemoryAddr> {
    Ok(MemoryAddr {
      name: self.name.clone(),
      id: 0,
    })
  }
}

impl Drop for MemoryListener {
  fn drop(&mut self) {
    // Free the name so it can be reused; peers then see a closed channel.
    // A name is held by exactly one listener (listen_memory rejects
    // duplicates), so this entry is unambiguously ours.
    registry_lock().remove(&self.name);
  }
}

struct RegistryEntry {
  tx: mpsc::UnboundedSender<MemoryStream>,
  next_id: Arc<AtomicU32>,
}

type Registry = Mutex<HashMap<String, RegistryEntry>>;

fn registry() -> &'static Registry {
  static REGISTRY: OnceLock<Registry> = OnceLock::new();
  REGISTRY.get_or_init(Default::default)
}

fn registry_lock() -> MutexGuard<'static, HashMap<String, RegistryEntry>> {
  registry().lock().unwrap_or_else(|e| e.into_inner())
}

/// Create a memory listener registered under `name`. Fails if `name` is already
/// in use.
pub fn listen_memory(name: &str) -> io::Result<MemoryListener> {
  let (tx, rx) = mpsc::unbounded_channel();
  let mut reg = registry_lock();
  if reg.contains_key(name) {
    return Err(io::Error::new(
      io::ErrorKind::AddrInUse,
      format!("memory address already in use: {name}"),
    ));
  }
  reg.insert(
    name.to_string(),
    RegistryEntry {
      tx,
      next_id: Arc::new(AtomicU32::new(1)),
    },
  );
  Ok(MemoryListener {
    name: name.to_string(),
    rx: tokio::sync::Mutex::new(rx),
  })
}

/// Whether a memory listener is currently registered under `name`. Used to
/// wait for an in-process server to be ready before directing traffic at it
/// (without opening a real connection that the listener would need to accept).
pub fn is_listening(name: &str) -> bool {
  registry_lock().contains_key(name)
}

/// Connect to the memory listener registered under `name`, returning the
/// caller's (client) end of a fresh duplex connection. The server end is
/// delivered to the listener's `accept`.
pub fn connect_memory(name: &str) -> io::Result<MemoryStream> {
  let (tx, id) = {
    let reg = registry_lock();
    let entry = reg.get(name).ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::NotFound,
        format!("no memory listener registered for: {name}"),
      )
    })?;
    (
      entry.tx.clone(),
      entry.next_id.fetch_add(1, Ordering::Relaxed),
    )
  };
  let (server, client) = tokio::io::duplex(DUPLEX_BUF_SIZE);
  let addr = MemoryAddr {
    name: name.to_string(),
    id,
  };
  tx.send(MemoryStream {
    inner: server,
    addr: addr.clone(),
  })
  .map_err(|_| {
    io::Error::new(io::ErrorKind::ConnectionRefused, "memory listener is gone")
  })?;
  Ok(MemoryStream {
    inner: client,
    addr,
  })
}

#[cfg(test)]
mod tests {
  use tokio::io::AsyncReadExt;
  use tokio::io::AsyncWriteExt;

  use super::*;

  #[tokio::test]
  async fn listen_connect_accept_roundtrip() {
    let listener = listen_memory("test-roundtrip").unwrap();
    let mut client = connect_memory("test-roundtrip").unwrap();

    let (mut server, addr) = listener.accept().await.unwrap();
    assert_eq!(addr.name, "test-roundtrip");
    assert_eq!(addr.id, 1);
    assert_eq!(server.local_addr().unwrap().id, 1);
    assert_eq!(server.peer_addr().unwrap().id, 1);
    assert_eq!(client.local_addr().unwrap().id, 1);
    assert_eq!(client.peer_addr().unwrap().id, 1);

    // client -> server
    client.write_all(b"ping").await.unwrap();
    let mut buf = [0u8; 4];
    server.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"ping");

    // server -> client
    server.write_all(b"pong").await.unwrap();
    client.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"pong");
  }

  #[test]
  fn duplicate_name_rejected() {
    let _l = listen_memory("test-dup").unwrap();
    assert_eq!(
      listen_memory("test-dup").unwrap_err().kind(),
      io::ErrorKind::AddrInUse
    );
  }

  #[test]
  fn connect_missing_listener_fails() {
    assert_eq!(
      connect_memory("nope-not-here").unwrap_err().kind(),
      io::ErrorKind::NotFound
    );
  }

  #[test]
  fn name_freed_on_drop() {
    {
      let _l = listen_memory("test-drop").unwrap();
    }
    // Name is reusable after the listener drops.
    let _l2 = listen_memory("test-drop").unwrap();
  }
}
