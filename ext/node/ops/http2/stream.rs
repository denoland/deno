// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;

use deno_core::Resource;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use libnghttp2 as ffi;
use serde::Serialize;

use super::session::Session;
use super::session::SessionCallbacks;
use super::session::on_stream_read_callback;
use super::types::STREAM_OPTION_GET_TRAILERS;
use crate::ops::handle_wrap::AsyncWrap;

// Http2Headers

pub struct Http2Headers {
  #[allow(dead_code)]
  backing_store: String,
  nva: Vec<ffi::nghttp2_nv>,
}

impl Http2Headers {
  pub fn data(&self) -> *const ffi::nghttp2_nv {
    self.nva.as_ptr()
  }

  pub fn len(&self) -> usize {
    self.nva.len()
  }

  pub fn parse(content: String, count: usize) -> Self {
    let bytes = content.as_bytes();
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
      backing_store: content,
      nva,
    }
  }
}

impl From<(String, usize)> for Http2Headers {
  fn from((content, count): (String, usize)) -> Self {
    Self::parse(content, count)
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
  #[allow(dead_code)]
  pub(crate) current_headers_category: ffi::nghttp2_headers_category,
  pub(crate) available_outbound_length: RefCell<usize>,
  pub(crate) pending_data: RefCell<bytes::BytesMut>,
  pub(crate) current_headers: RefCell<Vec<(String, String, u8)>>,
  pub(crate) current_headers_length: RefCell<usize>,
  pub(crate) has_trailers: RefCell<bool>,
}

impl Resource for Http2Stream {}

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
        has_trailers: RefCell::new(false),
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
    let Ok(name_str) = std::str::from_utf8(name) else {
      return false;
    };
    let Ok(value_str) = std::str::from_utf8(value) else {
      return false;
    };

    let header_length = name.len() + value.len() + 32;
    self.current_headers.borrow_mut().push((
      name_str.to_string(),
      value_str.to_string(),
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
    // SAFETY: session pointer is valid during stream lifetime
    let session = unsafe { &*self.session };

    // SAFETY: isolate pointer is valid
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

  fn nghttp2_session(&self) -> *mut ffi::nghttp2_session {
    // SAFETY: session pointer is valid during stream lifetime
    unsafe { (*self.session).session }
  }
}

#[op2]
impl Http2Stream {
  #[fast]
  fn id(&self) -> i32 {
    self.id
  }

  fn respond(&self, #[serde] headers: (String, usize), options: i32) {
    let headers = Http2Headers::from(headers);
    let session_ptr = self.nghttp2_session();

    if (options & STREAM_OPTION_GET_TRAILERS) != 0 {
      self.set_has_trailers(true);
    }

    let mut data_provider = ffi::nghttp2_data_provider {
      source: ffi::nghttp2_data_source {
        ptr: std::ptr::null_mut(),
      },
      read_callback: Some(on_stream_read_callback),
    };

    // SAFETY: session pointer and headers are valid
    unsafe {
      ffi::nghttp2_submit_response(
        session_ptr,
        self.id,
        headers.data(),
        headers.len(),
        &mut data_provider as *mut _,
      );
    }

    std::mem::forget(headers);
  }

  #[fast]
  fn write_utf8_string(
    &self,
    _req: v8::Local<v8::Object>,
    #[string] data: &str,
  ) -> i32 {
    let session_ptr = self.nghttp2_session();

    self
      .pending_data
      .borrow_mut()
      .extend_from_slice(data.as_bytes());
    *self.available_outbound_length.borrow_mut() += data.len();

    // SAFETY: session pointer is valid
    unsafe {
      ffi::nghttp2_session_resume_data(session_ptr, self.id);
    }
    0
  }

  #[fast]
  fn write_buffer(
    &self,
    _req: v8::Local<v8::Object>,
    #[buffer] data: &[u8],
  ) -> i32 {
    let session_ptr = self.nghttp2_session();

    self.pending_data.borrow_mut().extend_from_slice(data);
    *self.available_outbound_length.borrow_mut() += data.len();

    // SAFETY: session pointer is valid
    unsafe {
      ffi::nghttp2_session_resume_data(session_ptr, self.id);
    }
    0
  }

  #[fast]
  fn shutdown(&self, _req: v8::Local<v8::Object>) -> i32 {
    let session_ptr = self.nghttp2_session();
    // SAFETY: session pointer is valid
    unsafe {
      ffi::nghttp2_session_resume_data(session_ptr, self.id);
    }
    // Return 1 to signal synchronous completion
    1
  }

  fn trailers(&self, #[serde] headers: (String, usize)) -> i32 {
    let session_ptr = self.nghttp2_session();

    if headers.1 == 0 {
      let mut data_provider = ffi::nghttp2_data_provider {
        source: ffi::nghttp2_data_source {
          ptr: std::ptr::null_mut(),
        },
        read_callback: Some(on_stream_read_callback),
      };

      // SAFETY: session pointer is valid
      unsafe {
        ffi::nghttp2_submit_data(
          session_ptr,
          ffi::NGHTTP2_FLAG_END_STREAM as u8,
          self.id,
          &mut data_provider as *mut _,
        )
      }
    } else {
      let http2_headers = Http2Headers::from(headers);
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
  fn rst_stream(&self, code: u32) {
    log::debug!(
      "sending rst_stream with code {} for stream {}",
      code,
      self.id
    );
    let session_ptr = self.nghttp2_session();

    // SAFETY: session pointer is valid
    unsafe {
      ffi::nghttp2_submit_rst_stream(
        session_ptr,
        ffi::NGHTTP2_FLAG_NONE as u8,
        self.id,
        code,
      );
    }
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

    // SAFETY: session pointer is valid
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

  fn push_promise(
    &self,
    #[serde] headers: (String, usize),
    _options: i32,
  ) -> i32 {
    let session_ptr = self.nghttp2_session();
    let http2_headers = Http2Headers::from(headers);

    // SAFETY: session pointer and headers are valid
    unsafe {
      ffi::nghttp2_submit_push_promise(
        session_ptr,
        ffi::NGHTTP2_FLAG_NONE as u8,
        self.id,
        http2_headers.data(),
        http2_headers.len(),
        std::ptr::null_mut(),
      )
    }
  }

  fn info(&self, #[serde] headers: (String, usize)) -> i32 {
    let session_ptr = self.nghttp2_session();
    let http2_headers = Http2Headers::from(headers);

    // SAFETY: session pointer and headers are valid
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
    // SAFETY: session pointer is valid
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

    // SAFETY: stream_ptr is valid
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
