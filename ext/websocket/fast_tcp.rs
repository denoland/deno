// Copyright 2018-2026 the Deno authors. MIT license.

//! Fast-path server WebSocket for plain TCP transports.
//!
//! Restructured per Divy's scope-correction on the issue: this is now
//! a real ServerEngine-style integration of the fastwebsockets PR #133
//! optimizations, not just `try_write` on the write side.
//!
//! Per-frame loop, run inline inside `op_ws_next_event`:
//!
//! 1. Read bytes asynchronously into a per-resource scratch buffer
//!    (no `WebSocketRead` future state machine, no `BytesMut::split_to`
//!    per frame, no `FragmentCollectorRead` wrapper).
//! 2. Parse frame headers synchronously via `fastwebsockets::parse_header`
//!    (the public sync entry point introduced in PR #133).
//! 3. Unmask payload in place via `fastwebsockets::unmask`.
//! 4. For Ping / Close, synthesize the response header *into the freed
//!    mask slot in the scratch buffer* and send one contiguous slice
//!    via `try_write` — the in-place zero-copy outbound trick from
//!    PR #133's `ServerEngine::process_into`. No allocation, no
//!    scatter/gather, one `send()` syscall.
//! 5. For Text / Binary, copy the unmasked payload out into a small
//!    `VecDeque<PendingMessage>` so we can surface it to JS via
//!    `op_ws_get_buffer` / `op_ws_get_buffer_as_string`.
//! 6. For Pong, surface to JS (server-side idle-timeout heartbeats
//!    rely on this).
//! 7. Reassemble Continuation frames here in the loop — `ServerEngine`
//!    doesn't, so we hand-roll it against the bounded
//!    `max_message_size`.
//!
//! Write side stays on `try_write` / `try_write_vectored` for
//! user-initiated sends (the "tokio_fast" half of PR #133). Read and
//! write share the underlying `Rc<TcpStream>` so we don't pay
//! `OwnedWriteHalf::drop`'s extra `shutdown(SHUT_WR)`.

use std::collections::VecDeque;
use std::io::IoSlice;
use std::rc::Rc;

use bytes::Bytes;
use deno_core::AsyncRefCell;
use fastwebsockets::CloseCode;
use fastwebsockets::Frame;
use fastwebsockets::Header;
use fastwebsockets::HeaderParse;
use fastwebsockets::OpCode;
use fastwebsockets::Payload;
use fastwebsockets::WebSocketError;
use fastwebsockets::parse_header;
use fastwebsockets::unmask;
use tokio::net::TcpStream;

/// Initial size of the per-resource read scratch buffer. Big enough
/// to hold a typical message plus its 2-10 byte header without
/// growing.
const SCRATCH_INITIAL: usize = 64 * 1024;
/// Cap on per-resource scratch growth. Matches fastwebsockets's
/// default `max_message_size` so a 64 MiB single frame fits — but we
/// also enforce `max_message_size` on fragment reassembly directly.
const SCRATCH_MAX: usize = 64 << 20;
/// Matches `fastwebsockets::ReadHalf::after_handshake`'s default.
const MAX_MESSAGE_SIZE: usize = 64 << 20;

/// A complete message ready to be surfaced to JS via
/// `op_ws_next_event` / `op_ws_get_buffer` / `op_ws_get_buffer_as_string`.
pub(crate) enum PendingMessage {
  Text(Vec<u8>),
  Binary(Vec<u8>),
  Pong,
  Close {
    code: Option<u16>,
    reason: Option<Vec<u8>>,
  },
}

/// Per-resource read-side state for the fast-TCP path.
pub(crate) struct EngineReadState {
  /// Per-resource scratch buffer for the inline framing engine. We
  /// grow it on demand (up to `SCRATCH_MAX`) when a single frame
  /// straddles a recv, but the steady state is a fixed allocation.
  pub(crate) scratch: Vec<u8>,
  /// `scratch[unread_start..unread_end]` is the bytes-on-hand that
  /// haven't been parsed into a frame yet.
  pub(crate) unread_start: usize,
  pub(crate) unread_end: usize,
  /// Fragment-reassembly buffer. fastwebsockets' `ServerEngine` does
  /// not reassemble fragments (it errors with
  /// `InvalidFragment` / `InvalidContinuationFrame`); we hand-roll
  /// that bit since Deno's JS API does see reassembled messages.
  pub(crate) fragment_buf: Vec<u8>,
  /// `Some(opcode)` between the FIN=0 start frame and the FIN=1
  /// terminator. `opcode` is the data-message opcode (Text or Binary)
  /// so we know what to emit when the message completes.
  pub(crate) fragment_opcode: Option<OpCode>,
  /// FIFO of messages already pulled off the wire but not yet
  /// consumed by JS. In the steady state this is at most 1 element
  /// (JS calls `op_ws_next_event` once per message), but we let
  /// multi-frame coalescing land more than one parse into pending
  /// without spinning back through JS.
  pub(crate) pending: VecDeque<PendingMessage>,
  /// Initial bytes that the hyper upgrade left buffered before the
  /// upgrade response; feed these through the engine before the next
  /// `try_read`.
  pub(crate) prefix: Option<Bytes>,
}

impl EngineReadState {
  pub(crate) fn new(prefix: Option<Bytes>) -> Self {
    Self {
      scratch: vec![0u8; SCRATCH_INITIAL],
      unread_start: 0,
      unread_end: 0,
      fragment_buf: Vec::new(),
      fragment_opcode: None,
      pending: VecDeque::new(),
      prefix: prefix.filter(|b| !b.is_empty()),
    }
  }
}

/// Per-resource write state for the fast-TCP path. Just a sticky
/// `closed` flag matching `fastwebsockets::WebSocketWrite::is_closed`.
/// The TCP socket itself lives one level up (`FastTcpInner::tcp`,
/// shared with the read side via `Rc`).
pub(crate) struct WriteSerializer {
  pub(crate) closed: bool,
}

impl WriteSerializer {
  pub(crate) fn new() -> Self {
    Self { closed: false }
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

/// Drive bytes to the wire via `try_write` / `try_write_vectored`,
/// only entering `writable().await` when the kernel send buffer is
/// full. This is the "Deno-friendly fast path" from fastwebsockets
/// PR #133's `echo_server_tokio_fast.rs`: one direct `sendto` syscall
/// per frame in steady state, no per-call Future allocation.
async fn write_via_try(
  tcp: &TcpStream,
  head: &[u8],
  payload: &[u8],
) -> Result<(), std::io::Error> {
  if payload.is_empty() {
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
  let mut iovs = [IoSlice::new(head), IoSlice::new(payload)];
  let mut head_consumed = 0usize;
  let mut payload_consumed = 0usize;
  let mut total = head.len() + payload.len();
  while total > 0 {
    let remaining_head = head.len().saturating_sub(head_consumed);
    let n = if remaining_head > 0 {
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

/// Write one server-side frame from a generic `Frame` (used by
/// user-initiated sends from `op_ws_send_*` / `op_ws_close`). Sets
/// the sticky `closed` flag on Close so future writes are no-ops.
pub(crate) async fn write_frame_via_serializer<'f>(
  ws: &mut WriteSerializer,
  tcp: &TcpStream,
  frame: Frame<'f>,
) -> Result<(), WebSocketError> {
  if ws.closed {
    return Ok(());
  }
  let payload: &[u8] = match &frame.payload {
    Payload::Bytes(b) => b.as_ref(),
    Payload::Borrowed(b) => b,
    Payload::Owned(o) => o,
    Payload::BorrowedMut(b) => b,
  };
  let mut head = [0u8; 10];
  let head_len =
    fmt_server_head(&mut head, frame.fin, frame.opcode, payload.len());
  if matches!(frame.opcode, OpCode::Close) {
    ws.closed = true;
  }
  write_via_try(tcp, &head[..head_len], payload)
    .await
    .map_err(WebSocketError::IoError)
}

/// Emit a one-shot control-frame response (Pong for Ping, Close echo
/// for Close) using the in-place outbound-segment trick.
///
/// When the input frame was masked (always true for client-to-server
/// per RFC 6455) and its payload is small enough, the response header
/// fits in the 4-byte mask slot that's now dead state — we overwrite
/// the mask bytes with the response header and emit one contiguous
/// slice covering `[response_header || payload]`. No allocation, one
/// `send()` syscall. This is the optimization that gets
/// `process_into` to single-syscall steady state in fastwebsockets
/// PR #133, lifted into Deno.
///
/// `frame_start` is the offset of the inbound frame within the read
/// scratch buffer, and `hdr` is its parsed header.
async fn emit_in_place_response(
  scratch: &mut [u8],
  frame_start: usize,
  hdr: &Header,
  resp_opcode: OpCode,
  ws: &mut WriteSerializer,
  tcp: &TcpStream,
) -> Result<(), WebSocketError> {
  if ws.closed {
    return Ok(());
  }
  let payload_len = hdr.payload_len;
  let payload_start = frame_start + hdr.header_len;
  let payload_end = payload_start + payload_len;
  let masked = hdr.mask.is_some();
  if masked && payload_len < 65536 {
    let resp_hdr_len = if payload_len < 126 { 2 } else { 4 };
    let resp_start = payload_start - resp_hdr_len;
    scratch[resp_start] = 0x80 | (resp_opcode as u8);
    if payload_len < 126 {
      scratch[resp_start + 1] = payload_len as u8;
    } else {
      scratch[resp_start + 1] = 126;
      scratch[resp_start + 2..resp_start + 4]
        .copy_from_slice(&(payload_len as u16).to_be_bytes());
    }
    let total = resp_hdr_len + payload_len;
    if matches!(resp_opcode, OpCode::Close) {
      ws.closed = true;
    }
    write_via_try(tcp, &scratch[resp_start..resp_start + total], &[])
      .await
      .map_err(WebSocketError::IoError)
  } else {
    let mut head = [0u8; 10];
    let head_len = fmt_server_head(&mut head, true, resp_opcode, payload_len);
    if matches!(resp_opcode, OpCode::Close) {
      ws.closed = true;
    }
    write_via_try(tcp, &head[..head_len], &scratch[payload_start..payload_end])
      .await
      .map_err(WebSocketError::IoError)
  }
}

/// Drive the read loop forward until either a JS-surface-able message
/// is in `state.pending` or we have to return an error. The function
/// borrows `state` mutably for the duration of the call (the caller
/// holds the per-resource `read_state` AsyncRefCell lock), and reads
/// directly from the shared `Rc<TcpStream>` with `poll_read_ready` +
/// `try_read` — no per-frame future allocation.
///
/// `write_state` is the per-resource write-side `AsyncRefCell` shared
/// between this loop's auto-pong / auto-close emissions and
/// `op_ws_send_*`'s user-initiated sends, so the wire bytes never
/// interleave mid-frame.
pub(crate) async fn pump_until_message(
  state: &mut EngineReadState,
  tcp: &Rc<TcpStream>,
  write_state: &Rc<AsyncRefCell<WriteSerializer>>,
) -> Result<PendingMessage, WebSocketError> {
  loop {
    if let Some(msg) = state.pending.pop_front() {
      return Ok(msg);
    }

    // Parse as many frames as we can out of the buffered bytes.
    while state.unread_start < state.unread_end {
      let slice = &state.scratch[state.unread_start..state.unread_end];
      let hdr = match parse_header(slice)? {
        HeaderParse::Complete(h) => h,
        HeaderParse::Incomplete { .. } => break,
      };
      let total = hdr.total_len();
      if total > slice.len() {
        break;
      }

      let frame_start = state.unread_start;
      let payload_start = frame_start + hdr.header_len;
      let payload_end = payload_start + hdr.payload_len;

      if let Some(mask) = hdr.mask {
        unmask(&mut state.scratch[payload_start..payload_end], mask);
      }

      let opcode = hdr.opcode;
      let fin = hdr.fin;

      match opcode {
        OpCode::Ping => {
          // RFC 6455: control frames must be <=125 bytes and not
          // fragmented. fastwebsockets `parse_header` enforces this,
          // so by here we're safe to in-place-echo.
          let mut ws = write_state.borrow_mut().await;
          emit_in_place_response(
            &mut state.scratch,
            frame_start,
            &hdr,
            OpCode::Pong,
            &mut ws,
            tcp,
          )
          .await?;
          drop(ws);
          state.unread_start = frame_start + total;
        }
        OpCode::Close => {
          let close_msg = if hdr.payload_len < 2 {
            PendingMessage::Close {
              code: None,
              reason: None,
            }
          } else {
            let code = u16::from_be_bytes([
              state.scratch[payload_start],
              state.scratch[payload_start + 1],
            ]);
            let reason = if hdr.payload_len > 2 {
              Some(state.scratch[payload_start + 2..payload_end].to_vec())
            } else {
              None
            };
            PendingMessage::Close {
              code: Some(code),
              reason,
            }
          };
          let mut ws = write_state.borrow_mut().await;
          emit_in_place_response(
            &mut state.scratch,
            frame_start,
            &hdr,
            OpCode::Close,
            &mut ws,
            tcp,
          )
          .await?;
          drop(ws);
          state.unread_start = frame_start + total;
          return Ok(close_msg);
        }
        OpCode::Pong => {
          // No outbound. Just surface to JS for idle-timeout
          // heartbeats.
          state.unread_start = frame_start + total;
          return Ok(PendingMessage::Pong);
        }
        OpCode::Text | OpCode::Binary => {
          if state.fragment_opcode.is_some() {
            // RFC 6455: can't start a new data frame while a
            // fragmented message is mid-flight.
            return Err(WebSocketError::InvalidFragment);
          }
          if fin {
            let payload = state.scratch[payload_start..payload_end].to_vec();
            state.unread_start = frame_start + total;
            return Ok(match opcode {
              OpCode::Text => PendingMessage::Text(payload),
              OpCode::Binary => PendingMessage::Binary(payload),
              _ => unreachable!(),
            });
          }
          // Start of a fragmented message.
          state.fragment_opcode = Some(opcode);
          state.fragment_buf.clear();
          if hdr.payload_len > MAX_MESSAGE_SIZE {
            return Err(WebSocketError::FrameTooLarge);
          }
          state
            .fragment_buf
            .extend_from_slice(&state.scratch[payload_start..payload_end]);
          state.unread_start = frame_start + total;
        }
        OpCode::Continuation => {
          let Some(opcode) = state.fragment_opcode else {
            return Err(WebSocketError::InvalidContinuationFrame);
          };
          if state.fragment_buf.len() + hdr.payload_len > MAX_MESSAGE_SIZE {
            return Err(WebSocketError::FrameTooLarge);
          }
          state
            .fragment_buf
            .extend_from_slice(&state.scratch[payload_start..payload_end]);
          state.unread_start = frame_start + total;
          if fin {
            state.fragment_opcode = None;
            let payload = std::mem::take(&mut state.fragment_buf);
            return Ok(match opcode {
              OpCode::Text => PendingMessage::Text(payload),
              OpCode::Binary => PendingMessage::Binary(payload),
              _ => unreachable!(),
            });
          }
        }
      }
    }

    // Compact / reset / grow the scratch buffer before reading.
    if state.unread_start == state.unread_end {
      state.unread_start = 0;
      state.unread_end = 0;
    } else if state.unread_start > 0 {
      let len = state.unread_end - state.unread_start;
      state
        .scratch
        .copy_within(state.unread_start..state.unread_end, 0);
      state.unread_start = 0;
      state.unread_end = len;
    }

    if state.unread_end == state.scratch.len() {
      let next = state.scratch.len().saturating_mul(2);
      if next > SCRATCH_MAX {
        return Err(WebSocketError::FrameTooLarge);
      }
      state.scratch.resize(next, 0);
    }

    // Feed the hyper-upgrade prefix into the buffer before the first
    // real socket read.
    if let Some(prefix) = state.prefix.take() {
      let space = state.scratch.len() - state.unread_end;
      let copy = std::cmp::min(prefix.len(), space);
      state.scratch[state.unread_end..state.unread_end + copy]
        .copy_from_slice(&prefix[..copy]);
      state.unread_end += copy;
      if copy < prefix.len() {
        state.prefix = Some(prefix.slice(copy..));
      }
      continue;
    }

    tcp.readable().await.map_err(WebSocketError::IoError)?;
    match tcp.try_read(&mut state.scratch[state.unread_end..]) {
      Ok(0) => return Err(WebSocketError::ConnectionClosed),
      Ok(n) => state.unread_end += n,
      Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
      Err(e) => return Err(WebSocketError::IoError(e)),
    }
  }
}

/// Translate a `PendingMessage` into the `MessageKind` u16 that JS
/// expects from `op_ws_next_event`, stashing payloads on the resource
/// for later pickup by `op_ws_get_buffer` / `op_ws_get_buffer_as_string`.
///
/// `set_buffer` / `set_string` / `set_error` mirror `ServerWebSocket`'s
/// `buffer` / `string` / `error` Cell slots — taking closures here
/// avoids exposing `ServerWebSocket` to this module.
pub(crate) fn surface_message<S, B, E>(
  msg: PendingMessage,
  mut set_string: S,
  mut set_buffer: B,
  mut set_error: E,
) -> u16
where
  S: FnMut(String),
  B: FnMut(Vec<u8>),
  E: FnMut(Option<String>),
{
  match msg {
    PendingMessage::Text(bytes) => match String::from_utf8(bytes) {
      Ok(s) => {
        set_string(s);
        super::MessageKind::Text as u16
      }
      Err(_) => {
        set_error(Some("Invalid string data".into()));
        super::MessageKind::Error as u16
      }
    },
    PendingMessage::Binary(b) => {
      set_buffer(b);
      super::MessageKind::Binary as u16
    }
    PendingMessage::Pong => super::MessageKind::Pong as u16,
    PendingMessage::Close { code, reason } => match code {
      Some(c) => {
        let reason = reason.and_then(|r| String::from_utf8(r).ok());
        set_error(reason);
        CloseCode::from(c).into()
      }
      None => {
        set_error(None);
        super::MessageKind::ClosedDefault as u16
      }
    },
  }
}

/// Whether the fast path is enabled. Opt-in via `DENO_WS_FAST_TCP=1`.
///
/// This is opt-in for now while the engine read path (in-place unmask,
/// scratch-buf parsing, drop-time cleanup ordering) is being settled
/// against Deno's existing op_ws_next_event leak/cleanup expectations
/// and the Autobahn protocol-conformance suite. The generic
/// `WebSocketStream` path remains the default and stays byte-identical
/// to pre-PR behavior.
pub(crate) fn fast_tcp_enabled() -> bool {
  matches!(std::env::var("DENO_WS_FAST_TCP").as_deref(), Ok("1"))
}
