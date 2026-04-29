// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;

use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use libnghttp2 as ffi;
use serde::Serialize;

use super::session::Session;
use super::session::SessionCallbacks;
use super::session::on_stream_read_callback;
use super::types::STREAM_OPTION_EMPTY_PAYLOAD;
use super::types::STREAM_OPTION_GET_TRAILERS;
use crate::ops::handle_wrap::AsyncWrap;

/// (name bytes, value bytes, NGHTTP2 NV flags).
pub type HeaderEntry = (Vec<u8>, Vec<u8>, u8);

// Http2Headers

pub struct Http2Headers {
  #[allow(dead_code, reason = "owns the backing memory for nva pointers")]
  backing_store: Vec<u8>,
  nva: Vec<ffi::nghttp2_nv>,
}

impl Http2Headers {
  pub fn data(&self) -> *const ffi::nghttp2_nv {
    self.nva.as_ptr()
  }

  pub fn len(&self) -> usize {
    self.nva.len()
  }

  pub fn parse(bytes: Vec<u8>, count: usize) -> Self {
    let mut nva = Vec::with_capacity(count);
    let mut offset = 0;

    while offset < bytes.len() && nva.len() < count {
      let Some(name_end) = find_null(&bytes[offset..]) else {
        break;
      };
      // SAFETY: offset is within bounds
      let name_ptr = unsafe { bytes.as_ptr().add(offset) };
      let name_len = name_end;
      offset += name_end + 1;

      if offset >= bytes.len() {
        break;
      }

      let Some(value_end) = find_null(&bytes[offset..]) else {
        break;
      };
      // SAFETY: offset is within bounds
      let value_ptr = unsafe { bytes.as_ptr().add(offset) };
      let value_len = value_end;
      offset += value_end + 1;

      if offset >= bytes.len() {
        break;
      }

      let flags = bytes.get(offset).copied().unwrap_or(0);
      offset += 1;

      nva.push(ffi::nghttp2_nv {
        name: name_ptr as *mut _,
        namelen: name_len,
        value: value_ptr as *mut _,
        valuelen: value_len,
        flags,
      });
    }

    if nva.len() > count {
      static ZERO: u8 = 0;
      nva.clear();
      nva.push(ffi::nghttp2_nv {
        name: &ZERO as *const _ as *mut _,
        namelen: 1,
        value: &ZERO as *const _ as *mut _,
        valuelen: 1,
        flags: 0,
      });
    }

    Self {
      backing_store: bytes,
      nva,
    }
  }

  /// Decode the V8 string as Latin-1 (one byte per UTF-16 unit, truncated to
  /// the low byte) — matches Node's `StringBytes::Write(LATIN1)` so that JS
  /// chars like `Ċ` (U+010A) become the byte 0x0a (LF), letting nghttp2's
  /// receiver-side validation reject crafted header values for response
  /// splitting. UTF-8 encoding would hide the LF in a multibyte sequence.
  pub fn from_v8_string(
    scope: &mut v8::PinScope,
    string: v8::Local<v8::String>,
    count: usize,
  ) -> Self {
    let len = string.length();
    let mut buf: Vec<u8> = Vec::with_capacity(len);
    string.write_one_byte_uninit_v2(
      scope,
      0,
      buf.spare_capacity_mut(),
      v8::WriteFlags::empty(),
    );
    // SAFETY: write_one_byte_uninit_v2 initialized exactly `len` bytes.
    unsafe { buf.set_len(len) };
    Self::parse(buf, count)
  }
}

fn find_null(slice: &[u8]) -> Option<usize> {
  slice.iter().position(|&b| b == 0)
}

// Http2Priority

#[repr(C)]
pub struct Http2Priority {
  pub spec: ffi::nghttp2_priority_spec,
}

impl Http2Priority {
  pub fn new(parent: i32, weight: i32, exclusive: bool) -> Self {
    let mut spec =
      std::mem::MaybeUninit::<ffi::nghttp2_priority_spec>::uninit();
    // SAFETY: nghttp2_priority_spec_init initializes the struct
    unsafe {
      ffi::nghttp2_priority_spec_init(
        spec.as_mut_ptr(),
        parent,
        weight,
        if exclusive { 1 } else { 0 },
      );
      Self {
        spec: spec.assume_init(),
      }
    }
  }
}

// Http2Stream

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Http2StreamState {
  pub state: f64,
  pub weight: f64,
  pub sum_dependency_weight: f64,
  pub local_close: f64,
  pub remote_close: f64,
  pub local_window_size: f64,
}

#[derive(Debug)]
pub struct Http2Stream {
  pub(crate) session: *mut Session,
  pub(crate) id: i32,
  #[allow(dead_code, reason = "stored for future use")]
  pub(crate) current_headers_category: ffi::nghttp2_headers_category,
  pub(crate) available_outbound_length: RefCell<usize>,
  pub(crate) pending_data: RefCell<bytes::BytesMut>,
  pub(crate) current_headers: RefCell<Vec<HeaderEntry>>,
  pub(crate) current_headers_length: RefCell<usize>,
  /// `SETTINGS_MAX_HEADER_LIST_SIZE` snapshotted from the session at stream
  /// construction. Mirrors Node's `Http2Stream::max_header_length_`
  /// (`src/node_http2.cc`): a stream's enforcement value is fixed at
  /// construction, so post-init `session.settings({ maxHeaderListSize })`
  /// only affects streams created after the SETTINGS ACK round-trip.
  pub(crate) max_header_length: u64,
  pub(crate) has_trailers: RefCell<bool>,
  /// Set to true when shutdown is called (writable side ended).
  /// Used by the data source read callback to decide whether to
  /// return EOF or DEFERRED when pending_data is empty.
  pub(crate) writable_ended: RefCell<bool>,
  /// Stores the ShutdownWrap JS object when shutdown is async (pending data).
  /// complete_shutdown() calls req.oncomplete(0) to signal completion.
  pub(crate) shutdown_req: RefCell<Option<v8::Global<v8::Object>>>,
  /// Set when nghttp2 fires on_stream_close_callback for this stream.
  /// Prevents resume_data from being called during shutdown(), which
  /// would re-activate the data provider for a stream that nghttp2 is
  /// about to destroy (causing double-free with no_closed_streams=1).
  pub(crate) closed_by_nghttp2: RefCell<bool>,
  /// Set true once the data provider returned a chunk with
  /// NGHTTP2_DATA_FLAG_EOF. shutdown() then suppresses its
  /// resume_data so nghttp2 doesn't generate a second empty trailing
  /// DATA frame just to carry END_STREAM. Writable.end(data) hooks
  /// `mark_ending` to set `writable_ended` *before* the data write
  /// reaches read_callback, which lets that frame carry END_STREAM
  /// directly.
  pub(crate) eof_sent: RefCell<bool>,
}

// SAFETY: Http2Stream is GC-traced by cppgc
unsafe impl deno_core::GarbageCollected for Http2Stream {
  fn trace(&self, _: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Http2Stream"
  }
}

impl Http2Stream {
  pub fn new(
    session: &mut Session,
    id: i32,
    cat: ffi::nghttp2_headers_category,
  ) -> (v8::Global<v8::Object>, cppgc::Ref<Self>) {
    // SAFETY: isolate pointer is valid during session lifetime
    let mut isolate =
      unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
    v8::scope!(let scope, &mut isolate);
    let context = v8::Local::new(scope, session.context.clone());
    let scope = &mut v8::ContextScope::new(scope, context);

    let obj = cppgc::make_cppgc_empty_object::<Http2Stream>(scope);
    let _async_wrap = {
      let mut state = session.op_state.borrow_mut();
      AsyncWrap::create(&mut state, 0)
    };

    // Snapshot SETTINGS_MAX_HEADER_LIST_SIZE at stream construction. nghttp2
    // returns the *current* local value (which has already absorbed any
    // SETTINGS frames the peer ACKed), so streams created after a successful
    // post-init `session.settings({ maxHeaderListSize: N })` see N, while
    // streams created before keep the prior value. Mirrors Node's
    // `Http2Stream::Http2Stream` in `src/node_http2.cc`.
    // SAFETY: session.session is a valid nghttp2 session for the stream's lifetime
    let max_header_length = unsafe {
      ffi::nghttp2_session_get_local_settings(
        session.session,
        ffi::NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE,
      ) as u64
    };

    cppgc::wrap_object(
      scope,
      obj,
      Self {
        session: session as _,
        id,
        current_headers_category: cat,
        available_outbound_length: RefCell::new(0),
        pending_data: RefCell::new(bytes::BytesMut::new()),
        current_headers: RefCell::new(Vec::new()),
        current_headers_length: RefCell::new(0),
        max_header_length,
        has_trailers: RefCell::new(false),
        writable_ended: RefCell::new(false),
        shutdown_req: RefCell::new(None),
        closed_by_nghttp2: RefCell::new(false),
        eof_sent: RefCell::new(false),
      },
    );

    let stream = cppgc::try_unwrap_cppgc_persistent_object::<Http2Stream>(
      scope,
      obj.into(),
    )
    .unwrap();

    (v8::Global::new(scope, obj), stream)
  }

  pub fn add_header(&self, name: &[u8], value: &[u8], flags: u8) -> bool {
    // Empty header names are ignored (matches Node's Http2Stream::AddHeader).
    if name.is_empty() {
      return true;
    }

    let header_length = name.len() + value.len() + 32;

    // SAFETY: session pointer is valid for the stream's lifetime
    let session = unsafe { &*self.session };
    let max_header_length = self.max_header_length;

    let max_header_pairs = session.max_header_pairs as usize;
    let current_pairs = self.current_headers.borrow().len();
    let current_length = *self.current_headers_length.borrow() as u64;

    // Reject the header (and the whole stream) if adding it would exceed
    // either the configured max header pair count or the local
    // SETTINGS_MAX_HEADER_LIST_SIZE limit. Returning false here causes
    // nghttp2 to RST_STREAM with NGHTTP2_ENHANCE_YOUR_CALM.
    //
    // Node's `Http2Stream::AddHeader` additionally rejects when the session
    // would exceed its `maxSessionMemory` budget (see `src/node_http2.cc`).
    // Deno does not yet track per-session memory, so that arm is omitted; if
    // session memory accounting is added later, gate it here as well.
    if current_pairs >= max_header_pairs
      || current_length.saturating_add(header_length as u64) > max_header_length
    {
      return false;
    }

    self.current_headers.borrow_mut().push((
      name.to_vec(),
      value.to_vec(),
      flags,
    ));
    *self.current_headers_length.borrow_mut() += header_length;
    true
  }

  pub fn clear_headers(&self) {
    self.current_headers.borrow_mut().clear();
    *self.current_headers_length.borrow_mut() = 0;
  }

  pub fn start_headers(&self, _category: ffi::nghttp2_headers_category) {
    self.clear_headers();
  }

  pub fn has_trailers(&self) -> bool {
    *self.has_trailers.borrow()
  }

  pub fn set_has_trailers(&self, value: bool) {
    *self.has_trailers.borrow_mut() = value;
  }

  pub fn on_trailers(&self) {
    // SAFETY: session outlives the stream
    let session = unsafe { &*self.session };

    // SAFETY: isolate pointer is valid during session lifetime
    let mut isolate =
      unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
    v8::scope!(let scope, &mut isolate);
    let context = v8::Local::new(scope, session.context.clone());
    let scope = &mut v8::ContextScope::new(scope, context);

    let state = session.op_state.borrow();
    let callbacks = state.borrow::<SessionCallbacks>();
    let callback = v8::Local::new(scope, &callbacks.stream_trailers_cb);
    drop(state);

    let stream_obj = session.find_stream_obj(self.id).unwrap();
    let recv = v8::Local::new(scope, stream_obj);

    self.set_has_trailers(false);
    callback.call(scope, recv.into(), &[]);
  }

  /// Complete an async shutdown by calling req.oncomplete(0) on the
  /// stored ShutdownWrap object. Must NOT be called from inside
  /// nghttp2 callbacks (mem_send/mem_recv) to avoid double-free.
  pub fn complete_shutdown(&self) {
    let req = self.shutdown_req.borrow_mut().take();
    let Some(req) = req else {
      return;
    };

    // SAFETY: session outlives the stream
    let session = unsafe { &*self.session };

    // SAFETY: isolate pointer is valid during session lifetime
    let mut isolate =
      unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
    v8::scope!(let scope, &mut isolate);
    let context = v8::Local::new(scope, session.context.clone());
    let scope = &mut v8::ContextScope::new(scope, context);

    let req_local = v8::Local::new(scope, req);
    let key =
      v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
    if let Some(oncomplete) = req_local.get(scope, key.into())
      && let Ok(oncomplete) = v8::Local::<v8::Function>::try_from(oncomplete)
    {
      let zero = v8::Integer::new(scope, 0);
      oncomplete.call(scope, req_local.into(), &[zero.into()]);
    }
  }

  fn nghttp2_session(&self) -> *mut ffi::nghttp2_session {
    // SAFETY: session outlives the stream
    unsafe { (*self.session).session }
  }
}

#[op2]
impl Http2Stream {
  #[fast]
  fn id(&self) -> i32 {
    self.id
  }

  #[nofast]
  fn respond(
    &self,
    scope: &mut v8::PinScope,
    headers: v8::Local<v8::String>,
    count: u32,
    options: i32,
  ) {
    let headers = Http2Headers::from_v8_string(scope, headers, count as usize);
    let session_ptr = self.nghttp2_session();

    if (options & STREAM_OPTION_GET_TRAILERS) != 0 {
      self.set_has_trailers(true);
    }

    let has_data = (options & STREAM_OPTION_EMPTY_PAYLOAD) == 0;
    let mut data_provider = ffi::nghttp2_data_provider2 {
      source: ffi::nghttp2_data_source {
        ptr: std::ptr::null_mut(),
      },
      read_callback: Some(on_stream_read_callback),
    };

    let dp_ptr = if has_data {
      &mut data_provider as *mut _
    } else {
      std::ptr::null_mut()
    };

    // SAFETY: session pointer is valid during stream lifetime
    unsafe {
      ffi::nghttp2_submit_response2(
        session_ptr,
        self.id,
        headers.data(),
        headers.len(),
        dp_ptr,
      );
    }
  }

  #[fast]
  fn write_utf8_string(
    &self,
    _req: v8::Local<v8::Object>,
    #[string] data: &str,
  ) -> i32 {
    self
      .pending_data
      .borrow_mut()
      .extend_from_slice(data.as_bytes());
    *self.available_outbound_length.borrow_mut() += data.len();

    if !*self.closed_by_nghttp2.borrow() {
      let session_ptr = self.nghttp2_session();
      // SAFETY: session pointer is valid during stream lifetime
      unsafe {
        ffi::nghttp2_session_resume_data(session_ptr, self.id);
      }
    }

    0
  }

  #[fast]
  fn write_buffer(
    &self,
    _req: v8::Local<v8::Object>,
    #[buffer] data: &[u8],
  ) -> i32 {
    self.pending_data.borrow_mut().extend_from_slice(data);
    *self.available_outbound_length.borrow_mut() += data.len();

    if !*self.closed_by_nghttp2.borrow() {
      let session_ptr = self.nghttp2_session();
      // SAFETY: session pointer is valid during stream lifetime
      unsafe {
        ffi::nghttp2_session_resume_data(session_ptr, self.id);
      }
    }

    0
  }

  /// Pre-flag the stream as ended so the very next data frame the
  /// data provider builds carries NGHTTP2_DATA_FLAG_EOF. The polyfill
  /// calls this from `Http2Stream.end(chunk)` *before* the chunk's
  /// write reaches `write_buffer`, which lets `stream.end(data)`
  /// produce one DATA frame with END_STREAM instead of a data frame
  /// followed by an empty trailing DATA frame.
  #[fast]
  fn mark_ending(&self) {
    *self.writable_ended.borrow_mut() = true;
  }

  #[fast]
  fn shutdown(&self, req: v8::Local<v8::Object>) -> i32 {
    *self.writable_ended.borrow_mut() = true;
    // Skip resume_data if nghttp2 is closing this stream. Calling
    // resume_data inside on_stream_close_callback re-activates the
    // data provider, but close_stream then destroys the stream with
    // no_closed_streams=1. The re-activated item survives destruction
    // and mem_send later double-frees the stream.
    //
    // Also skip when EOF has already been emitted on a previous data
    // frame (Http2Stream.end(chunk) hooks `mark_ending` so the chunk's
    // frame carries END_STREAM). Without this guard nghttp2 would call
    // read_callback again, get 0 + EOF, and pack a redundant empty
    // trailing DATA frame.
    if !*self.closed_by_nghttp2.borrow() && !*self.eof_sent.borrow() {
      let session_ptr = self.nghttp2_session();
      // SAFETY: session pointer is valid
      unsafe {
        ffi::nghttp2_session_resume_data(session_ptr, self.id);
      }
    }

    // If there's pending data, return 0 (async). The data provider will
    // consume pending_data, then send_pending_data will call
    // complete_shutdown() after mem_send finishes.
    // If no pending data, return 1 (sync) like Node.js DoShutdown.
    if self.pending_data.borrow().is_empty() {
      1
    } else {
      // SAFETY: session outlives the stream
      let session = unsafe { &*self.session };
      // SAFETY: isolate pointer is valid during session lifetime
      let mut isolate =
        unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
      v8::scope!(let scope, &mut isolate);
      *self.shutdown_req.borrow_mut() = Some(v8::Global::new(scope, req));
      0
    }
  }

  #[nofast]
  fn trailers(
    &self,
    scope: &mut v8::PinScope,
    headers: v8::Local<v8::String>,
    count: u32,
  ) -> i32 {
    let session_ptr = self.nghttp2_session();

    if count == 0 {
      let mut data_provider = ffi::nghttp2_data_provider2 {
        source: ffi::nghttp2_data_source {
          ptr: std::ptr::null_mut(),
        },
        read_callback: Some(on_stream_read_callback),
      };

      // SAFETY: session pointer is valid during stream lifetime
      unsafe {
        ffi::nghttp2_submit_data2(
          session_ptr,
          ffi::NGHTTP2_FLAG_END_STREAM as u8,
          self.id,
          &mut data_provider as *mut _,
        )
      }
    } else {
      let http2_headers =
        Http2Headers::from_v8_string(scope, headers, count as usize);
      // SAFETY: session pointer and headers are valid
      unsafe {
        ffi::nghttp2_submit_trailer(
          session_ptr,
          self.id,
          http2_headers.data(),
          http2_headers.len(),
        )
      }
    }
  }

  #[fast]
  #[reentrant]
  fn rst_stream(&self, code: u32) {
    log::debug!(
      "sending rst_stream with code {} for stream {}",
      code,
      self.id
    );
    // Defer RST_STREAM if we're inside mem_recv/mem_send to avoid
    // nghttp2 double-free with no_closed_streams=1.
    // SAFETY: session outlives the stream
    let session = unsafe { &mut *self.session };
    session.submit_rst_stream(self.id, code);
  }

  #[fast]
  fn destroy(&self) {
    // SAFETY: session pointer is valid
    let session = unsafe { &mut *self.session };
    session.streams.remove(&self.id);
    log::debug!("destroyed stream {}", self.id);
  }

  #[fast]
  fn priority(
    &self,
    parent: i32,
    weight: i32,
    exclusive: bool,
    silent: bool,
  ) -> i32 {
    let session_ptr = self.nghttp2_session();
    let priority = Http2Priority::new(parent, weight, exclusive);

    // SAFETY: session pointer is valid during stream lifetime
    unsafe {
      if silent {
        ffi::nghttp2_session_change_stream_priority(
          session_ptr,
          self.id,
          &priority.spec,
        )
      } else {
        ffi::nghttp2_submit_priority(
          session_ptr,
          ffi::NGHTTP2_FLAG_NONE as u8,
          self.id,
          &priority.spec,
        )
      }
    }
  }

  fn push_promise<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    headers: v8::Local<v8::String>,
    count: u32,
    options: i32,
  ) -> v8::Local<'s, v8::Value> {
    let session_ptr = self.nghttp2_session();
    let http2_headers =
      Http2Headers::from_v8_string(scope, headers, count as usize);

    // SAFETY: session pointer is valid during stream lifetime
    let ret = unsafe {
      ffi::nghttp2_submit_push_promise(
        session_ptr,
        ffi::NGHTTP2_FLAG_NONE as u8,
        self.id,
        http2_headers.data(),
        http2_headers.len(),
        std::ptr::null_mut(),
      )
    };

    if ret <= 0 {
      return v8::Integer::new(scope, ret).into();
    }

    // SAFETY: self.session is valid for the lifetime of the stream
    let session = unsafe { &mut *self.session };
    let (obj, stream) =
      Http2Stream::new(session, ret, ffi::NGHTTP2_HCAT_HEADERS);
    stream.start_headers(ffi::NGHTTP2_HCAT_HEADERS);
    if (options & STREAM_OPTION_GET_TRAILERS) != 0 {
      stream.set_has_trailers(true);
    }
    let local = v8::Local::new(scope, &obj);
    session.streams.insert(ret, (obj, stream));
    session.send_pending_data();
    local.into()
  }

  #[nofast]
  fn info(
    &self,
    scope: &mut v8::PinScope,
    headers: v8::Local<v8::String>,
    count: u32,
  ) -> i32 {
    let session_ptr = self.nghttp2_session();
    let http2_headers =
      Http2Headers::from_v8_string(scope, headers, count as usize);

    // SAFETY: session pointer is valid during stream lifetime
    unsafe {
      ffi::nghttp2_submit_headers(
        session_ptr,
        ffi::NGHTTP2_FLAG_NONE as u8,
        self.id,
        std::ptr::null(),
        http2_headers.data(),
        http2_headers.len(),
        std::ptr::null_mut(),
      )
    }
  }

  #[fast]
  fn read_start(&self) -> i32 {
    let session_ptr = self.nghttp2_session();
    // SAFETY: session pointer is valid during stream lifetime
    unsafe {
      ffi::nghttp2_session_consume_stream(session_ptr, self.id, 0);
    }
    0
  }

  #[fast]
  fn read_stop(&self) -> i32 {
    0
  }

  #[serde]
  fn get_state(&self) -> Http2StreamState {
    let session_ptr = self.nghttp2_session();

    // SAFETY: session pointer is valid
    let stream_ptr =
      unsafe { ffi::nghttp2_session_find_stream(session_ptr, self.id) };

    if stream_ptr.is_null() {
      return Http2StreamState {
        state: ffi::NGHTTP2_STREAM_STATE_IDLE as f64,
        weight: 0.0,
        sum_dependency_weight: 0.0,
        local_close: 0.0,
        remote_close: 0.0,
        local_window_size: 0.0,
      };
    }

    // SAFETY: stream_ptr is non-null, checked above
    unsafe {
      Http2StreamState {
        state: ffi::nghttp2_stream_get_state(stream_ptr) as f64,
        weight: ffi::nghttp2_stream_get_weight(stream_ptr) as f64,
        sum_dependency_weight: ffi::nghttp2_stream_get_sum_dependency_weight(
          stream_ptr,
        ) as f64,
        local_close: ffi::nghttp2_session_get_stream_local_close(
          session_ptr,
          self.id,
        ) as f64,
        remote_close: ffi::nghttp2_session_get_stream_remote_close(
          session_ptr,
          self.id,
        ) as f64,
        local_window_size: ffi::nghttp2_session_get_stream_local_window_size(
          session_ptr,
          self.id,
        ) as f64,
      }
    }
  }
}
