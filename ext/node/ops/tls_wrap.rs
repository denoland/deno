// Copyright 2018-2026 the Deno authors. MIT license.

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

#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_variables)]

use std::cell::RefCell;
use std::ffi::c_char;
use std::io::Read;
use std::io::Write;
use std::ptr::NonNull;
use std::sync::Arc;

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

use crate::ops::handle_wrap::ProviderType;
use crate::ops::stream_wrap::StreamBaseState;
use crate::ops::tls::NodeTlsState;
use deno_node_crypto::x509::Certificate;
use deno_node_crypto::x509::CertificateObject;

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

  fn export_keying_material(
    &self,
    output: &mut [u8],
    label: &[u8],
    context: Option<&[u8]>,
  ) -> Result<(), rustls::Error> {
    match self {
      TlsConnection::Client(c) => {
        c.export_keying_material(&mut *output, label, context).map(|_| ())
      }
      TlsConnection::Server(c) => {
        c.export_keying_material(&mut *output, label, context).map(|_| ())
      }
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
// CallbackData stored on the underlying stream's data pointer.
// TLSWrap replaces the original data pointer to intercept reads.
// ---------------------------------------------------------------------------

struct TlsCallbackData {
  /// Raw pointer back to the TLSWrap. Valid for the lifetime of the
  /// TLSWrap (we null it out on destroy).
  tls_wrap: *mut TLSWrapInner,
  isolate: v8::UnsafeRawIsolatePtr,
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
  cycle_depth: i32,
  in_dowrite: bool,
  write_callback_scheduled: bool,

  // Pending cleartext from DoWrite that SSL_write couldn't accept yet
  pending_cleartext: Option<Vec<u8>>,

  // The underlying stream we're wrapping
  underlying_stream: *mut uv_stream_t,

  // Original data pointer on the underlying stream (so we can restore it)
  original_stream_data: *mut std::ffi::c_void,

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

  // Error string (like Node's error_)
  error: Option<String>,

  // Callback data stored on the underlying stream
  cb_data: Option<Box<TlsCallbackData>>,

  // Deferred TLS config — stored here until start() creates the connection.
  // This allows setALPNProtocols to modify the config before the connection
  // is established.
  pending_client_config: Option<Arc<rustls::ClientConfig>>,
  pending_server_name: Option<rustls::pki_types::ServerName<'static>>,
  pending_server_config: Option<Arc<rustls::ServerConfig>>,
}

fn tls_debug_enabled() -> bool {
  std::env::var_os("DENO_TLS_DEBUG").is_some()
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
      cycle_depth: 0,
      in_dowrite: false,
      write_callback_scheduled: false,
      pending_cleartext: None,
      underlying_stream: std::ptr::null_mut(),
      original_stream_data: std::ptr::null_mut(),
      js_handle: None,
      isolate: None,
      stream_base_state: None,
      onread: None,
      current_write_obj: None,
      current_write_bytes: 0,
      bytes_read: 0,
      bytes_written: 0,
      error: None,
      cb_data: None,
      pending_client_config: None,
      pending_server_name: None,
      pending_server_config: None,
    }
  }

  /// Drive the TLS state machine.
  /// Mirrors Node's TLSWrap::Cycle().
  /// # Safety
  /// Must only be called when we have valid isolate/context pointers.
  unsafe fn cycle(&mut self) {
    if self.cycle_depth > 0 {
      self.cycle_depth += 1;
      return;
    }
    self.cycle_depth = 1;
    while self.cycle_depth > 0 {
      self.clear_in();
      unsafe { self.clear_out() };
      self.enc_out();
      self.cycle_depth -= 1;
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

    match conn.writer().write_all(&data) {
      Ok(()) => {
        // All written to rustls
      }
      Err(_) => {
        // Put it back for retry
        self.pending_cleartext = Some(data);
      }
    }
  }

  /// Read decrypted cleartext from rustls and emit to JS via onread.
  /// Mirrors Node's TLSWrap::ClearOut().
  ///
  /// We collect all data first, then emit callbacks, to avoid borrow
  /// conflicts between the TLS connection and self.
  ///
  /// # Safety
  /// Must only be called when we have valid isolate/context pointers.
  unsafe fn clear_out(&mut self) {
    if self.eof {
      return;
    }

    let Some(ref mut conn) = self.tls_conn else {
      return;
    };

    let was_handshaking = conn.is_handshaking();

    // Process any buffered TLS records
    if !self.enc_in.is_empty() {
      let mut cursor = std::io::Cursor::new(&self.enc_in);
      match conn.read_tls(&mut cursor) {
        Ok(n) => {
          let consumed = cursor.position() as usize;
          if consumed > 0 {
            self.enc_in.drain(..consumed);
          }
          if n == 0 {
            self.eof = true;
          }
        }
        Err(_) => {}
      }

      match conn.process_new_packets() {
        Ok(_) => {
          if tls_debug_enabled() {
            eprintln!(
              "[tls_wrap] clear_out: process_new_packets ok handshaking={} established={}",
              conn.is_handshaking(),
              self.established
            );
          }
        }
        Err(e) => {
          if tls_debug_enabled() {
            eprintln!("[tls_wrap] clear_out: process_new_packets err={e}");
          }
          self.error = Some("TLS error during processing".to_string());
        }
      }
    }

    // Check if handshake just completed
    let is_handshaking_now = conn.is_handshaking();
    let handshake_done =
      was_handshaking && !is_handshaking_now && !self.established;
    if tls_debug_enabled() {
      eprintln!(
        "[tls_wrap] clear_out: was_handshaking={} is_handshaking_now={} handshake_done={}",
        was_handshaking,
        is_handshaking_now,
        handshake_done
      );
    }

    // Collect all readable cleartext chunks
    enum ReadResult {
      Data(Vec<u8>),
      Eof,
      Error,
    }
    let mut results = Vec::new();
    let mut out = [0u8; CLEAR_OUT_CHUNK_SIZE];
    loop {
      match conn.reader().read(&mut out) {
        Ok(0) => break,
        Ok(n) => {
          self.bytes_read += n as u64;
          results.push(ReadResult::Data(out[..n].to_vec()));
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
        Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
          self.eof = true;
          results.push(ReadResult::Eof);
          break;
        }
        Err(_) => {
          results.push(ReadResult::Error);
          break;
        }
      }
    }

    // Now emit — conn borrow is released
    if handshake_done {
      self.established = true;
      self.emit_handshake_done();
    }

    for result in results {
      match result {
        ReadResult::Data(data) => {
          self.emit_read(data.len() as isize, Some(&data));
          if self.tls_conn.is_none() {
            return;
          }
        }
        ReadResult::Eof => {
          self.emit_read(UV_EOF as isize, None);
        }
        ReadResult::Error => {
          self.emit_read(-1, None);
        }
      }
    }
  }

  /// Write encrypted data from rustls to the underlying stream.
  /// Mirrors Node's TLSWrap::EncOut().
  fn enc_out(&mut self) {
    let Some(ref mut conn) = self.tls_conn else {
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] enc_out: no tls_conn");
      }
      return;
    };

    if !conn.wants_write() {
      if tls_debug_enabled() {
        eprintln!(
          "[tls_wrap] enc_out: wants_write=false established={}",
          self.established
        );
      }
      // If we have a pending write and the connection is established,
      // signal write completion.
      if self.established && self.write_callback_scheduled {
        self.invoke_queued(0);
      }
      return;
    }

    // Collect encrypted output from rustls
    let mut enc_data = Vec::with_capacity(16384);
    match conn.write_tls(&mut enc_data) {
      Ok(0) => {
        if tls_debug_enabled() {
          eprintln!("[tls_wrap] enc_out: write_tls produced 0 bytes");
        }
        return;
      }
      Ok(n) => {
        if tls_debug_enabled() {
          eprintln!("[tls_wrap] enc_out: write_tls produced {n} bytes");
        }
      }
      Err(_) => return,
    }

    if enc_data.is_empty() {
      return;
    }

    if self.established && self.current_write_obj.is_some() {
      self.write_callback_scheduled = true;
    }

    let stream = self.underlying_stream;
    if stream.is_null() {
      return;
    }

    // Write encrypted data to underlying stream
    let data_len = enc_data.len();
    let has_write_cb = self.write_callback_scheduled;
    let self_ptr = self as *mut TLSWrapInner;
    let mut write_req = Box::new(EncryptedWriteReq {
      uv_req: uv_compat::new_write(),
      _data: enc_data,
      tls_wrap_inner: if has_write_cb {
        self_ptr
      } else {
        std::ptr::null_mut()
      },
      has_write_callback: has_write_cb,
    });
    let buf = uv_buf_t {
      base: write_req._data.as_mut_ptr() as *mut c_char,
      len: data_len,
    };
    let req_ptr = &mut write_req.uv_req as *mut uv_write_t;
    let _ = Box::into_raw(write_req); // freed in enc_write_cb

    unsafe {
      let ret =
        uv_compat::uv_write(req_ptr, stream, &buf, 1, Some(enc_write_cb));
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] enc_out: uv_write ret={ret}");
      }
      if ret != 0 {
        // Failed to write — reclaim
        let _ = Box::from_raw(req_ptr as *mut EncryptedWriteReq);
        if self.write_callback_scheduled {
          self.invoke_queued(ret);
        }
      }
      // Note: for successful writes, invoke_queued is called from enc_write_cb
      // when the uv_write completes asynchronously.
    }
  }

  /// Emit read data to JS via onread callback.
  fn emit_read(&self, nread: isize, data: Option<&[u8]>) {
    let Some(ref isolate_ptr) = self.isolate else {
      return;
    };
    let Some(ref js_handle) = self.js_handle else {
      return;
    };
    let Some(ref state_global) = self.stream_base_state else {
      return;
    };

    unsafe {
      let mut isolate = v8::Isolate::from_raw_isolate_ptr(*isolate_ptr);
      v8::scope!(let handle_scope, &mut isolate);

      // Recover context from underlying stream's loop
      let loop_ptr = if !self.underlying_stream.is_null() {
        (*self.underlying_stream).loop_
      } else {
        return;
      };
      let ctx_ptr = (*loop_ptr).data;
      if ctx_ptr.is_null() {
        return;
      }
      let raw = NonNull::new_unchecked(ctx_ptr as *mut v8::Context);
      let global = v8::Global::from_raw(handle_scope, raw);
      let cloned = global.clone();
      global.into_raw();

      let context = v8::Local::new(handle_scope, cloned);
      let scope = &mut v8::ContextScope::new(handle_scope, context);

      // Update stream_base_state
      let state_array = v8::Local::new(scope, state_global);
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

      let recv = v8::Local::new(scope, js_handle);

      // Look up onread from stored field or JS property on the handle
      let onread_fn = if let Some(ref onread) = self.onread {
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

  /// Emit handshake done callback.
  fn emit_handshake_done(&self) {
    let Some(ref isolate_ptr) = self.isolate else {
      return;
    };
    let Some(ref js_handle) = self.js_handle else {
      return;
    };

    unsafe {
      let mut isolate = v8::Isolate::from_raw_isolate_ptr(*isolate_ptr);
      v8::scope!(let handle_scope, &mut isolate);

      let loop_ptr = if !self.underlying_stream.is_null() {
        (*self.underlying_stream).loop_
      } else {
        return;
      };
      let ctx_ptr = (*loop_ptr).data;
      if ctx_ptr.is_null() {
        return;
      }
      let raw = NonNull::new_unchecked(ctx_ptr as *mut v8::Context);
      let global = v8::Global::from_raw(handle_scope, raw);
      let cloned = global.clone();
      global.into_raw();

      let context = v8::Local::new(handle_scope, cloned);
      let scope = &mut v8::ContextScope::new(handle_scope, context);

      let this = v8::Local::new(scope, js_handle);
      let key =
        v8::String::new_external_onebyte_static(scope, b"onhandshakedone")
          .unwrap();
      if let Some(val) = this.get(scope, key.into()) {
        if let Ok(func) = v8::Local::<v8::Function>::try_from(val) {
          func.call(scope, this.into(), &[]);
        }
      }
    }
  }

  /// Signal write completion to JS.
  fn invoke_queued(&mut self, status: i32) {
    self.write_callback_scheduled = false;
    let write_obj = self.current_write_obj.take();
    let _bytes = self.current_write_bytes;
    self.current_write_bytes = 0;

    let Some(ref isolate_ptr) = self.isolate else {
      return;
    };
    let Some(ref js_handle) = self.js_handle else {
      return;
    };
    let Some(write_obj) = write_obj else {
      return;
    };

    unsafe {
      let mut isolate = v8::Isolate::from_raw_isolate_ptr(*isolate_ptr);
      v8::scope!(let handle_scope, &mut isolate);

      let loop_ptr = if !self.underlying_stream.is_null() {
        (*self.underlying_stream).loop_
      } else {
        return;
      };
      let ctx_ptr = (*loop_ptr).data;
      if ctx_ptr.is_null() {
        return;
      }
      let raw = NonNull::new_unchecked(ctx_ptr as *mut v8::Context);
      let global = v8::Global::from_raw(handle_scope, raw);
      let cloned = global.clone();
      global.into_raw();

      let context = v8::Local::new(handle_scope, cloned);
      let scope = &mut v8::ContextScope::new(handle_scope, context);

      let req_obj = v8::Local::new(scope, &write_obj);
      let handle = v8::Local::new(scope, js_handle);
      let oncomplete_str =
        v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
      if let Some(oncomplete) = req_obj.get(scope, oncomplete_str.into()) {
        if let Ok(func) = v8::Local::<v8::Function>::try_from(oncomplete) {
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
  }
}

// ---------------------------------------------------------------------------
// C callbacks for intercepting the underlying stream
// ---------------------------------------------------------------------------

unsafe extern "C" fn tls_alloc_cb(
  _handle: *mut uv_compat::uv_handle_t,
  suggested_size: usize,
  buf: *mut uv_buf_t,
) {
  unsafe {
    let layout =
      std::alloc::Layout::from_size_align(suggested_size, 1).unwrap();
    let ptr = std::alloc::alloc(layout);
    if ptr.is_null() {
      (*buf).base = std::ptr::null_mut();
      (*buf).len = 0;
      return;
    }
    (*buf).base = ptr as *mut c_char;
    (*buf).len = suggested_size;
  }
}

/// Called when encrypted data arrives from the underlying stream.
/// We buffer it and feed to rustls.
unsafe extern "C" fn tls_read_cb(
  stream: *mut uv_stream_t,
  nread: isize,
  buf: *const uv_buf_t,
) {
  unsafe {
    let cb_data_ptr = (*stream).data as *mut TlsCallbackData;
    if cb_data_ptr.is_null() {
      free_uv_buf(buf);
      return;
    }
    let cb_data = &*cb_data_ptr;
    let inner = &mut *cb_data.tls_wrap;

    if inner.eof {
      free_uv_buf(buf);
      return;
    }

    if nread < 0 {
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] tls_read_cb: nread={nread}");
      }
      free_uv_buf(buf);
      // Flush any remaining cleartext
      inner.clear_out();
      if nread == UV_EOF as isize {
        inner.eof = true;
      }
      inner.emit_read(nread, None);
      return;
    }

    if nread == 0 {
      free_uv_buf(buf);
      return;
    }

    if tls_debug_enabled() {
      eprintln!("[tls_wrap] tls_read_cb: received {} bytes", nread);
    }

    // Buffer the encrypted data
    let n = nread as usize;
    let buf_ref = &*buf;
    let slice = std::slice::from_raw_parts(buf_ref.base as *const u8, n);
    inner.enc_in.extend_from_slice(slice);
    free_uv_buf(buf);

    // Drive the TLS state machine
    inner.cycle();
  }
}

fn free_uv_buf(buf: *const uv_buf_t) {
  unsafe {
    if !(*buf).base.is_null() && (*buf).len > 0 {
      let layout = std::alloc::Layout::from_size_align((*buf).len, 1).unwrap();
      std::alloc::dealloc((*buf).base as *mut u8, layout);
    }
  }
}

/// Callback for when encrypted write to underlying stream completes.
unsafe extern "C" fn enc_write_cb(req: *mut uv_write_t, status: i32) {
  unsafe {
    let write_req = Box::from_raw(req as *mut EncryptedWriteReq);
    if write_req.has_write_callback && !write_req.tls_wrap_inner.is_null() {
      let inner = &mut *write_req.tls_wrap_inner;
      inner.invoke_queued(status);
    }
  }
}

// ---------------------------------------------------------------------------
// TLSWrap — the CppGC object visible to JS
// ---------------------------------------------------------------------------

pub struct TLSWrap {
  inner: RefCell<Box<TLSWrapInner>>,
  provider: i32,
  async_id: i64,
}

unsafe impl GarbageCollected for TLSWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TLSWrap"
  }

  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}
}

impl Drop for TLSWrap {
  fn drop(&mut self) {
    self.destroy_inner();
  }
}

impl TLSWrap {
  fn write_data(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    data: &[u8],
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let byte_length = data.len();
    let mut inner = self.inner.borrow_mut();

    if inner.tls_conn.is_none() {
      inner.error = Some("Write after DestroySSL".to_string());
      return -1;
    }

    inner.bytes_written += byte_length as u64;

    if byte_length == 0 {
      unsafe { inner.clear_out() };
      inner.enc_out();
      return 0;
    }

    // Store current write for completion tracking
    inner.current_write_obj = Some(v8::Global::new(scope, req_wrap_obj));
    inner.current_write_bytes = byte_length;

    // Try to write cleartext into rustls
    let conn = inner.tls_conn.as_mut().unwrap();
    let data_vec = data.to_vec();
    match conn.writer().write_all(&data_vec) {
      Ok(()) => {}
      Err(_) => {
        inner.pending_cleartext = Some(data_vec);
      }
    }

    // Write encrypted output to underlying stream
    inner.in_dowrite = true;
    inner.enc_out();
    inner.in_dowrite = false;

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

  fn destroy_inner(&self) {
    let mut inner = self.inner.borrow_mut();

    // Restore original data pointer on underlying stream
    if !inner.underlying_stream.is_null() {
      unsafe {
        (*inner.underlying_stream).data = inner.original_stream_data;
      }
    }

    // Drop callback data
    inner.cb_data = None;
    inner.tls_conn = None;
    inner.js_handle = None;
    inner.onread = None;
    inner.stream_base_state = None;
    inner.current_write_obj = None;
  }
}

#[op2]
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
    let async_id = {
      let counter = op_state.borrow_mut::<crate::ops::handle_wrap::AsyncId>();
      counter.next()
    };

    // Create a placeholder — the actual TLS connection is set up later
    // via initTls() once we have the secure context and underlying stream.
    let kind = if kind == 1 {
      Kind::Server
    } else {
      Kind::Client
    };

    TLSWrap {
      inner: RefCell::new(Box::new(TLSWrapInner::new(kind))),
      provider: ProviderType::TcpWrap as i32, // TODO: add TlsWrap provider
      async_id,
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
    let server_name = match rustls::pki_types::ServerName::try_from(server_name)
    {
      Ok(name) => name,
      Err(_) => return -1,
    };

    let client_config = match build_client_config(scope, context, op_state) {
      Some(c) => c,
      None => return -1,
    };

    let mut inner = self.inner.borrow_mut();
    inner.pending_client_config = Some(Arc::new(client_config));
    inner.pending_server_name = Some(server_name);
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
    if tls_debug_enabled() {
      eprintln!("[tls_wrap] init_server_tls: begin");
    }
    let server_config = match build_server_config(scope, context) {
      Some(c) => c,
      None => {
        if tls_debug_enabled() {
          eprintln!("[tls_wrap] init_server_tls: build_server_config failed");
        }
        return -1;
      }
    };

    let mut inner = self.inner.borrow_mut();
    inner.pending_server_config = Some(Arc::new(server_config));
    if tls_debug_enabled() {
      eprintln!("[tls_wrap] init_server_tls: success");
    }
    0
  }

  /// Attach to an underlying stream and set up read interception.
  #[nofast]
  fn attach(
    &self,
    #[cppgc] tcp: &crate::ops::libuv_stream::TCP,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    if tls_debug_enabled() {
      eprintln!("[tls_wrap] attach: begin");
    }
    let stream = tcp.stream();

    if stream.is_null() {
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] attach: stream is null");
      }
      return UV_EBADF;
    }

    let mut inner = self.inner.borrow_mut();
    inner.underlying_stream = stream;
    inner.isolate = Some(unsafe { scope.as_raw_isolate_ptr() });

    // Get stream_base_state from OpState
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    inner.stream_base_state =
      Some(v8::Global::new(scope, v8::Local::new(scope, state_global)));

    // Save original data pointer (but don't replace yet - connect_cb needs it)
    unsafe {
      inner.original_stream_data = (*stream).data;
    }

    // Create callback data (installed on stream later in start())
    let cb_data = Box::new(TlsCallbackData {
      tls_wrap: &mut **inner as *mut TLSWrapInner,
      isolate: unsafe { scope.as_raw_isolate_ptr() },
    });
    inner.cb_data = Some(cb_data);

    if tls_debug_enabled() {
      eprintln!("[tls_wrap] attach: success");
    }
    0
  }

  /// Store the JS handle reference for callbacks.
  #[nofast]
  fn set_handle(
    &self,
    handle: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) {
    let mut inner = self.inner.borrow_mut();
    inner.js_handle = Some(v8::Global::new(scope, handle));
  }

  /// Set the onread callback.
  #[nofast]
  fn set_onread(
    &self,
    onread: v8::Local<v8::Function>,
    scope: &mut v8::PinScope,
  ) {
    let mut inner = self.inner.borrow_mut();
    inner.onread = Some(v8::Global::new(scope, onread));
  }

  /// Start the TLS handshake.
  /// Creates the actual TLS connection from pending config, then begins
  /// the handshake. Mirrors Node's TLSWrap::Start().
  #[fast]
  #[reentrant]
  fn start(&self) -> i32 {
    let mut inner = self.inner.borrow_mut();
    if inner.started {
      return -1;
    }
    inner.started = true;

    // Create the TLS connection from pending config
    match inner.kind {
      Kind::Client => {
        if let (Some(config), Some(server_name)) = (
          inner.pending_client_config.take(),
          inner.pending_server_name.take(),
        ) {
          if tls_debug_enabled() {
            eprintln!("[tls_wrap] start: creating client connection");
          }
          match rustls::ClientConnection::new(config, server_name) {
            Ok(conn) => {
              inner.tls_conn = Some(TlsConnection::Client(conn));
            }
            Err(_) => return -1,
          }
        }
      }
      Kind::Server => {
        if let Some(config) = inner.pending_server_config.take() {
          if tls_debug_enabled() {
            eprintln!("[tls_wrap] start: creating server connection");
          }
          match rustls::ServerConnection::new(config) {
            Ok(conn) => {
              inner.tls_conn = Some(TlsConnection::Server(conn));
            }
            Err(_) => return -1,
          }
        }
      }
    }

    // Now install our callback data on the stream (deferred from attach()
    // because connect_cb needs the original StreamHandleData pointer)
    if !inner.underlying_stream.is_null() {
      if let Some(ref cb_data) = inner.cb_data {
        let cb_data_ptr =
          &**cb_data as *const TlsCallbackData as *mut std::ffi::c_void;
        unsafe {
          (*inner.underlying_stream).data = cb_data_ptr;
        }
      }
    }

    // Start reading from the underlying stream (both client and server need this)
    if !inner.underlying_stream.is_null() {
      unsafe {
        uv_compat::uv_read_start(
          inner.underlying_stream,
          Some(tls_alloc_cb),
          Some(tls_read_cb),
        );
      }
    }

    // For client mode, initiate handshake by cycling.
    if inner.kind == Kind::Client {
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] start: priming client handshake");
      }
      unsafe {
        inner.clear_out();
        inner.enc_out();
      }
    }

    0
  }

  /// ReadStart — start reading cleartext from TLS.
  /// Mirrors Node's TLSWrap::ReadStart().
  #[fast]
  fn read_start(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
    _op_state: &mut OpState,
  ) -> i32 {
    let mut inner = self.inner.borrow_mut();

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

    // Start reading from underlying stream if not already
    if !inner.underlying_stream.is_null() && inner.started {
      unsafe {
        uv_compat::uv_read_start(
          inner.underlying_stream,
          Some(tls_alloc_cb),
          Some(tls_read_cb),
        );
      }
    }

    0
  }

  /// ReadStop
  #[fast]
  fn read_stop(&self) -> i32 {
    let inner = self.inner.borrow();
    if !inner.underlying_stream.is_null() {
      unsafe {
        uv_compat::uv_read_stop(inner.underlying_stream);
      }
    }
    0
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
  #[fast]
  fn shutdown(
    &self,
    _req_wrap_obj: v8::Local<v8::Object>,
    _scope: &mut v8::PinScope,
  ) -> i32 {
    let mut inner = self.inner.borrow_mut();

    if let Some(ref mut conn) = inner.tls_conn {
      conn.send_close_notify();
    }
    inner.shutdown = true;
    inner.enc_out();

    // Forward shutdown to underlying stream
    if !inner.underlying_stream.is_null() {
      let req = Box::new(uv_compat::new_shutdown());
      let req_ptr = Box::into_raw(req);
      unsafe {
        let ret =
          uv_compat::uv_shutdown(req_ptr, inner.underlying_stream, None);
        if ret != 0 {
          let _ = Box::from_raw(req_ptr);
        }
      }
    }

    // Return 1 to indicate synchronous completion — the close_notify
    // has been queued for sending and the underlying stream shutdown
    // has been initiated.
    1
  }

  /// Destroy the SSL connection.
  #[fast]
  fn destroy_ssl(&self) {
    self.destroy_inner();
  }

  /// Get the negotiated ALPN protocol.
  /// Writes the protocol name into the out object as { alpnProtocol: "..." }.
  #[fast]
  fn get_alpn_negotiated_protocol(
    &self,
    out: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let inner = self.inner.borrow();
    let key =
      v8::String::new_external_onebyte_static(scope, b"alpnProtocol").unwrap();
    if let Some(ref conn) = inner.tls_conn {
      if let Some(proto) = conn.alpn_protocol() {
        if let Ok(s) = std::str::from_utf8(proto) {
          let val = v8::String::new(scope, s).unwrap();
          out.set(scope, key.into(), val.into());
          return 0;
        }
      }
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
    let inner = self.inner.borrow();
    let key =
      v8::String::new_external_onebyte_static(scope, b"protocol").unwrap();
    if let Some(ref conn) = inner.tls_conn {
      if let Some(version) = conn.protocol_version() {
        let name = match version {
          rustls::ProtocolVersion::TLSv1_2 => "TLSv1.2",
          rustls::ProtocolVersion::TLSv1_3 => "TLSv1.3",
          _ => "unknown",
        };
        let val = v8::String::new(scope, name).unwrap();
        out.set(scope, key.into(), val.into());
        return 0;
      }
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
    let inner = self.inner.borrow();
    if let Some(ref conn) = inner.tls_conn {
      if let Some(suite) = conn.negotiated_cipher_suite() {
        let name_key =
          v8::String::new_external_onebyte_static(scope, b"name").unwrap();
        let name_str =
          v8::String::new(scope, &format!("{:?}", suite.suite())).unwrap();
        out.set(scope, name_key.into(), name_str.into());

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
    }
    -1
  }

  #[serde]
  fn get_peer_certificate_chain(&self) -> Option<PeerCertificateChain> {
    let inner = self.inner.borrow();
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
    let inner = self.inner.borrow();
    let conn = inner.tls_conn.as_ref()?;
    let certs = conn.peer_certificates()?;
    let cert = certs.first()?;
    let cert = Certificate::from_der(cert.as_ref()).ok()?;
    cert.to_object(detailed).ok()
  }

  #[buffer]
  fn get_finished(&self) -> Option<Box<[u8]>> {
    let inner = self.inner.borrow();
    if !inner.established {
      return None;
    }
    let conn = inner.tls_conn.as_ref()?;
    let mut output = vec![0u8; 32];
    conn
      .export_keying_material(
        &mut output,
        b"EXPORTER_DENO_TLS_FINISHED",
        None,
      )
      .ok()?;
    Some(output.into_boxed_slice())
  }

  #[buffer]
  fn get_peer_finished(&self) -> Option<Box<[u8]>> {
    let inner = self.inner.borrow();
    if !inner.established {
      return None;
    }
    let conn = inner.tls_conn.as_ref()?;
    let mut output = vec![0u8; 32];
    conn
      .export_keying_material(
        &mut output,
        b"EXPORTER_DENO_TLS_FINISHED",
        None,
      )
      .ok()?;
    Some(output.into_boxed_slice())
  }

  /// Check if the connection is established (handshake complete).
  #[fast]
  fn is_established(&self) -> bool {
    self.inner.borrow().established
  }

  /// Get the async ID.
  #[fast]
  fn get_async_id(&self) -> f64 {
    self.async_id as f64
  }

  /// Get the provider type.
  #[fast]
  fn get_provider_type(&self) -> i32 {
    self.provider
  }

  #[fast]
  fn get_bytes_read(&self) -> f64 {
    self.inner.borrow().bytes_read as f64
  }

  #[fast]
  fn get_bytes_written(&self) -> f64 {
    self.inner.borrow().bytes_written as f64
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
        if let Some(val) = arr.get_index(scope, i) {
          if let Ok(s) = v8::Local::<v8::String>::try_from(val) {
            let len = s.utf8_length(scope);
            let mut buf = vec![0u8; len];
            s.write_utf8_v2(scope, &mut buf, v8::WriteFlags::default(), None);
            alpn.push(buf);
          }
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

    let mut inner = self.inner.borrow_mut();
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
  fn set_servername(&self, #[string] _name: &str) {
    // SNI is set during ClientConnection creation via server_name parameter.
    // This is a no-op after construction.
  }

  /// Inject encrypted data (for testing / JSStreamSocket integration).
  /// Mirrors Node's TLSWrap::Receive().
  #[fast]
  fn receive(&self, #[buffer] data: &[u8]) {
    let mut inner = self.inner.borrow_mut();
    inner.enc_in.extend_from_slice(data);
    unsafe { inner.cycle() };
  }

  /// Get verification error, if any. Returns empty string if no error.
  #[string]
  fn verify_error(&self) -> String {
    let inner = self.inner.borrow();
    inner.error.clone().unwrap_or_default()
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
  /// rustls resumption is not wired to Node-compatible opaque session blobs yet,
  /// so this currently behaves as a no-op native surface for JS compatibility.
  #[fast]
  fn set_session(&self, #[buffer] _session: &[u8]) {}
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

/// Build a rustls ClientConfig from a SecureContext JS object.
fn build_client_config(
  scope: &mut v8::PinScope,
  context: v8::Local<v8::Object>,
  op_state: &mut OpState,
) -> Option<rustls::ClientConfig> {
  use deno_net::DefaultTlsOptions;
  use deno_net::UnsafelyIgnoreCertificateErrors;
  use deno_tls::NoCertificateVerification;
  use deno_tls::NoServerNameVerification;
  use deno_tls::TlsKeys;
  use deno_tls::TlsKeysHolder;

  let reject_unauthorized =
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
        if let Some(v) = arr.get_index(scope, i) {
          if let Some(s) = v.to_string(scope) {
            ca_certs.push(s.to_rust_string_lossy(scope).into_bytes());
          }
        }
      }
    } else if !ca_val.is_undefined() && !ca_val.is_null() {
      if let Some(s) = ca_val.to_string(scope) {
        ca_certs.push(s.to_rust_string_lossy(scope).into_bytes());
      }
    }
  }

  let unsafely_ignore_certificate_errors = if reject_unauthorized {
    op_state
      .try_borrow::<UnsafelyIgnoreCertificateErrors>()
      .and_then(|it| it.0.clone())
  } else {
    Some(Vec::new())
  };

  let mut root_cert_store = op_state
    .borrow::<DefaultTlsOptions>()
    .root_cert_store()
    .ok()
    .flatten();

  if let Some(node_tls_state) = op_state.try_borrow::<NodeTlsState>()
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

  let mut root_cert_store =
    root_cert_store.unwrap_or_else(rustls::RootCertStore::empty);

  for cert in &ca_certs {
    let reader = &mut std::io::BufReader::new(std::io::Cursor::new(cert));
    for parsed in rustls_pemfile::certs(reader) {
      match parsed {
        Ok(cert) => {
          if root_cert_store.add(cert).is_err() {
            return None;
          }
        }
        Err(_) => return None,
      }
    }
  }

  let maybe_cert_chain_and_key = tls_keys.take();
  let config_builder = if let Some(ic_allowlist) = unsafely_ignore_certificate_errors {
    rustls::ClientConfig::builder_with_protocol_versions(protocol_versions)
      .dangerous()
      .with_custom_certificate_verifier(Arc::new(
        NoCertificateVerification::new(ic_allowlist),
      ))
  } else {
    rustls::ClientConfig::builder_with_protocol_versions(protocol_versions)
      .with_root_certificates(root_cert_store.clone())
  };

  let mut config = match maybe_cert_chain_and_key {
    TlsKeys::Static(deno_tls::TlsKey(cert_chain, private_key)) => config_builder
      .with_client_auth_cert(cert_chain, private_key.clone_key())
      .ok()?,
    TlsKeys::Null => config_builder.with_no_client_auth(),
    TlsKeys::Resolver(_) => return None,
  };

  if reject_unauthorized {
    let inner = rustls::client::WebPkiServerVerifier::builder(Arc::new(root_cert_store))
      .build()
      .ok()?;
    config
      .dangerous()
      .set_certificate_verifier(Arc::new(NoServerNameVerification::new(inner)));
  }

  Some(config)
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
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] build_server_config: missing cert");
      }
      return None;
    }
  };
  let key_str = match get_js_string(scope, context, "key") {
    Some(value) => value,
    None => {
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] build_server_config: missing key");
      }
      return None;
    }
  };

  let certs: Vec<_> =
    rustls_pemfile::certs(&mut std::io::BufReader::new(cert_str.as_bytes()))
      .filter_map(|r| r.ok())
      .collect();

  if tls_debug_enabled() {
    eprintln!(
      "[tls_wrap] build_server_config: parsed {} certs",
      certs.len()
    );
  }

  let private_key = rustls_pemfile::private_key(&mut std::io::BufReader::new(
    key_str.as_bytes(),
  ))
  .ok()
  .flatten();

  if tls_debug_enabled() {
    eprintln!(
      "[tls_wrap] build_server_config: private key parsed? {}",
      private_key.is_some()
    );
  }

  let Some(private_key) = private_key else {
    return None;
  };

  let signing_key = match rustls::crypto::ring::default_provider()
    .key_provider
    .load_private_key(private_key.clone_key())
  {
    Ok(key) => key,
    Err(err) => {
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] build_server_config: load_private_key err={err}");
      }
      return None;
    }
  };

  let certified_key = rustls::sign::CertifiedKey::new(certs, signing_key);
  match certified_key.keys_match() {
    Ok(()) | Err(rustls::Error::InconsistentKeys(rustls::InconsistentKeys::Unknown)) => {}
    Err(rustls::Error::InvalidCertificate(rustls::CertificateError::Other(
      other,
    ))) if other.to_string().contains("UnsupportedCertVersion") => {
      if tls_debug_enabled() {
        eprintln!(
          "[tls_wrap] build_server_config: allowing legacy server cert version"
        );
      }
    }
    Err(err) => {
      if tls_debug_enabled() {
        eprintln!("[tls_wrap] build_server_config: keys_match err={err}");
      }
      return None;
    }
  }

  Some(
    rustls::ServerConfig::builder_with_protocol_versions(protocol_versions)
      .with_no_client_auth()
      .with_cert_resolver(Arc::new(rustls::sign::SingleCertAndKey::from(
        certified_key,
      ))),
  )
}

/// A certificate verifier that accepts anything (for rejectUnauthorized=false).
#[derive(Debug)]
struct UnsafeCertVerifier;

impl rustls::client::danger::ServerCertVerifier for UnsafeCertVerifier {
  fn verify_server_cert(
    &self,
    _end_entity: &rustls::pki_types::CertificateDer<'_>,
    _intermediates: &[rustls::pki_types::CertificateDer<'_>],
    _server_name: &rustls::pki_types::ServerName<'_>,
    _ocsp_response: &[u8],
    _now: rustls::pki_types::UnixTime,
  ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
    Ok(rustls::client::danger::ServerCertVerified::assertion())
  }

  fn verify_tls12_signature(
    &self,
    _message: &[u8],
    _cert: &rustls::pki_types::CertificateDer<'_>,
    _dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
  }

  fn verify_tls13_signature(
    &self,
    _message: &[u8],
    _cert: &rustls::pki_types::CertificateDer<'_>,
    _dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
  {
    Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    rustls::crypto::ring::default_provider()
      .signature_verification_algorithms
      .supported_schemes()
  }
}
