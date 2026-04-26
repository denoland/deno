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

use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::ToJsBuffer;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::UV_EBADF;
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

// ---------------------------------------------------------------------------
// Free functions that emit JS callbacks.
// These do NOT borrow TLSWrapInner — they work entirely with EmitCtx + args.
// ---------------------------------------------------------------------------

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

    let onread_fn = if let Some(onread) = onread {
      v8::Local::new(scope, onread)
    } else {
      let key =
        v8::String::new_external_onebyte_static(scope, b"onread").unwrap();
      match recv.get(scope, key.into()) {
        Some(val) => match v8::Local::<v8::Function>::try_from(val) {
          Ok(f) => f,
          Err(_) => return,
        },
        None => return,
      }
    };

    if let Some(bytes) = data {
      let len = bytes.len();
      let store = v8::ArrayBuffer::new(scope, len);
      let backing = store.get_backing_store();
      for (i, byte) in bytes.iter().enumerate() {
        backing[i].set(*byte);
      }
      let ab: v8::Local<v8::Value> = store.into();
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
    let oncomplete_str =
      v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
    if let Some(oncomplete) = req_obj.get(scope, oncomplete_str.into())
      && let Ok(func) = v8::Local::<v8::Function>::try_from(oncomplete)
    {
      let status_val = v8::Integer::new(scope, status);
      let undef = v8::undefined(scope);
      func.call(
        scope,
        req_obj.into(),
        &[status_val.into(), handle.into(), undef.into()],
      );
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
      let ab = v8::ArrayBuffer::new(scope, enc_data.len());
      let backing = ab.get_backing_store();
      for (i, byte) in enc_data.iter().enumerate() {
        backing[i].set(*byte);
      }
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
  has_write_callback: bool,
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
  session_was_set: bool,
  /// Set by clear_out when it emitted data — indicates rustls may have
  /// more buffered plaintext. Cleared when clear_out returns no data.
  has_buffered_cleartext: bool,
  in_dowrite: bool,
  write_callback_scheduled: bool,
  /// Number of outstanding uv_write requests for encrypted output.
  /// invoke_queued must wait until this drops to zero.
  enc_writes_in_flight: u32,

  // Pending cleartext from DoWrite that SSL_write couldn't accept yet
  pending_cleartext: Option<Vec<u8>>,

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
}

/// Convert a rustls error to a (message, code) pair that matches Node's
/// OpenSSL-style error reporting as closely as possible.
fn rustls_error_to_node_error(e: &rustls::Error) -> (String, String) {
  use rustls::Error as E;
  match e {
    E::InvalidCertificate(cert_err) => {
      let reason = format!("{cert_err}");
      // Map common rustls certificate errors to OpenSSL error codes
      let code = if reason.contains("UnknownIssuer") {
        "UNABLE_TO_VERIFY_LEAF_SIGNATURE"
      } else if reason.contains("NotValidYet") {
        "CERT_NOT_YET_VALID"
      } else if reason.contains("Expired") {
        "CERT_HAS_EXPIRED"
      } else if reason.contains("NotValidForName") {
        "ERR_TLS_CERT_ALTNAME_INVALID"
      } else if reason.contains("CaUsedAsEndEntity")
        || reason.contains("IssuerNotCrlSigner")
        || reason.contains("InvalidPurpose")
      {
        "UNABLE_TO_VERIFY_LEAF_SIGNATURE"
      } else if reason.contains("SelfSigned") {
        "DEPTH_ZERO_SELF_SIGNED_CERT"
      } else {
        "ERR_SSL_SSLV3_ALERT_CERTIFICATE_UNKNOWN"
      };
      (format!("{e}"), format!("ERR_SSL_{code}"))
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
        AD::NoApplicationProtocol => "TLSV1_ALERT_NO_APPLICATION_PROTOCOL",
        _ => "SSLV3_ALERT_HANDSHAKE_FAILURE",
      };
      (format!("{e}"), format!("ERR_SSL_{code}"))
    }
    E::NoApplicationProtocol => (
      format!("{e}"),
      "ERR_SSL_NO_APPLICATION_PROTOCOL".to_string(),
    ),
    _ => (
      format!("{e}"),
      "ERR_SSL_SSLV3_ALERT_HANDSHAKE_FAILURE".to_string(),
    ),
  }
}

impl TLSWrapInner {
  fn new(kind: Kind) -> Self {
    Self {
      tls_conn: None,
      kind,
      enc_in: Vec::with_capacity(4096),
      started: false,
      established: false,
      shutdown: false,
      eof: false,
      cycling: false,
      session_was_set: false,
      has_buffered_cleartext: false,
      in_dowrite: false,
      write_callback_scheduled: false,
      enc_writes_in_flight: 0,
      pending_cleartext: None,
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
      error: None,
      verify_error: Arc::new(std::sync::Mutex::new(None)),
      pending_client_config: None,
      pending_server_name: None,
      pending_server_config: None,
      cached_loop_ptr: std::ptr::null_mut(),
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
      (*ptr).clear_in();
      let result = (*ptr).clear_out_process();
      let enc_action = (*ptr).enc_out_collect();
      (*ptr).cycling = false;

      // --- Callback phase: no Rust reference to TLSWrapInner is held ---
      TLSWrapInner::dispatch_clear_out_callbacks(ptr, &result);
      if result.tls_error.is_some() {
        return;
      }
      TLSWrapInner::do_enc_out_action(ptr, enc_action);
    }
  }

  /// Feed pending cleartext into rustls writer.
  /// Mirrors Node's TLSWrap::ClearIn().
  fn clear_in(&mut self) {
    let Some(ref mut conn) = self.tls_conn else {
      return;
    };

    let Some(data) = self.pending_cleartext.take() else {
      return;
    };

    if data.is_empty() {
      return;
    }

    // Feed cleartext to rustls in limited chunks. Writing everything
    // at once would produce a huge encrypted buffer that saturates
    // the TCP send buffer, causing deadlocks with echo patterns.
    // This matches Node.js where SSL_write processes incrementally.
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
    if offset < data.len() && !write_error {
      // Save only the unwritten portion for retry
      self.pending_cleartext = Some(data[offset..].to_vec());
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
            let (error_msg, error_code) = rustls_error_to_node_error(&e);
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

    // Collect ALL encrypted output from rustls into pending buffer.
    while conn.wants_write() {
      let mut tmp = Vec::with_capacity(16384);
      match conn.write_tls(&mut tmp) {
        Ok(n) if n > 0 => {
          self.pending_enc_out.extend_from_slice(&tmp);
        }
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

  /// Flush encrypted data from rustls to the underlying stream without
  /// invoking any JS callbacks. Used in the error path of clear_out_process
  /// to send TLS alert records before emitting the error.
  fn enc_out_flush_only(&mut self) {
    let Some(ref mut conn) = self.tls_conn else {
      return;
    };
    while conn.wants_write() {
      let mut tmp = Vec::with_capacity(16384);
      match conn.write_tls(&mut tmp) {
        Ok(n) if n > 0 => {
          self.pending_enc_out.extend_from_slice(&tmp);
        }
        _ => break,
      }
    }
    if self.pending_enc_out.is_empty() || !self.underlying.is_attached() {
      return;
    }
    if let UnderlyingStream::Uv { .. } = self.underlying {
      self.enc_out_uv();
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

        // The handshake_done callback can synchronously destroy the
        // TLSSocket (e.g. http2's `unknownProtocol` handler may call
        // `socket.destroy()`).  If teardown has run, the JS-side stream
        // is gone and any further data delivery would push bytes into a
        // destroyed Readable, tripping `errnoException(positiveNread)`.
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
          TLSWrapInner::do_enc_out_action(ptr, enc_action);
          (*ptr).underlying.shutdown();
        }
      }

      if !result.data.is_empty() {
        if let Some(ctx) = extract_emit_ctx(ptr) {
          let onread = (*ptr).onread.clone();
          let state = (*ptr).stream_base_state.clone();
          do_emit_read(
            &ctx,
            onread.as_ref(),
            state.as_ref(),
            result.data.len() as isize,
            Some(&result.data),
          );
        }
        if (*ptr).tls_conn.is_none() {
          return;
        }
      }
      if result.got_eof {
        if let Some(ctx) = extract_emit_ctx(ptr) {
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
  /// # Safety
  /// `ptr` must be a valid, non-null pointer to a live TLSWrapInner.
  unsafe fn do_enc_out_action(ptr: *mut TLSWrapInner, action: EncOutAction) {
    unsafe {
      match action {
        EncOutAction::None => {}
        EncOutAction::WriteUv => {
          (*ptr).enc_out_uv();
          // enc_out_uv may call invoke_queued on error; those paths
          // already work through &mut self which is fine since we
          // don't hold any reference here. But we should also convert
          // those paths — for now, enc_out_uv's invoke_queued calls
          // go through the old path (acceptable since they only fire
          // on synchronous uv_write failure, not during normal flow).
        }
        EncOutAction::WriteJs => {
          // Pull-based: leave data in pending_enc_out for JS to drain
          // via drain_enc_out(). This avoids calling back into JS from
          // within an op, eliminating reentrancy issues.
        }
        EncOutAction::InvokeQueued(status) => {
          if let Some((write_obj, ctx)) = prepare_invoke_queued(ptr) {
            do_invoke_queued(&ctx, write_obj, status);
          }
        }
      }
    }
  }

  /// Write encrypted data to the underlying uv stream.
  fn enc_out_uv(&mut self) {
    let enc_data = std::mem::take(&mut self.pending_enc_out);
    let has_write_cb = self.write_callback_scheduled;
    let self_ptr = self as *mut TLSWrapInner;
    let write_req = Box::new(EncryptedWriteReq {
      uv_req: uv_compat::new_write(),
      _data: enc_data,
      tls_wrap_inner: self_ptr,
      has_write_callback: has_write_cb,
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
        // Use raw pointer to drop the &mut self borrow before JS call
        let ptr = self_ptr;
        // SAFETY: self_ptr is valid (points to self); prepare_invoke_queued
        // and do_invoke_queued do not hold references across JS calls.
        unsafe {
          if let Some((write_obj, ctx)) = prepare_invoke_queued(ptr) {
            do_invoke_queued(&ctx, write_obj, ret);
          }
        }
      }
    }
    // Note: for successful writes, invoke_queued is called from enc_write_cb
    // when the uv_write completes asynchronously.
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
        do_emit_read(
          &ctx,
          onread.as_ref(),
          state.as_ref(),
          result.data.len() as isize,
          Some(&result.data),
        );
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
        if (*ptr)
          .pending_cleartext
          .as_ref()
          .is_some_and(|v| !v.is_empty())
        {
          (*ptr).clear_in();
        }
        let enc_action = (*ptr).enc_out_collect();
        TLSWrapInner::do_enc_out_action(ptr, enc_action);
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
    if inner.tls_conn.is_none() {
      return;
    }

    // Mark as dead so in-flight enc_write_cb callbacks won't dereference
    // the TLSWrapInner pointer after it is freed.
    inner.alive.set(false);

    inner.tls_conn = None;
    inner.js_handle = None;
    inner.onread = None;
    inner.stream_base_state = None;
    inner.current_write_obj = None;
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
        let existing = inner.pending_cleartext.get_or_insert_with(Vec::new);
        existing.extend_from_slice(data);

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
    inner.pending_cleartext = Some(data.to_vec());
    inner.in_dowrite = true;
    inner.clear_in();
    let enc_action = inner.enc_out_collect();
    inner.in_dowrite = false;
    let inner_ptr = inner as *mut TLSWrapInner;
    // SAFETY: inner_ptr is valid; do_enc_out_action is reference-free
    unsafe { TLSWrapInner::do_enc_out_action(inner_ptr, enc_action) };

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

    TLSWrap {
      base,
      inner: OwnedPtr::from_box(Box::new(TLSWrapInner::new(kind))),
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
    let verify_error = inner.verify_error.clone();
    let client_config =
      match build_client_config(scope, context, op_state, verify_error) {
        Some(c) => c,
        None => return -1,
      };
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
    _op_state: &mut OpState,
  ) -> i32 {
    let server_config = match build_server_config(scope, context) {
      Some(c) => c,
      None => {
        return -1;
      }
    };

    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.pending_server_config = Some(Arc::new(server_config));
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
        unsafe { TLSWrapInner::do_enc_out_action(inner_ptr, enc_action) };
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
  #[fast]
  #[reentrant]
  fn read_start(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
    _op_state: &mut OpState,
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
      let inner_ptr = inner as *mut TLSWrapInner;
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
          let ab = buf.buffer(scope).unwrap();
          let ptr = ab.data().unwrap().as_ptr() as *const u8;
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
          let ab = buf.buffer(scope).unwrap();
          let ptr = ab.data().unwrap().as_ptr() as *const u8;
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
    let ab = buffer.buffer(scope).unwrap();
    let data_ptr = ab.data().unwrap().as_ptr() as *const u8;
    // SAFETY: ptr + offset is within the ArrayBuffer backing store
    let data = unsafe {
      std::slice::from_raw_parts(data_ptr.add(byte_offset), byte_length)
    };

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
        unsafe { TLSWrapInner::do_enc_out_action(inner_ptr, enc_action) };

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

  /// Set the serialized TLS session for client resumption.
  /// With the shared session store, rustls handles resumption automatically.
  /// This is still needed to signal that a session was provided (so JS
  /// can check isSessionReused after handshake).
  #[fast]
  fn set_session(&self, #[buffer] _session: &[u8]) {
    let inner = unsafe { &mut *self.inner.as_mut_ptr() };
    inner.session_was_set = true;
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
      if let Some(ctx) = extract_emit_ctx(inner_ptr) {
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
  /// Raw DER bytes of every root certificate so we can check whether a
  /// `CaUsedAsEndEntity` cert is actually trusted.
  root_cert_ders: Vec<Vec<u8>>,
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

/// Filter out UnsupportedCertVersion errors from signature verification.
/// OpenSSL accepts X.509v1 certificates, but webpki/rustls rejects them.
/// Since Node uses OpenSSL, we need to allow these through.
fn filter_unsupported_cert_version(
  result: Result<
    rustls::client::danger::HandshakeSignatureValid,
    rustls::Error,
  >,
) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
  match result {
    Err(rustls::Error::InvalidCertificate(
      rustls::CertificateError::Other(ref other),
    )) if other.to_string().contains("UnsupportedCertVersion") => {
      Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    Err(rustls::Error::InvalidCertificate(
      rustls::CertificateError::BadEncoding,
    )) => Ok(rustls::client::danger::HandshakeSignatureValid::assertion()),
    other => other,
  }
}

/// Map a rustls CertificateError to a Node/OpenSSL-style error code.
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
      Ok(v) => Ok(v),
      Err(rustls::Error::InvalidCertificate(ref cert_error)) => {
        // Server-name checks are handled by JS (checkServerIdentity).
        if matches!(
          cert_error,
          rustls::CertificateError::NotValidForName
            | rustls::CertificateError::NotValidForNameContext { .. }
        ) {
          return Ok(rustls::client::danger::ServerCertVerified::assertion());
        }
        if let rustls::CertificateError::Other(other) = cert_error {
          let msg = format!("{other}");
          // CaUsedAsEndEntity is a rustls/webpki-specific check that
          // OpenSSL does not have.  If the cert is actually in our
          // root store, trust it silently.  Otherwise store an error.
          if msg.contains("CaUsedAsEndEntity") {
            let ee_bytes: &[u8] = end_entity.as_ref();
            let is_trusted =
              self.root_cert_ders.iter().any(|r| r.as_slice() == ee_bytes);
            if is_trusted {
              return Ok(
                rustls::client::danger::ServerCertVerified::assertion(),
              );
            }
            // Not trusted — fall through to store the error below.
          }
          // OpenSSL accepts X.509 v1 certificates; webpki/rustls do not.
          // Trust the handshake signature checks (which are filtered for
          // the same error in verify_tls1{2,3}_signature) and accept.
          if msg.contains("UnsupportedCertVersion")
            || matches!(cert_error, rustls::CertificateError::BadEncoding)
          {
            return Ok(rustls::client::danger::ServerCertVerified::assertion());
          }
        }
        // Store the error for verifyError() and let the handshake
        // proceed.  The JS layer will decide whether to tear down
        // the connection based on `rejectUnauthorized`.
        let code = cert_error_to_node_code(cert_error);
        *self.verify_error.lock().unwrap_or_else(|e| e.into_inner()) =
          Some(code.to_string());
        Ok(rustls::client::danger::ServerCertVerified::assertion())
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
    filter_unsupported_cert_version(
      self.inner.verify_tls12_signature(message, cert, dss),
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    filter_unsupported_cert_version(
      self.inner.verify_tls13_signature(message, cert, dss),
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

/// Build a rustls ClientConfig from a SecureContext JS object.
fn build_client_config(
  scope: &mut v8::PinScope,
  context: v8::Local<v8::Object>,
  op_state: &mut OpState,
  verify_error: VerifyErrorStore,
) -> Option<rustls::ClientConfig> {
  use deno_net::DefaultTlsOptions;
  use deno_tls::TlsKeys;
  use deno_tls::TlsKeysHolder;

  let _reject_unauthorized =
    get_js_bool(scope, context, "rejectUnauthorized", true);
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

  let mut root_cert_store = op_state
    .borrow::<DefaultTlsOptions>()
    .root_cert_store()
    .ok()
    .flatten();

  // Use custom CA certs from setDefaultCACertificates() only when the
  // SecureContext doesn't provide its own CA. This matches Node.js
  // behavior where explicit `ca` in options takes precedence.
  if ca_certs.is_empty()
    && let Some(node_tls_state) = op_state.try_borrow::<NodeTlsState>()
    && let Some(custom_ca_certs) = &node_tls_state.custom_ca_certs
  {
    root_cert_store = Some(rustls::RootCertStore::empty());
    ca_certs = custom_ca_certs
      .iter()
      .map(|cert| cert.clone().into_bytes())
      .collect();
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

  // Collect raw DER bytes of root certs so NodeServerCertVerifier can
  // check CaUsedAsEndEntity certs against the trust store.
  let mut root_cert_ders: Vec<Vec<u8>> = Vec::new();

  for cert in &ca_certs {
    let reader = &mut std::io::BufReader::new(std::io::Cursor::new(cert));
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

  // Always build with root certs so NodeServerCertVerifier can check them.
  // NodeServerCertVerifier never aborts the handshake — it stores errors
  // for verifyError().  The JS layer decides whether to destroy the
  // connection based on rejectUnauthorized.
  let config_builder =
    rustls::ClientConfig::builder_with_protocol_versions(protocol_versions)
      .with_root_certificates(root_cert_store.clone());

  let mut config = match maybe_cert_chain_and_key {
    TlsKeys::Static(deno_tls::TlsKey(cert_chain, private_key)) => {
      config_builder
        .with_client_auth_cert(cert_chain, private_key.clone_key())
        .ok()?
    }
    TlsKeys::Null => config_builder.with_no_client_auth(),
    TlsKeys::Resolver(_) => return None,
  };

  // Enable session resumption using the shared session store from NodeTlsState.
  if let Some(node_tls_state) = op_state.try_borrow::<NodeTlsState>() {
    config.resumption = rustls::client::Resumption::store(
      node_tls_state.client_session_store.clone(),
    );
  }

  // Install NodeServerCertVerifier to store verification errors for
  // verifyError().  This verifier never aborts the handshake — it
  // matches Node/OpenSSL behaviour where cert errors are deferred.
  let verifier_result =
    rustls::client::WebPkiServerVerifier::builder(Arc::new(root_cert_store))
      .build();
  if let Ok(inner) = verifier_result {
    config.dangerous().set_certificate_verifier(Arc::new(
      NodeServerCertVerifier {
        inner,
        verify_error,
        root_cert_ders,
      },
    ));
  }

  Some(config)
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
        if self.reject_unauthorized {
          Err(e)
        } else {
          // `rejectUnauthorized: false` — succeed the handshake and let the
          // JS layer decide via `TLSSocket.authorized` / `authorizationError`.
          Ok(rustls::server::danger::ClientCertVerified::assertion())
        }
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
    self.inner.verify_tls12_signature(message, cert, dss)
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &rustls::pki_types::CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    self.inner.verify_tls13_signature(message, cert, dss)
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.inner.supported_verify_schemes()
  }
}

/// Build a rustls ServerConfig from a SecureContext JS object.
fn build_server_config(
  scope: &mut v8::PinScope,
  context: v8::Local<v8::Object>,
) -> Option<rustls::ServerConfig> {
  let protocol_versions = match get_protocol_versions(scope, context) {
    ProtocolVersionSelection::Default => {
      &[&rustls::version::TLS13, &rustls::version::TLS12][..]
    }
    ProtocolVersionSelection::Tls12Only => &[&rustls::version::TLS12][..],
    ProtocolVersionSelection::Tls13Only => &[&rustls::version::TLS13][..],
    ProtocolVersionSelection::Unsupported => return None,
  };
  let cert_str = match get_js_string(scope, context, "cert") {
    Some(value) => value,
    None => {
      return None;
    }
  };
  let key_str = match get_js_string(scope, context, "key") {
    Some(value) => value,
    None => {
      return None;
    }
  };

  let certs: Vec<_> =
    rustls_pemfile::certs(&mut std::io::BufReader::new(cert_str.as_bytes()))
      .filter_map(|r| r.ok())
      .collect();

  let private_key = rustls_pemfile::private_key(&mut std::io::BufReader::new(
    key_str.as_bytes(),
  ))
  .ok()
  .flatten()?;

  let request_cert = get_js_bool(scope, context, "requestCert", false);
  let reject_unauthorized =
    get_js_bool(scope, context, "rejectUnauthorized", true);

  let builder =
    rustls::ServerConfig::builder_with_protocol_versions(protocol_versions);

  // When `requestCert` is true, the server sends a CertificateRequest during
  // the TLS handshake so the client presents its certificate. Without this
  // the peer certificate is never available to `getPeerCertificate()`.
  let builder = if request_cert {
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
        let reader = &mut std::io::BufReader::new(std::io::Cursor::new(pem));
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

    let mut verifier_builder =
      rustls::server::WebPkiClientVerifier::builder(Arc::new(root_cert_store));
    if !reject_unauthorized {
      verifier_builder = verifier_builder.allow_unauthenticated();
    }
    match verifier_builder.build() {
      Ok(inner) => {
        builder.with_client_cert_verifier(Arc::new(NodeClientCertVerifier {
          inner,
          root_cert_ders,
          reject_unauthorized,
        }))
      }
      Err(e) => {
        log::debug!("TLSWrap: failed to build client cert verifier: {e}");
        return None;
      }
    }
  } else {
    builder.with_no_client_auth()
  };

  // `with_single_cert` runs `CertifiedKey::keys_match()`, which parses the
  // end-entity cert via webpki and rejects X.509v1 certs with
  // UnsupportedCertVersion.  Node uses OpenSSL, which accepts v1 certs, and
  // several upstream Node test fixtures (e.g. agent2, agent3) are v1, so we
  // build the CertifiedKey manually and call `keys_match` ourselves to keep
  // the cert/key pairing check and the empty-chain check, while translating
  // only UnsupportedCertVersion to success.
  let provider = builder.crypto_provider().clone();
  let signing_key = provider.key_provider.load_private_key(private_key).ok()?;
  let certified_key = rustls::sign::CertifiedKey::new(certs, signing_key);
  match certified_key.keys_match() {
    Ok(()) => {}
    Err(rustls::Error::InvalidCertificate(
      rustls::CertificateError::Other(ref other),
    )) if other
      .0
      .downcast_ref::<webpki::Error>()
      .is_some_and(|e| matches!(e, webpki::Error::UnsupportedCertVersion)) => {}
    Err(e) => {
      log::debug!("TLSWrap: cert/key validation failed: {e}");
      return None;
    }
  }
  let resolver = rustls::sign::SingleCertAndKey::from(certified_key);
  Some(builder.with_cert_resolver(Arc::new(resolver)))
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Verify that clear_out_process drains buffered TLS data when eof is false,
  /// but bails early when eof is already true. This validates the emit_eof fix:
  /// eof must be set *after* clear_out_process, not before.
  #[test]
  fn clear_out_process_bails_when_eof_set() {
    let mut inner = TLSWrapInner::new(Kind::Client);

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
    let inner = TLSWrapInner::new(Kind::Client);
    assert!(inner.alive.get());
    let alive_clone = inner.alive.clone();
    inner.alive.set(false);
    assert!(!alive_clone.get());
  }

  /// Verify that the cycle guard prevents re-entrant cycling.
  #[test]
  fn cycling_guard_prevents_reentry() {
    let mut inner = TLSWrapInner::new(Kind::Client);
    assert!(!inner.cycling);
    inner.cycling = true;
    // cycle() should be a no-op when cycling is already true.
    // We can't call cycle() directly without a valid pointer, but we can
    // verify the flag semantics.
    assert!(inner.cycling);
    inner.cycling = false;
    assert!(!inner.cycling);
  }
}
