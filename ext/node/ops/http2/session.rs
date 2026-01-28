// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use deno_core::AsyncResult;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::serde_v8;
use deno_core::v8;
use deno_net::io::TcpStreamResource;
use deno_net::ops_tls::TlsStreamResource;
use libnghttp2 as ffi;
use serde::Serialize;

use super::stream::Http2Headers;
use super::stream::Http2Priority;
use super::stream::Http2Stream;
use super::types::*;

// Thread-local state buffers

const SESSION_STATE_LEN: usize = SessionStateIndex::Count as usize;
const STREAM_STATE_LEN: usize = StreamStateIndex::Count as usize;
const OPTIONS_LEN: usize = OptionsIndex::Flags as usize + 1;
const SETTINGS_LEN: usize =
  SettingsIndex::Count as usize + 1 + 1 + (2 * MAX_ADDITIONAL_SETTINGS);

thread_local! {
  static SESSION_STATE: UnsafeCell<[f32; SESSION_STATE_LEN]> =
    const { UnsafeCell::new([0.0; SESSION_STATE_LEN]) };

  static STREAM_STATE: UnsafeCell<[f32; STREAM_STATE_LEN]> =
    const { UnsafeCell::new([0.0; STREAM_STATE_LEN]) };

  static OPTIONS: UnsafeCell<[u32; OPTIONS_LEN]> =
    const { UnsafeCell::new([0; OPTIONS_LEN]) };

  static SETTINGS: UnsafeCell<[u32; SETTINGS_LEN]> =
    const { UnsafeCell::new([0; SETTINGS_LEN]) };
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JSHttp2State<'a> {
  session_state: serde_v8::Value<'a>,
  stream_state: serde_v8::Value<'a>,
  options_buffer: serde_v8::Value<'a>,
  settings_buffer: serde_v8::Value<'a>,
}

impl<'a> JSHttp2State<'a> {
  pub fn create(scope: &mut v8::PinScope<'a, 'a>) -> Self {
    let session_state = SESSION_STATE.with(|cell| {
      let ptr = unsafe { (*cell.get()).as_mut_ptr() };
      create_f32_array(scope, ptr, SESSION_STATE_LEN)
    });

    let stream_state = STREAM_STATE.with(|cell| {
      let ptr = unsafe { (*cell.get()).as_mut_ptr() };
      create_f32_array(scope, ptr, STREAM_STATE_LEN)
    });

    let options_buffer = OPTIONS.with(|cell| {
      let ptr = unsafe { (*cell.get()).as_mut_ptr() };
      create_u32_array(scope, ptr, OPTIONS_LEN)
    });

    let settings_buffer = SETTINGS.with(|cell| {
      let ptr = unsafe { (*cell.get()).as_mut_ptr() };
      create_u32_array(scope, ptr, SETTINGS_LEN)
    });

    Self {
      session_state,
      stream_state,
      options_buffer,
      settings_buffer,
    }
  }
}

fn create_f32_array<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  buffer: *mut f32,
  len: usize,
) -> serde_v8::Value<'a> {
  unsafe {
    let bs = v8::ArrayBuffer::new_backing_store_from_ptr(
      buffer as *mut c_void,
      len * std::mem::size_of::<f32>(),
      nop_deleter,
      std::ptr::null_mut(),
    );
    let ab = v8::ArrayBuffer::with_backing_store(scope, &bs.make_shared());
    v8::Float32Array::new(scope, ab, 0, len).unwrap().into()
  }
}

fn create_u32_array<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  buffer: *mut u32,
  len: usize,
) -> serde_v8::Value<'a> {
  unsafe {
    let bs = v8::ArrayBuffer::new_backing_store_from_ptr(
      buffer as *mut c_void,
      len * std::mem::size_of::<u32>(),
      nop_deleter,
      std::ptr::null_mut(),
    );
    let ab = v8::ArrayBuffer::with_backing_store(scope, &bs.make_shared());
    v8::Uint32Array::new(scope, ab, 0, len).unwrap().into()
  }
}

unsafe extern "C" fn nop_deleter(
  _data: *mut c_void,
  _byte_length: usize,
  _deleter_data: *mut c_void,
) {
}

fn with_settings<F, R>(f: F) -> R
where
  F: FnOnce(&mut [u32; SETTINGS_LEN]) -> R,
{
  SETTINGS.with(|cell| {
    let buffer = unsafe { &mut *cell.get() };
    f(buffer)
  })
}

fn with_options<F, R>(f: F) -> R
where
  F: FnOnce(&[u32; OPTIONS_LEN]) -> R,
{
  OPTIONS.with(|cell| {
    let buffer = unsafe { &*cell.get() };
    f(buffer)
  })
}

// Http2Options

const OPTIONS_FLAG_NO_AUTO_WINDOW_UPDATE: u32 = 0x1;
const OPTIONS_FLAG_NO_RECV_CLIENT_MAGIC: u32 = 0x2;
const OPTIONS_FLAG_NO_HTTP_MESSAGING: u32 = 0x4;

struct Http2Options {
  options: *mut ffi::nghttp2_option,
  padding_strategy: PaddingStrategy,
}

impl Http2Options {
  fn new(session_type: SessionType) -> Self {
    let mut options: *mut ffi::nghttp2_option = std::ptr::null_mut();
    unsafe { ffi::nghttp2_option_new(&mut options) };

    let padding_strategy = with_options(|buffer| {
      let flags = buffer[OptionsIndex::Flags as usize];

      unsafe {
        ffi::nghttp2_option_set_no_closed_streams(options, 1);

        if flags & OPTIONS_FLAG_NO_AUTO_WINDOW_UPDATE != 0 {
          ffi::nghttp2_option_set_no_auto_window_update(options, 1);
        }

        if flags & OPTIONS_FLAG_NO_RECV_CLIENT_MAGIC != 0 {
          ffi::nghttp2_option_set_no_recv_client_magic(options, 1);
        }

        if flags & OPTIONS_FLAG_NO_HTTP_MESSAGING != 0 {
          ffi::nghttp2_option_set_no_http_messaging(options, 1);
        }

        let max_deflate =
          buffer[OptionsIndex::MaxDeflateDynamicTableSize as usize];
        if max_deflate > 0 {
          ffi::nghttp2_option_set_max_deflate_dynamic_table_size(
            options,
            max_deflate as usize,
          );
        }

        let max_reserved =
          buffer[OptionsIndex::MaxReservedRemoteStreams as usize];
        if max_reserved > 0 {
          ffi::nghttp2_option_set_max_reserved_remote_streams(
            options,
            max_reserved,
          );
        }

        let max_send_header =
          buffer[OptionsIndex::MaxSendHeaderBlockLength as usize];
        if max_send_header > 0 {
          ffi::nghttp2_option_set_max_send_header_block_length(
            options,
            max_send_header as usize,
          );
        }

        let peer_max_concurrent =
          buffer[OptionsIndex::PeerMaxConcurrentStreams as usize];
        if peer_max_concurrent > 0 {
          ffi::nghttp2_option_set_peer_max_concurrent_streams(
            options,
            peer_max_concurrent,
          );
        } else {
          ffi::nghttp2_option_set_peer_max_concurrent_streams(options, 100);
        }

        let max_outstanding_pings =
          buffer[OptionsIndex::MaxOutstandingPings as usize];
        if max_outstanding_pings > 0 {
          ffi::nghttp2_option_set_max_outbound_ack(
            options,
            max_outstanding_pings as usize,
          );
        }

        let max_outstanding_settings =
          buffer[OptionsIndex::MaxOutstandingSettings as usize];
        if max_outstanding_settings > 0 {
          ffi::nghttp2_option_set_max_settings(
            options,
            max_outstanding_settings as usize,
          );
        }

        if matches!(session_type, SessionType::Client) {
          ffi::nghttp2_option_set_builtin_recv_extension_type(
            options,
            ffi::NGHTTP2_ALTSVC as u8,
          );
          ffi::nghttp2_option_set_builtin_recv_extension_type(
            options,
            ffi::NGHTTP2_ORIGIN as u8,
          );
        }
      }

      let padding = buffer[OptionsIndex::PaddingStrategy as usize];
      match padding {
        1 => PaddingStrategy::Aligned,
        2 => PaddingStrategy::Max,
        3 => PaddingStrategy::Callback,
        _ => PaddingStrategy::None,
      }
    });

    Self {
      options,
      padding_strategy,
    }
  }

  fn ptr(&self) -> *mut ffi::nghttp2_option {
    self.options
  }

  fn padding_strategy(&self) -> PaddingStrategy {
    self.padding_strategy
  }
}

impl Drop for Http2Options {
  fn drop(&mut self) {
    if !self.options.is_null() {
      unsafe { ffi::nghttp2_option_del(self.options) };
    }
  }
}

// Http2Settings

const SETTINGS_ENTRY_COUNT: usize =
  SettingsIndex::Count as usize + MAX_ADDITIONAL_SETTINGS;

struct Http2Settings {
  entries: [ffi::nghttp2_settings_entry; SETTINGS_ENTRY_COUNT],
  count: usize,
  session: *mut Session,
}

impl Http2Settings {
  fn init(session: *mut Session) -> Self {
    with_settings(|buffer| {
      let flags = buffer[SettingsIndex::Count as usize];
      let mut count: usize = 0;
      let mut entries = [ffi::nghttp2_settings_entry {
        settings_id: 0,
        value: 0,
      }; SETTINGS_ENTRY_COUNT];

      macro_rules! grab_setting {
        ($index:expr, $nghttp2_id:expr) => {
          if flags & (1 << $index as u8) != 0 {
            let val = buffer[$index as usize];
            if count < entries.len() {
              entries[count] = ffi::nghttp2_settings_entry {
                settings_id: $nghttp2_id as _,
                value: val,
              };
            }
            count += 1;
          }
        };
      }

      grab_setting!(
        SettingsIndex::HeaderTableSize,
        ffi::NGHTTP2_SETTINGS_HEADER_TABLE_SIZE
      );
      grab_setting!(
        SettingsIndex::EnablePush,
        ffi::NGHTTP2_SETTINGS_ENABLE_PUSH
      );
      grab_setting!(
        SettingsIndex::InitialWindowSize,
        ffi::NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE
      );
      grab_setting!(
        SettingsIndex::MaxFrameSize,
        ffi::NGHTTP2_SETTINGS_MAX_FRAME_SIZE
      );
      grab_setting!(
        SettingsIndex::MaxConcurrentStreams,
        ffi::NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS
      );
      grab_setting!(
        SettingsIndex::MaxHeaderListSize,
        ffi::NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE
      );

      let num_add_settings = buffer[SettingsIndex::Count as usize + 1] as usize;

      if num_add_settings > 0 {
        let offset = SettingsIndex::Count as usize + 2;
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
    })
  }

  fn send(&self) {
    unsafe {
      let session = &*self.session;
      ffi::nghttp2_submit_settings(
        session.session,
        ffi::NGHTTP2_FLAG_NONE as _,
        self.entries.as_ptr(),
        self.count,
      );
    }
  }
}

// Driver

pub enum NetworkStream {
  Tcp(Rc<TcpStreamResource>),
  Tls(Rc<TlsStreamResource>),
}

pub struct Http2SessionDriver {
  pub stream: NetworkStream,
  pub session: *mut Session,
}

impl Http2SessionDriver {
  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    let nread = match &self.stream {
      NetworkStream::Tcp(stream) => stream.clone().read(data).await?,
      NetworkStream::Tls(stream) => stream.clone().read(data).await?,
    };

    let session = unsafe { &*self.session };

    unsafe {
      ffi::nghttp2_session_mem_recv(
        session.session,
        data.as_mut_ptr() as _,
        nread,
      );
    }

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
      let src_len =
        unsafe { ffi::nghttp2_session_mem_send(session.session, &mut src) };

      if src_len > 0 {
        let data = unsafe { std::slice::from_raw_parts(src, src_len as usize) };
        match &self.stream {
          NetworkStream::Tcp(stream) => stream.clone().write(data).await?,
          NetworkStream::Tls(stream) => stream.clone().write(data).await?,
        };
      } else {
        break;
      }
    }

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

// Callbacks

fn frame_id(frame: *const ffi::nghttp2_frame) -> i32 {
  unsafe {
    let frame = &*frame;
    if frame.hd.type_ as u32 == ffi::NGHTTP2_PUSH_PROMISE {
      frame.push_promise.promised_stream_id
    } else {
      frame.hd.stream_id
    }
  }
}

fn frame_type(frame: *const ffi::nghttp2_frame) -> u8 {
  unsafe { (*frame).hd.type_ }
}

fn frame_flags(frame: *const ffi::nghttp2_frame) -> u8 {
  unsafe { (*frame).hd.flags }
}

fn frame_headers_category(
  frame: *const ffi::nghttp2_frame,
) -> ffi::nghttp2_headers_category {
  unsafe { (*frame).headers.cat }
}

fn rcbuf_to_slice(rcbuf: *mut ffi::nghttp2_rcbuf) -> &'static [u8] {
  unsafe {
    let buf = ffi::nghttp2_rcbuf_get_buf(rcbuf);
    std::slice::from_raw_parts(buf.base, buf.len)
  }
}

fn frame_header_length(frame: *const ffi::nghttp2_frame) -> usize {
  unsafe { (*frame).hd.length }
}

unsafe extern "C" fn on_begin_headers_callbacks(
  ng_session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  data: *mut c_void,
) -> i32 {
  let session = unsafe { Session::from_user_data(data) };
  let id = frame_id(frame);
  let cat = frame_headers_category(frame);

  match session.find_stream(id) {
    None => {
      if session.is_graceful_closing() {
        unsafe {
          ffi::nghttp2_submit_rst_stream(
            ng_session,
            ffi::NGHTTP2_FLAG_NONE as u8,
            id,
            ffi::NGHTTP2_REFUSED_STREAM,
          );
        }
        return ffi::NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE;
      }
      let (obj, stream) = Http2Stream::new(session, id, cat);
      stream.start_headers(cat);
      session.streams.insert(id, (obj, stream));
    }
    Some(s) => {
      s.start_headers(cat);
    }
  }

  0
}

unsafe extern "C" fn on_header_callback(
  _session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  name: *mut ffi::nghttp2_rcbuf,
  value: *mut ffi::nghttp2_rcbuf,
  flags: u8,
  data: *mut c_void,
) -> i32 {
  let session = unsafe { Session::from_user_data(data) };
  let id = frame_id(frame);

  if let Some(stream) = session.find_stream(id) {
    let name_slice = rcbuf_to_slice(name);
    let value_slice = rcbuf_to_slice(value);

    if !stream.add_header(name_slice, value_slice, flags) {
      unsafe {
        ffi::nghttp2_submit_rst_stream(
          session.session,
          ffi::NGHTTP2_FLAG_NONE as u8,
          id,
          ffi::NGHTTP2_ENHANCE_YOUR_CALM,
        );
      }
      return ffi::NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE;
    }
  }

  0
}

unsafe extern "C" fn on_frame_recv_callback(
  _session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  data: *mut c_void,
) -> i32 {
  let session = unsafe { Session::from_user_data(data) };

  match frame_type(frame) as u32 {
    ffi::NGHTTP2_DATA => {}
    ffi::NGHTTP2_PUSH_PROMISE | ffi::NGHTTP2_HEADERS => {
      handle_headers_frame(session, frame);
    }
    ffi::NGHTTP2_SETTINGS => {}
    ffi::NGHTTP2_PRIORITY => {
      handle_priority_frame(session, frame);
    }
    ffi::NGHTTP2_GOAWAY => {
      handle_goaway_frame(session, frame);
    }
    ffi::NGHTTP2_PING => {
      handle_ping_frame(session);
    }
    ffi::NGHTTP2_ALTSVC => {
      handle_alt_svc_frame(session, frame);
    }
    ffi::NGHTTP2_ORIGIN => {
      handle_origin_frame(session, frame);
    }
    _ => {}
  }

  0
}

fn handle_headers_frame(session: &Session, frame: *const ffi::nghttp2_frame) {
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
  v8::scope!(let scope, &mut isolate);
  let context = v8::Local::new(scope, session.context.clone());
  let scope = &mut v8::ContextScope::new(scope, context);

  let id = frame_id(frame);
  let Some(stream_ref) = session.find_stream(id) else {
    return;
  };

  let headers = stream_ref.current_headers.borrow();
  if headers.is_empty() {
    return;
  }

  let headers_array = v8::Array::new(scope, (headers.len() * 2) as i32);
  for (i, (name, value, _flags)) in headers.iter().enumerate() {
    let name_str = v8::String::new(scope, name).unwrap();
    let value_str = v8::String::new(scope, value).unwrap();
    headers_array.set_index(scope, (i * 2) as u32, name_str.into());
    headers_array.set_index(scope, (i * 2 + 1) as u32, value_str.into());
  }

  drop(headers);
  stream_ref.clear_headers();

  let stream_obj = session.find_stream_obj(id).unwrap();
  let state = session.op_state.borrow();
  let callbacks = state.borrow::<SessionCallbacks>();
  let recv = v8::Local::new(scope, &session.this);
  let callback = v8::Local::new(scope, &callbacks.headers_frame_cb);
  drop(state);

  let handle = v8::Local::new(scope, stream_obj);
  let id_num = v8::Number::new(scope, id.into());
  let cat = v8::null(scope);
  let flags = v8::Number::new(scope, frame_flags(frame).into());

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

fn handle_ping_frame(session: &Session) {
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
  v8::scope!(let scope, &mut isolate);
  let context = v8::Local::new(scope, session.context.clone());
  let scope = &mut v8::ContextScope::new(scope, context);

  let state = session.op_state.borrow();
  let callbacks = state.borrow::<SessionCallbacks>();
  let recv = v8::Local::new(scope, &session.this);
  let callback = v8::Local::new(scope, &callbacks.ping_frame_cb);
  drop(state);

  let arg = v8::null(scope);
  callback.call(scope, recv.into(), &[arg.into()]);
}

fn handle_goaway_frame(session: &Session, frame: *const ffi::nghttp2_frame) {
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
  v8::scope!(let scope, &mut isolate);
  let context = v8::Local::new(scope, session.context.clone());
  let scope = &mut v8::ContextScope::new(scope, context);

  let goaway_frame = unsafe { (*frame).goaway };

  let error_code = v8::Number::new(scope, goaway_frame.error_code.into());
  let last_stream_id =
    v8::Number::new(scope, goaway_frame.last_stream_id.into());

  let opaque_data: v8::Local<v8::Value> = if goaway_frame.opaque_data_len > 0 {
    let data_slice = unsafe {
      std::slice::from_raw_parts(
        goaway_frame.opaque_data,
        goaway_frame.opaque_data_len,
      )
    };
    let array_buffer = v8::ArrayBuffer::new(scope, data_slice.len());
    let backing_store = array_buffer.get_backing_store();
    if let Some(backing_data) = backing_store.data() {
      unsafe {
        std::ptr::copy_nonoverlapping(
          data_slice.as_ptr(),
          backing_data.as_ptr() as *mut u8,
          data_slice.len(),
        );
      }
    }
    v8::Uint8Array::new(scope, array_buffer, 0, data_slice.len())
      .unwrap()
      .into()
  } else {
    v8::undefined(scope).into()
  };

  let state = session.op_state.borrow();
  let callbacks = state.borrow::<SessionCallbacks>();
  let recv = v8::Local::new(scope, &session.this);
  let callback = v8::Local::new(scope, &callbacks.goaway_data_cb);
  drop(state);

  callback.call(
    scope,
    recv.into(),
    &[error_code.into(), last_stream_id.into(), opaque_data],
  );
}

fn handle_priority_frame(session: &Session, frame: *const ffi::nghttp2_frame) {
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
  v8::scope!(let scope, &mut isolate);
  let context = v8::Local::new(scope, session.context.clone());
  let scope = &mut v8::ContextScope::new(scope, context);

  let priority_frame = unsafe { (*frame).priority };
  let id = frame_id(frame);
  let spec = priority_frame.pri_spec;

  let stream_id = v8::Number::new(scope, id.into());
  let parent_stream_id = v8::Number::new(scope, spec.stream_id.into());
  let weight = v8::Number::new(scope, spec.weight.into());
  let exclusive = v8::Boolean::new(scope, spec.exclusive != 0);

  let state = session.op_state.borrow();
  let callbacks = state.borrow::<SessionCallbacks>();
  let recv = v8::Local::new(scope, &session.this);
  let callback = v8::Local::new(scope, &callbacks.priority_frame_cb);
  drop(state);

  callback.call(
    scope,
    recv.into(),
    &[
      stream_id.into(),
      parent_stream_id.into(),
      weight.into(),
      exclusive.into(),
    ],
  );
}

fn handle_alt_svc_frame(session: &Session, frame: *const ffi::nghttp2_frame) {
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
  v8::scope!(let scope, &mut isolate);
  let context = v8::Local::new(scope, session.context.clone());
  let scope = &mut v8::ContextScope::new(scope, context);

  let id = frame_id(frame);

  let ext = unsafe { (*frame).ext };
  let altsvc = ext.payload as *const ffi::nghttp2_ext_altsvc;

  let origin_slice = unsafe {
    std::slice::from_raw_parts((*altsvc).origin, (*altsvc).origin_len)
  };
  let field_value_slice = unsafe {
    std::slice::from_raw_parts((*altsvc).field_value, (*altsvc).field_value_len)
  };

  let origin_str = std::str::from_utf8(origin_slice)
    .map(|s| v8::String::new(scope, s).unwrap())
    .unwrap_or_else(|_| v8::String::new(scope, "").unwrap());
  let field_value_str = std::str::from_utf8(field_value_slice)
    .map(|s| v8::String::new(scope, s).unwrap())
    .unwrap_or_else(|_| v8::String::new(scope, "").unwrap());

  let state = session.op_state.borrow();
  let callbacks = state.borrow::<SessionCallbacks>();
  let recv = v8::Local::new(scope, &session.this);
  let callback = v8::Local::new(scope, &callbacks.alt_svc_cb);
  drop(state);

  let stream_id = v8::Number::new(scope, id.into());
  callback.call(
    scope,
    recv.into(),
    &[stream_id.into(), origin_str.into(), field_value_str.into()],
  );
}

fn handle_origin_frame(session: &Session, frame: *const ffi::nghttp2_frame) {
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
  v8::scope!(let scope, &mut isolate);
  let context = v8::Local::new(scope, session.context.clone());
  let scope = &mut v8::ContextScope::new(scope, context);

  let ext = unsafe { (*frame).ext };
  let origin = ext.payload as *const ffi::nghttp2_ext_origin;

  let nov = unsafe { (*origin).nov };
  let origins_ptr = unsafe { (*origin).ov };

  if nov == 0 {
    return;
  }

  let origins_array = v8::Array::new(scope, nov as i32);
  for i in 0..nov {
    let entry = unsafe { *origins_ptr.add(i) };
    let origin_slice =
      unsafe { std::slice::from_raw_parts(entry.origin, entry.origin_len) };
    if let Ok(origin_str) = std::str::from_utf8(origin_slice) {
      let js_string = v8::String::new(scope, origin_str).unwrap();
      origins_array.set_index(scope, i as u32, js_string.into());
    }
  }

  let state = session.op_state.borrow();
  let callbacks = state.borrow::<SessionCallbacks>();
  let recv = v8::Local::new(scope, &session.this);
  let callback = v8::Local::new(scope, &callbacks.origin_frame_cb);
  drop(state);

  callback.call(scope, recv.into(), &[origins_array.into()]);
}

unsafe extern "C" fn on_stream_close_callback(
  _session: *mut ffi::nghttp2_session,
  stream_id: i32,
  error_code: u32,
  data: *mut c_void,
) -> i32 {
  let session = unsafe { Session::from_user_data(data) };

  let Some(stream_obj) = session.find_stream_obj(stream_id).cloned() else {
    return 0;
  };

  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(session.isolate) };
  v8::scope!(let scope, &mut isolate);
  let context = v8::Local::new(scope, session.context.clone());
  let scope = &mut v8::ContextScope::new(scope, context);

  let state = session.op_state.borrow();
  let callbacks = state.borrow::<SessionCallbacks>();
  let callback = v8::Local::new(scope, &callbacks.stream_close_cb);
  drop(state);

  let recv = v8::Local::new(scope, stream_obj);
  let code = v8::Integer::new_from_unsigned(scope, error_code);

  let result = callback.call(scope, recv.into(), &[code.into()]);

  if result.is_none() || result.map(|v| v.is_false()).unwrap_or(false) {
    session.streams.remove(&stream_id);
  }

  0
}

unsafe extern "C" fn on_data_chunk_recv_callback(
  session: *mut ffi::nghttp2_session,
  _flags: u8,
  _stream_id: i32,
  _data: *const u8,
  len: usize,
  _user_data: *mut c_void,
) -> i32 {
  if len == 0 {
    return 0;
  }
  unsafe { ffi::nghttp2_session_consume_connection(session, len) };
  0
}

pub unsafe extern "C" fn on_stream_read_callback(
  _session: *mut ffi::nghttp2_session,
  stream_id: i32,
  buf: *mut u8,
  length: usize,
  data_flags: *mut u32,
  _source: *mut ffi::nghttp2_data_source,
  user_data: *mut c_void,
) -> isize {
  let session = unsafe { Session::from_user_data(user_data) };

  if let Some(stream) = session.find_stream(stream_id) {
    let mut pending_data = stream.pending_data.borrow_mut();

    if !pending_data.is_empty() {
      let amount = std::cmp::min(pending_data.len(), length);
      if amount > 0 {
        let data_slice = pending_data.split_to(amount);
        unsafe {
          std::ptr::copy_nonoverlapping(data_slice.as_ptr(), buf, amount)
        };
        *stream.available_outbound_length.borrow_mut() -= amount;

        if pending_data.is_empty() {
          unsafe { *data_flags |= ffi::NGHTTP2_DATA_FLAG_EOF };
          if stream.has_trailers() {
            unsafe { *data_flags |= ffi::NGHTTP2_DATA_FLAG_NO_END_STREAM };
            stream.on_trailers();
          }
        }
        return amount as isize;
      }
    }

    if pending_data.is_empty() {
      unsafe { *data_flags |= ffi::NGHTTP2_DATA_FLAG_EOF };
      if stream.has_trailers() {
        unsafe { *data_flags |= ffi::NGHTTP2_DATA_FLAG_NO_END_STREAM };
        stream.on_trailers();
      }
      return 0;
    }
  }

  ffi::NGHTTP2_ERR_DEFERRED as _
}

unsafe extern "C" fn on_select_padding(
  _session: *mut ffi::nghttp2_session,
  frame: *const ffi::nghttp2_frame,
  max_payload_len: usize,
  user_data: *mut c_void,
) -> isize {
  let session = unsafe { Session::from_user_data(user_data) };
  let padding = frame_header_length(frame);

  let result = match session.padding_strategy {
    PaddingStrategy::None => padding,
    PaddingStrategy::Max => {
      session.on_max_frame_size_padding(padding, max_payload_len)
    }
    PaddingStrategy::Aligned | PaddingStrategy::Callback => {
      session.on_dword_aligned_padding(padding, max_payload_len)
    }
  };

  result as isize
}

unsafe extern "C" fn on_frame_not_send_callback(
  _session: *mut ffi::nghttp2_session,
  _frame: *const ffi::nghttp2_frame,
  _lib_error_code: i32,
  _data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_invalid_header_callback(
  _session: *mut ffi::nghttp2_session,
  _frame: *const ffi::nghttp2_frame,
  _name: *mut ffi::nghttp2_rcbuf,
  _value: *mut ffi::nghttp2_rcbuf,
  _flags: u8,
  _data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_nghttp_error_callback(
  _session: *mut ffi::nghttp2_session,
  _lib_error_code: i32,
  _msg: *const std::ffi::c_char,
  _len: usize,
  _data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_send_data_callback(
  _session: *mut ffi::nghttp2_session,
  _frame: *mut ffi::nghttp2_frame,
  _framehd: *const u8,
  _length: usize,
  _source: *mut ffi::nghttp2_data_source,
  _data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_invalid_frame_recv_callback(
  _session: *mut ffi::nghttp2_session,
  _frame: *const ffi::nghttp2_frame,
  _lib_error_code: i32,
  _data: *mut c_void,
) -> i32 {
  0
}

unsafe extern "C" fn on_frame_send_callback(
  _session: *mut ffi::nghttp2_session,
  _frame: *const ffi::nghttp2_frame,
  _data: *mut c_void,
) -> i32 {
  0
}

fn create_callbacks() -> *mut ffi::nghttp2_session_callbacks {
  let mut callbacks: *mut ffi::nghttp2_session_callbacks = std::ptr::null_mut();

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
    ffi::nghttp2_session_callbacks_set_select_padding_callback(
      callbacks,
      Some(on_select_padding),
    );
  }

  callbacks
}

// Session

#[allow(dead_code)]
pub struct SessionCallbacks {
  pub session_internal_error_cb: v8::Global<v8::Function>,
  pub priority_frame_cb: v8::Global<v8::Function>,
  pub settings_frame_cb: v8::Global<v8::Function>,
  pub ping_frame_cb: v8::Global<v8::Function>,
  pub headers_frame_cb: v8::Global<v8::Function>,
  pub frame_error_cb: v8::Global<v8::Function>,
  pub goaway_data_cb: v8::Global<v8::Function>,
  pub alt_svc_cb: v8::Global<v8::Function>,
  pub stream_trailers_cb: v8::Global<v8::Function>,
  pub stream_close_cb: v8::Global<v8::Function>,
  pub origin_frame_cb: v8::Global<v8::Function>,
}

#[derive(Debug)]
pub struct NgHttp2StreamWrite {
  pub data: bytes::Bytes,
  #[allow(dead_code)]
  pub stream_id: i32,
}

impl NgHttp2StreamWrite {
  pub fn new(data: bytes::Bytes, stream_id: i32) -> Self {
    Self { data, stream_id }
  }

  pub fn len(&self) -> usize {
    self.data.len()
  }
}

pub struct Session {
  pub session: *mut ffi::nghttp2_session,
  pub streams: HashMap<i32, (v8::Global<v8::Object>, cppgc::Ref<Http2Stream>)>,
  pub outgoing_buffers: Vec<NgHttp2StreamWrite>,
  pub outgoing_length: usize,
  pub isolate: v8::UnsafeRawIsolatePtr,
  pub context: v8::Global<v8::Context>,
  pub op_state: Rc<RefCell<OpState>>,
  pub this: v8::Global<v8::Object>,
  pub padding_strategy: PaddingStrategy,
  pub graceful_close_initiated: bool,
}

impl Session {
  pub fn find_stream(&self, id: i32) -> Option<&cppgc::Ref<Http2Stream>> {
    self.streams.get(&id).map(|v| &v.1)
  }

  pub fn find_stream_obj(&self, id: i32) -> Option<&v8::Global<v8::Object>> {
    self.streams.get(&id).map(|v| &v.0)
  }

  pub fn push_outgoing_buffer(&mut self, write: NgHttp2StreamWrite) {
    self.outgoing_length += write.len();
    self.outgoing_buffers.push(write);
  }

  pub fn clear_outgoing(&mut self) {
    self.outgoing_buffers.clear();
    self.outgoing_length = 0;
  }

  pub fn is_graceful_closing(&self) -> bool {
    self.graceful_close_initiated
  }

  pub fn start_graceful_close(&mut self) {
    self.graceful_close_initiated = true;
  }

  pub fn active_stream_count(&self) -> usize {
    self.streams.len()
  }

  pub unsafe fn from_user_data<'a>(user_data: *mut c_void) -> &'a mut Self {
    unsafe { &mut *(user_data as *mut Session) }
  }

  pub fn on_dword_aligned_padding(
    &self,
    frame_len: usize,
    max_payload_len: usize,
  ) -> usize {
    let r = (frame_len + 9) % 8;
    if r == 0 {
      return frame_len;
    }
    let pad = frame_len + (8 - r);
    std::cmp::min(max_payload_len, pad)
  }

  pub fn on_max_frame_size_padding(
    &self,
    _frame_len: usize,
    max_payload_len: usize,
  ) -> usize {
    max_payload_len
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Http2SessionState {
  pub effective_local_window_size: f64,
  pub effective_recv_data_length: f64,
  pub next_stream_id: f64,
  pub local_window_size: f64,
  pub last_proc_stream_id: f64,
  pub remote_window_size: f64,
  pub outbound_queue_size: f64,
  pub hd_deflate_dynamic_table_size: f64,
  pub hd_inflate_dynamic_table_size: f64,
}

pub struct Http2Session {
  #[allow(dead_code)]
  type_: SessionType,
  session: *mut ffi::nghttp2_session,
  #[allow(dead_code)]
  callbacks: *mut ffi::nghttp2_session_callbacks,
  pub(crate) inner: *mut Session,
}

unsafe impl deno_core::GarbageCollected for Http2Session {
  fn trace(&self, _: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Http2Session"
  }
}

impl Http2Session {
  fn create(
    this: v8::Global<v8::Object>,
    isolate: &v8::Isolate,
    scope: &mut v8::PinScope<'_, '_>,
    op_state: Rc<RefCell<OpState>>,
    session_type: SessionType,
  ) -> Self {
    let mut session: *mut ffi::nghttp2_session = std::ptr::null_mut();
    let options = Http2Options::new(session_type);

    let context = scope.get_current_context();
    let context = v8::Global::new(scope, context);
    let isolate_ptr = unsafe { isolate.as_raw_isolate_ptr() };

    let inner = Box::into_raw(Box::new(Session {
      session,
      streams: HashMap::new(),
      op_state,
      context,
      isolate: isolate_ptr,
      this,
      outgoing_buffers: Vec::with_capacity(32),
      outgoing_length: 0,
      padding_strategy: options.padding_strategy(),
      graceful_close_initiated: false,
    }));

    unsafe {
      let callbacks = create_callbacks();
      match session_type {
        SessionType::Server => ffi::nghttp2_session_server_new3(
          &mut session,
          callbacks,
          inner as *mut _,
          options.ptr(),
          std::ptr::null_mut(),
        ),
        SessionType::Client => ffi::nghttp2_session_client_new3(
          &mut session,
          callbacks,
          inner as *mut _,
          options.ptr(),
          std::ptr::null_mut(),
        ),
      };
      (*inner).session = session;
    }

    Self {
      type_: session_type,
      session,
      callbacks: std::ptr::null_mut(),
      inner,
    }
  }

  fn submit_request(
    &self,
    priority: Http2Priority,
    headers: Http2Headers,
    options: i32,
  ) -> i32 {
    let mut data_provider = ffi::nghttp2_data_provider {
      source: ffi::nghttp2_data_source {
        ptr: std::ptr::null_mut(),
      },
      read_callback: Some(on_stream_read_callback),
    };

    let ret = unsafe {
      ffi::nghttp2_submit_request(
        self.session,
        &priority.spec,
        headers.data(),
        headers.len(),
        &mut data_provider as *mut _,
        std::ptr::null_mut(),
      )
    };

    const NGHTTP2_ERR_NOMEM: i32 = -901;
    assert_ne!(ret, NGHTTP2_ERR_NOMEM);

    if ret > 0 {
      let session = unsafe { &mut *self.inner };
      let (obj, stream) =
        Http2Stream::new(session, ret, ffi::NGHTTP2_HCAT_HEADERS);
      stream.start_headers(ffi::NGHTTP2_HCAT_HEADERS);
      if (options & STREAM_OPTION_GET_TRAILERS) != 0 {
        stream.set_has_trailers(true);
      }
      session.streams.insert(ret, (obj, stream));
    }

    ret
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
    let stream =
      if let Ok(tcp) = state.resource_table.take::<TcpStreamResource>(rid) {
        NetworkStream::Tcp(tcp)
      } else {
        NetworkStream::Tls(
          state.resource_table.take::<TlsStreamResource>(rid).unwrap(),
        )
      };

    state.resource_table.add(Http2SessionDriver {
      stream,
      session: self.inner,
    })
  }

  #[fast]
  fn destroy(&self) {}

  #[fast]
  fn settings(&self, _cb: v8::Local<v8::Function>) -> bool {
    let settings = Http2Settings::init(self.inner);
    settings.send();
    true
  }

  fn goaway(
    &self,
    code: u32,
    last_stream_id: i32,
    #[anybuffer] maybe_data: Option<&[u8]>,
  ) {
    let (data_ptr, data_len) = maybe_data
      .map(|d| (d.as_ptr(), d.len()))
      .unwrap_or((std::ptr::null(), 0));

    unsafe {
      ffi::nghttp2_submit_goaway(
        self.session,
        ffi::NGHTTP2_FLAG_NONE as _,
        last_stream_id,
        code,
        data_ptr,
        data_len,
      );
    }
  }

  #[fast]
  fn set_graceful_close(&self) {
    let session = unsafe { &mut *self.inner };
    session.graceful_close_initiated = true;
  }

  #[fast]
  fn is_graceful_closing(&self) -> bool {
    let session = unsafe { &*self.inner };
    session.is_graceful_closing()
  }

  #[fast]
  fn submit_shutdown_notice(&self) {
    unsafe { ffi::nghttp2_submit_shutdown_notice(self.session) };
    let session = unsafe { &mut *self.inner };
    session.start_graceful_close();
  }

  #[fast]
  #[smi]
  fn active_stream_count(&self) -> u32 {
    let session = unsafe { &*self.inner };
    session.active_stream_count() as u32
  }

  #[fast]
  fn has_pending_data(&self) -> bool {
    unsafe {
      let want_write = ffi::nghttp2_session_want_write(self.session);
      let want_read = ffi::nghttp2_session_want_read(self.session);
      want_write != 0 || want_read != 0
    }
  }

  #[fast]
  fn local_settings(&self) {
    with_settings(|buffer| unsafe {
      buffer[SettingsIndex::HeaderTableSize as usize] =
        ffi::nghttp2_session_get_local_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_HEADER_TABLE_SIZE,
        ) as u32;
      buffer[SettingsIndex::EnablePush as usize] =
        ffi::nghttp2_session_get_local_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_ENABLE_PUSH,
        ) as u32;
      buffer[SettingsIndex::MaxConcurrentStreams as usize] =
        ffi::nghttp2_session_get_local_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS,
        ) as u32;
      buffer[SettingsIndex::InitialWindowSize as usize] =
        ffi::nghttp2_session_get_local_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE,
        ) as u32;
      buffer[SettingsIndex::MaxFrameSize as usize] =
        ffi::nghttp2_session_get_local_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_MAX_FRAME_SIZE,
        ) as u32;
      buffer[SettingsIndex::MaxHeaderListSize as usize] =
        ffi::nghttp2_session_get_local_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE,
        ) as u32;
      buffer[SettingsIndex::EnableConnectProtocol as usize] =
        ffi::nghttp2_session_get_local_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL,
        ) as u32;
    });
  }

  #[fast]
  fn remote_settings(&self) {
    with_settings(|buffer| unsafe {
      buffer[SettingsIndex::HeaderTableSize as usize] =
        ffi::nghttp2_session_get_remote_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_HEADER_TABLE_SIZE,
        ) as u32;
      buffer[SettingsIndex::EnablePush as usize] =
        ffi::nghttp2_session_get_remote_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_ENABLE_PUSH,
        ) as u32;
      buffer[SettingsIndex::MaxConcurrentStreams as usize] =
        ffi::nghttp2_session_get_remote_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS,
        ) as u32;
      buffer[SettingsIndex::InitialWindowSize as usize] =
        ffi::nghttp2_session_get_remote_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE,
        ) as u32;
      buffer[SettingsIndex::MaxFrameSize as usize] =
        ffi::nghttp2_session_get_remote_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_MAX_FRAME_SIZE,
        ) as u32;
      buffer[SettingsIndex::MaxHeaderListSize as usize] =
        ffi::nghttp2_session_get_remote_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE,
        ) as u32;
      buffer[SettingsIndex::EnableConnectProtocol as usize] =
        ffi::nghttp2_session_get_remote_settings(
          self.session,
          ffi::NGHTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL,
        ) as u32;
    });
  }

  #[serde]
  fn get_state(&self) -> Http2SessionState {
    unsafe {
      Http2SessionState {
        effective_local_window_size:
          ffi::nghttp2_session_get_effective_local_window_size(self.session)
            as f64,
        effective_recv_data_length:
          ffi::nghttp2_session_get_effective_recv_data_length(self.session)
            as f64,
        next_stream_id: ffi::nghttp2_session_get_next_stream_id(self.session)
          as f64,
        local_window_size: ffi::nghttp2_session_get_local_window_size(
          self.session,
        ) as f64,
        last_proc_stream_id: ffi::nghttp2_session_get_last_proc_stream_id(
          self.session,
        ) as f64,
        remote_window_size: ffi::nghttp2_session_get_remote_window_size(
          self.session,
        ) as f64,
        outbound_queue_size: ffi::nghttp2_session_get_outbound_queue_size(
          self.session,
        ) as f64,
        hd_deflate_dynamic_table_size:
          ffi::nghttp2_session_get_hd_deflate_dynamic_table_size(self.session)
            as f64,
        hd_inflate_dynamic_table_size:
          ffi::nghttp2_session_get_hd_inflate_dynamic_table_size(self.session)
            as f64,
      }
    }
  }

  #[fast]
  fn set_next_stream_id(&self, id: i32) -> bool {
    let ret =
      unsafe { ffi::nghttp2_session_set_next_stream_id(self.session, id) };
    if ret < 0 {
      log::debug!("failed to set next stream id to {}", id);
      return false;
    }
    log::debug!("set next stream id to {}", id);
    true
  }

  #[fast]
  fn set_local_window_size(&self, window_size: i32) -> i32 {
    unsafe {
      ffi::nghttp2_session_set_local_window_size(
        self.session,
        ffi::NGHTTP2_FLAG_NONE as u8,
        0,
        window_size,
      )
    }
  }

  #[fast]
  fn update_chunks_sent(&self) -> u32 {
    let session = unsafe { &*self.inner };
    session.outgoing_buffers.len() as u32
  }

  #[fast]
  fn origin(&self, #[string] origins: &str, count: i32) -> i32 {
    let mut ov: Vec<ffi::nghttp2_origin_entry> =
      Vec::with_capacity(count as usize);
    let origins_bytes = origins.as_bytes();
    let mut offset = 0;

    for _ in 0..count {
      if offset + 2 > origins_bytes.len() {
        break;
      }
      let len = ((origins_bytes[offset] as usize) << 8)
        | (origins_bytes[offset + 1] as usize);
      offset += 2;

      if offset + len > origins_bytes.len() {
        break;
      }

      ov.push(ffi::nghttp2_origin_entry {
        origin: origins_bytes[offset..].as_ptr() as *mut u8,
        origin_len: len,
      });
      offset += len;
    }

    unsafe {
      ffi::nghttp2_submit_origin(
        self.session,
        ffi::NGHTTP2_FLAG_NONE as u8,
        ov.as_ptr(),
        ov.len(),
      )
    }
  }

  #[fast]
  fn altsvc(
    &self,
    stream_id: i32,
    #[string] origin: &str,
    #[string] value: &str,
  ) -> i32 {
    let origin_bytes = origin.as_bytes();
    let value_bytes = value.as_bytes();

    if origin_bytes.len() + value_bytes.len() > 16382 {
      return -1;
    }

    if (origin_bytes.is_empty() && stream_id == 0)
      || (!origin_bytes.is_empty() && stream_id != 0)
    {
      return -1;
    }

    unsafe {
      ffi::nghttp2_submit_altsvc(
        self.session,
        ffi::NGHTTP2_FLAG_NONE as u8,
        stream_id,
        origin_bytes.as_ptr(),
        origin_bytes.len(),
        value_bytes.as_ptr(),
        value_bytes.len(),
      )
    }
  }

  #[fast]
  fn ping(&self, #[buffer] payload: &[u8]) -> i32 {
    if payload.len() != 8 {
      return -1;
    }

    unsafe {
      ffi::nghttp2_submit_ping(
        self.session,
        ffi::NGHTTP2_FLAG_NONE as u8,
        payload.as_ptr(),
      )
    }
  }

  fn request<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    #[serde] headers: (String, usize),
    options: i32,
    stream_id: i32,
    weight: i32,
    exclusive: bool,
  ) -> v8::Local<'s, v8::Value> {
    let priority = Http2Priority::new(stream_id, weight, exclusive);
    let headers = Http2Headers::from(headers);

    let ret = self.submit_request(priority, headers, options);
    if ret <= 0 {
      return v8::Integer::new(scope, ret).into();
    }

    let session = unsafe { &*self.inner };
    if let Some(stream_obj) = session.find_stream_obj(ret) {
      return v8::Local::new(scope, stream_obj).into();
    }
    v8::Integer::new(scope, -1).into()
  }
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
  #[global] origin_frame_cb: v8::Global<v8::Function>,
  #[global] stream_trailers_cb: v8::Global<v8::Function>,
  #[global] stream_close_cb: v8::Global<v8::Function>,
) {
  state.put(SessionCallbacks {
    session_internal_error_cb,
    priority_frame_cb,
    settings_frame_cb,
    ping_frame_cb,
    headers_frame_cb,
    frame_error_cb,
    goaway_data_cb,
    alt_svc_cb,
    origin_frame_cb,
    stream_trailers_cb,
    stream_close_cb,
  });
}

#[op2]
#[serde]
pub fn op_http2_http_state<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
) -> JSHttp2State<'a> {
  JSHttp2State::create(scope)
}
