// Copyright 2018-2026 the Deno authors. MIT license.

//! Fast-path server WebSocket for plain TCP transports.
//!
//! When `ws_create_server_stream` receives a `NetworkStream::Tcp`, we
//! avoid wrapping the socket in a full async write state machine and
//! instead drive sends with `try_write` / `try_write_vectored`,
//! matching the strategy explored in fastwebsockets PR #133's
//! `echo_server_tokio_fast` example. The read side still uses
//! `fastwebsockets::WebSocketRead` so behavior — including auto-pong,
//! auto-close, pong surfacing for idle-timeout, and fragmentation
//! reassembly — is byte-for-byte identical to the generic path.
//!
//! We keep the `TcpStream` shared between read and write through an
//! `Rc<TcpStream>` rather than splitting via `TcpStream::into_split`.
//! tokio's `OwnedWriteHalf::drop` calls `shutdown(SHUT_WR)` which
//! would send an extra `FIN` on the write side before the inner
//! `Arc<TcpStreamInner>` count reaches zero, shifting the
//! peer-disconnect timing relative to the generic
//! `tokio::io::split`-based path and tripping the
//! `tests/unit/websocket_test.ts::websocketTlsSocketWorks` leak
//! sanitizer on linux-aarch64 / linux-x86_64.

use std::io::IoSlice;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Poll;

use bytes::Buf;
use bytes::Bytes;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::WebSocketError;
use tokio::io::AsyncRead;
use tokio::io::ReadBuf;
use tokio::net::TcpStream;

/// `AsyncRead` adapter over a shared `Rc<TcpStream>` that prepends any
/// bytes already buffered by the upstream HTTP upgrade before
/// delivering fresh bytes from the socket. Mirrors
/// `stream::WebSocketStream`'s `pre` field for the fast-TCP path.
///
/// We poll the socket directly with `TcpStream::poll_read_ready` +
/// `try_read` rather than going through `OwnedReadHalf` so that the
/// write side can hold the same `Rc<TcpStream>` and use `try_write` /
/// `try_write_vectored` without an `into_split` half-close on drop —
/// see the module-level doc comment.
pub(crate) struct TcpReadStream {
  tcp: Rc<TcpStream>,
  pre: Option<Bytes>,
}

impl TcpReadStream {
  pub(crate) fn new(tcp: Rc<TcpStream>, prefix: Option<Bytes>) -> Self {
    Self {
      tcp,
      pre: prefix.filter(|b| !b.is_empty()),
    }
  }
}

impl AsyncRead for TcpReadStream {
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<std::io::Result<()>> {
    if let Some(mut prefix) = self.pre.take() {
      let copy_len = std::cmp::min(prefix.len(), buf.remaining());
      buf.put_slice(&prefix[..copy_len]);
      prefix.advance(copy_len);
      if !prefix.is_empty() {
        self.pre = Some(prefix);
      }
      return Poll::Ready(Ok(()));
    }
    // tokio's `TcpStream::poll_read` is implemented via
    // `poll_read_priv`, which only needs a `&TcpStream` internally —
    // exposing it through `Pin<&mut Self>` here is just because the
    // `AsyncRead` contract requires `&mut Self`.
    let tcp: &TcpStream = &self.tcp;
    tcp.poll_read_ready(cx).map(|res| {
      res.and_then(|()| {
        let unfilled = buf.initialize_unfilled();
        match tcp.try_read(unfilled) {
          Ok(n) => {
            buf.advance(n);
            Ok(())
          }
          Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            // Spurious wakeup — `Poll::Pending`. Returning Ok(()) here
            // (Poll::Ready(Ok(()))) with 0 bytes filled would look
            // like EOF to the caller. Re-poll-read-ready instead:
            // tokio clears the readiness bit on `try_read` returning
            // WouldBlock, so the next call to `poll_read_ready` will
            // wait for fresh readiness rather than spinning.
            Ok(())
          }
          Err(e) => Err(e),
        }
      })
    })
  }
}

/// Per-resource write state for the fast-TCP path. Holds a clone of
/// the shared `Rc<TcpStream>` (same socket as the read side, so the
/// inner-Arc count is bookkept correctly on drop) plus a sticky
/// `closed` flag matching `fastwebsockets::WebSocketWrite::is_closed`.
pub(crate) struct TcpWriteState {
  pub(crate) tcp: Rc<TcpStream>,
  pub(crate) closed: bool,
}

impl TcpWriteState {
  pub(crate) fn new(tcp: Rc<TcpStream>) -> Self {
    Self { tcp, closed: false }
  }
}

/// Format an RFC 6455 server-side frame header (no mask) into `head`.
/// Returns the number of bytes written.
#[inline]
fn fmt_server_head(
  head: &mut [u8; 10],
  fin: bool,
  opcode: OpCode,
  payload_len: usize,
) -> usize {
  let fin_bit = if fin { 0x80 } else { 0x00 };
  head[0] = fin_bit | (opcode as u8);
  if payload_len < 126 {
    head[1] = payload_len as u8;
    2
  } else if payload_len < 65536 {
    head[1] = 126;
    head[2..4].copy_from_slice(&(payload_len as u16).to_be_bytes());
    4
  } else {
    head[1] = 127;
    head[2..10].copy_from_slice(&(payload_len as u64).to_be_bytes());
    10
  }
}

/// Drive a single frame onto the wire via `try_write_vectored`,
/// falling back to `writable().await` only when the kernel send buffer
/// is full. This is the "Deno-friendly fast path" from
/// fastwebsockets PR #133: one direct send syscall per frame in
/// steady state, no per-call Future allocation.
async fn write_via_try(
  tcp: &TcpStream,
  head: &[u8],
  payload: &[u8],
) -> Result<(), std::io::Error> {
  if payload.is_empty() {
    // No payload: single contiguous write of just the header.
    let mut buf = head;
    while !buf.is_empty() {
      match tcp.try_write(buf) {
        Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
        Ok(n) => buf = &buf[n..],
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          tcp.writable().await?;
        }
        Err(e) => return Err(e),
      }
    }
    return Ok(());
  }

  // Two-segment vectored write: header + payload.
  let mut iovs = [IoSlice::new(head), IoSlice::new(payload)];
  let mut head_consumed = 0usize;
  let mut payload_consumed = 0usize;
  let mut total = head.len() + payload.len();
  while total > 0 {
    let remaining_head = head.len().saturating_sub(head_consumed);
    let n = if remaining_head > 0 {
      // Rebuild iovecs to reflect partial-consumption of the head.
      iovs[0] = IoSlice::new(&head[head_consumed..]);
      iovs[1] = IoSlice::new(&payload[payload_consumed..]);
      match tcp.try_write_vectored(&iovs) {
        Ok(n) => n,
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          tcp.writable().await?;
          continue;
        }
        Err(e) => return Err(e),
      }
    } else {
      // Head fully sent; finish payload with non-vectored writes.
      match tcp.try_write(&payload[payload_consumed..]) {
        Ok(n) => n,
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          tcp.writable().await?;
          continue;
        }
        Err(e) => return Err(e),
      }
    };
    if n == 0 {
      return Err(std::io::ErrorKind::WriteZero.into());
    }
    if remaining_head > 0 {
      let n_into_head = std::cmp::min(remaining_head, n);
      head_consumed += n_into_head;
      payload_consumed += n - n_into_head;
    } else {
      payload_consumed += n;
    }
    total -= n;
  }
  Ok(())
}

/// Write one server-side frame to a TCP socket via the fast path.
///
/// Mirrors `fastwebsockets::WebSocketWrite::write_frame` semantics for
/// the server role: no mask, single header + payload, sets the
/// `closed` flag once a Close frame has been transmitted (so future
/// writes become no-ops and we don't tear the framing mid-stream).
pub(crate) async fn write_frame_fast<'f>(
  state: &mut TcpWriteState,
  frame: Frame<'f>,
) -> Result<(), WebSocketError> {
  if state.closed {
    return Ok(());
  }
  let payload: &[u8] = match &frame.payload {
    fastwebsockets::Payload::Bytes(b) => b.as_ref(),
    fastwebsockets::Payload::Borrowed(b) => b,
    fastwebsockets::Payload::Owned(o) => o,
    fastwebsockets::Payload::BorrowedMut(b) => b,
  };
  let mut head = [0u8; 10];
  let head_len =
    fmt_server_head(&mut head, frame.fin, frame.opcode, payload.len());
  if matches!(frame.opcode, OpCode::Close) {
    state.closed = true;
  }
  write_via_try(&state.tcp, &head[..head_len], payload)
    .await
    .map_err(WebSocketError::IoError)?;
  Ok(())
}

/// Whether the fast path is enabled. Set `DENO_WS_DISABLE_FAST_TCP=1`
/// to fall back to the generic path for any reason (regression hunt,
/// instrumented build, etc.).
pub(crate) fn fast_tcp_enabled() -> bool {
  match std::env::var("DENO_WS_DISABLE_FAST_TCP") {
    Ok(v) => v.is_empty(),
    Err(_) => true,
  }
}
