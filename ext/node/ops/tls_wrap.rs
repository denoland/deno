// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(
  clippy::undocumented_unsafe_blocks,
  reason = "TLSWrap is an FFI-heavy Node parity port; safety invariants are documented on the surrounding methods and types."
)]

// Ported from Node.js:
// - src/crypto/crypto_tls.h
// - src/crypto/crypto_tls.cc
//
// TLSWrap is a stream interceptor that sits between JS and an underlying
// transport stream (typically TCP). It encrypts outgoing data and decrypts
// incoming data using rustls.
//
// Data flow:
//
//   JS app  ↔  TLSWrap (cleartext)  ↔  rustls  ↔  TLSWrap (encrypted)  ↔  underlying stream
//
// The key operations:
//   - ClearIn:  Take pending cleartext from JS writes → feed to rustls writer
//   - ClearOut: Read decrypted data from rustls reader → emit to JS as onread
//   - EncOut:   Take encrypted output from rustls → write to underlying stream
//   - OnStreamRead: Encrypted data from underlying stream → feed to rustls
//   - Cycle:    Drive the state machine: ClearIn → ClearOut → EncOut

use std::cell::Cell;
use std::ffi::c_char;
use std::io::Read;
use std::io::Write;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::ToJsBuffer;
use deno_core::V8TaskSpawner;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::UV_EBADF;
use deno_core::uv_compat::UV_ECANCELED;
use deno_core::uv_compat::UV_EOF;
use deno_core::uv_compat::uv_buf_t;
use deno_core::uv_compat::uv_stream_t;
use deno_core::uv_compat::uv_write_t;
use deno_core::v8;
use deno_core::v8_static_strings;
use deno_node_crypto::x509::Certificate;
use deno_node_crypto::x509::CertificateObject;
use deno_tls::rustls;
use deno_tls::rustls_pemfile;

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::OwnedPtr;
use crate::ops::handle_wrap::ProviderType;
use crate::ops::stream_wrap::LibUvStreamWrap;
use crate::ops::stream_wrap::StreamBaseState;
use crate::ops::stream_wrap::call_fatal_exception;
use crate::ops::stream_wrap::free_uv_buf;
use crate::ops::stream_wrap_state::ReadInterceptor;
use crate::ops::tls::NodeTlsState;

// ---------------------------------------------------------------------------
// TLS connection wrapper — abstracts over client vs server
// ---------------------------------------------------------------------------

enum TlsConnection {
  Client(rustls::ClientConnection),
  Server(rustls::ServerConnection),
}

impl TlsConnection {
  fn read_tls(&mut self, rd: &mut dyn Read) -> Result<usize, std::io::Error> {
    match self {
      TlsConnection::Client(c) => c.read_tls(rd),
      TlsConnection::Server(c) => c.read_tls(rd),
    }
  }

  fn write_tls(&mut self, wr: &mut dyn Write) -> Result<usize, std::io::Error> {
    match self {
      TlsConnection::Client(c) => c.write_tls(wr),
      TlsConnection::Server(c) => c.write_tls(wr),
    }
  }

  fn process_new_packets(&mut self) -> Result<rustls::IoState, rustls::Error> {
    match self {
      TlsConnection::Client(c) => c.process_new_packets(),
      TlsConnection::Server(c) => c.process_new_packets(),
    }
  }

  fn reader(&mut self) -> rustls::Reader<'_> {
    match self {
      TlsConnection::Client(c) => c.reader(),
      TlsConnection::Server(c) => c.reader(),
    }
  }

  fn writer(&mut self) -> rustls::Writer<'_> {
    match self {
      TlsConnection::Client(c) => c.writer(),
      TlsConnection::Server(c) => c.writer(),
    }
  }

  fn send_close_notify(&mut self) {
    match self {
      TlsConnection::Client(c) => c.send_close_notify(),
      TlsConnection::Server(c) => c.send_close_notify(),
    }
  }

  fn wants_write(&self) -> bool {
    match self {
      TlsConnection::Client(c) => c.wants_write(),
      TlsConnection::Server(c) => c.wants_write(),
    }
  }

  fn is_handshaking(&self) -> bool {
    match self {
      TlsConnection::Client(c) => c.is_handshaking(),
      TlsConnection::Server(c) => c.is_handshaking(),
    }
  }

  fn alpn_protocol(&self) -> Option<&[u8]> {
    match self {
      TlsConnection::Client(c) => c.alpn_protocol(),
      TlsConnection::Server(c) => c.alpn_protocol(),
    }
  }

  fn server_name(&self) -> Option<&str> {
    match self {
      TlsConnection::Client(_) => None,
      TlsConnection::Server(c) => c.server_name(),
    }
  }

  fn protocol_version(&self) -> Option<rustls::ProtocolVersion> {
    match self {
      TlsConnection::Client(c) => c.protocol_version(),
      TlsConnection::Server(c) => c.protocol_version(),
    }
  }

  fn negotiated_cipher_suite(&self) -> Option<rustls::SupportedCipherSuite> {
    match self {
      TlsConnection::Client(c) => c.negotiated_cipher_suite(),
      TlsConnection::Server(c) => c.negotiated_cipher_suite(),
    }
  }

  fn peer_certificates(
    &self,
  ) -> Option<&[rustls::pki_types::CertificateDer<'static>]> {
    match self {
      TlsConnection::Client(c) => c.peer_certificates(),
      TlsConnection::Server(c) => c.peer_certificates(),
    }
  }

  fn handshake_kind(&self) -> Option<rustls::HandshakeKind> {
    match self {
      TlsConnection::Client(c) => c.handshake_kind(),
      TlsConnection::Server(c) => c.handshake_kind(),
    }
  }

  fn export_keying_material(
    &self,
    output: &mut [u8],
    label: &[u8],
    context: Option<&[u8]>,
  ) -> Result<(), rustls::Error> {
    match self {
      TlsConnection::Client(c) => c
        .export_keying_material(&mut *output, label, context)
        .map(|_| ()),
      TlsConnection::Server(c) => c
        .export_keying_material(&mut *output, label, context)
        .map(|_| ()),
    }
  }
}

#[derive(serde::Serialize)]
struct PeerCertificateChain {
  certificates: Vec<ToJsBuffer>,
}

// ---------------------------------------------------------------------------
// Kind — client or server
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum Kind {
  Client = 0,
  Server = 1,
}

// ---------------------------------------------------------------------------
// StreamBaseStateFields indices (must match stream_wrap.rs)
// ---------------------------------------------------------------------------

#[repr(usize)]
enum StreamBaseStateFields {
  ReadBytesOrError = 0,
  ArrayBufferOffset = 1,
  BytesWritten = 2,
  LastWriteWasAsync = 3,
}

// ---------------------------------------------------------------------------
// Constants matching Node's crypto_tls.h
// ---------------------------------------------------------------------------

const CLEAR_OUT_CHUNK_SIZE: usize = 16384;

// ---------------------------------------------------------------------------
// Callback context — data extracted from TLSWrapInner before invoking JS.
//
// JS callbacks can re-enter Rust ops that access TLSWrapInner, so we must
// not hold any Rust reference (&/&mut) to TLSWrapInner across a JS call.
// The EmitCtx holds cloned/copied data so the JS call is reference-free.
// ---------------------------------------------------------------------------

struct EmitCtx {
  isolate_ptr: v8::UnsafeRawIsolatePtr,
  js_handle: v8::Global<v8::Object>,
  loop_ptr: *mut uv_compat::uv_loop_t,
}

/// Extract callback context from the raw TLSWrapInner pointer.
/// Returns None if isolate or js_handle are not set.
///
/// # Safety
/// `ptr` must be a valid, non-null pointer to a live TLSWrapInner.
/// The returned EmitCtx owns cloned Globals and does not borrow TLSWrapInner.
unsafe fn extract_emit_ctx(ptr: *mut TLSWrapInner) -> Option<EmitCtx> {
  unsafe {
    let isolate_ptr = (*ptr).isolate?;
    let js_handle = (*ptr).js_handle.clone()?;
    // Use cached_loop_ptr for Uv streams to avoid dereferencing
    // a potentially dangling stream pointer.
    let loop_ptr = if (*ptr).cached_loop_ptr.is_null() {
      (*ptr).underlying.loop_ptr()
    } else {
      (*ptr).cached_loop_ptr
    };
    Some(EmitCtx {
      isolate_ptr,
      js_handle,
      loop_ptr,
    })
  }
}

/// Clone a v8::Context Global from a raw pointer stored in a uv loop's data
/// field. The original global is "leaked back" via into_raw so the loop
/// retains ownership.
///
/// # Safety
/// `ctx_ptr` must be a valid pointer to a v8::Context that was previously
/// stored via `Global::into_raw`.
unsafe fn clone_context_global(
  isolate: &mut v8::Isolate,
  ctx_ptr: *mut std::ffi::c_void,
) -> v8::Global<v8::Context> {
  unsafe {
    let raw = NonNull::new_unchecked(ctx_ptr as *mut v8::Context);
    let global = v8::Global::from_raw(isolate, raw);
    let cloned = global.clone();
    // Leak the original back so the loop retains its reference.
    global.into_raw();
    cloned
  }
}

/// Result of `clear_out_process`: describes what JS callbacks to fire.
struct ClearOutResult {
  handshake_done: bool,
  data: Vec<u8>,
  got_eof: bool,
  got_error: bool,
  /// TLS error to emit (message, code). Only set when process_new_packets fails.
  tls_error: Option<(String, String)>,
}

/// Result of `enc_out_collect` — describes what action to take after collecting.
enum EncOutAction {
  /// Nothing to do.
  None,
  /// Write encrypted data to the uv stream.
  WriteUv,
  /// Write encrypted data via JS callback.
  WriteJs,
  /// Call invoke_queued with the given status (no encrypted data to write).
  InvokeQueued(i32),
}

/// How the write-completion callback (`req.oncomplete`) must be dispatched.
///
/// This is a spelled-out enum rather than a `bool` on purpose: picking the
/// wrong variant re-introduces the #35820 reentrancy panic, and a bare
/// `false` at a call site reads as harmless when it is not. Every dispatch
/// site must state its intent.
#[derive(Clone, Copy)]
enum WriteCompletion {
  /// Run `oncomplete` synchronously. Only sound when the caller does NOT hold
  /// the `OpState` borrow — libuv callbacks (`enc_write_cb`) and the `&self`
  /// ops that drive `cycle`/`start`/`shutdown`/`finish_accept`.
  Sync,
  /// Schedule `oncomplete` on the event loop. Required whenever the caller
  /// holds the `OpState` borrow — i.e. `write_data` (the writev/writeBuffer/
  /// writeUtf8String ops) — so the callback can't re-enter an op while
  /// `OpState` is borrowed and panic with "RefCell already borrowed" (#35820).
  Deferred,
}

// ---------------------------------------------------------------------------
// Free functions that emit JS callbacks.
// These do NOT borrow TLSWrapInner — they work entirely with EmitCtx + args.
// ---------------------------------------------------------------------------

/// Copy `data` through the user buffer registered on the TLSWrap
/// (Node's `onread.buffer` option), chunking into `user_buffer.len`
/// slices so the JS callback only ever sees up to that many bytes per
/// invocation. The user buffer may be rotated by the JS callback (via
/// `bufGen`), so it is re-read between chunks.
///
/// # Safety
/// `inner_ptr` must be valid and live for the duration of the call.
/// No `&` / `&mut` reference to `TLSWrapInner` may be held by the
/// caller (JS callbacks can re-enter ops on the same object). All
/// pointers in `ctx` must be valid.
unsafe fn do_emit_read_through_user_buffer(
  ctx: &EmitCtx,
  inner_ptr: *mut TLSWrapInner,
  onread: Option<&v8::Global<v8::Function>>,
  state: Option<&v8::Global<v8::Int32Array>>,
  data: &[u8],
) {
  let mut offset = 0;
  while offset < data.len() {
    // Re-read the user buffer pointer/length each iteration so a
    // `bufGen`-driven rotation during the previous callback is honored.
    let user_buf =
      unsafe { (*inner_ptr).user_buffer.as_ref().map(|ub| (ub.ptr, ub.len)) };
    let Some((ptr, len)) = user_buf else {
      // User buffer was cleared mid-emit — fall back to the
      // ArrayBuffer-allocating path for the remaining bytes so they
      // still reach the consumer.
      let remaining = &data[offset..];
      unsafe {
        do_emit_read(
          ctx,
          onread,
          state,
          remaining.len() as isize,
          Some(remaining),
        );
      }
      return;
    };
    if len == 0 {
      // Zero-length buffer can't make progress; stop to avoid spinning.
      return;
    }
    let chunk_len = std::cmp::min(data.len() - offset, len);
    // SAFETY: `ptr` references at least `len >= chunk_len` writable
    // bytes (the registered user buffer's backing store, held alive by
    // its retained `Global<Uint8Array>`). `data[offset..offset+chunk_len]`
    // is in-bounds. Source and destination are distinct allocations.
    unsafe {
      std::ptr::copy_nonoverlapping(
        data[offset..offset + chunk_len].as_ptr(),
        ptr,
        chunk_len,
      );
    }
    offset += chunk_len;
    // SAFETY: caller upholds the EmitCtx invariants; we hand off to a
    // free function that does not borrow TLSWrapInner.
    unsafe {
      do_emit_read_undefined_arg(ctx, onread, state, chunk_len as isize);
    }
  }
}

/// Variant of `do_emit_read` that signals the callback by passing
/// `undefined` rather than wrapping bytes in a fresh ArrayBuffer.
/// Used in the static-buffer path: `onStreamRead` reads the bytes via
/// `stream[kBuffer]` (the same Uint8Array previously passed to
/// `useUserBuffer`) and ignores the callback argument.
///
/// # Safety
/// EmitCtx must contain valid pointers. No TLSWrapInner reference may be held by the caller.
unsafe fn do_emit_read_undefined_arg(
  ctx: &EmitCtx,
  onread: Option<&v8::Global<v8::Function>>,
  state: Option<&v8::Global<v8::Int32Array>>,
  nread: isize,
) {
  let Some(state_global) = state else {
    return;
  };
  unsafe {
    let mut isolate = v8::Isolate::from_raw_isolate_ptr(ctx.isolate_ptr);
    if ctx.loop_ptr.is_null() {
      return;
    }
    let ctx_ptr = (*ctx.loop_ptr).data;
    if ctx_ptr.is_null() {
      return;
    }
    let context_global = clone_context_global(&mut isolate, ctx_ptr);

    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Local::new(handle_scope, context_global);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let state_array: v8::Local<v8::Int32Array> =
      v8::Local::new(scope, state_global);
    state_array.set_index(
      scope,
      StreamBaseStateFields::ReadBytesOrError as u32,
      v8::Integer::new(scope, nread as i32).into(),
    );
    state_array.set_index(
      scope,
      StreamBaseStateFields::ArrayBufferOffset as u32,
      v8::Integer::new(scope, 0).into(),
    );

    let recv = v8::Local::new(scope, &ctx.js_handle);
    let Some(onread) = onread else {
      return;
    };
    let onread_fn = v8::Local::new(scope, onread);
    let undef = v8::undefined(scope);
    onread_fn.call(scope, recv.into(), &[undef.into()]);
  }
}

/// Emit read data to JS via onread callback.
///
/// # Safety
/// EmitCtx must contain valid pointers. No TLSWrapInner reference may be held by the caller.
unsafe fn do_emit_read(
  ctx: &EmitCtx,
  onread: Option<&v8::Global<v8::Function>>,
  state: Option<&v8::Global<v8::Int32Array>>,
  nread: isize,
  data: Option<&[u8]>,
) {
  let Some(state_global) = state else {
    return;
  };
  unsafe {
    let mut isolate = v8::Isolate::from_raw_isolate_ptr(ctx.isolate_ptr);

    if ctx.loop_ptr.is_null() {
      return;
    }
    let ctx_ptr = (*ctx.loop_ptr).data;
    if ctx_ptr.is_null() {
      return;
    }
    // Clone context before creating the handle scope (which borrows isolate).
    let context_global = clone_context_global(&mut isolate, ctx_ptr);

    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Local::new(handle_scope, context_global);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let state_array: v8::Local<v8::Int32Array> =
      v8::Local::new(scope, state_global);
    state_array.set_index(
      scope,
      StreamBaseStateFields::ReadBytesOrError as u32,
      v8::Integer::new(scope, nread as i32).into(),
    );
    state_array.set_index(
      scope,
      StreamBaseStateFields::ArrayBufferOffset as u32,
      v8::Integer::new(scope, 0).into(),
    );

    let recv = v8::Local::new(scope, &ctx.js_handle);

    let Some(onread) = onread else {
      // readStop was called (onread is None) -- caller must buffer
      // the data instead of delivering it.  The old fallback of
      // looking up "onread" on the JS handle defeated readStop and
      // broke backpressure.
      return;
    };
    let onread_fn = v8::Local::new(scope, onread);

    if let Some(bytes) = data {
      // Single memcpy into a fresh backing store; ArrayBuffer::new would
      // zero-initialize first and the old byte-wise Cell writes made this
      // hot path two passes over every decrypted chunk.
      let store = v8::ArrayBuffer::new_backing_store_from_vec(bytes.to_vec())
        .make_shared();
      let ab: v8::Local<v8::Value> =
        v8::ArrayBuffer::with_backing_store(scope, &store).into();
      onread_fn.call(scope, recv.into(), &[ab]);
    } else {
      let undef = v8::undefined(scope);
      onread_fn.call(scope, recv.into(), &[undef.into()]);
    }
  }
}

/// Emit a TLS error to JS via the onerror callback.
///
/// # Safety
/// EmitCtx must contain valid pointers. No TLSWrapInner reference may be held by the caller.
unsafe fn do_emit_error(ctx: &EmitCtx, error_msg: &str, error_code: &str) {
  unsafe {
    let mut isolate = v8::Isolate::from_raw_isolate_ptr(ctx.isolate_ptr);

    if ctx.loop_ptr.is_null() {
      return;
    }
    let ctx_ptr = (*ctx.loop_ptr).data;
    if ctx_ptr.is_null() {
      return;
    }
    // Clone context before creating the handle scope (which borrows isolate).
    let context_global = clone_context_global(&mut isolate, ctx_ptr);

    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Local::new(handle_scope, context_global);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let this = v8::Local::new(scope, &ctx.js_handle);

    let msg = v8::String::new(scope, error_msg).unwrap();
    let error = v8::Exception::error(scope, msg);
    let error_obj = error.to_object(scope).unwrap();

    let code_key =
      v8::String::new_external_onebyte_static(scope, b"code").unwrap();
    let code_val = v8::String::new(scope, error_code).unwrap();
    error_obj.set(scope, code_key.into(), code_val.into());

    let key =
      v8::String::new_external_onebyte_static(scope, b"onerror").unwrap();
    if let Some(val) = this.get(scope, key.into())
      && let Ok(func) = v8::Local::<v8::Function>::try_from(val)
    {
      func.call(scope, this.into(), &[error]);
    }
  }
}

/// Emit handshake done callback.
///
/// # Safety
/// EmitCtx must contain valid pointers. No TLSWrapInner reference may be held by the caller.
unsafe fn do_emit_handshake_done(ctx: &EmitCtx) {
  unsafe {
    let mut isolate = v8::Isolate::from_raw_isolate_ptr(ctx.isolate_ptr);

    if ctx.loop_ptr.is_null() {
      return;
    }
    let ctx_ptr = (*ctx.loop_ptr).data;
    if ctx_ptr.is_null() {
      return;
    }
    // Clone context before creating the handle scope (which borrows isolate).
    let context_global = clone_context_global(&mut isolate, ctx_ptr);

    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Local::new(handle_scope, context_global);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let this = v8::Local::new(scope, &ctx.js_handle);
    let key =
      v8::String::new_external_onebyte_static(scope, b"onhandshakedone")
        .unwrap();
    if let Some(val) = this.get(scope, key.into())
      && let Ok(func) = v8::Local::<v8::Function>::try_from(val)
    {
      func.call(scope, this.into(), &[]);
    }
  }
}

/// Emit client hello callback (onclienthello) to JS.
/// Called when the Acceptor has extracted the ClientHello so JS can
/// invoke SNICallback and ALPNCallback before the handshake continues.
///
/// # Safety
/// EmitCtx must contain valid pointers. No TLSWrapInner reference may be held by the caller.
unsafe fn do_emit_client_hello(ctx: &EmitCtx) {
  unsafe {
    let mut isolate = v8::Isolate::from_raw_isolate_ptr(ctx.isolate_ptr);

    if ctx.loop_ptr.is_null() {
      return;
    }
    let ctx_ptr = (*ctx.loop_ptr).data;
    if ctx_ptr.is_null() {
      return;
    }
    let context_global = clone_context_global(&mut isolate, ctx_ptr);

    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Local::new(handle_scope, context_global);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let this = v8::Local::new(scope, &ctx.js_handle);
    let key =
      v8::String::new_external_onebyte_static(scope, b"onclienthello").unwrap();
    if let Some(val) = this.get(scope, key.into())
      && let Ok(func) = v8::Local::<v8::Function>::try_from(val)
    {
      func.call(scope, this.into(), &[]);
    }
  }
}

/// Look up `req.oncomplete` and invoke it with `(status, handle, undefined)`,
/// reporting any exception the callback throws as uncaught (matching Node's
/// MakeCallback). Shared by the synchronous (`do_invoke_queued`) and deferred
/// (`defer_invoke_queued`) completion paths so their behavior can't drift.
fn invoke_write_oncomplete(
  scope: &mut v8::PinScope,
  req_obj: v8::Local<v8::Object>,
  handle: v8::Local<v8::Object>,
  status: i32,
) {
  let oncomplete_str =
    v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
  let status_val = v8::Integer::new(scope, status);
  let undef = v8::undefined(scope);
  // The TryCatch covers the property lookup as well as the call: a throwing
  // getter makes `get` return None with the exception left pending, and on the
  // deferred path this scope belongs to the V8TaskSpawner, whose contract
  // forbids returning with an exception set.
  let caught_exception = {
    v8::tc_scope!(tc, scope);
    let Some(oncomplete) = req_obj.get(tc, oncomplete_str.into()) else {
      tc.reset();
      return;
    };
    let Ok(func) = v8::Local::<v8::Function>::try_from(oncomplete) else {
      return;
    };
    let result = func.call(
      tc,
      req_obj.into(),
      &[status_val.into(), handle.into(), undef.into()],
    );
    if result.is_none() && tc.has_caught() {
      let exc = tc.exception();
      tc.reset();
      exc
    } else {
      None
    }
  };
  if let Some(exception) = caught_exception {
    call_fatal_exception(scope, exception);
  }
}

/// Signal write completion to JS.
///
/// # Safety
/// EmitCtx must contain valid pointers. No TLSWrapInner reference may be held by the caller.
unsafe fn do_invoke_queued(
  ctx: &EmitCtx,
  write_obj: v8::Global<v8::Object>,
  status: i32,
) {
  unsafe {
    let mut isolate = v8::Isolate::from_raw_isolate_ptr(ctx.isolate_ptr);

    if ctx.loop_ptr.is_null() {
      return;
    }
    let ctx_ptr = (*ctx.loop_ptr).data;
    if ctx_ptr.is_null() {
      return;
    }
    // Clone context before creating the handle scope (which borrows isolate).
    let context_global = clone_context_global(&mut isolate, ctx_ptr);

    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Local::new(handle_scope, context_global);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let req_obj = v8::Local::new(scope, &write_obj);
    let handle = v8::Local::new(scope, &ctx.js_handle);
    invoke_write_oncomplete(scope, req_obj, handle, status);
  }
}

/// Signal write completion to JS on the next event loop iteration.
///
/// The synchronous `do_invoke_queued` is only safe from a libuv callback
/// dispatched by the event loop. The TLS write ops (`writev`, `writeBuffer`,
/// `writeUtf8String`, ...) hold the `OpState` borrow for their entire body,
/// so running `oncomplete` synchronously from them re-enters JS while
/// `OpState` is borrowed, and any op the callback reaches (e.g.
/// `op_node_new_async_id` via `process.nextTick`) panics with "RefCell
/// already borrowed" (#35820). Deferring also matches libuv/Node semantics:
/// write callbacks never fire synchronously from the write call itself.
///
/// Like `do_invoke_queued`, this recovers the TLSWrap's stored context via
/// `clone_context_global(loop_ptr->data)` so `oncomplete` and the `reportError`
/// lookup resolve against the realm that owns the socket rather than the
/// spawner's ambient (main) context. The clone happens here, at schedule time,
/// while the isolate is current and no spawner `HandleScope` is live yet;
/// cloning it inside the spawned closure instead would mean reconstructing the
/// isolate under the already-live event-loop `HandleScope`, which is not sound.
///
/// If the context can't be recovered (loop or stored context pointer is null)
/// the completion is dropped without spawning, exactly as the synchronous
/// `do_invoke_queued` returns early in that case. Falling back to the spawner's
/// ambient (main) context instead would run `oncomplete`/`reportError` against
/// the wrong realm — a silent divergence from the synchronous path. Dropping is
/// not benign, though: `prepare_invoke_queued` has already taken
/// `current_write_obj` and cleared `write_callback_scheduled`, so the write's
/// `oncomplete` never fires and the writable side stalls with no error. This
/// path is only reachable from a uv-backed write, where `cached_loop_ptr` is
/// always populated, so a null here is a broken invariant — log it loudly and
/// assert in debug builds rather than hanging silently.
///
/// Ordering: at most one write completion is outstanding per wrap
/// (`current_write_obj` is a single slot), but JS may issue a further write
/// before the queued task runs. If that write succeeds and its `enc_write_cb`
/// fires from the loop before the task, `oncomplete` callbacks are delivered
/// out of FIFO order. This is accepted: the deferred path is only reached after
/// a synchronous `uv_write` failure, which means the handle is already dead
/// (EBADF) and a subsequent successful write is not possible on it.
fn defer_invoke_queued(
  spawner: &V8TaskSpawner,
  ctx: EmitCtx,
  write_obj: v8::Global<v8::Object>,
  status: i32,
) {
  let EmitCtx {
    isolate_ptr,
    js_handle,
    loop_ptr,
  } = ctx;
  // Recover the stored context now, before spawning, so the closure only has
  // to enter it. SAFETY: at schedule time the isolate is current (we are inside
  // a write op) and `loop_ptr`/its `data` were populated at construction.
  let context_global = unsafe {
    debug_assert!(
      !loop_ptr.is_null(),
      "deferred write completion without a uv loop"
    );
    if loop_ptr.is_null() {
      log::error!(
        "TLSWrap: dropping deferred write completion, no uv loop; the write callback will not fire"
      );
      return;
    }
    let ctx_ptr = (*loop_ptr).data;
    debug_assert!(
      !ctx_ptr.is_null(),
      "deferred write completion without a stored v8 context"
    );
    if ctx_ptr.is_null() {
      log::error!(
        "TLSWrap: dropping deferred write completion, no stored v8 context; the write callback will not fire"
      );
      return;
    }
    let mut isolate = v8::Isolate::from_raw_isolate_ptr(isolate_ptr);
    clone_context_global(&mut isolate, ctx_ptr)
  };
  spawner.spawn(move |scope| {
    let context = v8::Local::new(scope, &context_global);
    let scope = &mut v8::ContextScope::new(scope, context);
    let req_obj = v8::Local::new(scope, &write_obj);
    let handle = v8::Local::new(scope, &js_handle);
    invoke_write_oncomplete(scope, req_obj, handle, status);
  });
}

/// Fire the queued write-completion callback, keeping the defer-vs-sync policy
/// in one place. `WriteCompletion::Deferred` (the call originates from a write
/// op that still holds the `OpState` borrow) schedules the callback on the
/// event loop via `defer_invoke_queued`; `WriteCompletion::Sync` (a libuv
/// callback or a non-borrowing `&self` op) runs it synchronously via
/// `do_invoke_queued`. See `defer_invoke_queued` for why write ops must defer.
///
/// # Safety
/// `ptr` must be a valid, non-null pointer to a live TLSWrapInner.
unsafe fn dispatch_invoke_queued(
  ptr: *mut TLSWrapInner,
  completion: WriteCompletion,
  status: i32,
) {
  unsafe {
    let Some((write_obj, ctx)) = prepare_invoke_queued(ptr) else {
      return;
    };
    match completion {
      WriteCompletion::Deferred => {
        // The spawner is populated at construction (see `TLSWrap::new`), so a
        // missing one here is a broken invariant; fail loudly rather than
        // degrading to the synchronous reentrancy panic (#35820).
        let spawner = (*ptr).task_spawner.clone().expect(
          "V8TaskSpawner must be present for deferred write completion",
        );
        defer_invoke_queued(&spawner, ctx, write_obj, status);
      }
      WriteCompletion::Sync => {
        do_invoke_queued(&ctx, write_obj, status);
      }
    }
  }
}

/// Write encrypted data to a JS-backed stream via the JS `encOut` callback.
///
/// # Safety
/// EmitCtx must contain valid pointers. No TLSWrapInner reference may be held by the caller.
#[allow(dead_code, reason = "retained for native TCP/Pipe enc-out path parity")]
unsafe fn do_enc_out_js(ctx: &EmitCtx, enc_data: Vec<u8>) {
  unsafe {
    let mut isolate = v8::Isolate::from_raw_isolate_ptr(ctx.isolate_ptr);

    if ctx.loop_ptr.is_null() {
      return;
    }
    let ctx_ptr = (*ctx.loop_ptr).data;
    if ctx_ptr.is_null() {
      return;
    }
    // Clone context before creating the handle scope (which borrows isolate).
    let context_global = clone_context_global(&mut isolate, ctx_ptr);

    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Local::new(handle_scope, context_global);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let this = v8::Local::new(scope, &ctx.js_handle);
    let key =
      v8::String::new_external_onebyte_static(scope, b"encOut").unwrap();
    if let Some(val) = this.get(scope, key.into())
      && let Ok(func) = v8::Local::<v8::Function>::try_from(val)
    {
      // Zero-copy: move the encrypted bytes into the ArrayBuffer's
      // backing store instead of copying byte-by-byte.
      let store =
        v8::ArrayBuffer::new_backing_store_from_vec(enc_data).make_shared();
      let ab = v8::ArrayBuffer::with_backing_store(scope, &store);
      func.call(scope, this.into(), &[ab.into()]);
    }
  }
}

/// Prepare invoke_queued by extracting state from TLSWrapInner.
/// Mutates inner (clears write_callback_scheduled, takes current_write_obj).
/// Returns (write_obj, ctx) if a JS call should be made, None otherwise.
///
/// # Safety
/// `ptr` must be valid and non-null.
unsafe fn prepare_invoke_queued(
  ptr: *mut TLSWrapInner,
) -> Option<(v8::Global<v8::Object>, EmitCtx)> {
  unsafe {
    (*ptr).write_callback_scheduled = false;
    let write_obj = (*ptr).current_write_obj.take()?;
    (*ptr).current_write_bytes = 0;
    let ctx = extract_emit_ctx(ptr)?;
    Some((write_obj, ctx))
  }
}

// ---------------------------------------------------------------------------
// UnderlyingStream — abstracts over libuv streams and JS-backed streams
// ---------------------------------------------------------------------------

/// The underlying transport that TLSWrap encrypts/decrypts over.
/// Mirrors Node's StreamBase polymorphism via enum dispatch.
#[derive(Default)]
enum UnderlyingStream {
  /// Not yet attached.
  #[default]
  None,
  /// Real libuv stream (e.g. TCP). Read lifecycle is owned by the underlying
  /// LibUvStreamWrap; TLS only intercepts the resulting native read callbacks.
  Uv { stream: *mut uv_stream_t },
  /// JS-backed stream (e.g. Duplex wrapped via JSStreamSocket).
  /// Reads are injected from JS via receive(). Writes call back into JS.
  Js {
    /// The uv_loop pointer, needed for recovering v8 context in callbacks.
    loop_ptr: *mut uv_compat::uv_loop_t,
  },
}

impl UnderlyingStream {
  fn is_attached(&self) -> bool {
    !matches!(self, UnderlyingStream::None)
  }

  #[allow(
    dead_code,
    reason = "Useful when debugging TLS/native stream attachment."
  )]
  fn uv_stream_ptr(&self) -> *mut uv_stream_t {
    match self {
      UnderlyingStream::Uv { stream } => *stream,
      _ => std::ptr::null_mut(),
    }
  }

  fn loop_ptr(&self) -> *mut uv_compat::uv_loop_t {
    match self {
      // For Uv streams, the loop pointer is cached in TLSWrapInner
      // to avoid dereferencing the stream pointer (which may be dangling).
      // Callers must use TLSWrapInner.cached_loop_ptr instead.
      UnderlyingStream::Uv { .. } => {
        debug_assert!(false, "use TLSWrapInner.cached_loop_ptr for Uv streams");
        std::ptr::null_mut()
      }
      UnderlyingStream::Js { loop_ptr, .. } => *loop_ptr,
      UnderlyingStream::None => std::ptr::null_mut(),
    }
  }

  fn read_start(&mut self) {
    match self {
      UnderlyingStream::Uv { .. } => {
        // Native reads are owned by the attached LibUvStreamWrap.
      }
      UnderlyingStream::Js { .. } => {
        // JS stream: reads are pushed via receive(), no action needed
      }
      UnderlyingStream::None => {}
    }
  }

  #[allow(
    dead_code,
    reason = "reserved for a future native read-stop path; today reads are stopped at the JS layer"
  )]
  fn read_stop(&self) {
    match self {
      UnderlyingStream::Uv { .. } => {
        // Native reads are owned by the attached LibUvStreamWrap.
      }
      UnderlyingStream::Js { .. } => {
        // JS stream: no-op, JS side controls read flow
      }
      UnderlyingStream::None => {}
    }
  }

  fn write(&self, write_req: Box<EncryptedWriteReq>) -> (*mut uv_write_t, i32) {
    match self {
      UnderlyingStream::Uv { stream } => {
        if stream.is_null() {
          return (std::ptr::null_mut(), UV_EBADF);
        }
        let mut write_req = write_req;
        let data_len = write_req._data.len();
        let buf = uv_buf_t {
          base: write_req._data.as_mut_ptr() as *mut c_char,
          len: data_len,
        };
        let req_ptr = &mut write_req.uv_req as *mut uv_write_t;
        let _ = Box::into_raw(write_req); // freed in enc_write_cb
        // SAFETY: req_ptr and stream are valid; req is reclaimed in enc_write_cb or on error
        let ret = unsafe {
          uv_compat::uv_write(req_ptr, *stream, &buf, 1, Some(enc_write_cb))
        };
        (req_ptr, ret)
      }
      UnderlyingStream::Js { .. } => {
        // For JS streams, enc_out should not be called — encrypted data
        // goes through the JS-side write callback. This path should not
        // be reached in normal operation. If it is, treat as EBADF.
        (std::ptr::null_mut(), UV_EBADF)
      }
      UnderlyingStream::None => (std::ptr::null_mut(), UV_EBADF),
    }
  }

  fn shutdown(&self) {
    match self {
      UnderlyingStream::Uv { stream } => {
        if !stream.is_null() {
          let req = Box::new(uv_compat::new_shutdown());
          let req_ptr = Box::into_raw(req);
          // SAFETY: stream is non-null (checked above); req_ptr reclaimed
          // in the callback on success or immediately on error.
          unsafe {
            let ret =
              uv_compat::uv_shutdown(req_ptr, *stream, Some(shutdown_cb));
            if ret != 0 {
              let _ = Box::from_raw(req_ptr);
            }
          }
        }
      }
      UnderlyingStream::Js { .. } => {
        // JS stream shutdown is handled at the JS level
      }
      UnderlyingStream::None => {}
    }
  }

  #[allow(
    dead_code,
    reason = "reserved for a future native read-interception path; today TLS receives ciphertext via a JS-layer onread forwarder"
  )]
  fn set_read_interceptor(&self, interceptor: Option<ReadInterceptor>) {
    if let UnderlyingStream::Uv { stream } = self {
      LibUvStreamWrap::set_read_interceptor_for_stream(*stream, interceptor);
    }
  }
}

// ---------------------------------------------------------------------------
// Write request tracking — we need to keep the encrypted data alive
// until the underlying stream's write completes.
// ---------------------------------------------------------------------------

#[repr(C)]
struct EncryptedWriteReq {
  uv_req: uv_write_t,
  _data: Vec<u8>,
  /// If non-null, invoke_queued will be called on this TLSWrapInner
  /// when the encrypted write completes.
  tls_wrap_inner: *mut TLSWrapInner,
  /// Shared flag that is set to `false` when the owning TLSWrapInner is
  /// destroyed.  Checked in `enc_write_cb` before dereferencing
  /// `tls_wrap_inner` to avoid use-after-free when GC collects the
  /// TLSWrap while writes are still in-flight.
  alive: Rc<Cell<bool>>,
}

// ---------------------------------------------------------------------------
// TLSWrapInner — mutable state that can be accessed from C callbacks.
// Stored in a Box, pointer held by the CppGC TLSWrap object.
// ---------------------------------------------------------------------------

struct TLSWrapInner {
  tls_conn: Option<TlsConnection>,
  kind: Kind,

  // Buffer for encrypted data read from the underlying stream,
  // waiting to be fed to rustls via read_tls.
  enc_in: Vec<u8>,

  // State flags matching Node's TLSWrap
  started: bool,
  established: bool,
  shutdown: bool,
  eof: bool,
  cycling: bool,
  /// Set by clear_out when it emitted data — indicates rustls may have
  /// more buffered plaintext. Cleared when clear_out returns no data.
  has_buffered_cleartext: bool,
  /// Decrypted cleartext that could not be delivered because readStop was
  /// active (onread was None). Flushed on the next readStart cycle.
  pending_clear_out: Vec<u8>,
  /// True when the TLS peer closed but the EOF could not be delivered
  /// because readStop was active. Delivered on the next readStart cycle.
  pending_eof: bool,
  in_dowrite: bool,
  write_callback_scheduled: bool,
  /// Number of outstanding uv_write requests for encrypted output.
  /// invoke_queued must wait until this drops to zero.
  enc_writes_in_flight: u32,

  // Pending cleartext from DoWrite that SSL_write couldn't accept yet.
  // `pending_cleartext_offset` tracks how much of the buffer has already
  // been fed to rustls, so clear_in's chunked feeding doesn't re-allocate
  // and copy the remaining tail on every chunk (O(n²) for large writes).
  pending_cleartext: Vec<u8>,
  pending_cleartext_offset: usize,

  // Buffered encrypted output that failed to write (e.g. EBADF because the
  // underlying stream wasn't connected yet).  Retried on the next enc_out().
  pending_enc_out: Vec<u8>,

  // The underlying stream we're wrapping
  underlying: UnderlyingStream,

  // JS references needed for callbacks
  js_handle: Option<v8::Global<v8::Object>>,
  isolate: Option<v8::UnsafeRawIsolatePtr>,

  // Stream base state for communicating with JS
  stream_base_state: Option<v8::Global<v8::Int32Array>>,
  onread: Option<v8::Global<v8::Function>>,

  // Tracking for write completion
  current_write_obj: Option<v8::Global<v8::Object>>,
  current_write_bytes: usize,

  // Bytes counters
  bytes_read: u64,
  bytes_written: u64,

  /// Shared flag checked by `enc_write_cb` to detect teardown.
  /// Set to `false` in `teardown` before the TLSWrapInner memory
  /// is freed, so in-flight write callbacks can avoid a dangling deref.
  alive: Rc<Cell<bool>>,

  /// Set to true by JS when the TLSSocket is being destroyed/closed.
  /// Checked after handshake callback to prevent sending application
  /// data when checkServerIdentity fails.
  closing: bool,

  // Error string (like Node's error_)
  error: Option<String>,

  // Certificate verification error stored by NodeServerCertVerifier.
  // Read by verifyError() to report to JS.
  verify_error: VerifyErrorStore,

  // (cb_data is stored inside UnderlyingStream::Uv)

  // Deferred TLS config — stored here until start() creates the connection.
  // This allows setALPNProtocols to modify the config before the connection
  // is established.
  pending_client_config: Option<Arc<rustls::ClientConfig>>,
  pending_server_name: Option<rustls::pki_types::ServerName<'static>>,
  pending_server_config: Option<Arc<rustls::ServerConfig>>,

  /// Cached uv_loop pointer, set during attach(). Avoids dereferencing
  /// the stream pointer (which may become dangling) to get the loop.
  cached_loop_ptr: *mut uv_compat::uv_loop_t,

  // Acceptor-based server handshake (when SNICallback or ALPNCallback is set).
  // Instead of creating ServerConnection immediately in start(), we use
  // an Acceptor to intercept the ClientHello and call back to JS.
  use_acceptor: bool,
  acceptor: Option<rustls::server::Acceptor>,
  accepted: Option<rustls::server::Accepted>,
  client_hello_servername: Option<String>,
  client_hello_alpn: Vec<Vec<u8>>,

  /// Whether this connection was configured with an explicit session to resume.
  /// Node.js client connections only attempt session resumption when
  /// `options.session` is passed (or `setSession()` is called).
  allow_resumption: Arc<AtomicBool>,

  /// User-supplied static read buffer (Node's `onread.buffer`).
  /// When set, decrypted data is copied into this buffer rather than
  /// emitted via a fresh ArrayBuffer; the JS callback receives the same
  /// Uint8Array each time. Updated by `TLSWrap::useUserBuffer`.
  user_buffer: Option<crate::ops::stream_wrap::UserBuffer>,

  /// Same-thread task spawner used to defer the JS write-completion
  /// callback out of ops that hold the `OpState` borrow (see
  /// `defer_invoke_queued`). Captured from `OpState` in `TLSWrap::new`.
  task_spawner: Option<V8TaskSpawner>,
}

/// Convert a rustls error to a (message, code) pair that matches Node's
/// OpenSSL-style error reporting as closely as possible.
fn rustls_error_to_node_error(
  e: &rustls::Error,
  protocol_version: Option<rustls::ProtocolVersion>,
) -> (String, String) {
  use rustls::Error as E;
  match e {
    // Match Node's OpenSSL wording for record-layer decode failures so user
    // code that pattern-matches err.message keeps working.
    E::InvalidMessage(_) => (
      format!("{e} (SSL routines:ssl3_get_record:wrong version number)"),
      "ERR_SSL_WRONG_VERSION_NUMBER".to_string(),
    ),
    E::InvalidCertificate(cert_err) => {
      // Prefer the precise code recorded by NodeServerCertVerifier
      // (e.g. UNABLE_TO_GET_ISSUER_CERT_LOCALLY from chain-structure analysis)
      // over the generic mapping derived from the rustls CertificateError.
      // Map the message text to the same wording that JS `makeVerifyError`
      // produces, so callers that pattern-match on err.message keep working
      // when the verifier aborts the handshake (strict mode) instead of
      // deferring the error to JS.
      let recorded = CURRENT_VERIFY_ERROR
        .with(|c| c.borrow().clone())
        .and_then(|store| {
          store.lock().unwrap_or_else(|e| e.into_inner()).clone()
        });
      let code = recorded
        .unwrap_or_else(|| cert_error_to_node_code(cert_err).to_string());
      let msg = node_verify_error_message(&code)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{e}"));
      (msg, code)
    }
    E::NoCertificatesPresented => (
      format!("{e}"),
      "ERR_SSL_PEER_DID_NOT_RETURN_A_CERTIFICATE".to_string(),
    ),
    E::AlertReceived(alert) => {
      use rustls::AlertDescription as AD;
      let code = match *alert {
        AD::HandshakeFailure => "SSLV3_ALERT_HANDSHAKE_FAILURE",
        AD::BadCertificate => "SSLV3_ALERT_BAD_CERTIFICATE",
        AD::UnsupportedCertificate => "SSLV3_ALERT_UNSUPPORTED_CERTIFICATE",
        AD::CertificateRevoked => "SSLV3_ALERT_CERTIFICATE_REVOKED",
        AD::CertificateExpired => "SSLV3_ALERT_CERTIFICATE_EXPIRED",
        AD::CertificateUnknown => "SSLV3_ALERT_CERTIFICATE_UNKNOWN",
        AD::IllegalParameter => "SSLV3_ALERT_ILLEGAL_PARAMETER",
        AD::UnknownCA => "TLSV1_ALERT_UNKNOWN_CA",
        AD::DecodeError => "SSLV3_ALERT_DECODE_ERROR",
        AD::DecryptError => "SSLV3_ALERT_DECRYPT_ERROR",
        AD::ProtocolVersion => "TLSV1_ALERT_PROTOCOL_VERSION",
        AD::InsufficientSecurity => "TLSV1_ALERT_INSUFFICIENT_SECURITY",
        AD::InternalError => "TLSV1_ALERT_INTERNAL_ERROR",
        AD::InappropriateFallback => "TLSV1_ALERT_INAPPROPRIATE_FALLBACK",
        AD::UserCanceled => "TLSV1_ALERT_USER_CANCELLED",
        AD::NoRenegotiation => "TLSV1_ALERT_NO_RENEGOTIATION",
        AD::CertificateRequired => {
          // rustls sends CertificateRequired for both TLS 1.2 and 1.3,
          // but OpenSSL sends HandshakeFailure for TLS 1.2.
          if protocol_version == Some(rustls::ProtocolVersion::TLSv1_2) {
            "SSLV3_ALERT_HANDSHAKE_FAILURE"
          } else {
            "TLSV13_ALERT_CERTIFICATE_REQUIRED"
          }
        }
        AD::NoApplicationProtocol => "TLSV1_ALERT_NO_APPLICATION_PROTOCOL",
        _ => "SSLV3_ALERT_HANDSHAKE_FAILURE",
      };
      (format!("{e}"), format!("ERR_SSL_{code}"))
    }
    E::NoApplicationProtocol => (
      format!("{e}"),
      "ERR_SSL_NO_APPLICATION_PROTOCOL".to_string(),
    ),
    // rustls returns `Error::General("no server certificate chain resolved")`
    // (and sends an AccessDenied fatal alert) when a server-side cert
    // resolver returns `None` — e.g. a TLS server configured with only
    // `SNICallback` whose callback hands back an empty SecureContext.
    // Node/OpenSSL raise "no suitable signature algorithm" on the server
    // and `ERR_SSL_SSLV3_ALERT_HANDSHAKE_FAILURE` on the client; mirror
    // both ends so existing Node code (and upstream tests) match.
    // NOTE: keep this string in sync with rustls upstream — there is no
    // structured `Error` variant for "resolver returned None".
    E::General(msg) if msg == "no server certificate chain resolved" => (
      "no suitable signature algorithm".to_string(),
      "ERR_SSL_NO_SUITABLE_SIGNATURE_ALGORITHM".to_string(),
    ),
    _ => (
      format!("{e}"),
      "ERR_SSL_SSLV3_ALERT_HANDSHAKE_FAILURE".to_string(),
    ),
  }
}

impl TLSWrapInner {
  fn new(kind: Kind, task_spawner: Option<V8TaskSpawner>) -> Self {
    Self {
      tls_conn: None,
      kind,
      enc_in: Vec::with_capacity(4096),
      started: false,
      established: false,
      shutdown: false,
      eof: false,
      cycling: false,
      has_buffered_cleartext: false,
      pending_clear_out: Vec::new(),
      pending_eof: false,
      in_dowrite: false,
      write_callback_scheduled: false,
      enc_writes_in_flight: 0,
      pending_cleartext: Vec::new(),
      pending_cleartext_offset: 0,
      pending_enc_out: Vec::new(),
      underlying: UnderlyingStream::None,
      js_handle: None,
      isolate: None,
      stream_base_state: None,
      onread: None,
      current_write_obj: None,
      current_write_bytes: 0,
      bytes_read: 0,
      bytes_written: 0,
      alive: Rc::new(Cell::new(true)),
      closing: false,
      error: None,
      verify_error: Arc::new(std::sync::Mutex::new(None)),
      pending_client_config: None,
      pending_server_name: None,
      pending_server_config: None,
      cached_loop_ptr: std::ptr::null_mut(),
      use_acceptor: false,
      acceptor: None,
      accepted: None,
      client_hello_servername: None,
      client_hello_alpn: Vec::new(),
      allow_resumption: Arc::new(AtomicBool::new(false)),
      user_buffer: None,
      task_spawner,
    }
  }

  /// Drive the TLS state machine through a raw pointer.
  /// Mirrors Node's TLSWrap::Cycle().
  ///
  /// Works entirely through raw pointer access to avoid holding any Rust
  /// reference across JS callbacks (which can re-enter ops on the same object).
  ///
  /// # Safety
  /// `ptr` must be a valid, non-null pointer to a live TLSWrapInner with
  /// valid isolate/context pointers.
  unsafe fn cycle(ptr: *mut TLSWrapInner) {
    unsafe {
      if (*ptr).cycling {
        return;
      }
      (*ptr).cycling = true;

      // Handle acceptor phase: feed encrypted data to the Acceptor until
      // the ClientHello is complete, then emit onclienthello to JS.
      if (*ptr).acceptor.is_some() {
        let got_hello = (*ptr).process_acceptor();
        (*ptr).cycling = false;
        if got_hello {
          if let Some(ctx) = extract_emit_ctx(ptr) {
            do_emit_client_hello(&ctx);
          }
        } else if let Some(ctx) = extract_emit_ctx(ptr) {
          // Acceptor failed (e.g. malformed ClientHello). Surface the
          // error to JS so the socket doesn't silently hang.
          // Check for emit context first so the error stays in
          // (*ptr).error if there's no context to deliver it to.
          if let Some(error) = (*ptr).error.take() {
            do_emit_error(
              &ctx,
              &error,
              "ERR_SSL_SSLV3_ALERT_HANDSHAKE_FAILURE",
            );
          }
        }
        return;
      }

      // Route the (possibly cached process-wide) `NodeServerCertVerifier`'s
      // writes to *this* connection's `verify_error` slot.  Without this
      // scope, a cached verifier shared with a previous TLSWrap would
      // either alias its store or carry stale errors across connections.
      let _verify_scope = VerifyErrorScope::enter((*ptr).verify_error.clone());
      (*ptr).clear_in();
      let result = (*ptr).clear_out_process();
      let enc_action = (*ptr).enc_out_collect();
      (*ptr).cycling = false;

      // --- Callback phase: no Rust reference to TLSWrapInner is held ---
      TLSWrapInner::dispatch_clear_out_callbacks(ptr, &result);
      if result.tls_error.is_some() {
        return;
      }
      // cycle() is only reached from ops that do NOT hold the OpState borrow
      // (read_buffer, receive, start, ...) or from uv callbacks, so the write
      // completion runs synchronously (WriteCompletion::Sync). Deferring here
      // would delay a JS-backed stream's `'finish'` by a macrotask and let a
      // peer FIN land first, spuriously aborting in-flight requests (#35820
      // only affected the write op, which holds the borrow and defers itself).
      TLSWrapInner::do_enc_out_action(ptr, enc_action, WriteCompletion::Sync);

      // After handshake completes, the JS callback (onhandshakedone ->
      // onConnectSecure) has run. If the connection was accepted (e.g.
      // checkServerIdentity passed), drain any pending cleartext that
      // was buffered before the handshake.  If JS destroyed the socket
      // (identity check failed), the closing flag prevents sending.
      if result.handshake_done && !(*ptr).closing {
        (*ptr).cycling = true;
        (*ptr).clear_in();
        let enc_action2 = (*ptr).enc_out_collect();
        (*ptr).cycling = false;
        // Synchronous for the same reason as the first dispatch above.
        TLSWrapInner::do_enc_out_action(
          ptr,
          enc_action2,
          WriteCompletion::Sync,
        );
      }
    }
  }

  /// Feed encrypted input to the Acceptor and try to extract the ClientHello.
  /// Returns true if the ClientHello was received and onclienthello should fire.
  fn process_acceptor(&mut self) -> bool {
    let acceptor = match self.acceptor.as_mut() {
      Some(a) => a,
      None => return false,
    };

    // Feed buffered encrypted data to the acceptor.
    if !self.enc_in.is_empty() {
      let mut cursor = std::io::Cursor::new(&self.enc_in[..]);
      match acceptor.read_tls(&mut cursor) {
        Ok(n) => {
          self.enc_in.drain(..n);
        }
        Err(_) => {
          self.error = Some("TLS acceptor read error".to_string());
          return false;
        }
      }
    }

    // Try to extract the ClientHello. accept(&mut self) puts the inner
    // back on Ok(None), so the Acceptor stays valid for the next cycle.
    let acceptor = self.acceptor.as_mut().unwrap();
    match acceptor.accept() {
      Ok(Some(accepted)) => {
        let hello = accepted.client_hello();
        self.client_hello_servername =
          hello.server_name().map(|s| s.to_string());
        self.client_hello_alpn = hello
          .alpn()
          .map(|iter| iter.map(|p| p.to_vec()).collect())
          .unwrap_or_default();
        // Acceptor is consumed on success, remove it.
        self.acceptor = None;
        self.accepted = Some(accepted);
        true
      }
      Ok(None) => {
        // Need more data; acceptor retains its state internally.
        false
      }
      Err((err, _alert)) => {
        self.acceptor = None;
        self.error = Some(format!("TLS accept error: {err}"));
        false
      }
    }
  }

  /// Feed pending cleartext into rustls writer.
  /// Mirrors Node's TLSWrap::ClearIn().
  fn clear_in(&mut self) {
    // Don't feed application data to rustls until the handshake is
    // complete and JS has confirmed the connection (via onhandshakedone).
    // In Node.js, the SSL_write for app data effectively happens after
    // the handshake callback because OpenSSL fires the info callback
    // synchronously during SSL_read. Without this gate, app data would
    // be encrypted and sent before checkServerIdentity can reject the
    // connection.
    if !self.established || self.closing {
      return;
    }

    let Some(ref mut conn) = self.tls_conn else {
      return;
    };

    debug_assert!(
      self.pending_cleartext_offset <= self.pending_cleartext.len()
    );
    let data = &self.pending_cleartext[self.pending_cleartext_offset..];
    if data.is_empty() {
      return;
    }

    // Feed cleartext to rustls in limited chunks. Writing everything
    // at once would produce a huge encrypted buffer that saturates
    // the TCP send buffer, causing deadlocks with echo patterns.
    // This matches Node.js where SSL_write processes incrementally.
    // Consumed bytes are tracked via pending_cleartext_offset instead of
    // re-allocating the unwritten tail on every chunk.
    const MAX_CLEAR_IN: usize = 48 * 1024;
    let feed_end = data.len().min(MAX_CLEAR_IN);
    let mut offset = 0;
    let mut write_error = false;
    while offset < feed_end {
      match conn.writer().write(&data[offset..feed_end]) {
        Ok(0) => break,
        Ok(n) => offset += n,
        Err(e) => {
          // Store the error so it can be surfaced to JS.
          self.error = Some(format!("SSL write error: {e}"));
          write_error = true;
          break;
        }
      }
    }
    self.pending_cleartext_offset += offset;
    if write_error
      || self.pending_cleartext_offset >= self.pending_cleartext.len()
    {
      // Fully consumed (or dropped on write error, matching the previous
      // behavior of not restoring the tail) — release the buffer.
      self.pending_cleartext = Vec::new();
      self.pending_cleartext_offset = 0;
    }
  }

  /// Process TLS records and collect decrypted cleartext.
  /// Returns a ClearOutResult describing what JS callbacks to fire.
  /// Does NOT call any JS callbacks — the caller handles that.
  fn clear_out_process(&mut self) -> ClearOutResult {
    let empty = ClearOutResult {
      handshake_done: false,
      data: Vec::new(),
      got_eof: false,
      got_error: false,
      tls_error: None,
    };

    if self.eof {
      return empty;
    }

    let Some(ref mut conn) = self.tls_conn else {
      return empty;
    };

    let was_handshaking = conn.is_handshaking();

    let mut data = Vec::new();
    let mut got_eof = false;
    let mut got_error = false;
    let tls_error = None;

    // Process all buffered TLS records.
    if !self.enc_in.is_empty() {
      let mut total_consumed = 0usize;
      loop {
        let remaining = &self.enc_in[total_consumed..];
        if remaining.is_empty() {
          break;
        }
        let mut cursor = std::io::Cursor::new(remaining);
        match conn.read_tls(&mut cursor) {
          Ok(_) => {
            let consumed = cursor.position() as usize;
            if consumed == 0 {
              break;
            }
            total_consumed += consumed;
          }
          Err(_) => break,
        }
        match conn.process_new_packets() {
          Ok(io_state) => {
            if io_state.peer_has_closed() {
              got_eof = true;
              self.eof = true;
            }
          }
          Err(e) => {
            if total_consumed > 0 {
              self.enc_in.drain(..total_consumed);
            }
            let (error_msg, error_code) =
              rustls_error_to_node_error(&e, conn.protocol_version());
            self.error = Some(error_msg.clone());
            // Flush the error alert to the underlying stream
            self.enc_out_flush_only();
            return ClearOutResult {
              handshake_done: false,
              data: Vec::new(),
              got_eof: false,
              got_error: false,
              tls_error: Some((error_msg, error_code)),
            };
          }
        }
        // Drain plaintext so rustls can accept more records
        {
          let mut tmp = [0u8; CLEAR_OUT_CHUNK_SIZE];
          loop {
            match conn.reader().read(&mut tmp) {
              Ok(0) => break,
              Ok(n) => {
                self.bytes_read += n as u64;
                data.extend_from_slice(&tmp[..n]);
              }
              Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
              Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                self.eof = true;
                got_eof = true;
                break;
              }
              Err(_) => {
                got_error = true;
                break;
              }
            }
          }
        }
        if got_eof || got_error {
          break;
        }
      }
      if total_consumed > 0 {
        self.enc_in.drain(..total_consumed);
      }
    }

    // Check if handshake just completed
    let is_handshaking_now = conn.is_handshaking();
    let handshake_done =
      was_handshaking && !is_handshaking_now && !self.established;

    self.has_buffered_cleartext = false;

    ClearOutResult {
      handshake_done,
      data,
      got_eof,
      got_error,
      tls_error,
    }
  }

  /// Collect encrypted output from rustls and determine what action to take.
  /// Does NOT call any JS callbacks or invoke_queued.
  fn enc_out_collect(&mut self) -> EncOutAction {
    let Some(ref mut conn) = self.tls_conn else {
      return EncOutAction::None;
    };

    // Collect ALL encrypted output from rustls into the pending buffer.
    // Vec's io::Write impl appends, so write_tls can serialize directly
    // into pending_enc_out without a temporary buffer + copy.
    while conn.wants_write() {
      match conn.write_tls(&mut self.pending_enc_out) {
        Ok(n) if n > 0 => {}
        _ => break,
      }
    }

    if self.pending_enc_out.is_empty() {
      if self.established
        && self.write_callback_scheduled
        && self.enc_writes_in_flight == 0
        && !self.in_dowrite
      {
        return EncOutAction::InvokeQueued(0);
      }
      return EncOutAction::None;
    }

    if self.current_write_obj.is_some() {
      self.write_callback_scheduled = true;
    }

    if !self.underlying.is_attached() {
      return EncOutAction::None;
    }

    match self.underlying {
      UnderlyingStream::Uv { .. } => EncOutAction::WriteUv,
      UnderlyingStream::Js { .. } => EncOutAction::WriteJs,
      UnderlyingStream::None => EncOutAction::None,
    }
  }

  /// Flush encrypted data from rustls to the underlying stream. Used in the
  /// error path of clear_out_process to send TLS alert records before emitting
  /// the error. In the common case no JS callback runs, but if a write is
  /// pending (`write_callback_scheduled`) and the underlying write fails
  /// synchronously, the completion fires here. That's safe: this is only
  /// reached from `cycle`, which never holds the OpState borrow, so the
  /// completion runs synchronously (`WriteCompletion::Sync`) like the rest of
  /// `cycle`.
  fn enc_out_flush_only(&mut self) {
    let Some(ref mut conn) = self.tls_conn else {
      return;
    };
    while conn.wants_write() {
      match conn.write_tls(&mut self.pending_enc_out) {
        Ok(n) if n > 0 => {}
        _ => break,
      }
    }
    if self.pending_enc_out.is_empty() || !self.underlying.is_attached() {
      return;
    }
    if let UnderlyingStream::Uv { .. } = self.underlying {
      self.enc_out_uv(WriteCompletion::Sync);
    }
    // JS stream: the data stays in pending_enc_out; cycle's callback phase
    // will handle it.
  }

  /// Dispatch JS callbacks from a ClearOutResult.
  /// Works through raw pointer — no Rust reference held across JS calls.
  ///
  /// # Safety
  /// `ptr` must be a valid, non-null pointer to a live TLSWrapInner.
  unsafe fn dispatch_clear_out_callbacks(
    ptr: *mut TLSWrapInner,
    result: &ClearOutResult,
  ) {
    unsafe {
      if let Some((ref error_msg, ref error_code)) = result.tls_error {
        if let Some(ctx) = extract_emit_ctx(ptr) {
          do_emit_error(&ctx, error_msg, error_code);
        }
        return;
      }

      if result.handshake_done {
        (*ptr).established = true;
        if let Some(ctx) = extract_emit_ctx(ptr) {
          do_emit_handshake_done(&ctx);
        }

        // Defensive: if the handshake_done callback ran teardown
        // synchronously (e.g. via a finalizer drained off the microtask
        // queue), bail before touching freed state.
        if !(*ptr).alive.get() {
          return;
        }

        // If shutdown was requested before handshake completed, execute
        // the deferred close_notify + underlying shutdown now.
        if (*ptr).shutdown {
          if let Some(ref mut conn) = (*ptr).tls_conn {
            conn.send_close_notify();
          }
          let enc_action = (*ptr).enc_out_collect();
          // Reached from cycle()'s callback phase, which never holds the
          // OpState borrow — dispatch synchronously (see cycle()).
          TLSWrapInner::do_enc_out_action(
            ptr,
            enc_action,
            WriteCompletion::Sync,
          );
          (*ptr).underlying.shutdown();
        }
      }

      if !result.data.is_empty() {
        if (*ptr).onread.is_none() {
          // readStop is active -- buffer the data for later delivery
          (*ptr).pending_clear_out.extend_from_slice(&result.data);
          (*ptr).has_buffered_cleartext = true;
        } else if let Some(ctx) = extract_emit_ctx(ptr) {
          let onread = (*ptr).onread.clone();
          let state = (*ptr).stream_base_state.clone();
          if (*ptr).user_buffer.is_some() {
            do_emit_read_through_user_buffer(
              &ctx,
              ptr,
              onread.as_ref(),
              state.as_ref(),
              &result.data,
            );
          } else {
            do_emit_read(
              &ctx,
              onread.as_ref(),
              state.as_ref(),
              result.data.len() as isize,
              Some(&result.data),
            );
          }
        }
        if (*ptr).tls_conn.is_none() {
          return;
        }
      }
      if result.got_eof {
        if (*ptr).onread.is_none() {
          // readStop is active -- defer EOF delivery
          (*ptr).pending_eof = true;
          (*ptr).has_buffered_cleartext = true;
        } else if let Some(ctx) = extract_emit_ctx(ptr) {
          let onread = (*ptr).onread.clone();
          let state = (*ptr).stream_base_state.clone();
          do_emit_read(
            &ctx,
            onread.as_ref(),
            state.as_ref(),
            UV_EOF as isize,
            None,
          );
        }
      } else if result.got_error
        && let Some(ctx) = extract_emit_ctx(ptr)
      {
        let onread = (*ptr).onread.clone();
        let state = (*ptr).stream_base_state.clone();
        do_emit_read(&ctx, onread.as_ref(), state.as_ref(), -1, None);
      }
    }
  }

  /// Execute the enc_out action determined by `enc_out_collect`.
  /// This may call JS callbacks, so it works through a raw pointer.
  ///
  /// `completion` selects how the write-completion callback is dispatched. It
  /// is `WriteCompletion::Deferred` only from `write_data` — the single path
  /// that holds the `OpState` borrow for its whole body (the writev/writeBuffer/
  /// writeUtf8String ops). There a synchronous callback would run JS while
  /// `OpState` is borrowed and panic with "RefCell already borrowed" the moment
  /// it reaches another op (#35820), so the callback is scheduled on the event
  /// loop instead.
  ///
  /// Every other caller — libuv callbacks (`enc_write_cb`) and the `&self` ops
  /// that drive `cycle`/`start`/`shutdown`/`finish_accept` (none of which
  /// borrow `OpState`) — passes `WriteCompletion::Sync` so the completion fires
  /// synchronously. Deferring on those paths would delay a JS-backed stream's
  /// `'finish'` by an event-loop turn, letting a peer FIN be processed first
  /// and spuriously aborting in-flight HTTP requests.
  ///
  /// # Safety
  /// `ptr` must be a valid, non-null pointer to a live TLSWrapInner.
  unsafe fn do_enc_out_action(
    ptr: *mut TLSWrapInner,
    action: EncOutAction,
    completion: WriteCompletion,
  ) {
    unsafe {
      match action {
        EncOutAction::None => {}
        EncOutAction::WriteUv => {
          (*ptr).enc_out_uv(completion);
        }
        EncOutAction::WriteJs => {
          // Pull-based: leave data in pending_enc_out for JS to drain
          // via drain_enc_out(). This avoids calling back into JS from
          // within an op, eliminating reentrancy issues.
        }
        EncOutAction::InvokeQueued(status) => {
          dispatch_invoke_queued(ptr, completion, status);
        }
      }
    }
  }

  /// Write encrypted data to the underlying uv stream.
  ///
  /// `completion` has the same meaning as in `do_enc_out_action`: it controls
  /// whether the synchronous-write-failure completion callback is scheduled on
  /// the event loop (`WriteCompletion::Deferred`, from write ops) or run inline
  /// (`WriteCompletion::Sync`, from libuv callbacks).
  fn enc_out_uv(&mut self, completion: WriteCompletion) {
    let enc_data = std::mem::take(&mut self.pending_enc_out);
    let self_ptr = self as *mut TLSWrapInner;
    let write_req = Box::new(EncryptedWriteReq {
      uv_req: uv_compat::new_write(),
      _data: enc_data,
      tls_wrap_inner: self_ptr,
      alive: self.alive.clone(),
    });

    self.enc_writes_in_flight += 1;
    let (req_ptr, ret) = self.underlying.write(write_req);
    if ret != 0 {
      self.enc_writes_in_flight -= 1;
      let should_invoke = if !req_ptr.is_null() {
        // Failed to write — reclaim the request
        // SAFETY: req_ptr was returned from underlying.write and is a valid EncryptedWriteReq
        let reclaimed =
          unsafe { Box::from_raw(req_ptr as *mut EncryptedWriteReq) };
        if ret == UV_EBADF && !self.established {
          // Stream not connected yet — put the data back so we
          // retry on the next enc_out() call (after connect).
          self.pending_enc_out = reclaimed._data;
          false
        } else {
          self.write_callback_scheduled
        }
      } else {
        self.write_callback_scheduled
      };
      if should_invoke {
        // A synchronous write failure (e.g. the underlying handle was
        // already closed -> UV_EBADF). When this is reached from a write op
        // (`completion` is `Deferred`) the op still holds the OpState borrow,
        // so running the JS `oncomplete` callback here would panic with
        // "RefCell already borrowed" as soon as it reaches another op
        // (#35820); dispatch_invoke_queued schedules it on the event loop.
        // Use raw pointer to drop the &mut self borrow before the JS call.
        let ptr = self_ptr;
        // SAFETY: self_ptr is valid (points to self); prepare_invoke_queued
        // and do_invoke_queued do not hold references across JS calls.
        unsafe {
          dispatch_invoke_queued(ptr, completion, ret);
        }
      }
    }
    // Note: for successful writes, invoke_queued is called from enc_write_cb
    // when the uv_write completes asynchronously.
  }

  /// Finalizer-safe cleanup that does NOT invoke JS callbacks.
  /// Called from `TLSWrap::destroy_ssl` and from cppgc `Drop`.
  fn teardown(&mut self) {
    // Mark as dead so in-flight enc_write_cb callbacks won't dereference
    // the TLSWrapInner pointer after it is freed. This must happen even
    // when no TLS connection was ever created: encrypted writes can be
    // in flight without one (e.g. the finish_accept error path flushes
    // a TLS alert via enc_out_uv before tls_conn is set).
    self.alive.set(false);

    if self.tls_conn.is_none() {
      return;
    }

    self.tls_conn = None;
    self.js_handle = None;
    self.onread = None;
    self.stream_base_state = None;
    self.current_write_obj = None;
  }

  // NOTE: The JS callback methods (emit_read, emit_error, emit_handshake_done,
  // invoke_queued, enc_out_js) are implemented as free functions above
  // (do_emit_read, do_emit_error, do_emit_handshake_done, do_invoke_queued,
  // do_enc_out_js) to avoid holding any Rust reference to TLSWrapInner
  // across a JS call that could re-enter ops on the same object.
}

// ---------------------------------------------------------------------------
// C callbacks for intercepting the underlying stream
// ---------------------------------------------------------------------------

/// Called when encrypted data arrives from the underlying stream.
/// The underlying LibUvStreamWrap would forward raw read events here if TLS
/// registered as its read interceptor.
///
/// Currently unused: read interception is performed at the JS layer, where
/// `nativeHandle.onread` forwards encrypted chunks to `TLSWrap.receive()`.
/// Kept for a future switch to native (Rust-side) read interception.
#[allow(
  dead_code,
  reason = "reserved for a future native read-interception path"
)]
unsafe fn tls_read_interceptor_cb(
  tls_wrap: *mut std::ffi::c_void,
  _stream: *mut uv_stream_t,
  nread: isize,
  buf: *const uv_buf_t,
) {
  unsafe {
    if tls_wrap.is_null() {
      free_uv_buf(buf);
      return;
    }
    let ptr = tls_wrap as *mut TLSWrapInner;

    if (*ptr).eof {
      free_uv_buf(buf);
      return;
    }

    if nread < 0 {
      free_uv_buf(buf);
      // Flush any remaining cleartext via the compute-only path
      let result = (*ptr).clear_out_process();
      if nread == UV_EOF as isize {
        (*ptr).eof = true;
      }
      // Emit read callbacks without holding a reference
      if !result.data.is_empty()
        && let Some(ctx) = extract_emit_ctx(ptr)
      {
        let onread = (*ptr).onread.clone();
        let state = (*ptr).stream_base_state.clone();
        if (*ptr).user_buffer.is_some() {
          do_emit_read_through_user_buffer(
            &ctx,
            ptr,
            onread.as_ref(),
            state.as_ref(),
            &result.data,
          );
        } else {
          do_emit_read(
            &ctx,
            onread.as_ref(),
            state.as_ref(),
            result.data.len() as isize,
            Some(&result.data),
          );
        }
      }
      if let Some(ctx) = extract_emit_ctx(ptr) {
        let onread = (*ptr).onread.clone();
        let state = (*ptr).stream_base_state.clone();
        do_emit_read(&ctx, onread.as_ref(), state.as_ref(), nread, None);
      }
      return;
    }

    if nread == 0 {
      free_uv_buf(buf);
      return;
    }

    // Buffer the encrypted data
    let n = nread as usize;
    let buf_ref = &*buf;
    let slice = std::slice::from_raw_parts(buf_ref.base as *const u8, n);
    (*ptr).enc_in.extend_from_slice(slice);
    free_uv_buf(buf);

    // Drive the TLS state machine (uses raw pointer internally)
    TLSWrapInner::cycle(ptr);
  }
}

/// Callback for shutdown request — just frees the request.
unsafe extern "C" fn shutdown_cb(
  req: *mut uv_compat::uv_shutdown_t,
  _status: i32,
) {
  if !req.is_null() {
    unsafe {
      let _ = Box::from_raw(req);
    }
  }
}

/// Callback for when encrypted write to underlying stream completes.
unsafe extern "C" fn enc_write_cb(req: *mut uv_write_t, status: i32) {
  // SAFETY: req was created via Box::into_raw in enc_out; tls_wrap_inner is
  // valid if non-null AND alive flag is set.
  unsafe {
    let write_req = Box::from_raw(req as *mut EncryptedWriteReq);
    if !write_req.tls_wrap_inner.is_null() && write_req.alive.get() {
      let ptr = write_req.tls_wrap_inner;
      (*ptr).enc_writes_in_flight =
        (*ptr).enc_writes_in_flight.saturating_sub(1);
      if (*ptr).enc_writes_in_flight == 0 && status >= 0 {
        // If clear_in() was rate-limited (MAX_CLEAR_IN) and left
        // pending cleartext, drain the next chunk now. Without
        // this the remaining bytes are never fed to rustls and the
        // peer never receives the full body ("socket hang up").
        if (*ptr).pending_cleartext.len() > (*ptr).pending_cleartext_offset {
          (*ptr).clear_in();
        }
        let enc_action = (*ptr).enc_out_collect();
        // enc_write_cb runs from the libuv event loop, not an op, so the
        // completion callback fires synchronously (WriteCompletion::Sync) to
        // match Node's write-callback timing. See `do_enc_out_action`.
        TLSWrapInner::do_enc_out_action(ptr, enc_action, WriteCompletion::Sync);
      } else if (*ptr).enc_writes_in_flight == 0
        && (*ptr).write_callback_scheduled
      {
        // Write failed — still need to fire the JS completion callback
        if let Some((write_obj, ctx)) = prepare_invoke_queued(ptr) {
          do_invoke_queued(&ctx, write_obj, status);
        }
      }
    }
  }
}

// ---------------------------------------------------------------------------
// TLSWrap — the CppGC object visible to JS
// ---------------------------------------------------------------------------

#[derive(CppgcInherits)]
#[cppgc_inherits_from(LibUvStreamWrap)]
#[repr(C)]
pub struct TLSWrap {
  base: LibUvStreamWrap,
  inner: OwnedPtr<TLSWrapInner>,
}

// SAFETY: TLSWrap is CppGC-managed; trace correctly visits the base member
unsafe impl GarbageCollected for TLSWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TLSWrap"
  }

  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}

impl Drop for TLSWrap {
  fn drop(&mut self) {
    self.teardown();
  }
}

impl TLSWrap {
  /// Finalizer-safe cleanup that does NOT invoke JS callbacks.
  /// Safe to call from cppgc Drop.
  fn teardown(&self) {
    let inner = unsafe { self.inner.as_mut() };
    inner.teardown();
  }

  fn write_data(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    data: &[u8],
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let byte_length = data.len();
    let inner = unsafe { self.inner.as_mut() };

    if inner.tls_conn.is_none() {
      if !inner.started {
        // TLS connection not yet established (start() hasn't been called).
        // Buffer the data so it's sent after the handshake completes.
        inner.current_write_obj = Some(v8::Global::new(scope, req_wrap_obj));
        inner.current_write_bytes = byte_length;
        inner.write_callback_scheduled = true;
        inner.pending_cleartext.extend_from_slice(data);

        let state_global = &op_state.borrow::<StreamBaseState>().array;
        let state_array = v8::Local::new(scope, state_global);
        state_array.set_index(
          scope,
          StreamBaseStateFields::BytesWritten as u32,
          v8::Number::new(scope, byte_length as f64).into(),
        );
        state_array.set_index(
          scope,
          StreamBaseStateFields::LastWriteWasAsync as u32,
          v8::Integer::new(scope, 1).into(),
        );
        return 0;
      }
      inner.error = Some("Write after DestroySSL".to_string());
      return -1;
    }

    inner.bytes_written += byte_length as u64;

    if byte_length == 0 {
      // Zero-byte writes are no-ops — don't interact with the TLS
      // state machine.  Processing enc_in/enc_out here can corrupt
      // the record stream (Node.js / OpenSSL treats a 0-byte
      // SSL_write the same way).
      return 0;
    }

    // Store current write for completion tracking
    inner.current_write_obj = Some(v8::Global::new(scope, req_wrap_obj));
    inner.current_write_bytes = byte_length;

    // Store all cleartext as pending, then drain a limited amount.
    // clear_in() feeds up to 48KB to rustls per call, preventing
    // the TCP send buffer from being overwhelmed.
    inner.pending_cleartext.clear();
    inner.pending_cleartext.extend_from_slice(data);
    inner.pending_cleartext_offset = 0;
    inner.in_dowrite = true;
    inner.clear_in();
    let enc_action = inner.enc_out_collect();
    inner.in_dowrite = false;
    let inner_ptr = inner as *mut TLSWrapInner;
    // This is the one write path that holds the `OpState` borrow (write_data
    // is the shared impl of writev/writeBuffer/writeUtf8String, all of which
    // take `&mut OpState`), so a synchronous completion here would re-enter an
    // op while OpState is borrowed and panic (#35820). Defer it to the event
    // loop. Every other dispatch site runs on a non-borrowing context and
    // passes `WriteCompletion::Sync`.
    // SAFETY: inner_ptr is valid; do_enc_out_action is reference-free
    unsafe {
      TLSWrapInner::do_enc_out_action(
        inner_ptr,
        enc_action,
        WriteCompletion::Deferred,
      )
    };

    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);
    state_array.set_index(
      scope,
      StreamBaseStateFields::BytesWritten as u32,
      v8::Number::new(scope, byte_length as f64).into(),
    );
    state_array.set_index(
      scope,
      StreamBaseStateFields::LastWriteWasAsync as u32,
      v8::Integer::new(scope, 1).into(),
    );

    0
  }
}

#[op2(inherit = LibUvStreamWrap)]
impl TLSWrap {
  /// Create a new TLSWrap around a SecureContext.
  /// Called from JS as: tls_wrap.wrap(handle, secureContext, isServer)
  ///
  /// For now, secureContext is a JS object with {rustls_client_config} or
  /// {rustls_server_config} stashed on it by the SecureContext implementation.
  #[constructor]
  #[cppgc]
  fn new(
    #[smi] kind: i32,
    #[smi] _underlying_provider: i32,
    op_state: &mut OpState,
  ) -> TLSWrap {
    // Create a placeholder — the actual TLS connection is set up later
    // via initTls() once we have the secure context and underlying stream.
    let kind = if kind == 1 {
      Kind::Server
    } else {
      Kind::Client
    };

    let provider = ProviderType::TlsWrap as i32;
    let base = LibUvStreamWrap::new(
      HandleWrap::create(AsyncWrap::create(op_state, provider), None),
      -1,
      std::ptr::null(),
    );

    // `V8TaskSpawner` is always present in `OpState` for a live runtime. Borrow
    // (rather than `try_borrow`) so a missing spawner fails loudly here instead
    // of silently constructing an inner with `task_spawner: None`, which would
    // degrade the deferred write-completion path back to the synchronous
    // reentrancy panic this fix avoids (#35820). It is threaded into the
    // constructor so the invariant "a runtime-built TLSWrapInner always has a
    // spawner" holds at construction rather than via a follow-up assignment.
    let task_spawner = op_state.borrow::<V8TaskSpawner>().clone();
    let inner = TLSWrapInner::new(kind, Some(task_spawner));

    TLSWrap {
      base,
      inner: OwnedPtr::from_box(Box::new(inner)),
    }
  }

  /// Store client TLS options for deferred connection creation.
  /// The actual ClientConnection is created in start() so that
  /// setALPNProtocols can modify the config first.
  ///
  /// Takes the SecureContext JS object { ca, cert, key } and builds
  /// the rustls ClientConfig from it.
  #[nofast]
  #[reentrant]
  fn init_client_tls(
    &self,
    #[string] server_name: String,
    context: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    // Empty string means no SNI (caller passes "" when servername is not set).
    let server_name = if server_name.is_empty() {
      None
    } else {
      // If the hostname is not a valid DNS name or IP address, skip SNI
      // rather than failing TLS initialization entirely.  Node.js allows
      // invalid hostnames through TLS setup and lets DNS resolution fail
      // later with the proper error code (ENOTFOUND / EAI_FAIL).
      rustls::pki_types::ServerName::try_from(server_name).ok()
    };

    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    let allow_resumption = inner.allow_resumption.clone();
    let client_config =
      match build_client_config(scope, context, op_state, allow_resumption) {
        Some((c, _)) => c,
        None => return -1,
      };
    // The verifier in `client_config` writes errors via the per-connection
    // `CURRENT_VERIFY_ERROR` thread-local set by `cycle`, so `inner`'s own
    // pre-allocated `verify_error` slot stays correctly scoped per
    // connection even when the verifier `Arc` is cached process-wide.
    inner.pending_client_config = Some(Arc::new(client_config));
    inner.pending_server_name = server_name;
    0
  }

  /// Store server TLS options for deferred connection creation.
  /// The actual ServerConnection is created in start().
  #[nofast]
  #[reentrant]
  fn init_server_tls(
    &self,
    context: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let (server_config, client_cert_verify_error) =
      match build_server_config(scope, context, op_state) {
        Some(c) => c,
        None => {
          return -1;
        }
      };

    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.pending_server_config = Some(Arc::new(server_config));
    // Share the client cert verify error store with the TLSWrap so that
    // `verifyError()` on the server side returns client cert errors.
    inner.verify_error = client_cert_verify_error;
    0
  }

  /// Attach to an underlying stream for encrypted writes.
  ///
  /// Read interception is handled at the JS layer: the JS binding sets
  /// `nativeHandle.onread` to forward encrypted data to `TLSWrap.receive()`.
  /// A native interceptor path exists (see `tls_read_interceptor_cb`) but
  /// is not currently wired up.
  #[nofast]
  fn attach(
    &self,
    #[cppgc] tcp: &crate::ops::tcp_wrap::TCPWrap,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let stream = tcp.stream_ptr();
    Self::do_attach_uv_stream(&self.inner, stream, scope, op_state)
  }

  /// Attach to a PipeWrap (Unix domain socket) for encrypted I/O.
  #[nofast]
  fn attach_pipe(
    &self,
    #[cppgc] pipe: &crate::ops::pipe_wrap::PipeWrap,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let stream = pipe.stream_ptr();
    Self::do_attach_uv_stream(&self.inner, stream, scope, op_state)
  }

  /// Store the JS handle reference for callbacks.
  #[nofast]
  fn set_handle(
    &self,
    handle: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.js_handle = Some(v8::Global::new(scope, handle));
  }

  /// Set the onread callback.
  #[nofast]
  fn set_onread(
    &self,
    onread: v8::Local<v8::Function>,
    scope: &mut v8::PinScope,
  ) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.onread = Some(v8::Global::new(scope, onread));
  }

  /// Register a static read buffer for decrypted plaintext.
  /// Overrides `LibUvStreamWrap::useUserBuffer` so the buffer is
  /// associated with the TLS layer rather than the encrypted underlying
  /// stream (Node's `onread.buffer` semantics).
  #[fast]
  #[rename("useUserBuffer")]
  fn use_user_buffer_tls(
    &self,
    buffer: v8::Local<v8::Uint8Array>,
    scope: &mut v8::PinScope,
  ) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.user_buffer =
      crate::ops::stream_wrap::UserBuffer::from_view(scope, buffer);
  }

  /// Start the TLS handshake.
  /// Creates the actual TLS connection from pending config, then begins
  /// the handshake. Mirrors Node's TLSWrap::Start().
  #[fast]
  #[reentrant]
  fn start(&self) -> i32 {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    if inner.started {
      // Already started — but the underlying stream may have just
      // connected.  Flush any buffered encrypted output (e.g. the
      // ClientHello that was generated before the socket connected).
      if !inner.pending_enc_out.is_empty() {
        let enc_action = inner.enc_out_collect();
        let inner_ptr = inner as *mut TLSWrapInner;
        // `start` is an `&self` op that does not hold the OpState borrow, so
        // any completion runs synchronously (WriteCompletion::Sync).
        unsafe {
          TLSWrapInner::do_enc_out_action(
            inner_ptr,
            enc_action,
            WriteCompletion::Sync,
          )
        };
      }
      return 0;
    }
    inner.started = true;

    // Create the TLS connection from pending config.
    // Return -1 if the config was never set (init_client_tls/init_server_tls
    // was not called or failed).
    match inner.kind {
      Kind::Client => {
        let Some(config) = inner.pending_client_config.take() else {
          inner.error = Some("TLS config not initialized".to_string());
          return -1;
        };
        let server_name = inner.pending_server_name.take();
        let conn_result = match server_name {
          Some(name) => rustls::ClientConnection::new(config, name),
          None => {
            // No SNI — use an IP address which suppresses the SNI extension.
            let no_sni = rustls::pki_types::ServerName::IpAddress(
              rustls::pki_types::IpAddr::from(std::net::Ipv4Addr::UNSPECIFIED),
            );
            rustls::ClientConnection::new(config, no_sni)
          }
        };
        match conn_result {
          Ok(conn) => {
            inner.tls_conn = Some(TlsConnection::Client(conn));
          }
          Err(e) => {
            inner.error = Some(format!("TLS connection error: {e}"));
            return -1;
          }
        }
      }
      Kind::Server => {
        if inner.use_acceptor {
          // Acceptor path: defer ServerConnection creation until we have
          // the ClientHello so we can invoke SNICallback / ALPNCallback.
          // pending_server_config is kept for finish_accept().
          inner.acceptor = Some(rustls::server::Acceptor::default());
        } else {
          let Some(config) = inner.pending_server_config.take() else {
            inner.error = Some("TLS config not initialized".to_string());
            return -1;
          };
          match rustls::ServerConnection::new(config) {
            Ok(conn) => {
              inner.tls_conn = Some(TlsConnection::Server(conn));
            }
            Err(e) => {
              inner.error = Some(format!("TLS connection error: {e}"));
              return -1;
            }
          }
        }
      }
    }

    // Start reading is driven by TLSSocket.read(0) -> TLSWrap.read_start(),
    // which mirrors Node's initRead timing and gives JS a chance to attach
    // listeners first.
    inner.underlying.read_start();

    // Drive the state machine. For client mode this initiates the
    // handshake (ClientHello). It also drains any pending_cleartext
    // that was buffered before start() was called.
    let inner_ptr = inner as *mut TLSWrapInner;
    // SAFETY: inner_ptr points to heap-allocated TLSWrapInner via OwnedPtr
    unsafe { TLSWrapInner::cycle(inner_ptr) };

    0
  }

  /// ReadStart — start reading cleartext from TLS.
  /// Mirrors Node's TLSWrap::ReadStart().
  #[nofast]
  #[reentrant]
  fn read_start(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };

    // Get onread from the JS object
    let this_local = v8::Local::new(scope, &this);
    let onread_key =
      v8::String::new_external_onebyte_static(scope, b"onread").unwrap();
    let Some(onread_val) = this_local.get(scope, onread_key.into()) else {
      return UV_EBADF;
    };
    let Ok(onread) = v8::Local::<v8::Function>::try_from(onread_val) else {
      return UV_EBADF;
    };

    inner.onread = Some(v8::Global::new(scope, onread));

    // Flush any cleartext that was buffered while readStop was active.
    // This must happen before the cycle below so the consumer sees the
    // data in the order it was decrypted.
    let inner_ptr = inner as *mut TLSWrapInner;
    let pending_data = std::mem::take(&mut inner.pending_clear_out);
    let pending_eof = inner.pending_eof;
    inner.pending_eof = false;
    if (!pending_data.is_empty() || pending_eof)
      && let Some(ctx) = (unsafe { extract_emit_ctx(inner_ptr) })
    {
      if !pending_data.is_empty() {
        let onread_clone = inner.onread.clone();
        let state = inner.stream_base_state.clone();
        let has_user_buffer = inner.user_buffer.is_some();
        unsafe {
          if has_user_buffer {
            do_emit_read_through_user_buffer(
              &ctx,
              inner_ptr,
              onread_clone.as_ref(),
              state.as_ref(),
              &pending_data,
            );
          } else {
            do_emit_read(
              &ctx,
              onread_clone.as_ref(),
              state.as_ref(),
              pending_data.len() as isize,
              Some(&pending_data),
            );
          }
        }
      }
      if pending_eof {
        let onread_clone = inner.onread.clone();
        let state = inner.stream_base_state.clone();
        unsafe {
          do_emit_read(
            &ctx,
            onread_clone.as_ref(),
            state.as_ref(),
            UV_EOF as isize,
            None,
          );
        }
      }
    }

    // For the Uv case, read interception is done at the JS layer via
    // nativeHandle.onread -> TLSWrap.receive(). The JS layer calls
    // nativeHandle.readStart() separately. We just need to cycle if
    // there's already buffered data.
    let should_cycle;
    if inner.underlying.is_attached() && inner.started {
      should_cycle = !inner.enc_in.is_empty() || inner.has_buffered_cleartext;
      if !matches!(inner.underlying, UnderlyingStream::Uv { .. }) {
        inner.underlying.read_start();
      }
    } else {
      should_cycle = false;
    }

    if should_cycle {
      // SAFETY: inner_ptr points to heap-allocated TLSWrapInner via OwnedPtr
      unsafe { TLSWrapInner::cycle(inner_ptr) };
    }

    0
  }

  /// ReadStop — for Uv streams, don't stop the native TCP reads.
  /// The underlying TCP handle keeps reading encrypted data; we just
  /// stop delivering decrypted plaintext to JS by clearing onread.
  ///
  /// Known limitation: the TCP socket keeps receiving and buffering
  /// encrypted data in the kernel even after read_stop(). For long-lived
  /// connections with flow control this could accumulate data. Properly
  /// plumbing a native uv_read_stop through TLSWrap is deferred until the
  /// native read-interception path (`tls_read_interceptor_cb`) is wired up.
  #[fast]
  fn read_stop(&self, _scope: &mut v8::PinScope) -> i32 {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.onread = None;
    0
  }

  /// Writev — collect multiple buffers into one and write through TLS.
  /// Without this override, the base LibUvStreamWrap::writev would bypass
  /// TLS and write directly to the underlying TCP stream.
  #[nofast]
  fn writev(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    chunks: v8::Local<v8::Array>,
    all_buffers: bool,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let mut data = Vec::new();
    if all_buffers {
      let len = chunks.length();
      for i in 0..len {
        let Some(chunk) = chunks.get_index(scope, i) else {
          continue;
        };
        if let Ok(buf) = TryInto::<v8::Local<v8::Uint8Array>>::try_into(chunk) {
          let byte_len = buf.byte_length();
          let byte_off = buf.byte_offset();
          // Skip chunks whose ArrayBuffer has been detached (e.g. during
          // sandbox teardown). Erroring on each would spam the close path;
          // surviving chunks are still written.
          let Some(ab) = buf.buffer(scope) else {
            continue;
          };
          let Some(data_ptr) = ab.data() else {
            continue;
          };
          let ptr = data_ptr.as_ptr() as *const u8;
          // SAFETY: ptr + offset is within the ArrayBuffer backing store
          let slice =
            unsafe { std::slice::from_raw_parts(ptr.add(byte_off), byte_len) };
          data.extend_from_slice(slice);
        }
      }
    } else {
      let len = chunks.length();
      let count = len / 2;
      for i in 0..count {
        let Some(chunk) = chunks.get_index(scope, i * 2) else {
          continue;
        };
        if let Ok(buf) = TryInto::<v8::Local<v8::Uint8Array>>::try_into(chunk) {
          let byte_len = buf.byte_length();
          let byte_off = buf.byte_offset();
          // Skip detached buffers (see comment in all_buffers=true branch).
          let Some(ab) = buf.buffer(scope) else {
            continue;
          };
          let Some(data_ptr) = ab.data() else {
            continue;
          };
          let ptr = data_ptr.as_ptr() as *const u8;
          // SAFETY: ptr + offset is within the ArrayBuffer backing store
          let slice =
            unsafe { std::slice::from_raw_parts(ptr.add(byte_off), byte_len) };
          data.extend_from_slice(slice);
        } else if let Ok(s) = TryInto::<v8::Local<v8::String>>::try_into(chunk)
        {
          let encoding_idx = i * 2 + 1;
          let _ = chunks.get_index(scope, encoding_idx);
          let len = s.utf8_length(scope);
          let mut buf = Vec::with_capacity(len);
          let written = s.write_utf8_uninit_v2(
            scope,
            buf.spare_capacity_mut(),
            v8::WriteFlags::kReplaceInvalidUtf8,
            None,
          );
          // SAFETY: written bytes are initialized by write_utf8_uninit_v2
          unsafe { buf.set_len(written) };
          data.extend_from_slice(&buf);
        }
      }
    }

    self.write_data(req_wrap_obj, &data, scope, op_state)
  }

  /// DoWrite — encrypt cleartext and write to underlying stream.
  /// Mirrors Node's TLSWrap::DoWrite().
  #[nofast]
  fn write_buffer(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    buffer: v8::Local<v8::Uint8Array>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let byte_length = buffer.byte_length();
    let byte_offset = buffer.byte_offset();
    let Some(ab) = buffer.buffer(scope) else {
      return -1;
    };
    let Some(data_ptr) = ab.data() else {
      return -1;
    };
    let ptr = data_ptr.as_ptr() as *const u8;
    // SAFETY: ptr + offset is within the ArrayBuffer backing store
    let data =
      unsafe { std::slice::from_raw_parts(ptr.add(byte_offset), byte_length) };

    self.write_data(req_wrap_obj, data, scope, op_state)
  }

  /// Write a UTF-8 string through TLS.
  #[nofast]
  fn write_utf8_string(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let len = string.utf8_length(scope);
    let mut buf = Vec::with_capacity(len);
    let written = string.write_utf8_uninit_v2(
      scope,
      buf.spare_capacity_mut(),
      v8::WriteFlags::kReplaceInvalidUtf8,
      None,
    );
    // SAFETY: written bytes are initialized by write_utf8_uninit_v2
    unsafe { buf.set_len(written) };
    self.write_data(req_wrap_obj, &buf, scope, op_state)
  }

  /// Write an ASCII string through TLS.
  #[nofast]
  fn write_ascii_string(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let len = string.utf8_length(scope);
    let mut buf = Vec::with_capacity(len);
    let written = string.write_utf8_uninit_v2(
      scope,
      buf.spare_capacity_mut(),
      v8::WriteFlags::kReplaceInvalidUtf8,
      None,
    );
    // SAFETY: written bytes are initialized by write_utf8_uninit_v2
    unsafe { buf.set_len(written) };
    self.write_data(req_wrap_obj, &buf, scope, op_state)
  }

  /// Write a Latin1 string through TLS.
  #[nofast]
  fn write_latin1_string(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let len = string.length();
    let mut buf = Vec::with_capacity(len);
    string.write_one_byte_uninit_v2(
      scope,
      0,
      buf.spare_capacity_mut(),
      v8::WriteFlags::empty(),
    );
    // SAFETY: len bytes are initialized by write_one_byte_uninit_v2
    unsafe { buf.set_len(len) };
    self.write_data(req_wrap_obj, &buf, scope, op_state)
  }

  /// Write a UCS-2 string through TLS.
  #[nofast]
  fn write_ucs2_string(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let len = string.length();
    let mut buf16 = vec![0u16; len];
    string.write_v2(scope, 0, &mut buf16, v8::WriteFlags::empty());
    let buf: Vec<u8> = buf16.iter().flat_map(|&c| c.to_le_bytes()).collect();
    self.write_data(req_wrap_obj, &buf, scope, op_state)
  }

  /// Graceful TLS shutdown — send close_notify.
  ///
  /// Matching Node's TLSWrap::DoShutdown: send close_notify, flush
  /// encrypted output, but do NOT immediately shut down the underlying
  /// TCP stream.  The underlying stream will be shut down when the
  /// TLS socket is destroyed, allowing the peer to receive the
  /// close_notify and respond before the TCP connection is torn down.
  #[fast]
  #[reentrant]
  fn shutdown(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    {
      let inner = unsafe { &mut *self.inner.as_mut_ptr() };

      inner.shutdown = true;

      let handshaking =
        inner.tls_conn.as_ref().is_some_and(|c| c.is_handshaking());

      if handshaking {
        // Handshake not yet complete — defer close_notify and underlying
        // shutdown.  dispatch_clear_out_callbacks will check the shutdown
        // flag once the handshake finishes and drive the close then.
      } else {
        if let Some(ref mut conn) = inner.tls_conn {
          conn.send_close_notify();
        }
        let enc_action = inner.enc_out_collect();
        let inner_ptr = inner as *mut TLSWrapInner;
        // `shutdown` is an `&self` op that does not hold the OpState borrow, so
        // any completion runs synchronously (WriteCompletion::Sync).
        unsafe {
          TLSWrapInner::do_enc_out_action(
            inner_ptr,
            enc_action,
            WriteCompletion::Sync,
          )
        };

        // Forward shutdown to underlying stream, matching Node's
        // TLSWrap::DoShutdown → underlying_stream()->DoShutdown().
        // uv_shutdown defers until the write queue drains, so the
        // close_notify (written by enc_out above) is sent first.
        inner.underlying.shutdown();
      }
    }

    // Call req.oncomplete(0) to signal completion to the JS side,
    // matching Node's StreamBase shutdown callback.
    let oncomplete_key =
      v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
    if let Some(val) = req_wrap_obj.get(scope, oncomplete_key.into())
      && let Ok(func) = v8::Local::<v8::Function>::try_from(val)
    {
      let status = v8::Integer::new(scope, 0);
      func.call(scope, req_wrap_obj.into(), &[status.into()]);
    }

    0
  }

  /// Mark the TLSWrap as closing. Called synchronously from JS
  /// TLSWrap.close() so the Rust side knows not to send buffered
  /// application data after a handshake callback rejects the connection.
  #[fast]
  fn set_closing(&self) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.closing = true;
  }

  /// Complete any pending JS write request with ECANCELED before teardown.
  #[nofast]
  #[reentrant]
  fn cancel_write(&self) {
    let ptr = self.inner.as_mut_ptr();
    // SAFETY: ptr points to this live TLSWrapInner; the helper clears the
    // stored write object before invoking JS and does not retain references
    // across the callback.
    unsafe {
      if let Some((write_obj, ctx)) = prepare_invoke_queued(ptr) {
        do_invoke_queued(&ctx, write_obj, UV_ECANCELED);
      }
    }
  }

  /// Destroy the SSL connection. Tears down the TLS state without
  /// re-entering JS (no write-completion callbacks).
  #[nofast]
  fn destroy_ssl(&self) {
    self.teardown();
  }

  /// Get the negotiated ALPN protocol.
  /// Writes the protocol name into the out object as { alpnProtocol: "..." }.
  #[fast]
  fn get_alpn_negotiated_protocol(
    &self,
    out: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    let key =
      v8::String::new_external_onebyte_static(scope, b"alpnProtocol").unwrap();
    if let Some(ref conn) = inner.tls_conn
      && let Some(proto) = conn.alpn_protocol()
      && let Ok(s) = std::str::from_utf8(proto)
    {
      let val = v8::String::new(scope, s).unwrap();
      out.set(scope, key.into(), val.into());
      return 0;
    }
    let false_val = v8::Boolean::new(scope, false);
    out.set(scope, key.into(), false_val.into());
    0
  }

  /// Get the negotiated TLS protocol version.
  /// Writes into out object as { protocol: "TLSv1.3" }.
  #[fast]
  fn get_protocol(
    &self,
    out: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    let key =
      v8::String::new_external_onebyte_static(scope, b"protocol").unwrap();
    if let Some(ref conn) = inner.tls_conn
      && let Some(version) = conn.protocol_version()
    {
      let name = match version {
        rustls::ProtocolVersion::TLSv1_2 => "TLSv1.2",
        rustls::ProtocolVersion::TLSv1_3 => "TLSv1.3",
        _ => "unknown",
      };
      let val = v8::String::new(scope, name).unwrap();
      out.set(scope, key.into(), val.into());
      return 0;
    }
    -1
  }

  /// Get the negotiated cipher suite info.
  /// Writes into out as { name: "...", version: "..." }.
  #[fast]
  fn get_cipher(
    &self,
    out: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    if let Some(ref conn) = inner.tls_conn
      && let Some(suite) = conn.negotiated_cipher_suite()
    {
      let (openssl_name, iana_name) = cipher_suite_to_names(suite.suite());

      let name_key =
        v8::String::new_external_onebyte_static(scope, b"name").unwrap();
      let name_str = v8::String::new(scope, openssl_name).unwrap();
      out.set(scope, name_key.into(), name_str.into());

      let standard_name_key =
        v8::String::new_external_onebyte_static(scope, b"standardName")
          .unwrap();
      let standard_name_str = v8::String::new(scope, iana_name).unwrap();
      out.set(scope, standard_name_key.into(), standard_name_str.into());

      if let Some(version) = conn.protocol_version() {
        let version_key =
          v8::String::new_external_onebyte_static(scope, b"version").unwrap();
        let version_str = match version {
          rustls::ProtocolVersion::TLSv1_2 => "TLSv1.2",
          rustls::ProtocolVersion::TLSv1_3 => "TLSv1.3",
          _ => "unknown",
        };
        let v = v8::String::new(scope, version_str).unwrap();
        out.set(scope, version_key.into(), v.into());
      }

      return 0;
    }
    -1
  }

  #[serde]
  fn get_peer_certificate_chain(&self) -> Option<PeerCertificateChain> {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    let conn = inner.tls_conn.as_ref()?;
    let certs = conn.peer_certificates()?;

    if certs.is_empty() {
      return None;
    }

    Some(PeerCertificateChain {
      certificates: certs
        .iter()
        .map(|cert| cert.as_ref().to_vec().into())
        .collect(),
    })
  }

  #[serde]
  fn get_peer_certificate(&self, detailed: bool) -> Option<CertificateObject> {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    let conn = inner.tls_conn.as_ref()?;
    let certs = conn.peer_certificates()?;
    let cert = certs.first()?;
    let cert = Certificate::from_der(cert.as_ref()).ok()?;
    cert.to_object(detailed).ok()
  }

  #[buffer]
  fn get_finished(&self) -> Option<Box<[u8]>> {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    if !inner.established {
      return None;
    }
    let conn = inner.tls_conn.as_ref()?;
    let mut output = vec![0u8; 32];
    // Note: rustls does not expose raw TLS Finished messages. We use
    // export_keying_material with role-based labels so that
    // server.getFinished() == client.getPeerFinished() and vice versa.
    // export_keying_material produces the same value on both sides for
    // the same label, so we use the local role's label here.
    let label = match inner.kind {
      Kind::Client => b"EXPORTER_DENO_TLS_FINISHED_CLIENT" as &[u8],
      Kind::Server => b"EXPORTER_DENO_TLS_FINISHED_SERVER" as &[u8],
    };
    conn.export_keying_material(&mut output, label, None).ok()?;
    Some(output.into_boxed_slice())
  }

  #[buffer]
  fn get_peer_finished(&self) -> Option<Box<[u8]>> {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    if !inner.established {
      return None;
    }
    let conn = inner.tls_conn.as_ref()?;
    let mut output = vec![0u8; 32];
    // Use the peer's role label so the values match across sides.
    let label = match inner.kind {
      Kind::Client => b"EXPORTER_DENO_TLS_FINISHED_SERVER" as &[u8],
      Kind::Server => b"EXPORTER_DENO_TLS_FINISHED_CLIENT" as &[u8],
    };
    conn.export_keying_material(&mut output, label, None).ok()?;
    Some(output.into_boxed_slice())
  }

  /// Check if the connection is established (handshake complete).
  #[fast]
  fn is_established(&self) -> bool {
    unsafe { &*self.inner.as_mut_ptr() }.established
  }

  // get_async_id and get_provider_type are inherited from AsyncWrap

  #[fast]
  fn get_bytes_read(&self) -> f64 {
    unsafe { &*self.inner.as_mut_ptr() }.bytes_read as f64
  }

  #[fast]
  fn get_bytes_written(&self) -> f64 {
    unsafe { &*self.inner.as_mut_ptr() }.bytes_written as f64
  }

  /// Set ALPN protocols on the pending TLS config.
  /// Accepts either a JS array of strings (e.g., ["h2", "http/1.1"])
  /// or a Buffer in Node.js wire-format (length-prefixed strings).
  /// Must be called before start() which creates the actual connection.
  #[nofast]
  #[reentrant]
  fn set_alpn_protocols(
    &self,
    protocols: v8::Local<v8::Value>,
    scope: &mut v8::PinScope,
  ) {
    let mut alpn = Vec::new();

    if let Ok(arr) = v8::Local::<v8::Array>::try_from(protocols) {
      // Array of strings: ["h2", "http/1.1"]
      for i in 0..arr.length() {
        if let Some(val) = arr.get_index(scope, i)
          && let Ok(s) = v8::Local::<v8::String>::try_from(val)
        {
          let len = s.utf8_length(scope);
          let mut buf = vec![0u8; len];
          s.write_utf8_v2(scope, &mut buf, v8::WriteFlags::default(), None);
          alpn.push(buf);
        }
      }
    } else if let Ok(uint8) = v8::Local::<v8::Uint8Array>::try_from(protocols) {
      // Wire format buffer: length-prefixed strings
      let len = uint8.byte_length();
      let mut data = vec![0u8; len];
      uint8.copy_contents(&mut data);
      let mut i = 0;
      while i < data.len() {
        let plen = data[i] as usize;
        i += 1;
        if i + plen > data.len() {
          break;
        }
        alpn.push(data[i..i + plen].to_vec());
        i += plen;
      }
    }

    if alpn.is_empty() {
      return;
    }

    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    // Apply to pending client config
    if let Some(ref config) = inner.pending_client_config {
      let mut new_config = rustls::ClientConfig::clone(config);
      new_config.alpn_protocols = alpn.clone();
      inner.pending_client_config = Some(Arc::new(new_config));
    }
    // Apply to pending server config
    if let Some(ref config) = inner.pending_server_config {
      let mut new_config = rustls::ServerConfig::clone(config);
      new_config.alpn_protocols = alpn;
      inner.pending_server_config = Some(Arc::new(new_config));
    }
  }

  /// Set the servername for SNI (client side).
  #[fast]
  fn set_servername(&self, #[string] name: &str) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    // If the connection hasn't started yet, update the pending server name
    // so SNI is correct when start() creates the ClientConnection.
    if !inner.started
      && let Ok(server_name) =
        rustls::pki_types::ServerName::try_from(name.to_string())
    {
      inner.pending_server_name = Some(server_name);
    }
    // After start(), this is a no-op — SNI is already set on the connection.
  }

  /// Enable the Acceptor-based server handshake path.
  /// Must be called before start(). When enabled, start() creates an
  /// Acceptor instead of a ServerConnection, allowing SNICallback and
  /// ALPNCallback to be invoked from JS before the handshake proceeds.
  #[fast]
  fn enable_client_hello_cb(&self) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.use_acceptor = true;
  }

  /// Get the server name (SNI) from the TLS connection.
  /// For server-side connections this returns the SNI sent by the client.
  /// Works both during the acceptor phase (from stored ClientHello info)
  /// and after the handshake (from the established connection).
  #[string]
  fn get_servername(&self) -> Option<String> {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    // Try established connection first
    if let Some(ref conn) = inner.tls_conn
      && let Some(name) = conn.server_name()
    {
      return Some(name.to_string());
    }
    // Fall back to client hello info (during acceptor phase)
    inner.client_hello_servername.clone()
  }

  /// Get the client's offered ALPN protocols from the ClientHello.
  /// Only available during the acceptor phase (after onclienthello fires).
  /// Returns a serialized Vec of strings.
  #[serde]
  fn get_client_hello_alpn(&self) -> Vec<String> {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    // ALPN protocol identifiers are opaque byte sequences per RFC 7301,
    // but all IANA-registered identifiers are ASCII. Non-UTF8 entries
    // are intentionally dropped since JS can't represent them as strings.
    inner
      .client_hello_alpn
      .iter()
      .filter_map(|p| std::str::from_utf8(p).ok().map(|s| s.to_string()))
      .collect()
  }

  /// Complete the Acceptor-based handshake after JS has processed
  /// SNICallback and ALPNCallback. Builds a per-connection ServerConfig
  /// from the provided SecureContext and ALPN selection, then creates
  /// the ServerConnection and continues the handshake.
  #[nofast]
  #[reentrant]
  fn finish_accept(
    &self,
    context: v8::Local<v8::Object>,
    alpn_protocol: v8::Local<v8::Value>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };

    let accepted = match inner.accepted.take() {
      Some(a) => a,
      None => {
        inner.error = Some("No pending Accepted".to_string());
        return -1;
      }
    };

    // Build server config from the provided SecureContext.
    // We borrow op_state in a limited scope so it is released before
    // any reentrant V8 calls (cycle / do_emit_error) which may trigger
    // prepare_stack_trace_callback, which also borrows op_state.
    let op_state_rc = deno_core::JsRuntime::op_state_from(scope);
    let (mut server_config, client_cert_verify_error) = {
      let mut op_state = op_state_rc.borrow_mut();
      match build_server_config(scope, context, &mut op_state) {
        Some(c) => c,
        None => {
          inner.error = Some("Failed to build server config".to_string());
          return -1;
        }
      }
    };
    inner.verify_error = client_cert_verify_error;

    // Determine ALPN protocols
    if let Ok(s) = v8::Local::<v8::String>::try_from(alpn_protocol) {
      // ALPNCallback selected a specific protocol
      let len = s.utf8_length(scope);
      let mut buf = vec![0u8; len];
      s.write_utf8_v2(scope, &mut buf, v8::WriteFlags::default(), None);
      server_config.alpn_protocols = vec![buf];
    } else if let Some(ref config) = inner.pending_server_config {
      // No ALPNCallback result; use static ALPN from original config
      server_config.alpn_protocols = config.alpn_protocols.clone();
    }

    // Create the ServerConnection from the Accepted + config
    match accepted.into_connection(Arc::new(server_config)) {
      Ok(conn) => {
        inner.tls_conn = Some(TlsConnection::Server(conn));
      }
      Err((e, mut alert)) => {
        let (error_msg, error_code) = rustls_error_to_node_error(&e, None);
        inner.error = Some(error_msg.clone());
        // Flush the TLS alert to the client so it sees a proper
        // handshake_failure instead of ECONNRESET.
        let mut alert_bytes = Vec::new();
        let _ = alert.write_all(&mut alert_bytes);
        if !alert_bytes.is_empty() {
          inner.pending_enc_out.extend_from_slice(&alert_bytes);
          if inner.underlying.is_attached()
            && let UnderlyingStream::Uv { .. } = inner.underlying
          {
            // `finish_accept` is an `&self` op that does not hold the OpState
            // borrow, so a synchronous write failure completes synchronously.
            inner.enc_out_uv(WriteCompletion::Sync);
          }
        }
        let inner_ptr = inner as *mut TLSWrapInner;
        unsafe {
          if let Some(ctx) = extract_emit_ctx(inner_ptr) {
            do_emit_error(&ctx, &error_msg, &error_code);
          }
        }
        return -1;
      }
    }

    // Drive the state machine to continue the handshake
    let inner_ptr = inner as *mut TLSWrapInner;
    unsafe { TLSWrapInner::cycle(inner_ptr) };

    0
  }

  /// Inject encrypted data (for testing / JSStreamSocket integration).
  /// Mirrors Node's TLSWrap::Receive().
  #[fast]
  #[reentrant]
  fn receive(&self, #[buffer] data: &[u8]) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.enc_in.extend_from_slice(data);
    let inner_ptr = inner as *mut TLSWrapInner;
    // SAFETY: inner_ptr points to heap-allocated TLSWrapInner via OwnedPtr
    unsafe { TLSWrapInner::cycle(inner_ptr) };
  }

  /// Get verification error code, if any. Returns empty string if no error.
  /// The JS wrapper converts this to an Error object.
  #[string]
  fn verify_error(&self) -> String {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    inner
      .verify_error
      .lock()
      .unwrap_or_else(|e| e.into_inner())
      .clone()
      .unwrap_or_default()
  }

  /// Set verify mode (requestCert, rejectUnauthorized).
  /// With rustls, certificate verification is configured at the
  /// ClientConfig/ServerConfig level, so this is mostly a no-op.
  #[fast]
  fn set_verify_mode(&self, _request_cert: bool, _reject_unauthorized: bool) {
    // Handled by rustls config
  }

  /// Enable session callbacks. Currently a no-op since rustls handles
  /// session resumption internally.
  #[fast]
  fn enable_session_callbacks(&self) {
    // No-op for rustls
  }

  /// Allow or disallow offering cached sessions for resumption on this
  /// connection. In Node.js, client connections only attempt session
  /// resumption when `options.session` is provided (or `setSession()` is
  /// called); the JS layer validates the session buffer against the
  /// destination and toggles this flag on the underlying
  /// `NodeClientSessionStoreWrapper`.
  #[fast]
  fn set_session_allowed(&self, allowed: bool) {
    let inner = unsafe { &*self.inner.as_ptr() };
    inner.allow_resumption.store(allowed, Ordering::Relaxed);
  }

  /// Check if the TLS session was resumed (reused from a previous connection).
  #[fast]
  fn is_session_reused(&self) -> bool {
    let inner = unsafe { &*self.inner.as_mut_ptr() };
    if let Some(ref conn) = inner.tls_conn {
      matches!(conn.handshake_kind(), Some(rustls::HandshakeKind::Resumed))
    } else {
      false
    }
  }

  // -------------------------------------------------------------------------
  // JSStreamSocket support — attach to a JS-backed stream instead of TCP
  // -------------------------------------------------------------------------

  /// Attach to a JS-backed stream (e.g. JSStreamSocket wrapping a Duplex).
  /// Instead of a uv_stream_t, I/O goes through JS callbacks:
  ///   - Encrypted reads: JS calls receive() to inject data
  ///   - Encrypted writes: Rust calls handle.encOut(data) to send data
  #[nofast]
  fn attach_js_stream(
    &self,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };

    let loop_ = &**op_state.borrow::<Box<uv_compat::uv_loop_t>>()
      as *const uv_compat::uv_loop_t
      as *mut uv_compat::uv_loop_t;

    inner.underlying = UnderlyingStream::Js { loop_ptr: loop_ };
    // SAFETY: scope is valid for the current isolate
    inner.isolate = Some(unsafe { scope.as_raw_isolate_ptr() });

    // Get stream_base_state from OpState
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    inner.stream_base_state =
      Some(v8::Global::new(scope, v8::Local::new(scope, state_global)));

    0
  }

  /// Inject encrypted data from JS (JSStreamSocket read path).
  /// Called when the underlying JS Duplex stream receives data.
  /// This is the same as receive() but named to match Node's ReadBuffer.
  #[fast]
  #[reentrant]
  fn read_buffer(&self, #[buffer] data: &[u8]) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.enc_in.extend_from_slice(data);
    let inner_ptr = inner as *mut TLSWrapInner;
    // SAFETY: inner_ptr points to heap-allocated TLSWrapInner via OwnedPtr
    unsafe { TLSWrapInner::cycle(inner_ptr) };
  }

  /// Drain buffered encrypted output for JS streams (pull-based).
  /// Returns the encrypted data that needs to be written to the
  /// underlying JS stream. Called by JS after operations that may
  /// produce encrypted output (readBuffer, start, writes).
  /// Write completion callbacks are handled by the existing
  /// cycle() -> InvokeQueued path (fired from reentrant ops like
  /// readBuffer/start, not from write ops where in_dowrite is true).
  #[buffer]
  fn drain_enc_out(&self) -> Box<[u8]> {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    if !matches!(inner.underlying, UnderlyingStream::Js { .. })
      || inner.pending_enc_out.is_empty()
    {
      return Box::new([]);
    }
    std::mem::take(&mut inner.pending_enc_out).into_boxed_slice()
  }

  /// Signal EOF on the encrypted input (JSStreamSocket path).
  /// Called when the underlying JS Duplex stream ends.
  #[fast]
  #[reentrant]
  fn emit_eof(&self) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    if inner.eof {
      return;
    }
    // Drain any buffered TLS state *before* setting eof, because
    // clear_out_process() bails early when self.eof is true.
    let result = inner.clear_out_process();
    inner.eof = true;
    let inner_ptr = inner as *mut TLSWrapInner;
    unsafe {
      TLSWrapInner::dispatch_clear_out_callbacks(inner_ptr, &result);
      if (*inner_ptr).onread.is_none() {
        // readStop is active -- defer EOF
        (*inner_ptr).pending_eof = true;
        (*inner_ptr).has_buffered_cleartext = true;
      } else if let Some(ctx) = extract_emit_ctx(inner_ptr) {
        let onread = (*inner_ptr).onread.clone();
        let state = (*inner_ptr).stream_base_state.clone();
        do_emit_read(
          &ctx,
          onread.as_ref(),
          state.as_ref(),
          deno_core::uv_compat::UV_EOF as isize,
          None,
        );
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Helper: build rustls configs from SecureContext JS object { ca, cert, key }
// ---------------------------------------------------------------------------

fn get_js_string(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> Option<String> {
  let k = v8::String::new(scope, key).unwrap();
  obj.get(scope, k.into()).and_then(|v| {
    if v.is_undefined() || v.is_null() {
      None
    } else {
      v.to_string(scope).map(|s| s.to_rust_string_lossy(scope))
    }
  })
}

fn get_js_bool(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  key: &str,
  default: bool,
) -> bool {
  let k = v8::String::new(scope, key).unwrap();
  obj
    .get(scope, k.into())
    .and_then(|v| {
      if v.is_undefined() || v.is_null() {
        None
      } else {
        Some(v.boolean_value(scope))
      }
    })
    .unwrap_or(default)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProtocolVersionSelection {
  Default,
  Tls12Only,
  Tls13Only,
  Unsupported,
}

fn protocol_version_number(version: &str) -> Option<i32> {
  match version {
    "TLSv1" => Some(0x0301),
    "TLSv1.1" => Some(0x0302),
    "TLSv1.2" => Some(0x0303),
    "TLSv1.3" => Some(0x0304),
    _ => None,
  }
}

fn get_protocol_versions(
  scope: &mut v8::PinScope,
  context: v8::Local<v8::Object>,
) -> ProtocolVersionSelection {
  let min_version = get_js_string(scope, context, "minVersion")
    .unwrap_or_else(|| "TLSv1.2".to_string());
  let max_version = get_js_string(scope, context, "maxVersion")
    .unwrap_or_else(|| "TLSv1.3".to_string());

  let Some(min) = protocol_version_number(&min_version) else {
    return ProtocolVersionSelection::Default;
  };
  let Some(max) = protocol_version_number(&max_version) else {
    return ProtocolVersionSelection::Default;
  };

  let allow_tls12 = min <= 0x0303 && max >= 0x0303;
  let allow_tls13 = min <= 0x0304 && max >= 0x0304;

  match (allow_tls12, allow_tls13) {
    (true, true) => ProtocolVersionSelection::Default,
    (true, false) => ProtocolVersionSelection::Tls12Only,
    (false, true) => ProtocolVersionSelection::Tls13Only,
    (false, false) => ProtocolVersionSelection::Unsupported,
  }
}

/// Shared storage for certificate verification errors.
/// The verifier stores errors here instead of failing the handshake,
/// and `verifyError()` reads them later — matching Node/OpenSSL behavior.
type VerifyErrorStore = Arc<std::sync::Mutex<Option<String>>>;

thread_local! {
  /// Per-connection cert-verification error sink, set just before each
  /// synchronous rustls handshake step (see `TLSWrapInner::cycle`) and
  /// cleared on the way out.  The cert verifier writes here instead of to
  /// its own field so that one process-wide cached `NodeServerCertVerifier`
  /// instance can serve many `TLSSocket` connections without their
  /// `verifyError()` results aliasing each other through the verifier's
  /// `Arc<Mutex<...>>` field.
  static CURRENT_VERIFY_ERROR: std::cell::RefCell<Option<VerifyErrorStore>> =
    const { std::cell::RefCell::new(None) };
}

/// RAII guard that sets `CURRENT_VERIFY_ERROR` for the lifetime of a sync
/// rustls call (e.g. `process_new_packets`) and clears it on drop, so the
/// verifier callback writes to *this* connection's error slot.
struct VerifyErrorScope {
  prev: Option<VerifyErrorStore>,
}

impl VerifyErrorScope {
  fn enter(store: VerifyErrorStore) -> Self {
    let prev = CURRENT_VERIFY_ERROR.with(|c| c.borrow_mut().replace(store));
    VerifyErrorScope { prev }
  }
}

impl Drop for VerifyErrorScope {
  fn drop(&mut self) {
    let prev = self.prev.take();
    CURRENT_VERIFY_ERROR.with(|c| *c.borrow_mut() = prev);
  }
}

fn store_verify_error(fallback: &VerifyErrorStore, code: String) {
  let stored = CURRENT_VERIFY_ERROR.with(|c| c.borrow().clone());
  let target = stored.as_ref().unwrap_or(fallback);
  *target.lock().unwrap_or_else(|e| e.into_inner()) = Some(code);
}

/// A certificate verifier for Node.js compatibility.
///
/// Unlike rustls's default WebPKI verifier, this does NOT abort the
/// TLS handshake on certificate errors.  Instead it stores the error
/// so that `verifyError()` can report it to JS after the handshake.
/// This matches OpenSSL/Node behaviour where certificate verification
/// errors are deferred.
///
/// Server-name checks are skipped because Node performs them in JS
/// via `checkServerIdentity`.
#[derive(Debug)]
struct NodeServerCertVerifier {
  inner: Arc<rustls::client::WebPkiServerVerifier>,
  verify_error: VerifyErrorStore,
  /// True for an explicit `ca: []`. rustls/webpki cannot build its verifier
  /// with no roots, so `inner` uses fallback roots only to classify
  /// certificate-specific failures. Otherwise-valid chains are still recorded
  /// as unauthorized below.
  empty_explicit_ca: bool,
  /// Raw DER bytes of every root certificate so we can check whether a
  /// `CaUsedAsEndEntity` cert is actually trusted.
  root_cert_ders: Vec<Vec<u8>>,
  /// When true (the `rejectUnauthorized: true` path), cert errors fail the
  /// handshake at the rustls layer so rustls never caches a session that a
  /// later strict connection could resume without re-running validation.
  /// When false, errors are stored in `verify_error` and the JS layer chooses
  /// whether to destroy based on `rejectUnauthorized`. Name-mismatch errors
  /// are always deferred to JS `checkServerIdentity` regardless.
  strict_verify: bool,
}

/// Map a rustls CipherSuite to (OpenSSL name, IANA name).
/// Node's getCipher() returns { name: <OpenSSL>, standardName: <IANA>, version }.
fn cipher_suite_to_names(
  suite: rustls::CipherSuite,
) -> (&'static str, &'static str) {
  use rustls::CipherSuite as CS;
  match suite {
    // TLS 1.3 — OpenSSL and IANA names are the same
    CS::TLS13_AES_128_GCM_SHA256 => {
      ("TLS_AES_128_GCM_SHA256", "TLS_AES_128_GCM_SHA256")
    }
    CS::TLS13_AES_256_GCM_SHA384 => {
      ("TLS_AES_256_GCM_SHA384", "TLS_AES_256_GCM_SHA384")
    }
    CS::TLS13_CHACHA20_POLY1305_SHA256 => (
      "TLS_CHACHA20_POLY1305_SHA256",
      "TLS_CHACHA20_POLY1305_SHA256",
    ),
    // TLS 1.2 ECDHE-RSA
    CS::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256 => (
      "ECDHE-RSA-AES128-GCM-SHA256",
      "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
    ),
    CS::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384 => (
      "ECDHE-RSA-AES256-GCM-SHA384",
      "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    ),
    CS::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256 => (
      "ECDHE-RSA-CHACHA20-POLY1305",
      "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
    ),
    // TLS 1.2 ECDHE-ECDSA
    CS::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256 => (
      "ECDHE-ECDSA-AES128-GCM-SHA256",
      "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
    ),
    CS::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384 => (
      "ECDHE-ECDSA-AES256-GCM-SHA384",
      "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
    ),
    CS::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256 => (
      "ECDHE-ECDSA-CHACHA20-POLY1305",
      "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
    ),
    _ => {
      // Fallback: use the Debug representation for both
      // This shouldn't happen with rustls's default config
      ("unknown", "unknown")
    }
  }
}

/// webpki refuses an X.509v1 certificate at parse time with this error.
/// OpenSSL, and therefore Node, parses and verifies v1 certificates normally.
fn is_unsupported_cert_version(err: &rustls::CertificateError) -> bool {
  matches!(
    err,
    rustls::CertificateError::Other(other) if other
      .0
      .downcast_ref::<webpki::Error>()
      .is_some_and(|e| matches!(e, webpki::Error::UnsupportedCertVersion))
  )
}

/// As [`is_unsupported_cert_version`], for a whole `rustls::Error`.
fn is_unsupported_cert_version_error(err: &rustls::Error) -> bool {
  matches!(
    err,
    rustls::Error::InvalidCertificate(cert_err)
      if is_unsupported_cert_version(cert_err)
  )
}

/// The signature verification algorithms used wherever this file verifies a
/// signature itself rather than going through webpki. This is the same
/// `aws_lc_rs` provider rustls is configured with elsewhere in this file, so
/// both paths accept exactly the same set of algorithms.
fn supported_signature_algorithms()
-> &'static rustls::crypto::WebPkiSupportedAlgorithms {
  static ALGORITHMS: std::sync::OnceLock<
    rustls::crypto::WebPkiSupportedAlgorithms,
  > = std::sync::OnceLock::new();
  ALGORITHMS.get_or_init(|| {
    rustls::crypto::aws_lc_rs::default_provider()
      .signature_verification_algorithms
  })
}

/// Verify a `CertificateVerify` signature, falling back to a direct
/// public-key verification when webpki could not parse the certificate.
///
/// Only webpki's *parser* rejects X.509v1; the signature primitives are the
/// same ones it would use. Extract the `SubjectPublicKeyInfo` and verify
/// against it, so that a peer still has to hold the private key belonging to
/// the certificate it presented.
fn verify_handshake_signature_allowing_v1(
  inner_result: Result<
    rustls::client::danger::HandshakeSignatureValid,
    rustls::Error,
  >,
  message: &[u8],
  cert: &rustls::pki_types::CertificateDer<'_>,
  dss: &rustls::DigitallySignedStruct,
  tls13: bool,
) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
  let err = match inner_result {
    Err(err) if is_unsupported_cert_version_error(&err) => err,
    other => return other,
  };
  match verify_signature_with_unparsed_cert(
    cert.as_ref(),
    dss.scheme,
    message,
    dss.signature(),
    tls13,
  ) {
    SignatureCheck::Valid => {
      Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    SignatureCheck::Invalid => Err(rustls::Error::InvalidCertificate(
      rustls::CertificateError::BadSignature,
    )),
    // Nothing was actually checked, so report webpki's original refusal
    // rather than inventing a verdict.
    SignatureCheck::Unchecked => Err(err),
  }
}

/// Outcome of [`verify_signature_with_unparsed_cert`]. Only `Valid` may be
/// treated as success.
#[derive(Debug, PartialEq, Eq)]
enum SignatureCheck {
  Valid,
  /// The signature did not verify under the certificate's public key.
  Invalid,
  /// The certificate could not be parsed, or no algorithm in the provider
  /// covers this signature scheme together with this public key, so the
  /// signature was not checked at all.
  Unchecked,
}

/// Verify `signature` over `message` using the public key of a certificate
/// webpki would not parse, choosing the algorithm from `scheme` the way
/// rustls's own `verify_tls1{2,3}_signature` does.
fn verify_signature_with_unparsed_cert(
  cert: &[u8],
  scheme: rustls::SignatureScheme,
  message: &[u8],
  signature: &[u8],
  tls13: bool,
) -> SignatureCheck {
  let Some(parsed) = parse_certificate(cert) else {
    return SignatureCheck::Unchecked;
  };
  let Some((_, candidates)) = supported_signature_algorithms()
    .mapping
    .iter()
    .find(|(mapped, _)| *mapped == scheme)
  else {
    return SignatureCheck::Unchecked;
  };
  // TLS 1.3 uses only the first algorithm mapped to a scheme while TLS 1.2
  // tries each in turn, mirroring rustls's own `verify_tls1{2,3}_signature`.
  let candidates = match tls13 {
    true => &candidates[..1.min(candidates.len())],
    false => *candidates,
  };
  let mut checked = false;
  for algorithm in candidates {
    if algorithm.public_key_alg_id().as_ref() != parsed.spki_algorithm {
      continue;
    }
    checked = true;
    if algorithm
      .verify_signature(parsed.spki_key, message, signature)
      .is_ok()
    {
      return SignatureCheck::Valid;
    }
  }
  match checked {
    true => SignatureCheck::Invalid,
    false => SignatureCheck::Unchecked,
  }
}

/// `CertifiedKey::keys_match` parses the end-entity certificate with webpki,
/// which refuses X.509v1. Compare the `SubjectPublicKeyInfo` bytes directly in
/// that case, which is what `keys_match` itself does once webpki has parsed
/// the certificate, so the certificate/key pairing is still enforced.
fn keys_match_allowing_v1(
  certified_key: &rustls::sign::CertifiedKey,
  signing_key: &dyn rustls::sign::SigningKey,
) -> Result<(), rustls::Error> {
  let err = match certified_key.keys_match() {
    Err(err) if is_unsupported_cert_version_error(&err) => err,
    other => return other,
  };
  let end_entity = certified_key.end_entity_cert()?;
  let Some(key_spki) = signing_key.public_key() else {
    return Err(rustls::Error::InconsistentKeys(
      rustls::InconsistentKeys::Unknown,
    ));
  };
  let Some(parsed) = parse_certificate(end_entity.as_ref()) else {
    return Err(err);
  };
  match key_spki.as_ref() == parsed.spki {
    true => Ok(()),
    false => Err(rustls::Error::InconsistentKeys(
      rustls::InconsistentKeys::KeyMismatch,
    )),
  }
}

// ---------------------------------------------------------------------------
// X.509 parsing and chain verification for certificates webpki will not parse.
//
// webpki rejects X.509v1 at parse time (`UnsupportedCertVersion`) while
// OpenSSL, and therefore Node, accepts it; several upstream Node test fixtures
// are genuinely v1. To keep that parity without giving up verification, parse
// just enough of the certificate here and run the checks OpenSSL's
// `X509_verify_cert` runs with default flags: the issuer's signature over each
// `tbsCertificate`, the validity window, and `basicConstraints` / `keyUsage`
// on every issuer that carries them.
//
// Distinguished names select candidate issuers, exactly as OpenSSL does. A
// name match on its own never grants trust: a subject DN is public
// information, so the signature is what decides.
//
// `nameConstraints`, `policyConstraints` and `inhibitAnyPolicy` are not
// evaluated, so a certificate carrying any of them is refused outright rather
// than treated as unconstrained, as is one carrying a critical extension this
// code does not recognise: an unevaluated restriction must never read as an
// absent one. Revocation is not checked, matching the non-v1 path here.
//
// The trust anchors reachable here are always ones the caller supplied
// explicitly (the per-context `ca` option or
// `tls.setDefaultCACertificates()`), never the bundled Mozilla roots. Names
// are compared as raw DER rather than canonicalised as OpenSSL's
// `X509_NAME_cmp` does, which can only reject a chain OpenSSL would accept,
// never the reverse.
// ---------------------------------------------------------------------------

/// A DER tag-length-value element.
struct DerElement<'a> {
  tag: u8,
  /// The complete element, including its tag and length header.
  all: &'a [u8],
  /// The element's contents, excluding its tag and length header.
  content: &'a [u8],
}

/// Read one DER element from the front of `data`, returning it and the
/// remainder. Rejects the encodings that cannot appear in the X.509 structures
/// parsed here: high-tag-number form, indefinite length, non-minimal length.
fn der_next(data: &[u8]) -> Option<(DerElement<'_>, &[u8])> {
  let tag = *data.first()?;
  if tag & 0x1f == 0x1f {
    return None;
  }
  let first = *data.get(1)?;
  let (content_len, header_len) = if first < 0x80 {
    (first as usize, 2)
  } else {
    let count = (first & 0x7f) as usize;
    if count == 0 || count > 4 {
      return None;
    }
    let bytes = data.get(2..2 + count)?;
    if bytes[0] == 0 {
      return None;
    }
    let mut len = 0usize;
    for byte in bytes {
      len = (len << 8) | (*byte as usize);
    }
    if len < 0x80 {
      return None;
    }
    (len, 2 + count)
  };
  let all = data.get(..header_len.checked_add(content_len)?)?;
  Some((
    DerElement {
      tag,
      all,
      content: &all[header_len..],
    },
    &data[all.len()..],
  ))
}

/// As [`der_next`], requiring a specific tag.
fn der_expect(data: &[u8], tag: u8) -> Option<(DerElement<'_>, &[u8])> {
  let (element, rest) = der_next(data)?;
  (element.tag == tag).then_some((element, rest))
}

/// The contents of a BIT STRING that has no unused trailing bits, matching
/// webpki's `bit_string_with_no_unused_bits`.
fn der_bit_string<'a>(element: &DerElement<'a>) -> Option<&'a [u8]> {
  (element.tag == 0x03 && element.content.first() == Some(&0))
    .then(|| &element.content[1..])
}

/// The value octets of a BIT STRING that may have unused trailing bits, as a
/// named-bit string such as `KeyUsage` uses. Rejects what DER forbids: an
/// unused-bit count above 7, a count with no octets for it to apply to, and
/// unused bits that are not zero. Without that last check a `keyCertSign` bit
/// sitting in the unused region would read as asserted.
fn der_named_bits<'a>(element: &DerElement<'a>) -> Option<&'a [u8]> {
  if element.tag != 0x03 {
    return None;
  }
  let (&unused, bits) = element.content.split_first()?;
  if unused > 7 || (unused > 0 && bits.is_empty()) {
    return None;
  }
  if unused > 0 && bits.last()? & ((1u8 << unused) - 1) != 0 {
    return None;
  }
  Some(bits)
}

/// A DER BOOLEAN. DER permits exactly one content octet, `0x00` or `0xff`.
fn der_boolean(element: &DerElement<'_>) -> Option<bool> {
  match element.content {
    [0x00] => Some(false),
    [0xff] => Some(true),
    _ => None,
  }
}

/// A non-negative DER INTEGER, as a `u64`. `None` for anything wider, which
/// callers treat as "no constraint".
fn der_unsigned(element: &DerElement<'_>) -> Option<u64> {
  if element.content.is_empty() || element.content[0] & 0x80 != 0 {
    return None;
  }
  let mut value = 0u64;
  for byte in element.content {
    value = value.checked_mul(256)?.checked_add(*byte as u64)?;
  }
  Some(value)
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
  let year = if month <= 2 { year - 1 } else { year };
  let era = if year >= 0 { year } else { year - 399 } / 400;
  let year_of_era = year - era * 400;
  let shifted_month = (month + 9) % 12;
  let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
  let day_of_era =
    year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
  era * 146097 + day_of_era - 719468
}

/// Seconds since the Unix epoch for a DER UTCTime or GeneralizedTime, in the
/// only forms RFC 5280 permits: `YYMMDDHHMMSSZ` and `YYYYMMDDHHMMSSZ`.
fn der_time_secs(element: &DerElement<'_>) -> Option<i64> {
  fn digits(bytes: &[u8]) -> Option<i64> {
    let mut value = 0i64;
    for byte in bytes {
      if !byte.is_ascii_digit() {
        return None;
      }
      value = value * 10 + (*byte - b'0') as i64;
    }
    Some(value)
  }
  let (year, rest) = match (element.tag, element.content.len()) {
    // UTCTime: a two-digit year, where 50..=99 means 19xx (RFC 5280 4.1.2.5.1).
    (0x17, 13) => {
      let year = digits(&element.content[..2])?;
      let year = if year < 50 { 2000 + year } else { 1900 + year };
      (year, &element.content[2..])
    }
    (0x18, 15) => (digits(&element.content[..4])?, &element.content[4..]),
    _ => return None,
  };
  if rest.last() != Some(&b'Z') {
    return None;
  }
  let month = digits(&rest[0..2])?;
  let day = digits(&rest[2..4])?;
  let hour = digits(&rest[4..6])?;
  let minute = digits(&rest[6..8])?;
  let second = digits(&rest[8..10])?;
  if !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 60 {
    return None;
  }
  let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
  let month_days = [
    31,
    if leap { 29 } else { 28 },
    31,
    30,
    31,
    30,
    31,
    31,
    30,
    31,
    30,
    31,
  ];
  if day < 1 || day > month_days[(month - 1) as usize] {
    return None;
  }
  Some(
    days_from_civil(year, month, day) * 86400
      + hour * 3600
      + minute * 60
      + second,
  )
}

/// The parts of an X.509 certificate needed to verify a certification path.
struct ParsedCertificate<'a> {
  /// The complete `TBSCertificate` element, which is what the signature of
  /// this certificate covers.
  tbs: &'a [u8],
  /// Contents of the `signatureAlgorithm` SEQUENCE, in the form
  /// `SignatureVerificationAlgorithm::signature_alg_id` returns.
  signature_algorithm: &'a [u8],
  /// Contents of the `signature` SEQUENCE inside `tbsCertificate`, which
  /// RFC 5280 requires to equal `signature_algorithm`. Unlike the outer one
  /// this copy is covered by the signature.
  tbs_signature_algorithm: &'a [u8],
  /// The `signatureValue`, with the BIT STRING's unused-bits octet removed.
  signature: &'a [u8],
  issuer: &'a [u8],
  subject: &'a [u8],
  not_before: i64,
  not_after: i64,
  /// The complete `SubjectPublicKeyInfo` element.
  spki: &'a [u8],
  /// Contents of the `SubjectPublicKeyInfo` `AlgorithmIdentifier` SEQUENCE, in
  /// the form `SignatureVerificationAlgorithm::public_key_alg_id` returns.
  spki_algorithm: &'a [u8],
  /// The `subjectPublicKey`, with the BIT STRING's unused-bits octet removed.
  spki_key: &'a [u8],
  extensions: CertExtensions,
}

/// The extension-derived facts chain verification needs. `None` means the
/// extension is absent, which for an X.509v1 certificate is always the case.
#[derive(Debug, Default, PartialEq, Eq)]
struct CertExtensions {
  /// `cA` from `basicConstraints`.
  is_ca: Option<bool>,
  /// `pathLenConstraint` from `basicConstraints`.
  path_len: Option<u64>,
  /// Whether `keyUsage` asserts `keyCertSign`.
  key_cert_sign: Option<bool>,
  /// Whether `extendedKeyUsage` permits TLS server authentication, either
  /// through `id-kp-serverAuth` or `anyExtendedKeyUsage`.
  server_auth: Option<bool>,
  /// Set when the certificate carries an extension that restricts what it is
  /// allowed to certify and that this verifier does not evaluate. Such a
  /// certificate is refused rather than used as if it were unconstrained.
  unhandled_constraint: bool,
}

/// Parse the extensions this verifier understands out of the `extensions [3]`
/// field of a `TBSCertificate`.
///
/// Everything here fails closed. A malformed value, or a repeat of an
/// extension, makes the whole certificate unparseable and therefore rejected,
/// rather than leaving a constraint half-read: RFC 5280 forbids duplicates and
/// OpenSSL marks such a certificate invalid, and silently preferring one copy
/// over another is how a `CA:FALSE, CA:TRUE` pair would end up read as a CA.
fn parse_extensions(data: &[u8]) -> Option<CertExtensions> {
  const BASIC_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x13]; // 2.5.29.19
  const KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x0f]; // 2.5.29.15
  const EXTENDED_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x25]; // 2.5.29.37
  const NAME_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x1e]; // 2.5.29.30
  const POLICY_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x24]; // 2.5.29.36
  const INHIBIT_ANY_POLICY: &[u8] = &[0x55, 0x1d, 0x36]; // 2.5.29.54
  // id-kp-serverAuth, 1.3.6.1.5.5.7.3.1
  const SERVER_AUTH: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x01];
  // anyExtendedKeyUsage, 2.5.29.37.0
  const ANY_EXTENDED_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x25, 0x00];
  /// Extensions that may be marked critical and still safely ignored here:
  /// the ones parsed below, the name extensions (`subjectAltName` is checked
  /// by rustls and by `checkServerIdentity` in JS, not here), the key and
  /// issuer identifiers, and the informational ones. Anything else carrying
  /// the critical bit is a restriction this code cannot evaluate.
  const IGNORABLE_WHEN_CRITICAL: &[&[u8]] = &[
    &[0x55, 0x1d, 0x0e], // subjectKeyIdentifier
    &[0x55, 0x1d, 0x0f], // keyUsage
    &[0x55, 0x1d, 0x11], // subjectAltName
    &[0x55, 0x1d, 0x12], // issuerAltName
    &[0x55, 0x1d, 0x13], // basicConstraints
    &[0x55, 0x1d, 0x1f], // cRLDistributionPoints
    &[0x55, 0x1d, 0x20], // certificatePolicies
    &[0x55, 0x1d, 0x23], // authorityKeyIdentifier
    &[0x55, 0x1d, 0x25], // extendedKeyUsage
    &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x01, 0x01], // authorityInfoAccess
  ];

  let mut out = CertExtensions::default();
  let (extensions, rest) = der_expect(data, 0x30)?;
  if !rest.is_empty() {
    return None;
  }
  let mut cursor = extensions.content;
  while !cursor.is_empty() {
    let (extension, next) = der_expect(cursor, 0x30)?;
    cursor = next;
    let (oid, after_oid) = der_expect(extension.content, 0x06)?;
    // critical BOOLEAN DEFAULT FALSE
    let mut critical = false;
    let after_critical = match der_next(after_oid) {
      Some((element, rest)) if element.tag == 0x01 => {
        critical = der_boolean(&element)?;
        rest
      }
      _ => after_oid,
    };
    if critical && !IGNORABLE_WHEN_CRITICAL.contains(&oid.content) {
      out.unhandled_constraint = true;
    }
    let (value, rest) = der_expect(after_critical, 0x04)?;
    if !rest.is_empty() {
      return None;
    }
    match oid.content {
      BASIC_CONSTRAINTS => {
        if out.is_ca.is_some() {
          return None;
        }
        let (constraints, rest) = der_expect(value.content, 0x30)?;
        if !rest.is_empty() {
          return None;
        }
        let mut inner = constraints.content;
        let mut is_ca = false;
        if let Some((element, rest)) = der_next(inner)
          && element.tag == 0x01
        {
          is_ca = der_boolean(&element)?;
          inner = rest;
        }
        out.is_ca = Some(is_ca);
        if !inner.is_empty() {
          // An unreadable pathLenConstraint must not read as "unconstrained".
          let (element, rest) = der_expect(inner, 0x02)?;
          out.path_len = Some(der_unsigned(&element)?);
          if !rest.is_empty() {
            return None;
          }
        }
      }
      KEY_USAGE => {
        if out.key_cert_sign.is_some() {
          return None;
        }
        let (bits, rest) = der_expect(value.content, 0x03)?;
        if !rest.is_empty() {
          return None;
        }
        // KeyUsage is a BIT STRING whose bit 0 is the most significant bit of
        // the first value octet, so keyCertSign (bit 5) is mask 0x04. A BIT
        // STRING too short to reach bit 5 reads as not asserted.
        let bits = der_named_bits(&bits)?;
        let first = bits.first().copied().unwrap_or(0);
        out.key_cert_sign = Some(first & 0x04 != 0);
      }
      EXTENDED_KEY_USAGE => {
        if out.server_auth.is_some() {
          return None;
        }
        let (purposes, rest) = der_expect(value.content, 0x30)?;
        if !rest.is_empty() {
          return None;
        }
        let mut inner = purposes.content;
        let mut server_auth = false;
        while !inner.is_empty() {
          let (purpose, rest) = der_expect(inner, 0x06)?;
          inner = rest;
          if purpose.content == SERVER_AUTH
            || purpose.content == ANY_EXTENDED_KEY_USAGE
          {
            server_auth = true;
          }
        }
        out.server_auth = Some(server_auth);
      }
      NAME_CONSTRAINTS | POLICY_CONSTRAINTS | INHIBIT_ANY_POLICY => {
        out.unhandled_constraint = true;
      }
      _ => {}
    }
  }
  Some(out)
}

/// Parse an X.509 certificate, including the v1 form webpki rejects.
fn parse_certificate(der: &[u8]) -> Option<ParsedCertificate<'_>> {
  let (certificate, trailing) = der_expect(der, 0x30)?;
  if !trailing.is_empty() {
    return None;
  }
  let (tbs, rest) = der_expect(certificate.content, 0x30)?;
  let (signature_algorithm, rest) = der_expect(rest, 0x30)?;
  let (signature, rest) = der_next(rest)?;
  if !rest.is_empty() {
    return None;
  }

  // version [0] EXPLICIT is absent in v1.
  let mut cursor = tbs.content;
  if cursor.first() == Some(&0xa0) {
    cursor = der_next(cursor)?.1;
  }
  let (_serial, cursor) = der_expect(cursor, 0x02)?;
  let (tbs_signature_algorithm, cursor) = der_expect(cursor, 0x30)?;
  let (issuer, cursor) = der_expect(cursor, 0x30)?;
  let (validity, cursor) = der_expect(cursor, 0x30)?;
  let (subject, cursor) = der_expect(cursor, 0x30)?;
  let (spki, mut cursor) = der_expect(cursor, 0x30)?;

  let (not_before, rest) = der_next(validity.content)?;
  let (not_after, rest) = der_next(rest)?;
  if !rest.is_empty() {
    return None;
  }

  let (spki_algorithm, rest) = der_expect(spki.content, 0x30)?;
  let (spki_key, rest) = der_next(rest)?;
  if !rest.is_empty() {
    return None;
  }

  // Remaining optional fields: issuerUniqueID [1], subjectUniqueID [2] and
  // extensions [3]. v1 certificates have none of them.
  let mut extensions = CertExtensions::default();
  let mut seen_extensions = false;
  while !cursor.is_empty() {
    let (element, next) = der_next(cursor)?;
    if element.tag == 0xa3 {
      if seen_extensions {
        return None;
      }
      seen_extensions = true;
      extensions = parse_extensions(element.content)?;
    }
    cursor = next;
  }

  Some(ParsedCertificate {
    tbs: tbs.all,
    signature_algorithm: signature_algorithm.content,
    tbs_signature_algorithm: tbs_signature_algorithm.content,
    signature: der_bit_string(&signature)?,
    issuer: issuer.all,
    subject: subject.all,
    not_before: der_time_secs(&not_before)?,
    not_after: der_time_secs(&not_after)?,
    spki: spki.all,
    spki_algorithm: spki_algorithm.content,
    spki_key: der_bit_string(&spki_key)?,
    extensions,
  })
}

fn is_self_signed(cert_der: &[u8]) -> bool {
  parse_certificate(cert_der).is_some_and(|cert| cert.issuer == cert.subject)
}

/// Verify `child`'s signature over its own `tbsCertificate` with `issuer`'s
/// public key, choosing the algorithm the way webpki's `verify_signed_data`
/// does: by matching both the signature and the public-key algorithm
/// identifiers.
fn signature_is_valid(
  child: &ParsedCertificate<'_>,
  issuer: &ParsedCertificate<'_>,
) -> bool {
  // RFC 5280 requires the two `signatureAlgorithm` copies to agree, and only
  // the one inside `tbsCertificate` is covered by the signature. OpenSSL's
  // `X509_verify` rejects a mismatch; do the same rather than trusting the
  // unprotected outer copy on its own.
  if child.signature_algorithm != child.tbs_signature_algorithm {
    return false;
  }
  supported_signature_algorithms()
    .all
    .iter()
    .filter(|algorithm| {
      algorithm.signature_alg_id().as_ref() == child.signature_algorithm
        && algorithm.public_key_alg_id().as_ref() == issuer.spki_algorithm
    })
    .any(|algorithm| {
      algorithm
        .verify_signature(issuer.spki_key, child.tbs, child.signature)
        .is_ok()
    })
}

/// Whether `cert` may sign other certificates, following OpenSSL's
/// `check_chain_extensions` with default flags.
///
/// An intermediate must assert `basicConstraints` `cA`: OpenSSL applies
/// `X509_V_FLAG_X509_STRICT` to intermediates implicitly, so a certificate
/// without the extension -- every X.509v1 certificate -- cannot sign another
/// certificate unless it is a trust anchor. A trust anchor the caller
/// configured is refused only when an extension it carries says it is not a
/// CA, which is how X.509v1 roots, having no extensions at all, are trusted.
///
/// A CA carrying a constraint this verifier does not evaluate is refused, so
/// that an unevaluated restriction can never read as an absent one.
///
/// Returns the Node/OpenSSL error code a refusal is reported with, or `None`
/// when `cert` may sign.
fn ca_rejection(
  cert: &ParsedCertificate<'_>,
  certs_below: u64,
  is_trust_anchor: bool,
) -> Option<&'static str> {
  let extensions = &cert.extensions;
  if extensions.unhandled_constraint {
    return Some("UNHANDLED_CRITICAL_EXTENSION");
  }
  let is_ca = match is_trust_anchor {
    true => extensions.is_ca != Some(false),
    false => extensions.is_ca == Some(true),
  };
  // OpenSSL checks the CA flag as part of the purpose check for an issuer,
  // so Node reports a non-CA issuer as `INVALID_PURPOSE`.
  if !is_ca
    || extensions.key_cert_sign == Some(false)
    || extensions.server_auth == Some(false)
  {
    return Some("INVALID_PURPOSE");
  }
  if extensions.path_len.is_some_and(|limit| limit < certs_below) {
    return Some("PATH_LENGTH_EXCEEDED");
  }
  None
}

#[cfg(test)]
fn usable_as_ca(
  cert: &ParsedCertificate<'_>,
  certs_below: u64,
  is_trust_anchor: bool,
) -> bool {
  ca_rejection(cert, certs_below, is_trust_anchor).is_none()
}

/// Longest certification path considered, counting the end entity and the
/// trust anchor.
const MAX_CHAIN_DEPTH: usize = 10;

/// Signature verifications one chain is allowed to cost. Without a bound a peer
/// can supply several same-subject intermediates per level and make the
/// backtracking search below branch exponentially; webpki bounds its own path
/// building the same way and for the same reason.
const MAX_SIGNATURE_CHECKS: u32 = 100;

/// A certificate offered as an issuer, with the DER it came from so that it can
/// be compared against the trust store.
struct ChainCandidate<'a> {
  parsed: ParsedCertificate<'a>,
  der: &'a [u8],
}

/// State shared across one certification-path search.
struct ChainSearch<'a> {
  candidates: &'a [ChainCandidate<'a>],
  roots: &'a [ChainCandidate<'a>],
  root_cert_ders: &'a [Vec<u8>],
  /// Which candidates are already on the path under construction, so that no
  /// certificate is used twice in one path.
  on_path: Vec<bool>,
  now: i64,
  budget: u32,
}

impl ChainSearch<'_> {
  fn is_trusted(&self, der: &[u8]) -> bool {
    self
      .root_cert_ders
      .iter()
      .any(|root| root.as_slice() == der)
  }

  /// `None` once the signature-verification budget is spent.
  fn check_signature(
    &mut self,
    child: &ParsedCertificate<'_>,
    issuer: &ParsedCertificate<'_>,
  ) -> Option<bool> {
    self.budget = self.budget.checked_sub(1)?;
    Some(signature_is_valid(child, issuer))
  }

  /// One step of the search: check `current`, then try every issuer that could
  /// have signed it.
  ///
  /// The search backtracks. Taking the first name-and-signature match and
  /// committing to it would reject a legitimate chain whenever two
  /// intermediates share a subject and key — cross-signing — and only the
  /// second one reaches a configured root.
  fn extend(
    &mut self,
    current: &ParsedCertificate<'_>,
    current_der: &[u8],
    depth: usize,
    certs_below: u64,
  ) -> Result<(), &'static str> {
    if depth >= MAX_CHAIN_DEPTH {
      return Err("UNABLE_TO_GET_ISSUER_CERT_LOCALLY");
    }
    if current.not_before > self.now {
      return Err("CERT_NOT_YET_VALID");
    }
    if current.not_after < self.now {
      return Err("CERT_HAS_EXPIRED");
    }
    // A certificate that *is* one of the trusted certificates terminates the
    // chain, at any depth. This is the only way to become a trust anchor.
    // webpki reports the end-entity case as `CaUsedAsEndEntity`; OpenSSL
    // accepts it.
    if self.is_trusted(current_der) {
      return Ok(());
    }

    let mut had_candidate = false;
    // The first failure that says something more specific than "no issuer",
    // kept so backtracking does not lose the reason the first path died.
    let mut error: Option<&'static str> = None;
    let mut note = |code: &'static str| {
      error = error.or(Some(code));
    };

    // Copied out so the borrow of the candidate lists is independent of the
    // `&mut self` the recursive call needs. Both are shared slices, so this is
    // a reference copy, not a clone.
    let roots = self.roots;
    let candidates = self.candidates;

    // Trusted certificates first, mirroring OpenSSL's preference for the trust
    // store over whatever the peer supplied.
    for root in roots.iter().filter(|r| r.parsed.subject == current.issuer) {
      had_candidate = true;
      if let Some(code) = ca_rejection(&root.parsed, certs_below, true) {
        note(code);
        continue;
      }
      match self.check_signature(current, &root.parsed) {
        None => return Err("UNABLE_TO_GET_ISSUER_CERT_LOCALLY"),
        Some(true) => return Ok(()),
        Some(false) => note("CERT_SIGNATURE_FAILURE"),
      }
    }

    for (index, issuer) in candidates.iter().enumerate() {
      if self.on_path[index] || issuer.parsed.subject != current.issuer {
        continue;
      }
      had_candidate = true;
      if let Some(code) = ca_rejection(&issuer.parsed, certs_below, false) {
        note(code);
        continue;
      }
      // A self-issued certificate the peer supplied is not a trust anchor,
      // however well it signs itself; `is_trusted` above is the only way.
      if issuer.parsed.subject == issuer.parsed.issuer {
        note("SELF_SIGNED_CERT_IN_CHAIN");
        continue;
      }
      match self.check_signature(current, &issuer.parsed) {
        None => return Err("UNABLE_TO_GET_ISSUER_CERT_LOCALLY"),
        Some(false) => {
          note("CERT_SIGNATURE_FAILURE");
          continue;
        }
        Some(true) => {}
      }
      // Every issuer reached here is non-self-issued, which is exactly what
      // `pathLenConstraint` counts.
      self.on_path[index] = true;
      let result =
        self.extend(&issuer.parsed, issuer.der, depth + 1, certs_below + 1);
      self.on_path[index] = false;
      match result {
        Ok(()) => return Ok(()),
        Err(code) => note(code),
      }
    }

    if let Some(code) = error {
      return Err(code);
    }
    if current.subject == current.issuer {
      return Err(match depth {
        0 => "DEPTH_ZERO_SELF_SIGNED_CERT",
        _ => "SELF_SIGNED_CERT_IN_CHAIN",
      });
    }
    Err(match (had_candidate, candidates.is_empty()) {
      (false, true) => "UNABLE_TO_VERIFY_LEAF_SIGNATURE",
      _ => "UNABLE_TO_GET_ISSUER_CERT_LOCALLY",
    })
  }
}

/// Verify that `end_entity` chains to one of `root_cert_ders`, for chains
/// webpki refused to parse. Returns `Ok(())` only when every link's signature
/// verifies; otherwise a Node/OpenSSL-style error code.
///
/// The validity window is checked on the end entity and on every intermediate,
/// but not on the trust anchor that terminates the chain: rustls reduces a
/// trust anchor to a subject and a public key and never checks its dates, so
/// checking them here would reject chains the rest of this file accepts.
fn verify_certificate_chain(
  end_entity: &[u8],
  intermediates: &[rustls::pki_types::CertificateDer<'_>],
  root_cert_ders: &[Vec<u8>],
  now: rustls::pki_types::UnixTime,
) -> Result<(), &'static str> {
  let leaf =
    parse_certificate(end_entity).ok_or("UNABLE_TO_VERIFY_LEAF_SIGNATURE")?;

  // `extendedKeyUsage` is checked on the end entity only, which is what webpki
  // does for the chains it can parse (`KeyUsage::server_auth`), so the two
  // paths in this file agree. Every caller of this function is verifying a
  // server certificate.
  if leaf.extensions.server_auth == Some(false) {
    return Err("INVALID_PURPOSE");
  }
  if leaf.extensions.unhandled_constraint {
    return Err("UNHANDLED_CRITICAL_EXTENSION");
  }

  let candidates: Vec<_> = intermediates
    .iter()
    .filter_map(|cert| {
      parse_certificate(cert.as_ref()).map(|parsed| ChainCandidate {
        parsed,
        der: cert.as_ref(),
      })
    })
    .collect();
  let roots: Vec<_> = root_cert_ders
    .iter()
    .filter_map(|root| {
      parse_certificate(root).map(|parsed| ChainCandidate {
        parsed,
        der: root.as_slice(),
      })
    })
    .collect();

  let mut search = ChainSearch {
    on_path: vec![false; candidates.len()],
    candidates: &candidates,
    roots: &roots,
    root_cert_ders,
    now: now.as_secs() as i64,
    budget: MAX_SIGNATURE_CHECKS,
  };
  search.extend(&leaf, end_entity, 0, 0)
}

/// Map a rustls CertificateError to a Node/OpenSSL-style error code.
/// Mirror of the JS-side `makeVerifyError` message table in `_tls_wrap.js`.
/// Returns `None` when the code has no Node-style human message; callers
/// fall back to the rustls error display.
fn node_verify_error_message(code: &str) -> Option<&'static str> {
  match code {
    "CERT_HAS_EXPIRED" => Some("certificate has expired"),
    "CERT_NOT_YET_VALID" => Some("certificate is not yet valid"),
    "CERT_SIGNATURE_FAILURE" => Some("certificate signature failure"),
    "INVALID_PURPOSE" => Some("unsupported certificate purpose"),
    "PATH_LENGTH_EXCEEDED" => Some("path length constraint exceeded"),
    "UNHANDLED_CRITICAL_EXTENSION" => Some("unhandled critical extension"),
    "DEPTH_ZERO_SELF_SIGNED_CERT" => Some("self-signed certificate"),
    "SELF_SIGNED_CERT_IN_CHAIN" => {
      Some("self-signed certificate in certificate chain")
    }
    "UNABLE_TO_GET_ISSUER_CERT" => Some("unable to get issuer certificate"),
    "UNABLE_TO_GET_ISSUER_CERT_LOCALLY" => {
      Some("unable to get local issuer certificate")
    }
    "UNABLE_TO_VERIFY_LEAF_SIGNATURE" => {
      Some("unable to verify the first certificate")
    }
    _ => None,
  }
}

fn cert_error_to_node_code(err: &rustls::CertificateError) -> &'static str {
  use rustls::CertificateError as CE;
  match err {
    CE::UnknownIssuer => "UNABLE_TO_VERIFY_LEAF_SIGNATURE",
    CE::NotValidYet => "CERT_NOT_YET_VALID",
    CE::Expired => "CERT_HAS_EXPIRED",
    CE::Revoked => "CERT_REVOKED",
    CE::NotValidForName | CE::NotValidForNameContext { .. } => {
      "ERR_TLS_CERT_ALTNAME_INVALID"
    }
    CE::InvalidPurpose => "INVALID_PURPOSE",
    CE::Other(other) => {
      let msg = format!("{other}");
      if msg.contains("SelfSigned") {
        "DEPTH_ZERO_SELF_SIGNED_CERT"
      } else if msg.contains("CaUsedAsEndEntity") {
        // Not a real OpenSSL error — treat like self-signed.
        "DEPTH_ZERO_SELF_SIGNED_CERT"
      } else {
        "UNABLE_TO_VERIFY_LEAF_SIGNATURE"
      }
    }
    _ => "UNABLE_TO_VERIFY_LEAF_SIGNATURE",
  }
}

impl NodeServerCertVerifier {
  /// In strict mode (`rejectUnauthorized: true`), return Err so rustls
  /// aborts the handshake before any session is cached. In lenient mode,
  /// record the Node-style error code and let the handshake proceed so the
  /// JS layer can surface it as `authorizationError`. In both cases the
  /// precise code is stashed in `verify_error` so `rustls_error_to_node_error`
  /// can surface it on the error path even when rustls aborts the handshake.
  fn record_or_fail(
    &self,
    err: rustls::Error,
    code: String,
  ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
    store_verify_error(&self.verify_error, code);
    if self.strict_verify {
      Err(err)
    } else {
      Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
  }
}

impl rustls::client::danger::ServerCertVerifier for NodeServerCertVerifier {
  fn verify_server_cert(
    &self,
    end_entity: &rustls::pki_types::CertificateDer<'_>,
    intermediates: &[rustls::pki_types::CertificateDer<'_>],
    server_name: &rustls::pki_types::ServerName<'_>,
    ocsp: &[u8],
    now: rustls::pki_types::UnixTime,
  ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
    match self.inner.verify_server_cert(
      end_entity,
      intermediates,
      server_name,
      ocsp,
      now,
    ) {
      Ok(v) => {
        if self.empty_explicit_ca {
          let code = verify_certificate_chain(
            end_entity.as_ref(),
            intermediates,
            &self.root_cert_ders,
            now,
          )
          .err()
          .unwrap_or("UNABLE_TO_VERIFY_LEAF_SIGNATURE");
          self.record_or_fail(
            rustls::Error::InvalidCertificate(
              rustls::CertificateError::UnknownIssuer,
            ),
            code.to_string(),
          )
        } else {
          Ok(v)
        }
      }
      Err(rustls::Error::InvalidCertificate(ref cert_error)) => {
        // Server-name checks are always deferred to JS (checkServerIdentity)
        // so that custom checkServerIdentity callbacks see a successful
        // handshake. The JS layer still runs that check and destroys the
        // connection if it fails.
        if matches!(
          cert_error,
          rustls::CertificateError::NotValidForName
            | rustls::CertificateError::NotValidForNameContext { .. }
        ) {
          return Ok(rustls::client::danger::ServerCertVerified::assertion());
        }
        // OpenSSL accepts X.509v1 certificates while webpki rejects them at
        // parse time, so verify such a chain here instead of letting webpki's
        // refusal to parse it stand in for a verdict. This is a full
        // verification -- signature, validity window and CA constraints on
        // every link -- so a broken chain still produces the right
        // Node/OpenSSL error.
        if is_unsupported_cert_version(cert_error) {
          match verify_certificate_chain(
            end_entity.as_ref(),
            intermediates,
            &self.root_cert_ders,
            now,
          ) {
            Ok(()) => {
              return Ok(
                rustls::client::danger::ServerCertVerified::assertion(),
              );
            }
            Err(code) => {
              // Chain is broken -- in strict mode fail the handshake so
              // rustls doesn't cache a resumable session; in lenient mode
              // store the error for JS to surface as authorizationError.
              return self.record_or_fail(
                rustls::Error::InvalidCertificate(cert_error.clone()),
                code.to_string(),
              );
            }
          }
        }
        if matches!(cert_error, rustls::CertificateError::UnknownIssuer) {
          let code = verify_certificate_chain(
            end_entity.as_ref(),
            intermediates,
            &self.root_cert_ders,
            now,
          )
          .err()
          .unwrap_or("UNABLE_TO_VERIFY_LEAF_SIGNATURE");
          return self.record_or_fail(
            rustls::Error::InvalidCertificate(cert_error.clone()),
            code.to_string(),
          );
        }
        if let rustls::CertificateError::Other(other) = cert_error
          && let Some(webpki_err) = other.0.downcast_ref::<webpki::Error>()
        {
          // CaUsedAsEndEntity is a webpki-specific check that OpenSSL
          // does not have. If the cert is actually in our root store,
          // trust it silently. Otherwise fall through.
          if matches!(webpki_err, webpki::Error::CaUsedAsEndEntity) {
            let ee_bytes: &[u8] = end_entity.as_ref();
            let is_trusted =
              self.root_cert_ders.iter().any(|r| r.as_slice() == ee_bytes);
            if is_trusted {
              return Ok(
                rustls::client::danger::ServerCertVerified::assertion(),
              );
            }
          }
        }
        // In strict mode, fail the handshake so rustls won't cache the
        // session; in lenient mode, store the error so JS can surface
        // it as authorizationError without aborting.
        let code = cert_error_to_node_code(cert_error);
        self.record_or_fail(
          rustls::Error::InvalidCertificate(cert_error.clone()),
          code.to_string(),
        )
      }
      Err(e) => Err(e),
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    verify_handshake_signature_allowing_v1(
      self.inner.verify_tls12_signature(message, cert, dss),
      message,
      cert,
      dss,
      false,
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    verify_handshake_signature_allowing_v1(
      self.inner.verify_tls13_signature(message, cert, dss),
      message,
      cert,
      dss,
      true,
    )
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.inner.supported_verify_schemes()
  }
}

impl TLSWrap {
  fn do_attach_uv_stream(
    inner_ptr: &OwnedPtr<TLSWrapInner>,
    stream: *mut uv_compat::uv_stream_t,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    if stream.is_null() {
      return UV_EBADF;
    }

    let inner = unsafe { &mut *inner_ptr.as_mut_ptr() };
    inner.underlying = UnderlyingStream::Uv { stream };
    inner.isolate = Some(unsafe { scope.as_raw_isolate_ptr() });
    inner.cached_loop_ptr = unsafe { (*stream).loop_ };

    let state_global = &op_state.borrow::<StreamBaseState>().array;
    inner.stream_base_state =
      Some(v8::Global::new(scope, v8::Local::new(scope, state_global)));

    0
  }
}

/// Normalize PEM data so that `BEGIN TRUSTED CERTIFICATE` and
/// `BEGIN X509 CERTIFICATE` (which OpenSSL accepts) are rewritten to
/// `BEGIN CERTIFICATE` (the only form rustls_pemfile recognises).
fn normalize_pem_headers(pem: &[u8]) -> std::borrow::Cow<'_, [u8]> {
  // Fast path: avoid allocation when no alternate headers are present.
  // PEM files are ASCII, so a simple substring search is fine.
  let needs_rewrite = pem
    .windows(b"TRUSTED CERTIFICATE".len())
    .any(|w| w == b"TRUSTED CERTIFICATE")
    || pem
      .windows(b"X509 CERTIFICATE".len())
      .any(|w| w == b"X509 CERTIFICATE");
  if !needs_rewrite {
    return std::borrow::Cow::Borrowed(pem);
  }
  let s = String::from_utf8_lossy(pem);
  let s = s
    .replace("TRUSTED CERTIFICATE", "CERTIFICATE")
    .replace("X509 CERTIFICATE", "CERTIFICATE");
  std::borrow::Cow::Owned(s.into_bytes())
}

#[derive(Debug)]
struct NodeClientSessionStoreWrapper {
  inner: Arc<dyn rustls::client::ClientSessionStore>,
  allow_resumption: Arc<AtomicBool>,
}

impl rustls::client::ClientSessionStore for NodeClientSessionStoreWrapper {
  fn set_kx_hint(
    &self,
    server_name: rustls::pki_types::ServerName<'static>,
    group: rustls::NamedGroup,
  ) {
    self.inner.set_kx_hint(server_name, group);
  }

  fn kx_hint(
    &self,
    server_name: &rustls::pki_types::ServerName<'_>,
  ) -> Option<rustls::NamedGroup> {
    self.inner.kx_hint(server_name)
  }

  fn set_tls12_session(
    &self,
    server_name: rustls::pki_types::ServerName<'static>,
    value: rustls::client::Tls12ClientSessionValue,
  ) {
    self.inner.set_tls12_session(server_name, value);
  }

  fn tls12_session(
    &self,
    server_name: &rustls::pki_types::ServerName<'_>,
  ) -> Option<rustls::client::Tls12ClientSessionValue> {
    if self.allow_resumption.load(Ordering::Relaxed) {
      self.inner.tls12_session(server_name)
    } else {
      None
    }
  }

  fn remove_tls12_session(
    &self,
    server_name: &rustls::pki_types::ServerName<'static>,
  ) {
    self.inner.remove_tls12_session(server_name);
  }

  fn insert_tls13_ticket(
    &self,
    server_name: rustls::pki_types::ServerName<'static>,
    value: rustls::client::Tls13ClientSessionValue,
  ) {
    self.inner.insert_tls13_ticket(server_name, value);
  }

  fn take_tls13_ticket(
    &self,
    server_name: &rustls::pki_types::ServerName<'static>,
  ) -> Option<rustls::client::Tls13ClientSessionValue> {
    if self.allow_resumption.load(Ordering::Relaxed) {
      self.inner.take_tls13_ticket(server_name)
    } else {
      None
    }
  }
}

/// Build a rustls ClientConfig from a SecureContext JS object.
fn build_client_config(
  scope: &mut v8::PinScope,
  context: v8::Local<v8::Object>,
  op_state: &mut OpState,
  allow_resumption: Arc<AtomicBool>,
) -> Option<(rustls::ClientConfig, VerifyErrorStore)> {
  use deno_net::DefaultTlsOptions;
  use deno_tls::TlsKeys;
  use deno_tls::TlsKeysHolder;

  let reject_unauthorized =
    get_js_bool(scope, context, "rejectUnauthorized", true);
  let use_default_ca = get_js_bool(scope, context, "useDefaultCA", true);
  let protocol_versions = match get_protocol_versions(scope, context) {
    ProtocolVersionSelection::Default => {
      &[&rustls::version::TLS13, &rustls::version::TLS12][..]
    }
    ProtocolVersionSelection::Tls12Only => &[&rustls::version::TLS12][..],
    ProtocolVersionSelection::Tls13Only => &[&rustls::version::TLS13][..],
    ProtocolVersionSelection::Unsupported => return None,
  };

  // Collect CA certs
  let mut ca_certs = Vec::new();
  let ca_key = v8::String::new(scope, "ca").unwrap();
  if let Some(ca_val) = context.get(scope, ca_key.into()) {
    if let Ok(arr) = v8::Local::<v8::Array>::try_from(ca_val) {
      for i in 0..arr.length() {
        if let Some(v) = arr.get_index(scope, i)
          && let Some(s) = v.to_string(scope)
        {
          ca_certs.push(s.to_rust_string_lossy(scope).into_bytes());
        }
      }
    } else if !ca_val.is_undefined()
      && !ca_val.is_null()
      && let Some(s) = ca_val.to_string(scope)
    {
      ca_certs.push(s.to_rust_string_lossy(scope).into_bytes());
    }
  }

  // Whether the SecureContext itself carries `ca` certs, as opposed to
  // CA certs inherited from the process (default store or
  // setDefaultCACertificates). Only explicit per-context certs disqualify
  // the connection from the cached-verifier path below.
  let has_explicit_ca = !ca_certs.is_empty();

  let mut root_cert_store = op_state
    .borrow::<DefaultTlsOptions>()
    .root_cert_store()
    .ok()
    .flatten();

  // Use custom CA certs from setDefaultCACertificates() only when the
  // SecureContext is on the default CA path. Explicit `ca` replaces the
  // root store, while context.addCACert() extends whatever default CA store
  // is active.
  if use_default_ca
    && let Some(node_tls_state) = op_state.try_borrow::<NodeTlsState>()
    && let Some(custom_ca_certs) = &node_tls_state.custom_ca_certs
  {
    root_cert_store = Some(rustls::RootCertStore::empty());
    ca_certs
      .extend(custom_ca_certs.iter().map(|cert| cert.clone().into_bytes()));
  } else if !use_default_ca {
    root_cert_store = Some(rustls::RootCertStore::empty());
  }

  // Build client key/cert if provided
  let cert_str = get_js_string(scope, context, "cert");
  let key_str = get_js_string(scope, context, "key");

  let tls_keys = if let (Some(cert), Some(key)) = (cert_str, key_str) {
    let certs: Vec<_> =
      rustls_pemfile::certs(&mut std::io::BufReader::new(cert.as_bytes()))
        .filter_map(|r| r.ok())
        .collect();

    let private_key =
      rustls_pemfile::private_key(&mut std::io::BufReader::new(key.as_bytes()))
        .ok()
        .flatten();

    if let Some(private_key) = private_key {
      TlsKeysHolder::from(TlsKeys::Static(deno_tls::TlsKey(certs, private_key)))
    } else {
      TlsKeysHolder::from(TlsKeys::Null)
    }
  } else {
    TlsKeysHolder::from(TlsKeys::Null)
  };

  // Fall back to the default Mozilla root cert store (same as deno_tls's
  // own `create_client_config`).  The old `RootCertStore::empty()` caused
  // every TLS connection without explicit CA options to fail verification.
  let mut root_cert_store =
    root_cert_store.unwrap_or_else(deno_tls::create_default_root_cert_store);
  let empty_explicit_ca = !use_default_ca && ca_certs.is_empty();

  // Collect raw DER bytes of root certs so NodeServerCertVerifier can
  // check CaUsedAsEndEntity certs against the trust store.
  let mut root_cert_ders: Vec<Vec<u8>> = Vec::new();

  for cert in &ca_certs {
    let normalized = normalize_pem_headers(cert);
    let reader =
      &mut std::io::BufReader::new(std::io::Cursor::new(normalized.as_ref()));
    for parsed in rustls_pemfile::certs(reader) {
      match parsed {
        Ok(cert) => {
          root_cert_ders.push(cert.as_ref().to_vec());
          if let Err(e) = root_cert_store.add(cert) {
            log::warn!("TLSWrap: ignoring invalid CA certificate: {e}");
          }
        }
        Err(e) => {
          log::warn!("TLSWrap: failed to parse CA PEM entry: {e}");
        }
      }
    }
  }

  let maybe_cert_chain_and_key = tls_keys.take();

  // The default-config fast path applies when the caller has not supplied
  // any of the per-connection knobs that would change cert validation or
  // client auth: no per-context `ca` certs and no explicit client
  // cert/key.  In that case we cache the verifier and the "no client
  // cert" resolver in `NodeTlsState` so successive `tls.connect()` calls
  // hand rustls the same `Arc`s and session resumption is allowed to
  // proceed (rustls keys its `compatible_config` check on
  // `Arc::downgrade(&verifier)` identity).  A process-level custom CA set
  // by `setDefaultCACertificates` stays on this path: it is shared by all
  // default-CA connections, and `op_set_default_ca_certificates` drops the
  // cached verifiers whenever it changes.
  let is_default_path = !has_explicit_ca
    && use_default_ca
    && matches!(maybe_cert_chain_and_key, TlsKeys::Null);

  // Always build with root certs so NodeServerCertVerifier can check them.
  // NodeServerCertVerifier never aborts the handshake — it stores errors
  // for verifyError().  The JS layer decides whether to destroy the
  // connection based on rejectUnauthorized.
  let config_builder =
    rustls::ClientConfig::builder_with_protocol_versions(protocol_versions)
      .with_root_certificates(root_cert_store.clone());

  let mut config = match maybe_cert_chain_and_key {
    TlsKeys::Static(deno_tls::TlsKey(cert_chain, private_key)) => {
      // `with_client_auth_cert` internally calls `CertifiedKey::keys_match()`
      // which parses the end-entity cert via webpki and rejects X.509v1 certs
      // with `UnsupportedCertVersion`.  Node uses OpenSSL, which accepts v1
      // certs, and several upstream test fixtures (e.g. agent3) are v1.
      // Build the CertifiedKey manually and compare the SubjectPublicKeyInfo
      // bytes ourselves for those, so the cert/key pairing is still checked,
      // matching the server-side path in `build_server_config`.
      let provider = config_builder.crypto_provider().clone();
      let signing_key = provider
        .key_provider
        .load_private_key(private_key.clone_key())
        .ok()?;
      let certified_key =
        rustls::sign::CertifiedKey::new(cert_chain, signing_key.clone());
      if let Err(e) =
        keys_match_allowing_v1(&certified_key, signing_key.as_ref())
      {
        log::debug!("TLSWrap: client cert/key validation failed: {e}");
        return None;
      }
      let resolver =
        Arc::new(StaticClientCertResolver(Arc::new(certified_key)));
      let mut cfg = config_builder.with_no_client_auth();
      cfg.client_auth_cert_resolver = resolver;
      cfg
    }
    TlsKeys::Null => config_builder.with_no_client_auth(),
    TlsKeys::Resolver(_) => return None,
  };

  // Enable session resumption using the shared session store from
  // NodeTlsState. Strict and `rejectUnauthorized: false` connections use
  // separate caches so a session whose cert error was deferred in the
  // first handshake cannot be picked up by a later strict connection
  // (which would resume without re-running verification and bypass
  // checkServerIdentity in JS).
  //
  // In Node.js, client session resumption is opt-in per connection:
  // connections only offer cached sessions when `options.session` was
  // explicitly provided (or `setSession()` was called). `NodeClientSessionStoreWrapper`
  // saves newly issued sessions unconditionally (so getSession / 'session'
  // events work), but only offers them for resumption when `allow_resumption`
  // is true.
  if let Some(node_tls_state) = op_state.try_borrow::<NodeTlsState>() {
    let store = if reject_unauthorized {
      node_tls_state.client_session_store.clone()
    } else {
      node_tls_state.client_session_store_insecure.clone()
    };
    config.resumption = rustls::client::Resumption::store(Arc::new(
      NodeClientSessionStoreWrapper {
        inner: store,
        allow_resumption,
      },
    ));
  }

  // Install NodeServerCertVerifier to store verification errors for
  // verifyError().  This verifier never aborts the handshake — it
  // matches Node/OpenSSL behaviour where cert errors are deferred.
  //
  // The verifier Arc participates in rustls's resumption compatibility
  // check, so keep separate stable identities for strict verification and
  // `rejectUnauthorized: false`. Otherwise a session first accepted with
  // deferred cert errors can be resumed by a later strict connection without
  // surfacing the original verification error.
  let (final_verify_error, verifier_arc): (
    VerifyErrorStore,
    Option<Arc<dyn rustls::client::danger::ServerCertVerifier>>,
  ) = if is_default_path {
    let state = op_state.borrow_mut::<NodeTlsState>();
    let cached_verifier = if reject_unauthorized {
      &mut state.cached_default_verifier
    } else {
      &mut state.cached_insecure_verifier
    };
    if let Some((v, e)) = cached_verifier.clone() {
      (e, Some(v))
    } else {
      let verifier_result = rustls::client::WebPkiServerVerifier::builder(
        Arc::new(root_cert_store),
      )
      .build();
      match verifier_result {
        Ok(inner) => {
          let store: VerifyErrorStore = Default::default();
          let v: Arc<dyn rustls::client::danger::ServerCertVerifier> =
            Arc::new(NodeServerCertVerifier {
              inner,
              verify_error: store.clone(),
              empty_explicit_ca: false,
              root_cert_ders,
              strict_verify: reject_unauthorized,
            });
          *cached_verifier = Some((v.clone(), store.clone()));
          (store, Some(v))
        }
        Err(_) => (Default::default(), None),
      }
    }
  } else {
    let store: VerifyErrorStore = Default::default();
    let verifier_root_store = if empty_explicit_ca {
      deno_tls::create_default_root_cert_store()
    } else {
      root_cert_store.clone()
    };
    let verifier_result = rustls::client::WebPkiServerVerifier::builder(
      Arc::new(verifier_root_store),
    )
    .build();
    let v: Option<Arc<dyn rustls::client::danger::ServerCertVerifier>> =
      verifier_result.ok().map(|inner| {
        Arc::new(NodeServerCertVerifier {
          inner,
          verify_error: store.clone(),
          empty_explicit_ca,
          root_cert_ders,
          strict_verify: reject_unauthorized,
        }) as Arc<dyn rustls::client::danger::ServerCertVerifier>
      });
    (store, v)
  };
  if let Some(v) = verifier_arc {
    config.dangerous().set_certificate_verifier(v);
  }

  // Install a stable "no client cert" resolver Arc on the default path so
  // rustls's `Arc::downgrade(&client_creds)` identity check keeps the
  // resumed session compatible across `tls.connect()` calls. Keep this
  // split by verification policy for the same reason as the verifier Arc.
  if is_default_path {
    let state = op_state.borrow_mut::<NodeTlsState>();
    let cached_no_client_auth = if reject_unauthorized {
      &mut state.cached_no_client_auth
    } else {
      &mut state.cached_insecure_no_client_auth
    };
    let resolver = cached_no_client_auth
      .get_or_insert_with(|| config.client_auth_cert_resolver.clone())
      .clone();
    config.client_auth_cert_resolver = resolver;
  }

  Some((config, final_verify_error))
}

/// A `ClientCertVerifier` for `node:tls` servers that wraps
/// `WebPkiClientVerifier` with two pieces of extra leniency to match the
/// OpenSSL-backed Node behaviour:
///
///  * `rejectUnauthorized: false` → verify_client_cert always returns Ok,
///    so the TLS handshake succeeds regardless of chain validity and JS
///    code can inspect the peer via `getPeerCertificate()` /
///    `TLSSocket.authorized`.
///  * Self-signed client certs used as their own CA (i.e. the cert DER is
///    also in the trusted `ca` list) are accepted — rustls/webpki rejects
///    these with `CaUsedAsEndEntity`, but OpenSSL/Node trusts them if
///    they're in the configured `ca`. Mirrors `NodeServerCertVerifier`'s
///    handling of the same case on the client side.
#[derive(Debug)]
struct NodeClientCertVerifier {
  inner: Arc<dyn rustls::server::danger::ClientCertVerifier>,
  root_cert_ders: Vec<Vec<u8>>,
  reject_unauthorized: bool,
  /// Shared with TLSWrapInner so `verifyError()` can return the error.
  verify_error: VerifyErrorStore,
}

impl rustls::server::danger::ClientCertVerifier for NodeClientCertVerifier {
  fn offer_client_auth(&self) -> bool {
    true
  }

  fn client_auth_mandatory(&self) -> bool {
    self.reject_unauthorized
  }

  fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
    self.inner.root_hint_subjects()
  }

  fn verify_client_cert(
    &self,
    end_entity: &rustls::pki_types::CertificateDer<'_>,
    intermediates: &[rustls::pki_types::CertificateDer<'_>],
    now: rustls::pki_types::UnixTime,
  ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
    // Fast path: if the presented client cert is byte-identical to one of
    // the trusted CA DERs, accept it even when webpki would say
    // `CaUsedAsEndEntity`.
    let ee_bytes: &[u8] = end_entity.as_ref();
    if self.root_cert_ders.iter().any(|r| r.as_slice() == ee_bytes) {
      return Ok(rustls::server::danger::ClientCertVerified::assertion());
    }
    match self
      .inner
      .verify_client_cert(end_entity, intermediates, now)
    {
      Ok(v) => Ok(v),
      Err(e) => {
        // Never abort the TLS handshake from client cert verification.
        // Store the error so verifyError() can return it, and let the
        // JS layer (onServerSocketSecure) decide whether to tear down
        // the connection based on `rejectUnauthorized`. This matches
        // Node/OpenSSL behaviour where client cert failures produce
        // ECONNRESET on the client (clean close) rather than a TLS
        // fatal alert.
        let code = if let rustls::Error::InvalidCertificate(ref cert_err) = e {
          cert_error_to_node_code(cert_err).to_string()
        } else {
          format!("{e}")
        };
        *self.verify_error.lock().unwrap_or_else(|p| p.into_inner()) =
          Some(code);
        Ok(rustls::server::danger::ClientCertVerified::assertion())
      }
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    // webpki cannot parse an X.509v1 client cert to get at its public key.
    // Node/OpenSSL can, so verify the signature directly rather than
    // asserting it: the peer must still hold the private key belonging to
    // the certificate it presented.
    verify_handshake_signature_allowing_v1(
      self.inner.verify_tls12_signature(message, cert, dss),
      message,
      cert,
      dss,
      false,
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    verify_handshake_signature_allowing_v1(
      self.inner.verify_tls13_signature(message, cert, dss),
      message,
      cert,
      dss,
      true,
    )
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.inner.supported_verify_schemes()
  }
}

/// A `ClientCertVerifier` for `node:tls` servers when `requestCert` is true
/// but no CA certificates were provided.  Node/OpenSSL still sends a
/// CertificateRequest (so the client presents its cert) and reports the
/// cert as unauthorized.  rustls's `WebPkiClientVerifier` requires at least
/// one trust anchor, so this standalone verifier fills that gap.
#[derive(Debug)]
struct NodeClientCertVerifierNoRoots {
  reject_unauthorized: bool,
  verify_error: VerifyErrorStore,
}

impl rustls::server::danger::ClientCertVerifier
  for NodeClientCertVerifierNoRoots
{
  fn offer_client_auth(&self) -> bool {
    true
  }

  fn client_auth_mandatory(&self) -> bool {
    self.reject_unauthorized
  }

  fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
    &[]
  }

  fn verify_client_cert(
    &self,
    end_entity: &rustls::pki_types::CertificateDer<'_>,
    intermediates: &[rustls::pki_types::CertificateDer<'_>],
    _now: rustls::pki_types::UnixTime,
  ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
    // No root CAs, so we cannot establish a trust chain. Match Node's
    // OpenSSL-derived error codes: a self-signed leaf (no intermediates,
    // subject == issuer) reports DEPTH_ZERO_SELF_SIGNED_CERT; everything
    // else falls back to UNABLE_TO_GET_ISSUER_CERT.
    let code =
      if intermediates.is_empty() && is_self_signed(end_entity.as_ref()) {
        "DEPTH_ZERO_SELF_SIGNED_CERT"
      } else {
        "UNABLE_TO_GET_ISSUER_CERT"
      };
    *self.verify_error.lock().unwrap_or_else(|p| p.into_inner()) =
      Some(code.to_string());
    Ok(rustls::server::danger::ClientCertVerified::assertion())
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    let algos = supported_signature_algorithms();
    verify_handshake_signature_allowing_v1(
      rustls::crypto::verify_tls12_signature(message, cert, dss, algos),
      message,
      cert,
      dss,
      false,
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    let algos = supported_signature_algorithms();
    verify_handshake_signature_allowing_v1(
      rustls::crypto::verify_tls13_signature(message, cert, dss, algos),
      message,
      cert,
      dss,
      true,
    )
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    rustls::crypto::aws_lc_rs::default_provider()
      .signature_verification_algorithms
      .supported_schemes()
  }
}

/// Build a rustls ServerConfig from a SecureContext JS object.
/// Returns (config, verify_error_store) where the store is shared with
/// `NodeClientCertVerifier` so the server-side JS can read client cert errors.
fn build_server_config(
  scope: &mut v8::PinScope,
  context: v8::Local<v8::Object>,
  op_state: &mut OpState,
) -> Option<(rustls::ServerConfig, VerifyErrorStore)> {
  let protocol_versions = match get_protocol_versions(scope, context) {
    ProtocolVersionSelection::Default => {
      &[&rustls::version::TLS13, &rustls::version::TLS12][..]
    }
    ProtocolVersionSelection::Tls12Only => &[&rustls::version::TLS12][..],
    ProtocolVersionSelection::Tls13Only => &[&rustls::version::TLS13][..],
    ProtocolVersionSelection::Unsupported => return None,
  };
  // `cert` and `key` are both optional: a SecureContext with neither (e.g.
  // one returned from a server's SNICallback, or the placeholder created by
  // `tls.createSecureContext()`) is valid input.  In that case rustls will
  // fail the handshake with a fatal alert when the cert resolver is asked
  // for a CertifiedKey — matching Node/OpenSSL, which reports a missing
  // server certificate as `ERR_SSL_SSLV3_ALERT_HANDSHAKE_FAILURE` on the
  // client and a "no suitable signature algorithm" error server-side.
  let cert_str = get_js_string(scope, context, "cert");
  let key_str = get_js_string(scope, context, "key");
  let no_server_cert = cert_str.is_none() && key_str.is_none();

  let (mut certs, private_key) = if no_server_cert {
    (Vec::new(), None)
  } else {
    let cert_str = cert_str?;
    let key_str = key_str?;
    let certs: Vec<_> =
      rustls_pemfile::certs(&mut std::io::BufReader::new(cert_str.as_bytes()))
        .filter_map(|r| r.ok())
        .collect();
    let private_key = rustls_pemfile::private_key(
      &mut std::io::BufReader::new(key_str.as_bytes()),
    )
    .ok()
    .flatten()?;
    (certs, Some(private_key))
  };

  // OpenSSL auto-chains: when `ca` certs are provided, it includes them in
  // the certificate chain sent during the handshake so that clients receive
  // the full chain (leaf + intermediates + root).  This is needed for
  // getPeerCertificate(true) to return the issuer chain.
  {
    let ca_key = v8::String::new(scope, "ca").unwrap();
    if let Some(ca_val) = context.get(scope, ca_key.into()) {
      let mut ca_pems: Vec<Vec<u8>> = Vec::new();
      if let Ok(arr) = v8::Local::<v8::Array>::try_from(ca_val) {
        for i in 0..arr.length() {
          if let Some(v) = arr.get_index(scope, i)
            && let Some(s) = v.to_string(scope)
          {
            ca_pems.push(s.to_rust_string_lossy(scope).into_bytes());
          }
        }
      } else if !ca_val.is_undefined()
        && !ca_val.is_null()
        && let Some(s) = ca_val.to_string(scope)
      {
        ca_pems.push(s.to_rust_string_lossy(scope).into_bytes());
      }
      for pem in &ca_pems {
        let normalized = normalize_pem_headers(pem);
        let reader = &mut std::io::BufReader::new(std::io::Cursor::new(
          normalized.as_ref(),
        ));
        certs.extend(rustls_pemfile::certs(reader).flatten());
      }
    }
  }

  let request_cert = get_js_bool(scope, context, "requestCert", false);
  let reject_unauthorized =
    get_js_bool(scope, context, "rejectUnauthorized", true);

  let builder =
    rustls::ServerConfig::builder_with_protocol_versions(protocol_versions);

  // When `requestCert` is true, the server sends a CertificateRequest during
  // the TLS handshake so the client presents its certificate. Without this
  // the peer certificate is never available to `getPeerCertificate()`.
  let (builder, client_cert_verify_error) = if request_cert {
    let mut root_cert_store = rustls::RootCertStore::empty();
    let mut root_cert_ders: Vec<Vec<u8>> = Vec::new();
    v8_static_strings! {
      CA = "ca",
    }
    let ca_key = CA.v8_string(scope).unwrap();
    if let Some(ca_val) = context.get(scope, ca_key.into()) {
      let mut ca_pems: Vec<Vec<u8>> = Vec::new();
      if let Ok(arr) = v8::Local::<v8::Array>::try_from(ca_val) {
        for i in 0..arr.length() {
          if let Some(v) = arr.get_index(scope, i)
            && let Some(s) = v.to_string(scope)
          {
            ca_pems.push(s.to_rust_string_lossy(scope).into_bytes());
          }
        }
      } else if !ca_val.is_undefined()
        && !ca_val.is_null()
        && let Some(s) = ca_val.to_string(scope)
      {
        ca_pems.push(s.to_rust_string_lossy(scope).into_bytes());
      }
      for pem in &ca_pems {
        let normalized = normalize_pem_headers(pem);
        let reader = &mut std::io::BufReader::new(std::io::Cursor::new(
          normalized.as_ref(),
        ));
        for parsed in rustls_pemfile::certs(reader) {
          match parsed {
            Ok(cert) => {
              root_cert_ders.push(cert.as_ref().to_vec());
              if let Err(e) = root_cert_store.add(cert) {
                log::debug!(
                  "TLSWrap: ignoring invalid client CA certificate: {e}"
                );
              }
            }
            Err(e) => {
              log::debug!("TLSWrap: failed to parse client CA PEM entry: {e}");
            }
          }
        }
      }
    }

    let client_verify_error: VerifyErrorStore = Default::default();
    if root_cert_store.is_empty() {
      // No CA certs provided.  Node/OpenSSL still sends a
      // CertificateRequest (so the client presents its cert) but cannot
      // actually verify it — `authorized` will be false.  rustls's
      // WebPkiClientVerifier requires at least one trust anchor, so use
      // our own verifier that accepts everything.
      (
        builder.with_client_cert_verifier(Arc::new(
          NodeClientCertVerifierNoRoots {
            reject_unauthorized,
            verify_error: client_verify_error.clone(),
          },
        )),
        client_verify_error,
      )
    } else {
      let mut verifier_builder = rustls::server::WebPkiClientVerifier::builder(
        Arc::new(root_cert_store),
      );
      if !reject_unauthorized {
        verifier_builder = verifier_builder.allow_unauthenticated();
      }
      match verifier_builder.build() {
        Ok(inner) => (
          builder.with_client_cert_verifier(Arc::new(NodeClientCertVerifier {
            inner,
            root_cert_ders,
            reject_unauthorized,
            verify_error: client_verify_error.clone(),
          })),
          client_verify_error,
        ),
        Err(e) => {
          log::debug!("TLSWrap: failed to build client cert verifier: {e}");
          return None;
        }
      }
    }
  } else {
    (builder.with_no_client_auth(), Default::default())
  };

  // No-cert path: when neither `cert` nor `key` is provided, install a
  // resolver that always returns `None`. rustls then aborts the handshake
  // with a fatal alert and surfaces `Error::General("no server certificate
  // chain resolved")`, which `rustls_error_to_node_error` translates back
  // to Node's "no suitable signature algorithm" error.
  let Some(private_key) = private_key else {
    return Some((
      builder.with_cert_resolver(Arc::new(NoCertResolver)),
      client_cert_verify_error,
    ));
  };

  // `with_single_cert` runs `CertifiedKey::keys_match()`, which parses the
  // end-entity cert via webpki and rejects X.509v1 certs with
  // UnsupportedCertVersion.  Node uses OpenSSL, which accepts v1 certs, and
  // several upstream Node test fixtures (e.g. agent2, agent3) are v1, so we
  // build the CertifiedKey manually and run the pairing check ourselves,
  // comparing SubjectPublicKeyInfo bytes for the certificates webpki will not
  // parse.  A server must not be able to serve a certificate whose private
  // key it does not hold.
  let provider = builder.crypto_provider().clone();
  let signing_key = provider.key_provider.load_private_key(private_key).ok()?;
  let certified_key =
    rustls::sign::CertifiedKey::new(certs, signing_key.clone());
  if let Err(e) = keys_match_allowing_v1(&certified_key, signing_key.as_ref()) {
    log::debug!("TLSWrap: cert/key validation failed: {e}");
    return None;
  }
  let resolver = rustls::sign::SingleCertAndKey::from(certified_key);
  let mut server_config = builder.with_cert_resolver(Arc::new(resolver));
  // Enable session ticket issuance (RFC 5077) so TLS 1.2 / 1.3 clients can
  // resume sessions.  Without this, rustls installs `NeverProducesTickets`
  // and Node's `tls.TLSSocket#isSessionReused()` always returns false even
  // when the client has a session cache configured.
  //
  // The ticketer is shared at the process level via `NodeTlsState` so that
  // every TLS connection accepted by the same `tls.createServer()` (which
  // builds a fresh ServerConfig per accepted socket) decrypts tickets with
  // the same keys.  Without this sharing each connection rotates keys and
  // resumption never succeeds.
  let ticketer = {
    let state = op_state.borrow_mut::<NodeTlsState>();
    if let Some(t) = &state.server_ticketer {
      Some(t.clone())
    } else {
      match rustls::crypto::aws_lc_rs::Ticketer::new() {
        Ok(t) => {
          state.server_ticketer = Some(t.clone());
          Some(t)
        }
        Err(e) => {
          log::debug!("TLSWrap: failed to build session ticketer: {e}");
          None
        }
      }
    }
  };
  if let Some(t) = ticketer {
    server_config.ticketer = t;
  }
  // Stateful TLS 1.2 session-ID resumption fallback.  TLS 1.3 uses the
  // ticketer above, so this only matters for TLS 1.2 peers.
  server_config.session_storage =
    rustls::server::ServerSessionMemoryCache::new(256);
  Some((server_config, client_cert_verify_error))
}

/// A `ResolvesClientCert` that always returns the same `CertifiedKey`.
/// Used instead of `with_client_auth_cert` to tolerate X.509v1 client certs
/// (which rustls's built-in path rejects via `keys_match`).
#[derive(Debug)]
struct StaticClientCertResolver(Arc<rustls::sign::CertifiedKey>);

impl rustls::client::ResolvesClientCert for StaticClientCertResolver {
  fn resolve(
    &self,
    _root_hint_subjects: &[&[u8]],
    _sigschemes: &[rustls::SignatureScheme],
  ) -> Option<Arc<rustls::sign::CertifiedKey>> {
    Some(self.0.clone())
  }

  fn has_certs(&self) -> bool {
    true
  }
}

/// `ResolvesServerCert` impl that always returns `None`, used when a
/// `tls.Server` is configured without a default cert/key (e.g. one whose
/// only configuration source is `SNICallback`, or a SecureContext returned
/// from `SNICallback` that was created with no cert/key).
#[derive(Debug)]
struct NoCertResolver;

impl rustls::server::ResolvesServerCert for NoCertResolver {
  fn resolve(
    &self,
    _client_hello: rustls::server::ClientHello<'_>,
  ) -> Option<Arc<rustls::sign::CertifiedKey>> {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Verify that clear_out_process drains buffered TLS data when eof is false,
  /// but bails early when eof is already true. This validates the emit_eof fix:
  /// eof must be set *after* clear_out_process, not before.
  #[test]
  fn clear_out_process_bails_when_eof_set() {
    let mut inner = TLSWrapInner::new(Kind::Client, None);

    // With no TLS connection, clear_out_process returns empty regardless.
    let result = inner.clear_out_process();
    assert!(result.data.is_empty());
    assert!(!result.got_eof);

    // When eof is set, clear_out_process should bail immediately.
    inner.eof = true;
    let result = inner.clear_out_process();
    assert!(result.data.is_empty());
    assert!(!result.got_eof);

    // When eof is cleared, it should proceed (still empty since no TLS conn).
    inner.eof = false;
    let result = inner.clear_out_process();
    assert!(result.data.is_empty());
  }

  /// Verify that TLSWrapInner::new starts with alive=true and that
  /// setting alive to false is reflected in the Rc.
  #[test]
  fn alive_flag_lifecycle() {
    let inner = TLSWrapInner::new(Kind::Client, None);
    assert!(inner.alive.get());
    let alive_clone = inner.alive.clone();
    inner.alive.set(false);
    assert!(!alive_clone.get());
  }

  /// teardown() must mark the wrap dead even when no TLS connection was
  /// ever created. Encrypted writes can be in flight without a connection:
  /// the finish_accept error path (e.g. no ALPN overlap) flushes a TLS
  /// alert via enc_out_uv() while tls_conn is still None. If teardown
  /// returns early without setting alive=false, the completing
  /// enc_write_cb dereferences the freed TLSWrapInner (use-after-free).
  #[test]
  fn teardown_marks_dead_even_without_tls_conn() {
    let mut inner = TLSWrapInner::new(Kind::Server, None);
    assert!(inner.tls_conn.is_none());
    // The clone held by an in-flight EncryptedWriteReq.
    let write_req_alive = inner.alive.clone();

    inner.teardown();

    assert!(
      !write_req_alive.get(),
      "teardown must set alive=false even when tls_conn is None"
    );
  }

  /// End-to-end check of the guard in enc_write_cb: when the owning
  /// TLSWrapInner was torn down (alive=false) before the uv write
  /// completed, the callback must not touch the TLSWrapInner. Mirrors the
  /// finish_accept error path where the TLS alert write is still in flight
  /// while JS destroys the socket. The allocation is kept alive here so a
  /// regression fails the assertion below instead of being a
  /// use-after-free.
  #[test]
  fn enc_write_cb_ignores_torn_down_wrap() {
    let mut inner = Box::new(TLSWrapInner::new(Kind::Server, None));
    // Simulate finish_accept's alert flush: one encrypted write in flight,
    // no TLS connection.
    inner.enc_writes_in_flight = 1;
    assert!(inner.tls_conn.is_none());

    // Built exactly like enc_out_uv builds it.
    let req = Box::new(EncryptedWriteReq {
      uv_req: uv_compat::new_write(),
      _data: b"tls alert".to_vec(),
      tls_wrap_inner: &mut *inner as *mut TLSWrapInner,
      alive: inner.alive.clone(),
    });

    // JS destroys the socket before the write completes.
    inner.teardown();

    // The uv write completes now. enc_write_cb must observe alive=false
    // and leave the TLSWrapInner untouched.
    let req_ptr = Box::into_raw(req) as *mut uv_write_t;
    // SAFETY: req_ptr was produced by Box::into_raw of a valid
    // EncryptedWriteReq (uv_req is its first field, repr(C)); enc_write_cb
    // reclaims and frees it.
    unsafe { enc_write_cb(req_ptr, 0) };

    assert_eq!(
      inner.enc_writes_in_flight, 1,
      "enc_write_cb must not dereference a torn-down TLSWrapInner"
    );
  }

  /// Verify that the cycle guard prevents re-entrant cycling.
  #[test]
  fn cycling_guard_prevents_reentry() {
    let mut inner = TLSWrapInner::new(Kind::Client, None);
    assert!(!inner.cycling);
    inner.cycling = true;
    // cycle() should be a no-op when cycling is already true.
    // We can't call cycle() directly without a valid pointer, but we can
    // verify the flag semantics.
    assert!(inner.cycling);
    inner.cycling = false;
    assert!(!inner.cycling);
  }

  /// Build a TLSWrapInner with a real (unconnected) rustls client
  /// connection, using the same config builder as `build_client_config`.
  fn test_client_inner() -> TLSWrapInner {
    // In workspace-wide builds feature unification enables both of rustls'
    // crypto backends, so the process-level provider must be set explicitly.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let config = rustls::ClientConfig::builder_with_protocol_versions(&[
      &rustls::version::TLS13,
      &rustls::version::TLS12,
    ])
    .with_root_certificates(rustls::RootCertStore::empty())
    .with_no_client_auth();
    let conn = rustls::ClientConnection::new(
      Arc::new(config),
      rustls::pki_types::ServerName::try_from("localhost").unwrap(),
    )
    .unwrap();
    let mut inner = TLSWrapInner::new(Kind::Client, None);
    inner.tls_conn = Some(TlsConnection::Client(conn));
    inner
  }

  /// enc_out_collect must gather rustls' encrypted output (the ClientHello
  /// for a fresh client connection) directly into pending_enc_out.
  #[test]
  fn enc_out_collect_gathers_encrypted_output() {
    let mut inner = test_client_inner();
    let action = inner.enc_out_collect();
    // No underlying stream is attached, so no write action is requested...
    assert!(matches!(action, EncOutAction::None));
    // ...but the encrypted ClientHello was collected.
    assert!(!inner.pending_enc_out.is_empty());
    // TLS record header: content type 0x16 (handshake).
    assert_eq!(inner.pending_enc_out[0], 0x16);

    // A second collect with nothing more to write must not duplicate data.
    let len = inner.pending_enc_out.len();
    let _ = inner.enc_out_collect();
    assert_eq!(inner.pending_enc_out.len(), len);
  }

  /// clear_in must feed pending cleartext to rustls in bounded chunks,
  /// advancing pending_cleartext_offset without mutating or losing the
  /// unconsumed remainder, and must release the buffer once consumed.
  #[test]
  fn clear_in_chunks_pending_cleartext_via_offset() {
    const MAX_CLEAR_IN: usize = 48 * 1024;

    let mut inner = test_client_inner();
    // Bypass the handshake gate; rustls buffers pre-handshake plaintext
    // writes internally, which is all this test needs.
    inner.established = true;
    // Lift rustls' default 64 KB plaintext buffer limit so consumption
    // cannot stall mid-payload (the handshake never completes here) and
    // the release path below is exercised deterministically.
    let Some(TlsConnection::Client(conn)) = &mut inner.tls_conn else {
      unreachable!("test_client_inner builds a client connection");
    };
    conn.set_buffer_limit(None);

    let payload: Vec<u8> = (0..200 * 1024).map(|i| (i % 251) as u8).collect();
    inner.pending_cleartext = payload.clone();
    inner.pending_cleartext_offset = 0;

    // The first call must feed a chunk without error.
    inner.clear_in();
    assert!(inner.error.is_none());
    assert!(
      inner.pending_cleartext_offset > 0,
      "first clear_in call made no progress"
    );

    let mut prev_offset = inner.pending_cleartext_offset;
    for _ in 0..64 {
      if inner.pending_cleartext.is_empty() {
        break;
      }
      // The buffer is never mutated while partially consumed.
      assert_eq!(inner.pending_cleartext, payload);
      assert!(inner.pending_cleartext_offset <= payload.len());

      inner.clear_in();
      assert!(inner.error.is_none());
      let offset = inner.pending_cleartext_offset;
      if inner.pending_cleartext.is_empty() {
        break;
      }
      // With the buffer limit lifted every call must make progress,
      // consuming at most one MAX_CLEAR_IN chunk.
      assert!(offset > prev_offset, "clear_in made no progress");
      assert!(offset - prev_offset <= MAX_CLEAR_IN);
      prev_offset = offset;
    }

    // Once fully consumed, the buffer must be released and offset reset.
    assert!(
      inner.pending_cleartext.is_empty(),
      "payload was not fully consumed"
    );
    assert_eq!(inner.pending_cleartext_offset, 0);
  }

  // -------------------------------------------------------------------------
  // X.509v1 certificate chain verification.
  //
  // webpki refuses to parse a v1 certificate, so `verify_certificate_chain`
  // takes over for those chains. Accepting a v1 end-entity certificate under
  // an explicitly supplied CA is Node parity and several upstream Node
  // fixtures depend on it, so these tests pin both directions: a legitimate
  // v1 chain is accepted, and a chain whose signatures do not actually verify
  // is not -- a matching issuer distinguished name is not enough, because a
  // subject DN is public information.
  //
  // `now` is pinned in every test so the fixtures cannot rot into passing or
  // failing for the wrong reason.
  // -------------------------------------------------------------------------

  const ROOT_CA: &[u8] = include_bytes!("testdata/tls_v1/root_ca.der");
  const ROOT_CA_OTHER_KEY: &[u8] =
    include_bytes!("testdata/tls_v1/root_ca_other_key.der");
  const UNRELATED_CA: &[u8] =
    include_bytes!("testdata/tls_v1/unrelated_ca.der");
  const LEAF_V3: &[u8] = include_bytes!("testdata/tls_v1/leaf_v3.der");
  const LEAF_V1: &[u8] = include_bytes!("testdata/tls_v1/leaf_v1.der");
  const LEAF_V1_WRONG_SIGNER: &[u8] =
    include_bytes!("testdata/tls_v1/leaf_v1_wrong_signer.der");
  const LEAF_V1_EXPIRED: &[u8] =
    include_bytes!("testdata/tls_v1/leaf_v1_expired.der");
  const LEAF_V1_SIGALG_MISMATCH: &[u8] =
    include_bytes!("testdata/tls_v1/leaf_v1_sigalg_mismatch.der");
  const CHILD_OF_LEAF_V1: &[u8] =
    include_bytes!("testdata/tls_v1/child_of_leaf_v1.der");
  const NODE_AGENT8: &[u8] = include_bytes!("testdata/tls_v1/node_agent8.der");
  const NODE_AGENT8_WRONG_SIGNER: &[u8] =
    include_bytes!("testdata/tls_v1/node_agent8_wrong_signer.der");
  const NODE_FAKE_STARTCOM_ROOT: &[u8] =
    include_bytes!("testdata/tls_v1/node_fake_startcom_root.der");
  const LEAF_KEY_PKCS8: &[u8] =
    include_bytes!("testdata/tls_v1/leaf_key_pkcs8.der");
  const OTHER_KEY_PKCS8: &[u8] =
    include_bytes!("testdata/tls_v1/other_key_pkcs8.der");

  /// Inside the validity window of every fixture that is meant to be valid,
  /// and after `LEAF_V1_EXPIRED` has expired.
  const NOW: u64 = 1811808000; // 2027-06-01T00:00:00Z
  /// Before `LEAF_V1`'s notBefore.
  const BEFORE_LEAF_V1: u64 = 1780272000; // 2026-06-01T00:00:00Z

  fn at(secs: u64) -> rustls::pki_types::UnixTime {
    rustls::pki_types::UnixTime::since_unix_epoch(
      std::time::Duration::from_secs(secs),
    )
  }

  fn check_chain(
    end_entity: &[u8],
    intermediates: &[&[u8]],
    roots: &[&[u8]],
    now: u64,
  ) -> Result<(), &'static str> {
    let intermediates: Vec<_> = intermediates
      .iter()
      .map(|der| rustls::pki_types::CertificateDer::from(der.to_vec()))
      .collect();
    let roots: Vec<Vec<u8>> = roots.iter().map(|der| der.to_vec()).collect();
    verify_certificate_chain(end_entity, &intermediates, &roots, at(now))
  }

  fn signing_key(pkcs8: &'static [u8]) -> Arc<dyn rustls::sign::SigningKey> {
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(
      rustls::pki_types::PrivatePkcs8KeyDer::from(pkcs8),
    );
    rustls::crypto::aws_lc_rs::default_provider()
      .key_provider
      .load_private_key(key)
      .expect("fixture private key should load")
  }

  // --- the parity direction: this must keep working ------------------------

  #[test]
  fn v1_chain_under_supplied_ca_is_accepted() {
    assert_eq!(check_chain(LEAF_V1, &[], &[ROOT_CA], NOW), Ok(()));
  }

  #[test]
  fn upstream_node_v1_fixture_is_accepted() {
    // agent8 is a genuine X.509v1 fixture from the Node test suite, signed by
    // Node's fake StartCom root. test-http2-https-fallback depends on this.
    assert_eq!(
      check_chain(NODE_AGENT8, &[], &[NODE_FAKE_STARTCOM_ROOT], NOW),
      Ok(())
    );
  }

  #[test]
  fn trusted_certificate_as_end_entity_is_accepted() {
    // A self-signed certificate that is itself in the trust store, which is
    // how fixtures like agent2 are used. webpki calls this CaUsedAsEndEntity;
    // OpenSSL accepts it.
    assert_eq!(check_chain(ROOT_CA, &[], &[ROOT_CA], NOW), Ok(()));
  }

  #[test]
  fn v1_chain_through_a_v1_ca_without_extensions_is_accepted() {
    // An X.509v1 CA carries neither basicConstraints nor keyUsage, so "absent"
    // has to mean "permitted" or no v1 chain could ever verify.
    let ca = parse_certificate(ROOT_CA).unwrap();
    assert_eq!(ca.extensions.is_ca, Some(true));
    let agent8_ca = parse_certificate(NODE_FAKE_STARTCOM_ROOT).unwrap();
    assert_eq!(agent8_ca.extensions.is_ca, Some(true));
    assert_eq!(agent8_ca.extensions.key_cert_sign, None);
    assert!(usable_as_ca(&agent8_ca, 0, true));
    // A v1 certificate configured as a trust anchor may sign; one the peer
    // sends as an intermediate may not, since only `cA` makes an intermediate
    // a CA.
    let v1_leaf = parse_certificate(LEAF_V1).unwrap();
    assert_eq!(v1_leaf.extensions, CertExtensions::default());
    assert!(usable_as_ca(&v1_leaf, 0, true));
    assert!(!usable_as_ca(&v1_leaf, 0, false));
  }

  // --- invalid chains: these must be rejected ----------------------------

  #[test]
  fn v1_with_matching_issuer_dn_but_other_signer_is_rejected() {
    // A v1 certificate whose issuer DN matches the trusted CA but which is
    // signed by a different key. A matching DN alone is not enough.
    let leaf = parse_certificate(LEAF_V1_WRONG_SIGNER).unwrap();
    let root = parse_certificate(ROOT_CA).unwrap();
    assert_eq!(
      leaf.issuer, root.subject,
      "fixture must share the trusted CA's DN for this test to mean anything"
    );
    assert_eq!(
      check_chain(LEAF_V1_WRONG_SIGNER, &[], &[ROOT_CA], NOW),
      Err("CERT_SIGNATURE_FAILURE")
    );
  }

  #[test]
  fn upstream_node_v1_fixture_with_other_signer_is_rejected() {
    assert_eq!(
      check_chain(
        NODE_AGENT8_WRONG_SIGNER,
        &[],
        &[NODE_FAKE_STARTCOM_ROOT],
        NOW
      ),
      Err("CERT_SIGNATURE_FAILURE")
    );
  }

  #[test]
  fn expired_v1_is_rejected() {
    assert_eq!(
      check_chain(LEAF_V1_EXPIRED, &[], &[ROOT_CA], NOW),
      Err("CERT_HAS_EXPIRED")
    );
  }

  #[test]
  fn not_yet_valid_v1_is_rejected() {
    assert_eq!(
      check_chain(LEAF_V1, &[], &[ROOT_CA], BEFORE_LEAF_V1),
      Err("CERT_NOT_YET_VALID")
    );
  }

  #[test]
  fn v1_with_no_candidate_issuer_is_rejected() {
    assert_eq!(
      check_chain(LEAF_V1, &[], &[UNRELATED_CA], NOW),
      Err("UNABLE_TO_VERIFY_LEAF_SIGNATURE")
    );
  }

  #[test]
  fn self_signed_v1_outside_the_trust_store_is_rejected() {
    // This root shares the trusted CA's DN but is not itself trusted.
    assert_eq!(
      check_chain(ROOT_CA_OTHER_KEY, &[], &[UNRELATED_CA], NOW),
      Err("DEPTH_ZERO_SELF_SIGNED_CERT")
    );
  }

  #[test]
  fn v1_signed_by_a_non_ca_certificate_is_rejected() {
    // `CHILD_OF_LEAF_V1` really is signed by LEAF_V3's key, so only
    // LEAF_V3's basicConstraints CA:FALSE stands between it and acceptance.
    let leaf_v3 = parse_certificate(LEAF_V3).unwrap();
    assert_eq!(leaf_v3.extensions.is_ca, Some(false));
    let child = parse_certificate(CHILD_OF_LEAF_V1).unwrap();
    assert!(
      signature_is_valid(&child, &leaf_v3),
      "fixture must be genuinely signed by the non-CA leaf"
    );
    assert!(
      check_chain(CHILD_OF_LEAF_V1, &[LEAF_V3], &[ROOT_CA], NOW).is_err()
    );
  }

  #[test]
  fn intermediate_without_basic_constraints_is_rejected() {
    // `LEAF_V1` shares `LEAF_V3`'s subject and key, so `CHILD_OF_LEAF_V1` is
    // validly signed by it too. A v1 certificate has no basicConstraints and
    // therefore cannot act as an intermediate CA: `openssl verify -untrusted
    // leaf_v1 child_of_leaf_v1` reports error 79, invalid CA certificate.
    let leaf_v1 = parse_certificate(LEAF_V1).unwrap();
    let child = parse_certificate(CHILD_OF_LEAF_V1).unwrap();
    assert!(
      signature_is_valid(&child, &leaf_v1),
      "fixture must be genuinely signed by the v1 leaf"
    );
    assert!(
      check_chain(CHILD_OF_LEAF_V1, &[LEAF_V1], &[ROOT_CA], NOW).is_err()
    );
  }

  #[test]
  fn signature_algorithm_mismatch_is_rejected() {
    // The outer signatureAlgorithm is not covered by the signature, so it must
    // agree with the copy inside tbsCertificate, as OpenSSL requires.
    let mismatched = parse_certificate(LEAF_V1_SIGALG_MISMATCH).unwrap();
    assert_ne!(
      mismatched.signature_algorithm, mismatched.tbs_signature_algorithm,
      "fixture must actually have mismatched algorithm identifiers"
    );
    assert!(
      check_chain(LEAF_V1_SIGALG_MISMATCH, &[], &[ROOT_CA], NOW).is_err()
    );
  }

  #[test]
  fn v1_chain_with_no_trusted_roots_is_rejected() {
    // The default trust path never populates `root_cert_ders` -- only the
    // caller's own `ca` option and `tls.setDefaultCACertificates()` do -- so
    // with no supplied roots this path can never succeed, and the bundled
    // Mozilla roots stay out of reach of it.
    assert!(check_chain(LEAF_V1, &[], &[], NOW).is_err());
    assert!(check_chain(NODE_AGENT8, &[], &[], NOW).is_err());
    assert!(check_chain(LEAF_V1_WRONG_SIGNER, &[], &[], NOW).is_err());
  }

  // --- CertificateVerify over a v1 certificate -----------------------------

  #[test]
  fn handshake_signature_over_v1_cert_is_verified() {
    const MESSAGE: &[u8] = b"a TLS CertificateVerify transcript";
    let scheme = rustls::SignatureScheme::RSA_PSS_SHA256;

    // The holder of the certificate's private key produces a signature that
    // verifies against the certificate.
    let key = signing_key(LEAF_KEY_PKCS8);
    let signer = key.choose_scheme(&[scheme]).unwrap();
    let signature = signer.sign(MESSAGE).unwrap();
    assert_eq!(
      verify_signature_with_unparsed_cert(
        LEAF_V1, scheme, MESSAGE, &signature, true
      ),
      SignatureCheck::Valid
    );

    // A signature made with a different key does not verify.
    let other_key = signing_key(OTHER_KEY_PKCS8);
    let other_signer = other_key.choose_scheme(&[scheme]).unwrap();
    let other_signature = other_signer.sign(MESSAGE).unwrap();
    assert_eq!(
      verify_signature_with_unparsed_cert(
        LEAF_V1,
        scheme,
        MESSAGE,
        &other_signature,
        true
      ),
      SignatureCheck::Invalid
    );

    // A tampered transcript does not verify either.
    assert_eq!(
      verify_signature_with_unparsed_cert(
        LEAF_V1,
        scheme,
        b"a different transcript",
        &signature,
        true
      ),
      SignatureCheck::Invalid
    );

    // Same for TLS 1.2, which tries every algorithm mapped to the scheme.
    assert_eq!(
      verify_signature_with_unparsed_cert(
        LEAF_V1, scheme, MESSAGE, &signature, false
      ),
      SignatureCheck::Valid
    );
    assert_eq!(
      verify_signature_with_unparsed_cert(
        LEAF_V1,
        scheme,
        MESSAGE,
        &other_signature,
        false
      ),
      SignatureCheck::Invalid
    );
  }

  // --- certificate/key pairing ---------------------------------------------

  #[test]
  fn keys_match_accepts_a_v1_cert_with_its_own_key() {
    let key = signing_key(LEAF_KEY_PKCS8);
    let certified = rustls::sign::CertifiedKey::new(
      vec![rustls::pki_types::CertificateDer::from(LEAF_V1.to_vec())],
      key.clone(),
    );
    // webpki cannot parse the v1 cert, so the built-in check cannot answer.
    assert!(is_unsupported_cert_version_error(
      &certified.keys_match().unwrap_err()
    ));
    assert_eq!(keys_match_allowing_v1(&certified, key.as_ref()), Ok(()));
  }

  #[test]
  fn keys_match_rejects_a_v1_cert_without_its_key() {
    // A v1 certificate must be paired with its own private key.
    let other_key = signing_key(OTHER_KEY_PKCS8);
    let certified = rustls::sign::CertifiedKey::new(
      vec![rustls::pki_types::CertificateDer::from(LEAF_V1.to_vec())],
      other_key.clone(),
    );
    assert!(matches!(
      keys_match_allowing_v1(&certified, other_key.as_ref()),
      Err(rustls::Error::InconsistentKeys(
        rustls::InconsistentKeys::KeyMismatch
      ))
    ));
  }

  #[test]
  fn keys_match_still_defers_to_webpki_for_v3() {
    // A v3 certificate is parseable, so the built-in check answers and the
    // fallback never runs.
    let key = signing_key(LEAF_KEY_PKCS8);
    let certified = rustls::sign::CertifiedKey::new(
      vec![rustls::pki_types::CertificateDer::from(LEAF_V3.to_vec())],
      key.clone(),
    );
    assert_eq!(certified.keys_match(), Ok(()));
    assert_eq!(keys_match_allowing_v1(&certified, key.as_ref()), Ok(()));

    let other_key = signing_key(OTHER_KEY_PKCS8);
    let mismatched = rustls::sign::CertifiedKey::new(
      vec![rustls::pki_types::CertificateDer::from(LEAF_V3.to_vec())],
      other_key.clone(),
    );
    assert!(keys_match_allowing_v1(&mismatched, other_key.as_ref()).is_err());
  }

  // --- parser ---------------------------------------------------------------

  #[test]
  fn certificate_parser_extracts_the_signed_fields() {
    let leaf = parse_certificate(LEAF_V1).unwrap();
    let root = parse_certificate(ROOT_CA).unwrap();
    assert_eq!(leaf.issuer, root.subject);
    assert_ne!(leaf.subject, leaf.issuer);
    assert!(leaf.not_before < leaf.not_after);
    // The SPKI slice must be the whole SubjectPublicKeyInfo element, since
    // that is what `SigningKey::public_key()` returns.
    assert_eq!(leaf.spki.first(), Some(&0x30));
    assert!(signature_is_valid(&leaf, &root));
    // ... and only under the real issuer.
    assert!(!signature_is_valid(
      &leaf,
      &parse_certificate(ROOT_CA_OTHER_KEY).unwrap()
    ));
  }

  #[test]
  fn certificate_parser_rejects_malformed_input() {
    assert!(parse_certificate(&[]).is_none());
    assert!(parse_certificate(&[0x30, 0x80]).is_none()); // indefinite length
    assert!(parse_certificate(&LEAF_V1[..LEAF_V1.len() - 1]).is_none());
    assert!(parse_certificate(&LEAF_V1[1..]).is_none());
    // Trailing bytes after the Certificate SEQUENCE.
    let mut trailing = LEAF_V1.to_vec();
    trailing.push(0);
    assert!(parse_certificate(&trailing).is_none());
  }

  const NAME_CONSTRAINED_CA: &[u8] =
    include_bytes!("testdata/tls_v1/name_constrained_ca.der");
  const LEAF_V1_UNDER_CONSTRAINED_CA: &[u8] =
    include_bytes!("testdata/tls_v1/leaf_v1_under_constrained_ca.der");
  const LEAF_V3_EKU_CLIENT_ONLY: &[u8] =
    include_bytes!("testdata/tls_v1/leaf_v3_eku_client_only.der");
  const CROSS_SIGNED_UNTRUSTED: &[u8] =
    include_bytes!("testdata/tls_v1/cross_signed_untrusted.der");
  const CROSS_SIGNED_TRUSTED: &[u8] =
    include_bytes!("testdata/tls_v1/cross_signed_trusted.der");
  const LEAF_V1_UNDER_CROSS_SIGNED: &[u8] =
    include_bytes!("testdata/tls_v1/leaf_v1_under_cross_signed.der");

  #[test]
  fn end_entity_without_server_auth_purpose_is_rejected() {
    // `openssl verify -purpose sslserver` reports error 26 for this
    // certificate, and webpki rejects the same thing on the chains it can
    // parse via `KeyUsage::server_auth`.
    let leaf = parse_certificate(LEAF_V3_EKU_CLIENT_ONLY).unwrap();
    assert_eq!(leaf.extensions.server_auth, Some(false));
    assert_eq!(
      check_chain(LEAF_V3_EKU_CLIENT_ONLY, &[], &[ROOT_CA], NOW),
      Err("INVALID_PURPOSE")
    );
    // A certificate with no extendedKeyUsage at all is unrestricted, which is
    // every v1 certificate.
    assert_eq!(
      parse_certificate(LEAF_V1).unwrap().extensions.server_auth,
      None
    );
    assert_eq!(check_chain(LEAF_V1, &[], &[ROOT_CA], NOW), Ok(()));
  }

  #[test]
  fn ca_with_unhandled_constraints_is_refused() {
    // This verifier does not evaluate nameConstraints, so it refuses a CA that
    // carries them instead of treating the CA as unconstrained. That is
    // deliberately stricter than OpenSSL, which accepts this exact chain
    // (`openssl verify -CAfile name_constrained_ca leaf` reports OK, because
    // the leaf has no dNSName to constrain). Erring towards rejection is the
    // safe direction: the alternative is honouring none of the constraint.
    let ca = parse_certificate(NAME_CONSTRAINED_CA).unwrap();
    assert!(ca.extensions.unhandled_constraint);
    assert_eq!(ca.extensions.is_ca, Some(true));
    assert!(!usable_as_ca(&ca, 0, true));
    assert!(!usable_as_ca(&ca, 0, false));
    // The chain is genuinely signed, so only the constraint stands in the way.
    let leaf = parse_certificate(LEAF_V1_UNDER_CONSTRAINED_CA).unwrap();
    assert!(signature_is_valid(&leaf, &ca));
    assert_eq!(
      check_chain(
        LEAF_V1_UNDER_CONSTRAINED_CA,
        &[],
        &[NAME_CONSTRAINED_CA],
        NOW
      ),
      Err("UNHANDLED_CRITICAL_EXTENSION")
    );
  }

  #[test]
  fn path_building_backtracks_past_a_dead_end() {
    // Two intermediates share a subject DN *and* a public key -- cross-signing
    // -- but only one of them chains to the trusted root. The first one
    // verifies the leaf's signature just as well, so a search that commits to
    // it would reject a chain OpenSSL accepts.
    let untrusted = parse_certificate(CROSS_SIGNED_UNTRUSTED).unwrap();
    let trusted = parse_certificate(CROSS_SIGNED_TRUSTED).unwrap();
    let leaf = parse_certificate(LEAF_V1_UNDER_CROSS_SIGNED).unwrap();
    assert_eq!(untrusted.subject, trusted.subject);
    assert_eq!(untrusted.spki, trusted.spki);
    assert_ne!(untrusted.issuer, trusted.issuer);
    assert!(signature_is_valid(&leaf, &untrusted));
    assert!(signature_is_valid(&leaf, &trusted));

    // Dead end first in the list.
    assert_eq!(
      check_chain(
        LEAF_V1_UNDER_CROSS_SIGNED,
        &[CROSS_SIGNED_UNTRUSTED, CROSS_SIGNED_TRUSTED],
        &[ROOT_CA],
        NOW
      ),
      Ok(())
    );
    // ... and the other order, which never needed backtracking.
    assert_eq!(
      check_chain(
        LEAF_V1_UNDER_CROSS_SIGNED,
        &[CROSS_SIGNED_TRUSTED, CROSS_SIGNED_UNTRUSTED],
        &[ROOT_CA],
        NOW
      ),
      Ok(())
    );
    // With only the dead end available the chain must still be rejected.
    assert!(
      check_chain(
        LEAF_V1_UNDER_CROSS_SIGNED,
        &[CROSS_SIGNED_UNTRUSTED],
        &[ROOT_CA],
        NOW
      )
      .is_err()
    );
  }

  const CLIENT_AUTH_ONLY_CA: &[u8] =
    include_bytes!("testdata/tls_v1/client_auth_only_ca.der");
  const LEAF_V1_UNDER_CLIENT_AUTH_CA: &[u8] =
    include_bytes!("testdata/tls_v1/leaf_v1_under_client_auth_ca.der");

  #[test]
  fn issuer_without_server_auth_purpose_is_rejected() {
    // `openssl verify -purpose sslserver` reports error 26 *at depth 1* for
    // this chain, so OpenSSL enforces extendedKeyUsage on the issuer and not
    // only on the leaf. The leaf here is v1 and therefore carries no
    // extensions of its own, which is exactly the case a leaf-only check
    // would miss.
    let ca = parse_certificate(CLIENT_AUTH_ONLY_CA).unwrap();
    assert_eq!(ca.extensions.server_auth, Some(false));
    assert_eq!(ca.extensions.is_ca, Some(true));
    assert!(!usable_as_ca(&ca, 0, true));
    assert!(!usable_as_ca(&ca, 0, false));
    // The chain is genuinely signed, so only the purpose stands in the way.
    let leaf = parse_certificate(LEAF_V1_UNDER_CLIENT_AUTH_CA).unwrap();
    assert_eq!(leaf.extensions, CertExtensions::default());
    assert!(signature_is_valid(&leaf, &ca));
    assert!(
      check_chain(
        LEAF_V1_UNDER_CLIENT_AUTH_CA,
        &[],
        &[CLIENT_AUTH_ONLY_CA],
        NOW
      )
      .is_err()
    );
  }

  #[test]
  fn unrecognised_critical_extension_is_rejected() {
    fn parse(extension: &[u8]) -> Option<CertExtensions> {
      let mut der = vec![0x30, extension.len() as u8];
      der.extend_from_slice(extension);
      parse_extensions(&der)
    }
    // An extension with a private OID (1.3.6.1.4.1.99999.1), value an empty
    // OCTET STRING, built both non-critical and critical.
    let make = |critical: bool| {
      let oid: &[u8] = &[0x2b, 0x06, 0x01, 0x04, 0x01, 0x8d, 0xa3, 0x1f, 0x01];
      let mut body = vec![0x06, oid.len() as u8];
      body.extend_from_slice(oid);
      if critical {
        body.extend_from_slice(&[0x01, 0x01, 0xff]);
      }
      body.extend_from_slice(&[0x04, 0x00]);
      let mut der = vec![0x30, body.len() as u8];
      der.extend_from_slice(&body);
      der
    };
    // Non-critical: safe to ignore, as every verifier does.
    assert!(
      !parse(&make(false)).unwrap().unhandled_constraint,
      "a non-critical unknown extension must be ignored"
    );
    // Critical: this code cannot evaluate it, so it must not read as absent.
    assert!(parse(&make(true)).unwrap().unhandled_constraint);

    // A critical extension that this code does understand, or that is safe to
    // ignore, must not trip the flag. basicConstraints is marked critical by
    // essentially every CA, including the test fixtures.
    assert!(
      !parse_certificate(ROOT_CA)
        .unwrap()
        .extensions
        .unhandled_constraint
    );
    assert!(
      !parse_certificate(LEAF_V3)
        .unwrap()
        .extensions
        .unhandled_constraint
    );
  }

  #[test]
  fn path_search_is_bounded() {
    // A peer controls the intermediates it sends. Without a bound on signature
    // verifications the backtracking search can be made to branch
    // exponentially, so a pile of same-subject dead ends must still resolve
    // promptly rather than burning CPU.
    let dead_ends = vec![CROSS_SIGNED_UNTRUSTED; 200];
    let started = std::time::Instant::now();
    assert!(
      check_chain(LEAF_V1_UNDER_CROSS_SIGNED, &dead_ends, &[ROOT_CA], NOW)
        .is_err()
    );
    // Generous: 100 RSA-2048 verifications take single-digit milliseconds.
    assert!(
      started.elapsed() < std::time::Duration::from_secs(10),
      "bounded search took {:?}",
      started.elapsed()
    );

    // The bound must not get in the way of a path that is found early.
    let mut with_trusted = vec![CROSS_SIGNED_TRUSTED];
    with_trusted.extend_from_slice(&dead_ends);
    assert_eq!(
      check_chain(LEAF_V1_UNDER_CROSS_SIGNED, &with_trusted, &[ROOT_CA], NOW),
      Ok(())
    );
  }

  #[test]
  fn malformed_constraint_extensions_fail_closed() {
    fn parse(extension: &[u8]) -> Option<CertExtensions> {
      let mut der = vec![0x30, extension.len() as u8];
      der.extend_from_slice(extension);
      parse_extensions(&der)
    }
    // basicConstraints with pathLenConstraint 0.
    assert_eq!(
      parse(&[
        0x30, 0x0f, 0x06, 0x03, 0x55, 0x1d, 0x13, 0x04, 0x08, 0x30, 0x06, 0x01,
        0x01, 0xff, 0x02, 0x01, 0x00,
      ])
      .unwrap()
      .path_len,
      Some(0)
    );
    // A negative pathLenConstraint must not read as "unconstrained".
    assert_eq!(
      parse(&[
        0x30, 0x0f, 0x06, 0x03, 0x55, 0x1d, 0x13, 0x04, 0x08, 0x30, 0x06, 0x01,
        0x01, 0xff, 0x02, 0x01, 0xff,
      ]),
      None
    );
    // A cA BOOLEAN that is neither 0x00 nor 0xff is not valid DER.
    assert_eq!(
      parse(&[
        0x30, 0x0c, 0x06, 0x03, 0x55, 0x1d, 0x13, 0x04, 0x05, 0x30, 0x03, 0x01,
        0x01, 0x01,
      ]),
      None
    );
    // keyUsage asserting keyCertSign, with 1 unused bit: 0x06 = bits 5 and 6.
    assert_eq!(
      parse(&[
        0x30, 0x0b, 0x06, 0x03, 0x55, 0x1d, 0x0f, 0x04, 0x04, 0x03, 0x02, 0x01,
        0x06,
      ])
      .unwrap()
      .key_cert_sign,
      Some(true)
    );
    // The same octet with 3 unused bits puts keyCertSign inside the unused
    // region, which DER forbids and which must not read as asserted.
    assert_eq!(
      parse(&[
        0x30, 0x0b, 0x06, 0x03, 0x55, 0x1d, 0x0f, 0x04, 0x04, 0x03, 0x02, 0x03,
        0x04,
      ]),
      None
    );
    // An unused-bit count above 7 is never valid.
    assert_eq!(
      parse(&[
        0x30, 0x0b, 0x06, 0x03, 0x55, 0x1d, 0x0f, 0x04, 0x04, 0x03, 0x02, 0x08,
        0x00,
      ]),
      None
    );
  }

  #[test]
  fn duplicate_extensions_are_rejected() {
    // basicConstraints with cA FALSE and with cA TRUE, as whole Extension
    // elements: SEQUENCE { OID 2.5.29.19, OCTET STRING { SEQUENCE { BOOLEAN } } }
    const CA_FALSE: &[u8] = &[
      0x30, 0x0c, 0x06, 0x03, 0x55, 0x1d, 0x13, 0x04, 0x05, 0x30, 0x03, 0x01,
      0x01, 0x00,
    ];
    const CA_TRUE: &[u8] = &[
      0x30, 0x0c, 0x06, 0x03, 0x55, 0x1d, 0x13, 0x04, 0x05, 0x30, 0x03, 0x01,
      0x01, 0xff,
    ];
    fn parse(extensions: &[u8]) -> Option<Option<bool>> {
      Some(parse_extensions(extensions)?.is_ca)
    }
    fn sequence_of(body: &[u8]) -> Vec<u8> {
      let mut der = vec![0x30, body.len() as u8];
      der.extend_from_slice(body);
      der
    }

    // A single basicConstraints is read normally, in both directions.
    assert_eq!(parse(&sequence_of(CA_TRUE)), Some(Some(true)));
    assert_eq!(parse(&sequence_of(CA_FALSE)), Some(Some(false)));

    // Two copies make the certificate unparseable rather than letting the
    // second override the first.
    let mut both = CA_FALSE.to_vec();
    both.extend_from_slice(CA_TRUE);
    assert_eq!(parse(&sequence_of(&both)), None);
    let mut reversed = CA_TRUE.to_vec();
    reversed.extend_from_slice(CA_FALSE);
    assert_eq!(parse(&sequence_of(&reversed)), None);
  }

  #[test]
  fn der_time_parsing_is_strict() {
    let utc = |s: &[u8]| {
      der_time_secs(&DerElement {
        tag: 0x17,
        all: s,
        content: s,
      })
    };
    // 1970-01-01T00:00:00Z
    assert_eq!(utc(b"700101000000Z"), Some(0));
    // UTCTime years 50..=99 mean 19xx.
    assert_eq!(utc(b"491231235959Z"), Some(2524607999));
    // Rejected: no trailing Z, out-of-range fields, impossible dates, and the
    // seconds-less form OpenSSL tolerates but RFC 5280 does not permit.
    assert_eq!(utc(b"7001010000000"), None);
    assert_eq!(utc(b"701301000000Z"), None);
    assert_eq!(utc(b"700132000000Z"), None);
    assert_eq!(utc(b"700229000000Z"), None); // 1970 was not a leap year
    assert_eq!(utc(b"700101250000Z"), None);
    assert_eq!(utc(b"7001010000Z"), None);
    assert_eq!(utc(b"70010100000aZ"), None);
    // GeneralizedTime, four-digit year.
    let generalized = |s: &[u8]| {
      der_time_secs(&DerElement {
        tag: 0x18,
        all: s,
        content: s,
      })
    };
    assert_eq!(generalized(b"19700101000000Z"), Some(0));
    assert_eq!(generalized(b"20000229000000Z"), Some(951782400)); // 2000 was a leap year
    assert_eq!(generalized(b"19000229000000Z"), None); // 1900 was not
  }
}
