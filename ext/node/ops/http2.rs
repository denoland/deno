// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use bytes::BufMut;
use deno_core::AsyncResult;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::serde_v8;
use deno_core::v8;
use deno_net::io::TcpStreamResource;
use deno_net::ops_tls::TlsStreamResource;
use libnghttp2_sys as ffi;
use serde::Serialize;

use crate::ops::handle_wrap::AsyncWrap;

// Stream is not going to have any DATA frames
const STREAM_OPTION_EMPTY_PAYLOAD: i32 = 0x1;
// Stream might have trailing headers
const STREAM_OPTION_GET_TRAILERS: i32 = 0x2;
// Number of max additional settings, thus settings not implemented by nghttp2
const MAX_ADDITIONAL_SETTINGS: usize = 10;

#[repr(usize)]
enum Http2SettingsIndex {
  IDX_SETTINGS_HEADER_TABLE_SIZE,
  IDX_SETTINGS_ENABLE_PUSH,
  IDX_SETTINGS_INITIAL_WINDOW_SIZE,
  IDX_SETTINGS_MAX_FRAME_SIZE,
  IDX_SETTINGS_MAX_CONCURRENT_STREAMS,
  IDX_SETTINGS_MAX_HEADER_LIST_SIZE,
  IDX_SETTINGS_ENABLE_CONNECT_PROTOCOL,
  IDX_SETTINGS_COUNT,
}

#[repr(usize)]
enum Http2SessionStateIndex {
  IDX_SESSION_STATE_EFFECTIVE_LOCAL_WINDOW_SIZE,
  IDX_SESSION_STATE_EFFECTIVE_RECV_DATA_LENGTH,
  IDX_SESSION_STATE_NEXT_STREAM_ID,
  IDX_SESSION_STATE_LOCAL_WINDOW_SIZE,
  IDX_SESSION_STATE_LAST_PROC_STREAM_ID,
  IDX_SESSION_STATE_REMOTE_WINDOW_SIZE,
  IDX_SESSION_STATE_OUTBOUND_QUEUE_SIZE,
  IDX_SESSION_STATE_HD_DEFLATE_DYNAMIC_TABLE_SIZE,
  IDX_SESSION_STATE_HD_INFLATE_DYNAMIC_TABLE_SIZE,
  IDX_SESSION_STATE_COUNT,
}

#[repr(usize)]
enum Http2StreamStateIndex {
  IDX_STREAM_STATE,
  IDX_STREAM_STATE_WEIGHT,
  IDX_STREAM_STATE_SUM_DEPENDENCY_WEIGHT,
  IDX_STREAM_STATE_LOCAL_CLOSE,
  IDX_STREAM_STATE_REMOTE_CLOSE,
  IDX_STREAM_STATE_LOCAL_WINDOW_SIZE,
  IDX_STREAM_STATE_COUNT,
}

#[repr(usize)]
enum Http2OptionsIndex {
  IDX_OPTIONS_MAX_DEFLATE_DYNAMIC_TABLE_SIZE,
  IDX_OPTIONS_MAX_RESERVED_REMOTE_STREAMS,
  IDX_OPTIONS_MAX_SEND_HEADER_BLOCK_LENGTH,
  IDX_OPTIONS_PEER_MAX_CONCURRENT_STREAMS,
  IDX_OPTIONS_PADDING_STRATEGY,
  IDX_OPTIONS_MAX_HEADER_LIST_PAIRS,
  IDX_OPTIONS_MAX_OUTSTANDING_PINGS,
  IDX_OPTIONS_MAX_OUTSTANDING_SETTINGS,
  IDX_OPTIONS_MAX_SESSION_MEMORY,
  IDX_OPTIONS_MAX_SETTINGS,
  IDX_OPTIONS_STREAM_RESET_RATE,
  IDX_OPTIONS_STREAM_RESET_BURST,
  IDX_OPTIONS_STRICT_HTTP_FIELD_WHITESPACE_VALIDATION,
  IDX_OPTIONS_FLAGS,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JSHttp2State<'a> {
  session_state: serde_v8::Value<'a>,
  stream_state: serde_v8::Value<'a>,
  options_buffer: serde_v8::Value<'a>,
  settings_buffer: serde_v8::Value<'a>,
}

static mut SESSION_STATE_BUFFER: [f32;
  Http2SessionStateIndex::IDX_SESSION_STATE_COUNT as usize] = [0.0; _];
static mut STREAM_STATE_BUFFER: [f32;
  Http2StreamStateIndex::IDX_STREAM_STATE_COUNT as usize] = [0.0; _];
static mut OPTIONS_BUFFER: [u32;
  Http2OptionsIndex::IDX_OPTIONS_FLAGS as usize + 1] = [0; _];
static mut SETTINGS_BUFFER: [u32;
  Http2SettingsIndex::IDX_SETTINGS_COUNT as usize
    + 1
    + 1
    + (2 * MAX_ADDITIONAL_SETTINGS)] = [0; _];

impl<'a> JSHttp2State<'a> {
  pub(crate) fn create(scope: &mut v8::PinScope<'a, 'a>) -> Self {
    fn static_f32array<'a>(
      scope: &mut v8::PinScope<'a, 'a>,
      buffer: *mut [f32],
      buffer_len: usize,
    ) -> serde_v8::Value<'a> {
      unsafe extern "C" fn nop_deleter(
        data: *mut std::ffi::c_void,
        byte_length: usize,
        deleter_data: *mut std::ffi::c_void,
      ) {
      }
      unsafe {
        let bs = v8::ArrayBuffer::new_backing_store_from_ptr(
          buffer as _,
          buffer_len * std::mem::size_of::<f32>(),
          nop_deleter,
          std::ptr::null_mut(),
        );
        let ab = v8::ArrayBuffer::with_backing_store(scope, &bs.make_shared());
        v8::Float32Array::new(scope, ab, 0, buffer.len())
          .unwrap()
          .into()
      }
    }

    fn static_u32array<'a>(
      scope: &mut v8::PinScope<'a, 'a>,
      buffer: *mut [u32],
      buffer_len: usize,
    ) -> serde_v8::Value<'a> {
      unsafe extern "C" fn nop_deleter(
        data: *mut std::ffi::c_void,
        byte_length: usize,
        deleter_data: *mut std::ffi::c_void,
      ) {
      }
      unsafe {
        let bs = v8::ArrayBuffer::new_backing_store_from_ptr(
          buffer as _,
          buffer_len * std::mem::size_of::<u32>(),
          nop_deleter,
          std::ptr::null_mut(),
        );
        let ab = v8::ArrayBuffer::with_backing_store(scope, &bs.make_shared());
        v8::Uint32Array::new(scope, ab, 0, buffer.len())
          .unwrap()
          .into()
      }
    }

    unsafe {
      Self {
        session_state: static_f32array(
          scope,
          &raw mut SESSION_STATE_BUFFER,
          Http2SessionStateIndex::IDX_SESSION_STATE_COUNT as usize,
        ),
        stream_state: static_f32array(
          scope,
          &raw mut STREAM_STATE_BUFFER,
          Http2StreamStateIndex::IDX_STREAM_STATE_COUNT as usize,
        ),
        options_buffer: static_u32array(
          scope,
          &raw mut OPTIONS_BUFFER,
          Http2OptionsIndex::IDX_OPTIONS_FLAGS as usize + 1,
        ),
        settings_buffer: static_u32array(
          scope,
          &raw mut SETTINGS_BUFFER,
          Http2SettingsIndex::IDX_SETTINGS_COUNT as usize
            + 1
            + 1
            + (2 * MAX_ADDITIONAL_SETTINGS),
        ),
      }
    }
  }
}

#[op2]
#[serde]
pub fn op_http2_http_state<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
) -> JSHttp2State<'a> {
  JSHttp2State::create(scope)
}

#[derive(Debug)]
struct NgHttp2StreamWrite {
  data: bytes::Bytes,
  stream_id: i32,
}

impl NgHttp2StreamWrite {
  fn new(data: bytes::Bytes, stream_id: i32) -> Self {
    Self { data, stream_id }
  }

  fn as_ptr(&self) -> *const u8 {
    self.data.as_ptr()
  }

  fn len(&self) -> usize {
    self.data.len()
  }
}

#[derive(Debug)]
#[repr(i32)]
enum SessionType {
  Server,
  Client,
}

struct Session {
  session: *mut ffi::nghttp2_session,
  // Keep track of each stream's reference for this session using
  // it's frame id.
  streams: HashMap<i32, (v8::Global<v8::Object>, cppgc::Ref<Http2Stream>)>,

  // Outbound data buffering system using bytes::Bytes for zero-copy efficiency
  outgoing_buffers: Vec<NgHttp2StreamWrite>,
  outgoing_length: usize,

  isolate: v8::UnsafeRawIsolatePtr,
  context: v8::Global<v8::Context>,
  op_state: Rc<RefCell<OpState>>,
  this: v8::Global<v8::Object>,
}

struct Http2Callbacks {
  session_internal_error_cb: v8::Global<v8::Function>,
  priority_frame_cb: v8::Global<v8::Function>,
  settings_frame_cb: v8::Global<v8::Function>,
  ping_frame_cb: v8::Global<v8::Function>,
  headers_frame_cb: v8::Global<v8::Function>,
  frame_error_cb: v8::Global<v8::Function>,
  goaway_data_cb: v8::Global<v8::Function>,
  alt_svc_cb: v8::Global<v8::Function>,
  stream_trailers_cb: v8::Global<v8::Function>,
  stream_close_cb: v8::Global<v8::Function>,
}

#[op2]
pub fn op_http2_callbacks(
  state: &mut OpState,
  #[global] session_internal_error_cb: v8::Global<v8::Function>,
  #[global] priority_frame_cb: v8::Global<v8::Function>,
  #[global] settings_frame_cb: v8::Global<v8::Function>,
  #[global] ping_frame_cb: v8::Global<v8::Function>,
  #[global] headers_frame_cb: v8::Global<v8::Function>,
  #[global] frame_error_cb: v8::Global<v8::Function>,
  #[global] goaway_data_cb: v8::Global<v8::Function>,
  #[global] alt_svc_cb: v8::Global<v8::Function>,
  #[global] stream_trailers_cb: v8::Global<v8::Function>,
  #[global] stream_close_cb: v8::Global<v8::Function>,
) {
  state.put(Http2Callbacks {
    session_internal_error_cb,
    priority_frame_cb,
    settings_frame_cb,
    ping_frame_cb,
    headers_frame_cb,
    frame_error_cb,
    goaway_data_cb,
    alt_svc_cb,
    stream_trailers_cb,
    stream_close_cb,
  });
}

impl Session {
  fn find_stream(&self, frame_id: i32) -> Option<&cppgc::Ref<Http2Stream>> {
    self.streams.get(&frame_id).map(|v| &v.1)
  }

  fn find_stream_obj(&self, frame_id: i32) -> Option<&v8::Global<v8::Object>> {
    self.streams.get(&frame_id).map(|v| &v.0)
  }

  fn push_outgoing_buffer(&mut self, write: NgHttp2StreamWrite) {
    self.outgoing_length += write.len();
    self.outgoing_buffers.push(write);
  }

  fn copy_data_into_outgoing(&mut self, data: &[u8], stream_id: i32) {
    let bytes_data = bytes::Bytes::copy_from_slice(data);
    let write = NgHttp2StreamWrite::new(bytes_data, stream_id);
    self.push_outgoing_buffer(write);
  }

  fn clear_outgoing(&mut self) {
    self.outgoing_buffers.clear();
    self.outgoing_length = 0;
  }

  fn should_send_pending_data(&self) -> bool {
    self.outgoing_length > 4096
  }

  // Called by `on_frame_recv` to notify JavaScript that a complete
  // HEADERS frame has been received and processed. This method converts the
  // received headers into a JavaScript array and pushes those out to JS.
  fn handle_headers_frame(&self, frame: *const ffi::nghttp2_frame) {
    // SAFETY: This method is called by `on_frame_recv`.
    // The isolate is valid and we are on the same thread as the isolate.
    let mut isolate =
      unsafe { v8::Isolate::from_raw_isolate_ptr(self.isolate) };
    v8::scope!(let scope, &mut isolate);
    let context = v8::Local::new(scope, self.context.clone());
    let scope = &mut v8::ContextScope::new(scope, context);

    let id = frame_id(frame);
    let stream_ref = match self.find_stream(id) {
      Some(s) => s,
      None => return,
    };

    let headers = stream_ref.current_headers.borrow();
    let header_count = headers.len();

    if header_count == 0 {
      return;
    }

    // Create the JS headers array
    // [name1, value1, name2, value2...]
    let headers_array = v8::Array::new(scope, (header_count * 2) as i32);
    for (i, (name, value, _flags)) in headers.iter().enumerate() {
      let name_str = v8::String::new(scope, name).unwrap();
      let value_str = v8::String::new(scope, value).unwrap();
      headers_array.set_index(scope, (i * 2) as u32, name_str.into());
      headers_array.set_index(scope, (i * 2 + 1) as u32, value_str.into());
    }

    drop(headers);
    stream_ref.clear_headers();

    let stream_obj = self.find_stream_obj(id).unwrap();
    let state = self.op_state.borrow();
    let callbacks = state.borrow::<Http2Callbacks>();
    let recv = v8::Local::new(scope, &self.this);
    let callback = v8::Local::new(scope, &callbacks.headers_frame_cb);

    drop(state);

    let frame_flags = unsafe { (*frame).hd.flags };
    let handle = v8::Local::new(scope, stream_obj);
    let id_num = v8::Number::new(scope, id.into());
    let cat = v8::null(scope);
    let flags = v8::Number::new(scope, frame_flags.into());

    // Call headers callback
    callback.call(
      scope,
      recv.into(),
      &[
        handle.into(),
        id_num.into(),
        cat.into(),
        flags.into(),
        headers_array.into(),
      ],
    );
  }

  fn handle_ping_frame(&self, frame: *const ffi::nghttp2_frame) {
    // SAFETY: This method is called by `on_frame_recv`.
    // The isolate is valid and we are on the same thread as the isolate.
    let mut isolate =
      unsafe { v8::Isolate::from_raw_isolate_ptr(self.isolate) };
    v8::scope!(let scope, &mut isolate);
    let context = v8::Local::new(scope, self.context.clone());
    let scope = &mut v8::ContextScope::new(scope, context);

    let state = self.op_state.borrow();
    let callbacks = state.borrow::<Http2Callbacks>();
    let recv = v8::Local::new(scope, &self.this);
    let callback = v8::Local::new(scope, &callbacks.ping_frame_cb);

    drop(state);

    let arg = v8::null(scope);
    callback.call(scope, recv.into(), &[arg.into()]);
  }

  fn handle_goaway_frame(&self, frame: *const ffi::nghttp2_frame) {}
}

pub struct Http2Session {
  type_: SessionType,
  session: *mut ffi::nghttp2_session,
  callbacks: *mut ffi::nghttp2_session_callbacks,
  // Shared data between nghttp2 callbacks and JS object. Must only
  // live as long as `self.session`.
  inner: *mut Session,
}

unsafe impl deno_core::GarbageCollected for Http2Session {
  fn trace(&self, _: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Http2Session"
  }
}

enum NetworkStream {
  Tcp(Rc<TcpStreamResource>),
  Tls(Rc<TlsStreamResource>),
}

struct Http2SessionDriver {
  stream: NetworkStream,
  session: *mut Session,
}

impl Http2SessionDriver {
  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    // TODO(littledivy): make op_read reentrace and implement Http2Scope
    // Prevent re-entry on same tick
    tokio::task::yield_now().await;
    self.send_pending_data().await?;

    let nread = match &self.stream {
      NetworkStream::Tcp(stream) => stream.clone().read(data).await?,
      NetworkStream::Tls(stream) => stream.clone().read(data).await?,
    };

    let session = unsafe { &*self.session };
    let ret = unsafe {
      ffi::nghttp2_session_mem_recv(
        session.session,
        data.as_mut_ptr() as _,
        nread,
      )
    };

    // Send any pending data that nghttp2 wants to output
    self.send_pending_data().await?;

    Ok(nread)
  }

  pub async fn write(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, std::io::Error> {
    match &self.stream {
      NetworkStream::Tcp(stream) => stream.clone().write(data).await,
      NetworkStream::Tls(stream) => stream.clone().write(data).await,
    }
  }

  async fn send_pending_data(&self) -> Result<(), std::io::Error> {
    let session = unsafe { &mut *self.session };

    loop {
      let mut src = std::ptr::null();
      unsafe {
        let src_len = ffi::nghttp2_session_mem_send(session.session, &mut src);
        if src_len > 0 {
          let data = std::slice::from_raw_parts(src, src_len as usize);
          match &self.stream {
            NetworkStream::Tcp(stream) => stream.clone().write(data).await?,
            NetworkStream::Tls(stream) => stream.clone().write(data).await?,
          };
        } else {
          break;
        }
      }
    }

    // If we have queued outgoing buffers, send them now
    if !session.outgoing_buffers.is_empty() {
      for buffer in &session.outgoing_buffers {
        let data = buffer.data.as_ref();
        match &self.stream {
          NetworkStream::Tcp(stream) => stream.clone().write(data).await?,
          NetworkStream::Tls(stream) => stream.clone().write(data).await?,
        };
      }
      session.clear_outgoing();
    }

    Ok(())
  }
}

impl Resource for Http2SessionDriver {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();
}

#[derive(Debug)]
pub struct Http2Stream {
  session: *mut Session,
  id: i32,
  // As headers are received for this stream, they are temporarily stored
  // until the full HEADER frame is received.
  current_headers_category: ffi::nghttp2_headers_category,
  // Track available outbound data length for this stream
  available_outbound_length: RefCell<usize>,
  // Buffer for stream data waiting to be sent
  pending_data: RefCell<bytes::BytesMut>,
  // Current headers being processed
  current_headers: RefCell<Vec<(String, String, u8)>>,
  current_headers_length: RefCell<usize>,
  // Flag to indicate if this stream has trailers to send
  has_trailers: RefCell<bool>,
}

impl Http2Stream {
  fn new(
    session: &mut Session,
    id: i32,
    cat: ffi::nghttp2_headers_category,
  ) -> (v8::Global<v8::Object>, Option<cppgc::Ref<Self>>) {
    // SAFETY: This method is called by `on_frame_recv`.
    // The isolate is valid and we are on the same thread as the isolate.
    let mut isolate =
      unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
    v8::scope!(let scope, &mut isolate);
    let context = v8::Local::new(scope, session.context.clone());
    let scope = &mut v8::ContextScope::new(scope, context);

    let obj = cppgc::make_cppgc_empty_object::<Http2Stream>(scope);
    let async_wrap = {
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

    (
      v8::Global::new(scope, obj),
      cppgc::try_unwrap_cppgc_persistent_object::<Http2Stream>(
        scope,
        obj.into(),
      ),
    )
  }
}

impl Resource for Http2Stream {}

unsafe impl deno_core::GarbageCollected for Http2Stream {
  fn trace(&self, _: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Http2Stream"
  }
}

struct Http2Headers {
  backing_store: String,
  nva: Vec<ffi::nghttp2_nv>,
}

impl Http2Headers {
  fn data(&self) -> *const ffi::nghttp2_nv {
    self.nva.as_ptr()
  }

  fn len(&self) -> usize {
    self.nva.len()
  }
}

impl From<(String, usize)> for Http2Headers {
  fn from(arr: (String, usize)) -> Http2Headers {
    let mut nva = Vec::new();
    let mut i = 0;

    let header_contents = arr.0.as_bytes();
    let count = arr.1;
    while i < header_contents.len() {
      if nva.len() >= count {
        static ZERO: u8 = 0;
        nva.clear();
        nva.push(ffi::nghttp2_nv {
          name: &ZERO as *const _ as *mut _,
          namelen: 1,
          value: &ZERO as *const _ as *mut _,
          valuelen: 1,
          flags: 0,
        });
        break;
      }

      let name_end = match header_contents[i..].iter().position(|&b| b == 0) {
        Some(p) => i + p,
        None => break,
      };
      let name_ptr = unsafe { header_contents.as_ptr().add(i) };
      let namelen = name_end - i;
      i = name_end + 1;
      if i >= header_contents.len() {
        break;
      }

      let value_end = match header_contents[i..].iter().position(|&b| b == 0) {
        Some(p) => i + p,
        None => break,
      };
      let value_ptr = unsafe { header_contents.as_ptr().add(i) };
      let valuelen = value_end - i;
      i = value_end + 1;
      if i >= header_contents.len() {
        break;
      }

      let flags = *header_contents.get(i).unwrap_or(&0);
      i += 1;

      nva.push(ffi::nghttp2_nv {
        name: name_ptr as *mut _,
        namelen,
        value: value_ptr as *mut _,
        valuelen,
        flags,
      });
    }

    Http2Headers {
      nva,
      backing_store: arr.0,
    }
  }
}

#[op2]
impl Http2Stream {
  fn respond(&self, #[serde] headers: (String, usize), options: i32) {
    let headers = Http2Headers::from(headers);

    let session = unsafe { &*self.session };

    // Check if the stream will have trailers based on options
    // STREAM_OPTION_GET_TRAILERS = 0x2 from the constants
    if (options & 0x2) != 0 {
      self.set_has_trailers(true);
    }

    let mut data_provider = ffi::nghttp2_data_provider {
      source: ffi::nghttp2_data_source {
        ptr: std::ptr::null_mut(),
      },
      read_callback: Some(on_stream_read_callback),
    };

    unsafe {
      ffi::nghttp2_submit_response(
        session.session,
        self.id,
        headers.data(),
        headers.len(),
        &mut data_provider as *mut _,
      );
    }

    std::mem::forget(headers); // TODO: tie backing store up to stream's lifetime
  }

  #[fast]
  fn refresh_state(&self) {}

  #[fast]
  fn write_utf8_string(
    &self,
    _req: v8::Local<v8::Object>,
    #[string] data: &str,
  ) -> i32 {
    let session = unsafe { &mut *self.session };

    self
      .pending_data
      .borrow_mut()
      .extend_from_slice(data.as_bytes());
    *self.available_outbound_length.borrow_mut() += data.len();

    // Resume data for this stream so nghttp2 knows there's data available
    unsafe {
      ffi::nghttp2_session_resume_data(session.session, self.id);
    }

    0
  }

  #[fast]
  fn shutdown(&self) {
    let session = unsafe { &*self.session };
    unsafe {
      ffi::nghttp2_session_resume_data(session.session, self.id);
    }
  }

  // Submit informational headers for a stream.
  fn trailers(&self, #[serde] headers: (String, usize)) -> i32 {
    let session = unsafe { &*self.session };

    // Sending an empty trailers frame poses problems in Safari, Edge & IE.
    // Instead we can just send an empty data frame with NGHTTP2_FLAG_END_STREAM
    // to indicate that the stream is ready to be closed.
    if headers.1 == 0 {
      let mut data_provider = ffi::nghttp2_data_provider {
        source: ffi::nghttp2_data_source {
          ptr: std::ptr::null_mut(),
        },
        read_callback: Some(on_stream_read_callback),
      };

      unsafe {
        ffi::nghttp2_submit_data(
          session.session,
          ffi::NGHTTP2_FLAG_END_STREAM as u8,
          self.id,
          &mut data_provider as *mut _,
        )
      }
    } else {
      let http2_headers = Http2Headers::from(headers);
      unsafe {
        ffi::nghttp2_submit_trailer(
          session.session,
          self.id,
          http2_headers.data(),
          http2_headers.len(),
        )
      }
    }
  }
}

impl Http2Stream {
  fn add_header(&self, name: &[u8], value: &[u8], flags: u8) -> bool {
    let name_str = match std::str::from_utf8(name) {
      Ok(s) => s.to_string(),
      Err(_) => return false,
    };
    let value_str = match std::str::from_utf8(value) {
      Ok(s) => s.to_string(),
      Err(_) => return false,
    };

    let header_length = name.len() + value.len() + 32; // Add some overhead
    self
      .current_headers
      .borrow_mut()
      .push((name_str, value_str, flags));
    *self.current_headers_length.borrow_mut() += header_length;
    true
  }

  fn clear_headers(&self) {
    self.current_headers.borrow_mut().clear();
    *self.current_headers_length.borrow_mut() = 0;
  }

  fn headers_count(&self) -> usize {
    self.current_headers.borrow().len()
  }

  fn start_headers(&self, category: ffi::nghttp2_headers_category) {
    self.clear_headers();
    // TODO: Store category for later use
  }

  // Called when stream is ready to send trailers
  fn on_trailers(&self) {
    let session = unsafe { &*self.session };

    // SAFETY: This method is called from nghttp2 callback context.
    // The isolate is valid and we are on the same thread as the isolate.
    let mut isolate =
      unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
    v8::scope!(let scope, &mut isolate);
    let context = v8::Local::new(scope, session.context.clone());
    let scope = &mut v8::ContextScope::new(scope, context);

    let state = session.op_state.borrow();
    let callbacks = state.borrow::<Http2Callbacks>();
    let callback = v8::Local::new(scope, &callbacks.stream_trailers_cb);

    drop(state);

    let stream_obj = session.find_stream_obj(self.id).unwrap();
    let recv = v8::Local::new(scope, stream_obj);

    self.set_has_trailers(false);

    callback.call(scope, recv.into(), &[]);
  }

  // Set the trailers flag
  fn set_has_trailers(&self, has_trailers: bool) {
    *self.has_trailers.borrow_mut() = has_trailers;
  }

  // Check if stream has trailers
  fn has_trailers(&self) -> bool {
    *self.has_trailers.borrow()
  }
}

fn frame_id(frame: *const ffi::nghttp2_frame) -> i32 {
  // SAFETY: valid pointer and union tag check
  unsafe {
    let frame = &*frame;
    if frame.hd.type_ as u32 == ffi::NGHTTP2_PUSH_PROMISE {
      frame.push_promise.promised_stream_id
    } else {
      frame.hd.stream_id
    }
  }
}

// Called by nghttp2 at the start of receiving a HEADERS frame. We use this
// callback to determine if a new stream is being created or if we are simply
// adding a new block of headers to an existing stream. The header pairs
// themselves are set in the OnHeaderCallback
unsafe extern "C" fn on_begin_headers_callbacks(
  session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  data: *mut c_void,
) -> i32 {
  let session = &mut *(data as *mut Session);
  let id = frame_id(frame);
  let headers_category = (&*frame).headers.cat;

  let stream = session.find_stream(id);
  match stream {
    // The common case is that we're creating a new stream. The less likely
    // case is that we're receiving a set of trailers
    None => {
      let (obj, stream) = Http2Stream::new(session, id, headers_category);
      if let Some(stream_ref) = &stream {
        stream_ref.start_headers(headers_category);
      }
      session.streams.insert(id, (obj, stream.unwrap()));
    }
    Some(s) => {
      s.start_headers(headers_category);
    }
  }

  0
}

unsafe extern "C" fn on_header_callback(
  session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  name: *mut ffi::nghttp2_rcbuf,
  value: *mut ffi::nghttp2_rcbuf,
  flags: u8,
  data: *mut c_void,
) -> i32 {
  let session = &mut *(data as *mut Session);
  let id = frame_id(frame);

  if let Some(stream) = session.find_stream(id) {
    let name_vec = ffi::nghttp2_rcbuf_get_buf(name);
    let value_vec = ffi::nghttp2_rcbuf_get_buf(value);

    let name_slice = std::slice::from_raw_parts(name_vec.base, name_vec.len);
    let value_slice = std::slice::from_raw_parts(value_vec.base, value_vec.len);

    if !stream.add_header(name_slice, value_slice, flags) {
      // Too many headers
      ffi::nghttp2_submit_rst_stream(
        session.session,
        ffi::NGHTTP2_FLAG_NONE as u8,
        id,
        ffi::NGHTTP2_ENHANCE_YOUR_CALM,
      );
      return ffi::NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE as i32;
    }
  }

  0
}

// Called by nghttp2 when a complete HTTP2 frame has been received. There are
// only a handful of frame types that we care about handling here.
unsafe extern "C" fn on_frame_recv_callback(
  session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  data: *mut c_void,
) -> i32 {
  let session = &mut *(data as *mut Session);
  let type_ = (&*frame).hd.type_;

  match type_ as u32 {
    ffi::NGHTTP2_DATA => {
      // session.handle_data_frame(frame);
    }
    ffi::NGHTTP2_PUSH_PROMISE | ffi::NGHTTP2_HEADERS => {
      session.handle_headers_frame(frame);
    }
    ffi::NGHTTP2_SETTINGS => {
      // session.handle_settings_frame(frame);
    }
    ffi::NGHTTP2_PRIORITY => {
      // session.handle_priority_frame(frame);
    }
    ffi::NGHTTP2_GOAWAY => {
      session.handle_goaway_frame(frame);
    }
    ffi::NGHTTP2_PING => {
      session.handle_ping_frame(frame);
    }
    ffi::NGHTTP2_ALTSVC => {
      // session.handle_alt_svc_frame(frame);
    }
    ffi::NGHTTP2_ORIGIN => {
      // session.handle_origin_frame(frame);
    }
    _ => {}
  }

  0
}

unsafe extern "C" fn on_stream_close_callback(
  session: *mut ffi::nghttp2_session,
  stream_id: i32,
  error_code: u32,
  data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_data_chunk_recv_callback(
  session: *mut ffi::nghttp2_session,
  flags: u8,
  stream_id: i32,
  data: *const u8,
  len: usize,
  user_data: *mut c_void,
) -> i32 {
  // We should never actually get a 0-length chunk so this check is
  // only a precaution at this point.
  if len == 0 {
    return 0;
  }

  // Notify nghttp2 that we've consumed a chunk of data on the connection
  // so that it can send a WINDOW_UPDATE frame. This is a critical part of
  // the flow control process in http2
  unsafe {
    ffi::nghttp2_session_consume_connection(session, len);
  }

  0
}

unsafe extern "C" fn on_frame_not_send_callback(
  session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  lib_error_code: i32,
  data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_invalid_header_callback(
  session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  name: *mut ffi::nghttp2_rcbuf,
  value: *mut ffi::nghttp2_rcbuf,
  flags: u8,
  data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_nghttp_error_callback(
  session: *mut ffi::nghttp2_session,
  lib_error_code: i32,
  msg: *const std::ffi::c_char,
  len: usize,
  data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_send_data_callback(
  session: *mut ffi::nghttp2_session,
  frame: *mut ffi::nghttp2_frame,
  framehd: *const u8,
  length: usize,
  source: *mut ffi::nghttp2_data_source,
  data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_invalid_frame_recv_callback(
  session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  lib_error_code: i32,
  data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_frame_send_callback(
  session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  data: *mut c_void,
) -> i32 {
  0
}

// Data provider callback for nghttp2 to read stream data
unsafe extern "C" fn on_stream_read_callback(
  session: *mut ffi::nghttp2_session,
  stream_id: i32,
  buf: *mut u8,
  length: usize,
  data_flags: *mut u32,
  source: *mut ffi::nghttp2_data_source,
  user_data: *mut c_void,
) -> isize {
  let session = &mut *(user_data as *mut Session);

  if let Some(stream) = session.find_stream(stream_id) {
    let mut pending_data = stream.pending_data.borrow_mut();
    if !pending_data.is_empty() {
      let amount = std::cmp::min(pending_data.len(), length);
      if amount > 0 {
        let data_slice = pending_data.split_to(amount);
        std::ptr::copy_nonoverlapping(data_slice.as_ptr(), buf, amount);
        *stream.available_outbound_length.borrow_mut() -= amount;

        if pending_data.is_empty() {
          *data_flags |= ffi::NGHTTP2_DATA_FLAG_EOF as u32;
          // If stream has trailers, don't end stream yet and trigger trailers callback
          if stream.has_trailers() {
            *data_flags |= ffi::NGHTTP2_DATA_FLAG_NO_END_STREAM as u32;
            stream.on_trailers();
          }
        }

        return amount as isize;
      }
    }

    // TODO(littledivy): emit wants write.

    if pending_data.is_empty() {
      *data_flags |= ffi::NGHTTP2_DATA_FLAG_EOF as u32;
      // If stream has trailers, don't end stream yet and trigger trailers callback
      if stream.has_trailers() {
        *data_flags |= ffi::NGHTTP2_DATA_FLAG_NO_END_STREAM as u32;
        stream.on_trailers();
      }

      return 0;
    }
  }

  // No data available, defer
  ffi::NGHTTP2_ERR_DEFERRED as _
}

impl Http2Session {
  fn callbacks() -> *mut ffi::nghttp2_session_callbacks {
    let mut callbacks: *mut ffi::nghttp2_session_callbacks =
      std::ptr::null_mut();
    unsafe {
      assert_eq!(ffi::nghttp2_session_callbacks_new(&mut callbacks), 0);

      ffi::nghttp2_session_callbacks_set_on_begin_headers_callback(
        callbacks,
        Some(on_begin_headers_callbacks),
      );
      ffi::nghttp2_session_callbacks_set_on_header_callback2(
        callbacks,
        Some(on_header_callback),
      );
      ffi::nghttp2_session_callbacks_set_on_frame_recv_callback(
        callbacks,
        Some(on_frame_recv_callback),
      );
      ffi::nghttp2_session_callbacks_set_on_stream_close_callback(
        callbacks,
        Some(on_stream_close_callback),
      );
      ffi::nghttp2_session_callbacks_set_on_data_chunk_recv_callback(
        callbacks,
        Some(on_data_chunk_recv_callback),
      );
      ffi::nghttp2_session_callbacks_set_on_frame_not_send_callback(
        callbacks,
        Some(on_frame_not_send_callback),
      );
      ffi::nghttp2_session_callbacks_set_on_invalid_header_callback2(
        callbacks,
        Some(on_invalid_header_callback),
      );
      ffi::nghttp2_session_callbacks_set_error_callback2(
        callbacks,
        Some(on_nghttp_error_callback),
      );
      ffi::nghttp2_session_callbacks_set_send_data_callback(
        callbacks,
        Some(on_send_data_callback),
      );
      ffi::nghttp2_session_callbacks_set_on_invalid_frame_recv_callback(
        callbacks,
        Some(on_invalid_frame_recv_callback),
      );
      ffi::nghttp2_session_callbacks_set_on_frame_send_callback(
        callbacks,
        Some(on_frame_send_callback),
      );
    }
    callbacks
  }

  fn create(
    this: v8::Global<v8::Object>,
    isolate: &v8::Isolate,
    scope: &mut v8::PinScope<'_, '_>,
    op_state: Rc<RefCell<OpState>>,
    session_type: SessionType,
  ) -> Self {
    let mut session: *mut ffi::nghttp2_session = std::ptr::null_mut();
    let func = match session_type {
      SessionType::Server => ffi::nghttp2_session_server_new2,
      SessionType::Client => ffi::nghttp2_session_client_new2,
    };

    let context = scope.get_current_context();
    let context = v8::Global::new(scope, context);
    // SAFETY: just grabbing the raw pointer
    let isolate = unsafe { isolate.as_raw_isolate_ptr() };

    let mut inner = Box::into_raw(Box::new(Session {
      session,
      streams: HashMap::new(),
      op_state,
      context,
      isolate,
      this,
      outgoing_buffers: Vec::with_capacity(32),
      outgoing_length: 0,
    }));

    // SAFETY: inner is owned by Http2Session but
    // never holds a mutable reference.
    //
    // TODO(littledivy): there are safer ways to do this
    unsafe {
      (func)(
        &mut session,
        Self::callbacks(),
        inner as *mut _ as *mut _,
        std::ptr::null_mut(),
      );

      (&mut *inner).session = session;
    }

    Self {
      type_: session_type,
      session,
      callbacks: std::ptr::null_mut(),
      inner,
    }
  }
}

#[op2]
impl Http2Session {
  #[constructor]
  #[cppgc]
  fn new(
    #[this] this: v8::Global<v8::Object>,
    isolate: &v8::Isolate,
    scope: &mut v8::PinScope<'_, '_>,
    op_state: Rc<RefCell<OpState>>,
    #[smi] type_: i32,
  ) -> Http2Session {
    Http2Session::create(
      this,
      isolate,
      scope,
      op_state,
      match type_ {
        0 => SessionType::Server,
        1 => SessionType::Client,
        _ => unreachable!(),
      },
    )
  }

  #[fast]
  #[smi]
  fn consume(&self, state: &mut OpState, rid: u32) -> u32 {
    let mut stream;

    if let Ok(tcp) = state.resource_table.take::<TcpStreamResource>(rid) {
      stream = NetworkStream::Tcp(tcp);
    } else {
      stream = NetworkStream::Tls(
        state.resource_table.take::<TlsStreamResource>(rid).unwrap(),
      );
    };

    state.resource_table.add(Http2SessionDriver {
      stream,
      session: self.inner,
    })
  }

  #[fast]
  fn destroy(&self) {}

  // Submit SETTINGS frame for the Http2Session
  #[fast]
  fn settings(&self, cb: v8::Local<v8::Function>) -> bool {
    let settings = Http2Settings::init(self.inner);
    settings.send();
    true
  }

  #[fast]
  fn goaway(&self) {}

  #[fast]
  fn set_graceful_close(&self) {}

  #[fast]
  fn has_pending_data(&self) -> bool {
    false
  }

  #[fast]
  fn local_settings(&self) {}

  #[fast]
  fn refresh_state(&self) {}
}

struct Http2Settings {
  entries: [ffi::nghttp2_settings_entry;
    Http2SettingsIndex::IDX_SETTINGS_COUNT as usize + MAX_ADDITIONAL_SETTINGS],
  count: usize,
  session: *mut Session,
}

impl Http2Settings {
  fn init(session: *mut Session) -> Self {
    unsafe {
      #[allow(static_mut_refs)]
      let buffer = &mut SETTINGS_BUFFER;
      let flags = buffer[Http2SettingsIndex::IDX_SETTINGS_COUNT as usize];
      let mut count: usize = 0;

      let mut entries = [ffi::nghttp2_settings_entry {
        settings_id: 0,
        value: 0,
      }; _];
      macro_rules! grab_setting {
            ($name:ident) => {
                paste::paste! {
                if flags & (1 << Http2SettingsIndex::[<IDX_SETTINGS_ $name>] as u8 ) != 0 {
                    let val = buffer[ Http2SettingsIndex::[<IDX_SETTINGS_ $name>] as usize ];
                    if count < entries.len() {
                        entries[count] = ffi::nghttp2_settings_entry {
                            settings_id: ffi:: [<NGHTTP2_SETTINGS_ $name>] as _,
                            value: val,
                        };
                    }
                    count += 1;
                }
                }
            };
        }

      grab_setting!(HEADER_TABLE_SIZE);
      grab_setting!(ENABLE_PUSH);
      grab_setting!(INITIAL_WINDOW_SIZE);
      grab_setting!(MAX_FRAME_SIZE);
      grab_setting!(MAX_CONCURRENT_STREAMS);
      grab_setting!(MAX_HEADER_LIST_SIZE);
      // grab_setting!(ENABLE_CONNECT_PROTOCOL);

      let num_add_settings =
        buffer[Http2SettingsIndex::IDX_SETTINGS_COUNT as usize + 1] as usize;
      if num_add_settings > 0 {
        let offset = Http2SettingsIndex::IDX_SETTINGS_COUNT as usize + 2;
        for i in 0..num_add_settings {
          let key = buffer[offset + i * 2];
          let val = buffer[offset + i * 2 + 1];
          if count < entries.len() {
            entries[count] = ffi::nghttp2_settings_entry {
              settings_id: key as i32,
              value: val,
            };
          }
          count += 1;
        }
      }

      Self {
        session,
        entries,
        count,
      }
    }
  }

  fn send(&self) {
    unsafe {
      let session = &*self.session;
      // TODO: update local settings
      ffi::nghttp2_submit_settings(
        session.session,
        ffi::NGHTTP2_FLAG_NONE as _,
        self.entries.as_ptr(),
        self.count,
      );
    }
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct Http2Constants {
  nghttp2_hcat_request: u32,
  nghttp2_hcat_response: u32,
  nghttp2_hcat_push_response: u32,
  nghttp2_hcat_headers: u32,
  nghttp2_nv_flag_none: u32,
  nghttp2_nv_flag_no_index: u32,
  nghttp2_err_deferred: i32,
  nghttp2_err_stream_id_not_available: i32,
  nghttp2_err_invalid_argument: i32,
  nghttp2_err_stream_closed: i32,
  nghttp2_err_nomem: i32,
  stream_option_empty_payload: i32,
  stream_option_get_trailers: i32,

  http2_header_status: &'static str,
  http2_header_method: &'static str,
  http2_header_authority: &'static str,
  http2_header_scheme: &'static str,
  http2_header_path: &'static str,
  http2_header_protocol: &'static str,
  http2_header_access_control_allow_credentials: &'static str,
  http2_header_access_control_max_age: &'static str,
  http2_header_access_control_request_method: &'static str,
  http2_header_age: &'static str,
  http2_header_authorization: &'static str,
  http2_header_content_encoding: &'static str,
  http2_header_content_language: &'static str,
  http2_header_content_length: &'static str,
  http2_header_content_location: &'static str,
  http2_header_content_md5: &'static str,
  http2_header_content_range: &'static str,
  http2_header_content_type: &'static str,
  http2_header_cookie: &'static str,
  http2_header_date: &'static str,
  http2_header_dnt: &'static str,
  http2_header_etag: &'static str,
  http2_header_expires: &'static str,
  http2_header_from: &'static str,
  http2_header_host: &'static str,
  http2_header_if_match: &'static str,
  http2_header_if_none_match: &'static str,
  http2_header_if_modified_since: &'static str,
  http2_header_if_range: &'static str,
  http2_header_if_unmodified_since: &'static str,
  http2_header_last_modified: &'static str,
  http2_header_location: &'static str,
  http2_header_max_forwards: &'static str,
  http2_header_proxy_authorization: &'static str,
  http2_header_range: &'static str,
  http2_header_referer: &'static str,
  http2_header_retry_after: &'static str,
  http2_header_set_cookie: &'static str,
  http2_header_tk: &'static str,
  http2_header_upgrade_insecure_requests: &'static str,
  http2_header_user_agent: &'static str,
  http2_header_x_content_type_options: &'static str,

  http2_header_connection: &'static str,
  http2_header_upgrade: &'static str,
  http2_header_http2_settings: &'static str,
  http2_header_te: &'static str,
  http2_header_transfer_encoding: &'static str,
  http2_header_keep_alive: &'static str,
  http2_header_proxy_connection: &'static str,

  http2_method_connect: &'static str,
  http2_method_delete: &'static str,
  http2_method_get: &'static str,
  http2_method_head: &'static str,

  http_status_ok: u32,

  nghttp2_err_frame_size_error: u32,
  nghttp2_session_server: u32,
  nghttp2_session_client: u32,
  nghttp2_stream_state_idle: u32,
  nghttp2_stream_state_open: u32,
  nghttp2_stream_state_reserved_local: u32,
  nghttp2_stream_state_reserved_remote: u32,
  nghttp2_stream_state_half_closed_local: u32,
  nghttp2_stream_state_half_closed_remote: u32,
  nghttp2_stream_state_closed: u32,
  nghttp2_flag_none: u32,
  nghttp2_flag_end_stream: u32,
  nghttp2_flag_end_headers: u32,
  nghttp2_flag_ack: u32,
  nghttp2_flag_padded: u32,
  nghttp2_flag_priority: u32,
  default_settings_header_table_size: u32,
  default_settings_enable_push: u32,
  default_settings_max_concurrent_streams: u32,
  default_settings_initial_window_size: u32,
  default_settings_max_frame_size: u32,
  default_settings_max_header_list_size: u32,
  default_settings_enable_connect_protocol: u32,
  max_max_frame_size: u32,
  min_max_frame_size: u32,
  max_initial_window_size: u32,
  nghttp2_settings_header_table_size: u32,
  nghttp2_settings_enable_push: u32,
  nghttp2_settings_max_concurrent_streams: u32,
  nghttp2_settings_initial_window_size: u32,
  nghttp2_settings_max_frame_size: u32,
  nghttp2_settings_max_header_list_size: u32,
  nghttp2_settings_enable_connect_protocol: u32,
  padding_strategy_none: u32,
  padding_strategy_aligned: u32,
  padding_strategy_max: u32,
  padding_strategy_callback: u32,

  nghttp2_no_error: u32,
  nghttp2_protocol_error: u32,
  nghttp2_internal_error: u32,
  nghttp2_flow_control_error: u32,
  nghttp2_settings_timeout: u32,
  nghttp2_stream_closed: u32,
  nghttp2_frame_size_error: u32,
  nghttp2_refused_stream: u32,
  nghttp2_cancel: u32,
  nghttp2_compression_error: u32,
  nghttp2_connect_error: u32,
  nghttp2_enhance_your_calm: u32,
  nghttp2_inadequate_security: u32,
  nghttp2_http_1_1_required: u32,

  header_table_size: u32,
  enable_push: u32,
  max_concurrent_streams: u32,
  initial_window_size: u32,
  max_frame_size: u32,
  max_header_list_size: u32,
  enable_connect_protocol: u32,
}

#[op2]
#[serde]
pub fn op_http2_constants() -> Http2Constants {
  Http2Constants {
    nghttp2_hcat_request: ffi::NGHTTP2_HCAT_REQUEST,
    nghttp2_hcat_response: ffi::NGHTTP2_HCAT_RESPONSE,
    nghttp2_hcat_push_response: ffi::NGHTTP2_HCAT_PUSH_RESPONSE,
    nghttp2_hcat_headers: ffi::NGHTTP2_HCAT_HEADERS,
    nghttp2_nv_flag_none: ffi::NGHTTP2_NV_FLAG_NONE,
    nghttp2_nv_flag_no_index: ffi::NGHTTP2_NV_FLAG_NO_INDEX,
    nghttp2_err_deferred: ffi::NGHTTP2_ERR_DEFERRED,
    nghttp2_err_stream_id_not_available:
      ffi::NGHTTP2_ERR_STREAM_ID_NOT_AVAILABLE,
    nghttp2_err_invalid_argument: ffi::NGHTTP2_ERR_INVALID_ARGUMENT,
    nghttp2_err_stream_closed: ffi::NGHTTP2_ERR_STREAM_CLOSED,
    nghttp2_err_nomem: ffi::NGHTTP2_ERR_NOMEM,
    stream_option_empty_payload: STREAM_OPTION_EMPTY_PAYLOAD,
    stream_option_get_trailers: STREAM_OPTION_GET_TRAILERS,

    http2_header_status: ":status",
    http2_header_method: ":method",
    http2_header_authority: ":authority",
    http2_header_scheme: ":scheme",
    http2_header_path: ":path",
    http2_header_protocol: ":protocol",
    http2_header_access_control_allow_credentials: "access-control_allow_credentials",
    http2_header_access_control_max_age: "access-control-max-age",
    http2_header_access_control_request_method: "access-control-request-method",
    http2_header_age: "age",
    http2_header_authorization: "authorization",
    http2_header_content_encoding: "content-encoding",
    http2_header_content_language: "content-language",
    http2_header_content_length: "content-length",
    http2_header_content_location: "content-location",
    http2_header_content_md5: "content-md5",
    http2_header_content_range: "content-range",
    http2_header_content_type: "content-type",
    http2_header_cookie: "cookie",
    http2_header_date: "date",
    http2_header_dnt: "dnt",
    http2_header_etag: "etag",
    http2_header_expires: "expires",
    http2_header_from: "from",
    http2_header_host: "host",
    http2_header_if_match: "if-match",
    http2_header_if_none_match: "if-none-match",
    http2_header_if_modified_since: "if-modified-since",
    http2_header_if_range: "if-range",
    http2_header_if_unmodified_since: "if-unmodified-since",
    http2_header_last_modified: "last-modified",
    http2_header_location: "location",
    http2_header_max_forwards: "max-forwards",
    http2_header_proxy_authorization: "proxy-authorization",
    http2_header_range: "range",
    http2_header_referer: "referer",
    http2_header_retry_after: "retry-after",
    http2_header_set_cookie: "set-cookie",
    http2_header_tk: "tk",
    http2_header_upgrade_insecure_requests: "upgrade-insecure-requests",
    http2_header_user_agent: "agent",
    http2_header_x_content_type_options: "x-content-type-options",

    http2_header_connection: "connection",
    http2_header_upgrade: "upgrade",
    http2_header_http2_settings: "http2-settings",
    http2_header_te: "te",
    http2_header_transfer_encoding: "transfer-encoding",
    http2_header_keep_alive: "keep-alive",
    http2_header_proxy_connection: "proxy-connection",

    http2_method_connect: "CONNECT",
    http2_method_delete: "DELETE",
    http2_method_get: "GET",
    http2_method_head: "HEAD",

    http_status_ok: 200,

    nghttp2_err_frame_size_error: ffi::NGHTTP2_ERR_FRAME_SIZE_ERROR as u32,
    nghttp2_session_server: 0,
    nghttp2_session_client: 1,
    nghttp2_stream_state_idle: ffi::NGHTTP2_STREAM_STATE_IDLE as u32,
    nghttp2_stream_state_open: ffi::NGHTTP2_STREAM_STATE_OPEN as u32,
    nghttp2_stream_state_reserved_local:
      ffi::NGHTTP2_STREAM_STATE_RESERVED_LOCAL as u32,
    nghttp2_stream_state_reserved_remote:
      ffi::NGHTTP2_STREAM_STATE_RESERVED_REMOTE as u32,
    nghttp2_stream_state_half_closed_local:
      ffi::NGHTTP2_STREAM_STATE_HALF_CLOSED_LOCAL as u32,
    nghttp2_stream_state_half_closed_remote:
      ffi::NGHTTP2_STREAM_STATE_HALF_CLOSED_REMOTE as u32,
    nghttp2_stream_state_closed: ffi::NGHTTP2_STREAM_STATE_CLOSED as u32,
    nghttp2_flag_none: ffi::NGHTTP2_FLAG_NONE,
    nghttp2_flag_end_stream: ffi::NGHTTP2_FLAG_END_STREAM,
    nghttp2_flag_end_headers: ffi::NGHTTP2_FLAG_END_HEADERS,
    nghttp2_flag_ack: ffi::NGHTTP2_FLAG_ACK,
    nghttp2_flag_padded: ffi::NGHTTP2_FLAG_PADDED,
    nghttp2_flag_priority: ffi::NGHTTP2_FLAG_PRIORITY,
    default_settings_header_table_size: 4096,
    default_settings_enable_push: 1,
    default_settings_max_concurrent_streams: 0xffffffff,
    default_settings_initial_window_size: 65535,
    default_settings_max_frame_size: 16384,
    default_settings_max_header_list_size: 0xffffffff,
    default_settings_enable_connect_protocol: 0,
    max_max_frame_size: 16777215,
    min_max_frame_size: 16384,
    max_initial_window_size: 2147483647,
    nghttp2_settings_header_table_size: ffi::NGHTTP2_SETTINGS_HEADER_TABLE_SIZE
      as u32,
    nghttp2_settings_enable_push: ffi::NGHTTP2_SETTINGS_ENABLE_PUSH as u32,
    nghttp2_settings_max_concurrent_streams:
      ffi::NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS as u32,
    nghttp2_settings_initial_window_size:
      ffi::NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE as u32,
    nghttp2_settings_max_frame_size: ffi::NGHTTP2_SETTINGS_MAX_FRAME_SIZE
      as u32,
    nghttp2_settings_max_header_list_size:
      ffi::NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE as u32,
    nghttp2_settings_enable_connect_protocol: 8,
    padding_strategy_none: 0,
    padding_strategy_aligned: 1,
    padding_strategy_max: 2,
    padding_strategy_callback: 3,

    nghttp2_no_error: ffi::NGHTTP2_NO_ERROR,
    nghttp2_protocol_error: ffi::NGHTTP2_PROTOCOL_ERROR,
    nghttp2_internal_error: ffi::NGHTTP2_INTERNAL_ERROR,
    nghttp2_flow_control_error: ffi::NGHTTP2_FLOW_CONTROL_ERROR,
    nghttp2_settings_timeout: ffi::NGHTTP2_SETTINGS_TIMEOUT,
    nghttp2_stream_closed: ffi::NGHTTP2_STREAM_CLOSED,
    nghttp2_frame_size_error: ffi::NGHTTP2_FRAME_SIZE_ERROR,
    nghttp2_refused_stream: ffi::NGHTTP2_REFUSED_STREAM,
    nghttp2_cancel: ffi::NGHTTP2_CANCEL,
    nghttp2_compression_error: ffi::NGHTTP2_COMPRESSION_ERROR,
    nghttp2_connect_error: ffi::NGHTTP2_CONNECT_ERROR,
    nghttp2_enhance_your_calm: ffi::NGHTTP2_ENHANCE_YOUR_CALM,
    nghttp2_inadequate_security: ffi::NGHTTP2_INADEQUATE_SECURITY,
    nghttp2_http_1_1_required: ffi::NGHTTP2_HTTP_1_1_REQUIRED,

    header_table_size: ffi::NGHTTP2_SETTINGS_HEADER_TABLE_SIZE as u32,
    enable_push: ffi::NGHTTP2_SETTINGS_ENABLE_PUSH as u32,
    max_concurrent_streams: ffi::NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS as u32,
    initial_window_size: ffi::NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE as u32,
    max_frame_size: ffi::NGHTTP2_SETTINGS_MAX_FRAME_SIZE as u32,
    max_header_list_size: ffi::NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE as u32,
    enable_connect_protocol: 8,
  }
}
