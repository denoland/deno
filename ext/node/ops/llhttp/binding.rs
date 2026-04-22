// Copyright 2018-2026 the Deno authors. MIT license.

// The C FFI callbacks require extensive unsafe for pointer dereferences.
// All unsafe operations follow the same pattern: during execute(),
// llhttp_t.data points to a stack-allocated ExecuteContext which holds
// raw pointers to the Inner state and v8 PinScope, both of which are
// valid for the duration of the execute() call.
// CppGC-based HTTPParser binding for `internalBinding('http_parser')`.
//
// FFI callbacks use a uniform unsafe pattern: during execute(),
// llhttp_t.data points to a stack-allocated ExecuteContext holding
// raw pointers to Inner state and PinScope, both valid for the
// duration of the execute() call.
#![allow(
  clippy::undocumented_unsafe_blocks,
  reason = "uniform FFI callback pattern, see module comment above"
)]
//!
//! This exposes llhttp to JavaScript matching Node.js's native
//! `HTTPParser` class. JS callbacks are stored as indexed properties
//! on the parser object and invoked synchronously during `execute()`.

use std::cell::UnsafeCell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::os::raw::c_int;
use std::os::raw::c_void;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::uv_compat::uv_buf_t;
use deno_core::uv_compat::uv_stream_t;
use deno_core::v8;

use super::sys;
use crate::ops::stream_wrap::LibUvStreamWrap;
use crate::ops::stream_wrap::clone_context_from_uv_loop;
use crate::ops::stream_wrap_state::ReadInterceptor;

// JS callback indices — must match the constants in http_parser.ts
const K_ON_MESSAGE_BEGIN: u32 = 0;
const K_ON_HEADERS: u32 = 1;
const K_ON_HEADERS_COMPLETE: u32 = 2;
const K_ON_BODY: u32 = 3;
const K_ON_MESSAGE_COMPLETE: u32 = 4;
const K_ON_EXECUTE: u32 = 5;
// const K_ON_TIMEOUT: u32 = 6;

/// Maximum number of header field/value pairs to accumulate before
/// flushing to JS via the kOnHeaders callback (matches Node.js).
const MAX_HEADER_PAIRS: usize = 32;

/// Mutable inner state of the HTTPParser, accessed via UnsafeCell.
struct Inner {
  parser: sys::llhttp_t,
  settings: sys::llhttp_settings_t,

  header_fields: Vec<Vec<u8>>,
  header_values: Vec<Vec<u8>>,
  current_header_field: Vec<u8>,
  current_header_value: Vec<u8>,
  in_header_value: bool,
  /// Set to true when a partial batch of headers was flushed to JS via
  /// kOnHeaders during parsing (when total exceeded MAX_HEADER_PAIRS).
  /// When false at `on_headers_complete` with accumulated headers, we
  /// can skip the flush and pass the headers array directly to
  /// parserOnHeadersComplete — matching node's fast path.
  headers_flushed: bool,

  url: Vec<u8>,
  status_message: Vec<u8>,

  current_buffer_data: *const u8,
  current_buffer_len: usize,

  got_exception: bool,
  pending_pause: bool,

  max_header_size: u32,
  header_nread: u32,
  header_overflow: bool,
  initialized: bool,

  /// The stream being consumed (for parser.consume optimization).
  /// When set, the parser reads directly from the stream handle
  /// via a ReadInterceptor, bypassing the JS readable stream.
  consumed_stream: Option<*mut uv_stream_t>,
  /// Persistent handle to the JS wrapper object for callbacks during consume.
  consume_callbacks: Option<v8::Global<v8::Object>>,
  /// Raw isolate pointer for creating scopes in the interceptor callback.
  consume_isolate: v8::UnsafeRawIsolatePtr,
}

/// The CppGC-managed HTTP parser object.
pub struct HTTPParser {
  inner: UnsafeCell<Inner>,
}

// SAFETY: HTTPParser is only accessed from the main JS thread.
unsafe impl Send for HTTPParser {}

unsafe impl GarbageCollected for HTTPParser {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static CStr {
    c"HTTPParser"
  }
}

impl HTTPParser {
  fn create() -> Self {
    let mut settings =
      std::mem::MaybeUninit::<sys::llhttp_settings_t>::uninit();
    unsafe {
      sys::llhttp_settings_init(settings.as_mut_ptr());
    }
    let settings = unsafe { settings.assume_init() };
    let parser = unsafe { std::mem::zeroed::<sys::llhttp_t>() };

    Self {
      inner: UnsafeCell::new(Inner {
        parser,
        settings,
        header_fields: Vec::new(),
        header_values: Vec::new(),
        current_header_field: Vec::new(),
        current_header_value: Vec::new(),
        in_header_value: false,
        headers_flushed: false,
        url: Vec::new(),
        status_message: Vec::new(),
        current_buffer_data: std::ptr::null(),
        current_buffer_len: 0,
        got_exception: false,
        pending_pause: false,
        max_header_size: 0,
        header_nread: 0,
        header_overflow: false,
        initialized: false,
        consumed_stream: None,
        consume_callbacks: None,
        consume_isolate: v8::UnsafeRawIsolatePtr::null(),
      }),
    }
  }

  /// Get mutable access to inner state.
  /// SAFETY: only one caller at a time (single-threaded JS).
  #[inline]
  #[allow(
    clippy::mut_from_ref,
    reason = "interior mutability via UnsafeCell, single-threaded JS access"
  )]
  fn inner(&self) -> &mut Inner {
    unsafe { &mut *self.inner.get() }
  }
}

impl Inner {
  fn init(&mut self, parser_type: i32, lenient_flags: i32) {
    self.settings.on_message_begin = Some(on_message_begin);
    self.settings.on_url = Some(on_url);
    self.settings.on_status = Some(on_status);
    self.settings.on_header_field = Some(on_header_field);
    self.settings.on_header_value = Some(on_header_value);
    self.settings.on_headers_complete = Some(on_headers_complete);
    self.settings.on_body = Some(on_body);
    self.settings.on_message_complete = Some(on_message_complete);
    self.settings.on_header_value_complete = Some(on_header_value_complete);

    unsafe {
      sys::llhttp_init(&mut self.parser, parser_type, &self.settings);
    }

    // Apply lenient flags
    if lenient_flags != 0 {
      let p = &mut self.parser;
      unsafe {
        if lenient_flags & 1 != 0 {
          sys::llhttp_set_lenient_headers(p, 1);
        }
        if lenient_flags & 2 != 0 {
          sys::llhttp_set_lenient_chunked_length(p, 1);
        }
        if lenient_flags & 4 != 0 {
          sys::llhttp_set_lenient_keep_alive(p, 1);
        }
        if lenient_flags & 8 != 0 {
          sys::llhttp_set_lenient_transfer_encoding(p, 1);
        }
        if lenient_flags & 16 != 0 {
          sys::llhttp_set_lenient_version(p, 1);
        }
        if lenient_flags & 32 != 0 {
          sys::llhttp_set_lenient_data_after_close(p, 1);
        }
        if lenient_flags & 64 != 0 {
          sys::llhttp_set_lenient_optional_lf_after_cr(p, 1);
        }
        if lenient_flags & 128 != 0 {
          sys::llhttp_set_lenient_optional_crlf_after_chunk(p, 1);
        }
        if lenient_flags & 256 != 0 {
          sys::llhttp_set_lenient_optional_cr_before_lf(p, 1);
        }
        if lenient_flags & 512 != 0 {
          sys::llhttp_set_lenient_spaces_after_chunk_size(p, 1);
        }
      }
    }

    self.header_fields.clear();
    self.header_values.clear();
    self.current_header_field.clear();
    self.current_header_value.clear();
    self.in_header_value = false;
    self.headers_flushed = false;
    self.url.clear();
    self.status_message.clear();
    self.got_exception = false;
    self.pending_pause = false;
    self.header_nread = 0;
    self.initialized = true;
  }

  /// Create a flat JS array [field1, val1, field2, val2, ...]
  fn create_headers_array<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    fields: &[Vec<u8>],
    values: &[Vec<u8>],
  ) -> v8::Local<'a, v8::Array> {
    let len = fields.len() * 2;
    let arr = v8::Array::new(scope, len as i32);
    for (i, (field, value)) in fields.iter().zip(values.iter()).enumerate() {
      let f =
        v8::String::new_from_one_byte(scope, field, v8::NewStringType::Normal)
          .unwrap_or_else(|| v8::String::empty(scope));
      let v =
        v8::String::new_from_one_byte(scope, value, v8::NewStringType::Normal)
          .unwrap_or_else(|| v8::String::empty(scope));
      arr.set_index(scope, (i * 2) as u32, f.into());
      arr.set_index(scope, (i * 2 + 1) as u32, v.into());
    }
    arr
  }
}

// ---- ExecuteContext stored in llhttp_t.data during execute() ----

struct ExecuteContext {
  inner: *mut Inner,
  /// Type-erased PinScope pointer. Only valid during execute().
  scope_ptr: *mut (),
  /// The JS wrapper object with callback properties.
  callbacks: v8::Local<'static, v8::Object>,
}

impl ExecuteContext {
  /// Get scope and callbacks as a tuple to avoid borrow conflicts.
  ///
  /// # Safety: only valid during execute()
  unsafe fn scope_and_callbacks(
    &mut self,
  ) -> (
    &mut v8::PinScope<'static, 'static>,
    v8::Local<'static, v8::Object>,
  ) {
    let callbacks = self.callbacks;
    let scope =
      unsafe { &mut *(self.scope_ptr as *mut v8::PinScope<'static, 'static>) };
    (scope, callbacks)
  }
}

unsafe fn get_ctx<'a>(parser: *mut sys::llhttp_t) -> &'a mut ExecuteContext {
  unsafe { &mut *((*parser).data as *mut ExecuteContext) }
}

/// Check if header size exceeds the configured maximum.
/// Returns -1 (HPE_USER) if overflow, 0 if ok.
/// Matches Node.js's TrackHeader() behavior in node_http_parser.cc.
fn check_header_overflow(inner: &mut Inner) -> c_int {
  if inner.max_header_size > 0 && inner.header_nread >= inner.max_header_size {
    inner.header_overflow = true;
    return -1;
  }
  0
}

// ---- llhttp C callbacks ----

unsafe extern "C" fn on_message_begin(parser: *mut sys::llhttp_t) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  inner.url.clear();
  inner.status_message.clear();
  inner.header_fields.clear();
  inner.header_values.clear();
  inner.current_header_field.clear();
  inner.current_header_value.clear();
  inner.in_header_value = false;
  inner.headers_flushed = false;
  inner.header_nread = 0;
  inner.header_overflow = false;

  let (scope, cb_obj) = unsafe { ctx.scope_and_callbacks() };
  let cb = cb_obj.get_index(scope, K_ON_MESSAGE_BEGIN);
  if let Some(cb) = cb
    && let Ok(func) = v8::Local::<v8::Function>::try_from(cb)
  {
    v8::tc_scope!(tc, scope);
    if func.call(tc, cb_obj.into(), &[]).is_none() {
      if tc.has_caught() {
        if let Some(exc) = tc.exception() {
          let key = v8::String::new(tc, "__lastException").unwrap();
          cb_obj.set(tc, key.into(), exc);
        }
        tc.reset();
      }
      inner.got_exception = true;
      return -1;
    }
  }
  0
}

unsafe extern "C" fn on_url(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };
  inner.url.extend_from_slice(data);
  inner.header_nread += length as u32;
  check_header_overflow(inner)
}

unsafe extern "C" fn on_status(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };
  inner.status_message.extend_from_slice(data);
  inner.header_nread += length as u32;
  check_header_overflow(inner)
}

unsafe extern "C" fn on_header_field(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };
  if inner.in_header_value {
    inner.in_header_value = false;
  }
  inner.current_header_field.extend_from_slice(data);
  inner.header_nread += length as u32;
  check_header_overflow(inner)
}

unsafe extern "C" fn on_header_value(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };
  inner.in_header_value = true;
  inner.current_header_value.extend_from_slice(data);
  inner.header_nread += length as u32;
  check_header_overflow(inner)
}

unsafe extern "C" fn on_header_value_complete(
  parser: *mut sys::llhttp_t,
) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  let field = std::mem::take(&mut inner.current_header_field);
  let mut value = std::mem::take(&mut inner.current_header_value);
  // Strip leading/trailing OWS (spaces and tabs) from header values,
  // matching Node.js's HTTP parser behavior (RFC 9110 §5.5).
  let trimmed = {
    let bytes = value.as_slice();
    let start = bytes
      .iter()
      .position(|&b| b != b' ' && b != b'\t')
      .unwrap_or(bytes.len());
    let end = bytes
      .iter()
      .rposition(|&b| b != b' ' && b != b'\t')
      .map_or(start, |p| p + 1);
    start..end
  };
  if trimmed.start > 0 || trimmed.end < value.len() {
    value = value[trimmed].to_vec();
  }
  inner.header_fields.push(field);
  inner.header_values.push(value);
  inner.in_header_value = false;

  if inner.header_fields.len() >= MAX_HEADER_PAIRS {
    let (scope, cb_obj) = unsafe { ctx.scope_and_callbacks() };
    let headers = Inner::create_headers_array(
      scope,
      &inner.header_fields,
      &inner.header_values,
    );
    let url = v8::String::new_from_one_byte(
      scope,
      &inner.url,
      v8::NewStringType::Normal,
    )
    .unwrap_or_else(|| v8::String::empty(scope));

    if let Some(cb) = cb_obj.get_index(scope, K_ON_HEADERS)
      && let Ok(func) = v8::Local::<v8::Function>::try_from(cb)
    {
      let _ = func.call(scope, cb_obj.into(), &[headers.into(), url.into()]);
    }
    inner.header_fields.clear();
    inner.header_values.clear();
    inner.url.clear();
    inner.headers_flushed = true;
  }
  0
}

unsafe extern "C" fn on_headers_complete(parser: *mut sys::llhttp_t) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner_ptr = ctx.inner;
  let inner = unsafe { &mut *inner_ptr };
  let (scope, cb_obj) = unsafe { ctx.scope_and_callbacks() };

  let Some(cb) = cb_obj.get_index(scope, K_ON_HEADERS_COMPLETE) else {
    return 0;
  };
  let Ok(func) = v8::Local::<v8::Function>::try_from(cb) else {
    return 0;
  };

  // Fast path: when no prior kOnHeaders flush occurred (all headers
  // fit in the current parser.execute batch and total stayed under
  // MAX_HEADER_PAIRS), pass the accumulated headers directly to
  // parserOnHeadersComplete as the 3rd/5th args, skipping the
  // kOnHeaders flush JS call. Slow path (below) handles the
  // chunked-across-packets or >MAX_HEADER_PAIRS case by flushing
  // leftover headers via kOnHeaders so JS reads the full list
  // from parser._headers / parser._url.
  let skip_flush = !inner.headers_flushed && !inner.header_fields.is_empty();
  if !skip_flush && !inner.header_fields.is_empty() {
    let flush_headers = Inner::create_headers_array(
      scope,
      &inner.header_fields,
      &inner.header_values,
    );
    let flush_url = v8::String::new_from_one_byte(
      scope,
      &inner.url,
      v8::NewStringType::Normal,
    )
    .unwrap_or_else(|| v8::String::empty(scope));

    if let Some(on_hdr) = cb_obj.get_index(scope, K_ON_HEADERS)
      && let Ok(on_hdr_fn) = v8::Local::<v8::Function>::try_from(on_hdr)
    {
      let _ = on_hdr_fn.call(
        scope,
        cb_obj.into(),
        &[flush_headers.into(), flush_url.into()],
      );
    }
    inner.header_fields.clear();
    inner.header_values.clear();
    inner.url.clear();
  }

  let version_major =
    v8::Integer::new(scope, unsafe { (*parser).http_major } as i32);
  let version_minor =
    v8::Integer::new(scope, unsafe { (*parser).http_minor } as i32);
  let is_request = unsafe { (*parser).type_ } == sys::HTTP_REQUEST as u8;
  let undef = v8::undefined(scope);

  // Fast path: pass headers + url directly so parserOnHeadersComplete
  // uses them instead of reading from parser._headers/_url.
  // Slow path: pass undefined, JS reads from parser._headers/_url
  // which were populated by the flush above.
  let (headers, url): (v8::Local<v8::Value>, v8::Local<v8::Value>) =
    if skip_flush {
      let headers_arr = Inner::create_headers_array(
        scope,
        &inner.header_fields,
        &inner.header_values,
      );
      let url_val: v8::Local<v8::Value> = if is_request {
        v8::String::new_from_one_byte(
          scope,
          &inner.url,
          v8::NewStringType::Normal,
        )
        .unwrap_or_else(|| v8::String::empty(scope))
        .into()
      } else {
        undef.into()
      };
      (headers_arr.into(), url_val)
    } else {
      (undef.into(), undef.into())
    };

  // For requests: method is set, statusCode/statusMessage are undefined
  // For responses: statusCode/statusMessage are set, method is undefined
  let method: v8::Local<v8::Value> = if is_request {
    v8::Integer::new_from_unsigned(scope, unsafe { (*parser).method } as u32)
      .into()
  } else {
    undef.into()
  };
  let status_code: v8::Local<v8::Value> = if !is_request {
    v8::Integer::new(scope, unsafe { (*parser).status_code } as i32).into()
  } else {
    undef.into()
  };
  let status_message: v8::Local<v8::Value> = if !is_request {
    v8::String::new_from_one_byte(
      scope,
      &inner.status_message,
      v8::NewStringType::Normal,
    )
    .unwrap_or_else(|| v8::String::empty(scope))
    .into()
  } else {
    undef.into()
  };
  let upgrade = v8::Boolean::new(scope, unsafe { (*parser).upgrade } != 0);
  let should_keep_alive = v8::Boolean::new(
    scope,
    unsafe { sys::llhttp_should_keep_alive(parser) } != 0,
  );

  let args: [v8::Local<v8::Value>; 9] = [
    version_major.into(),
    version_minor.into(),
    headers,
    method,
    url,
    status_code,
    status_message,
    upgrade.into(),
    should_keep_alive.into(),
  ];

  v8::tc_scope!(tc, scope);
  let result = func.call(tc, cb_obj.into(), &args);
  let inner = unsafe { &mut *inner_ptr };
  inner.header_fields.clear();
  inner.header_values.clear();
  inner.url.clear();
  inner.status_message.clear();

  match result {
    None => {
      if tc.has_caught() {
        if let Some(exc) = tc.exception() {
          let key = v8::String::new(tc, "__lastException").unwrap();
          cb_obj.set(tc, key.into(), exc);
        }
        tc.reset();
      }
      inner.got_exception = true;
      -1
    }
    Some(val) => val.int32_value(tc).unwrap_or(0),
  }
}

unsafe extern "C" fn on_body(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner_ptr = ctx.inner;
  let (scope, cb_obj) = unsafe { ctx.scope_and_callbacks() };

  let Some(cb) = cb_obj.get_index(scope, K_ON_BODY) else {
    return 0;
  };
  let Ok(func) = v8::Local::<v8::Function>::try_from(cb) else {
    return 0;
  };

  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };
  let store = v8::ArrayBuffer::new_backing_store_from_bytes(
    data.to_vec().into_boxed_slice(),
  )
  .make_shared();
  let ab = v8::ArrayBuffer::with_backing_store(scope, &store);
  let buffer = v8::Uint8Array::new(scope, ab, 0, length).unwrap();

  v8::tc_scope!(tc, scope);
  let result = func.call(tc, cb_obj.into(), &[buffer.into()]);
  if result.is_none() {
    let inner = unsafe { &mut *inner_ptr };
    if tc.has_caught() {
      if let Some(exc) = tc.exception() {
        let key = v8::String::new(tc, "__lastException").unwrap();
        cb_obj.set(tc, key.into(), exc);
      }
      tc.reset();
    }
    inner.got_exception = true;
    unsafe {
      sys::llhttp_set_error_reason(
        parser,
        c"HPE_JS_EXCEPTION:JS Exception".as_ptr(),
      );
    }
    return sys::HPE_USER;
  }
  0
}

unsafe extern "C" fn on_message_complete(parser: *mut sys::llhttp_t) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  let (scope, cb_obj) = unsafe { ctx.scope_and_callbacks() };

  // Flush any remaining headers (e.g. trailing headers after chunked body)
  if !inner.header_fields.is_empty() {
    let headers = Inner::create_headers_array(
      scope,
      &inner.header_fields,
      &inner.header_values,
    );
    let url = v8::String::new_from_one_byte(
      scope,
      &inner.url,
      v8::NewStringType::Normal,
    )
    .unwrap_or_else(|| v8::String::empty(scope));

    if let Some(cb) = cb_obj.get_index(scope, K_ON_HEADERS)
      && let Ok(func) = v8::Local::<v8::Function>::try_from(cb)
    {
      let _ = func.call(scope, cb_obj.into(), &[headers.into(), url.into()]);
    }
    inner.header_fields.clear();
    inner.header_values.clear();
    inner.url.clear();
  }

  if let Some(cb) = cb_obj.get_index(scope, K_ON_MESSAGE_COMPLETE)
    && let Ok(func) = v8::Local::<v8::Function>::try_from(cb)
  {
    v8::tc_scope!(tc, scope);
    if func.call(tc, cb_obj.into(), &[]).is_none() {
      if tc.has_caught() {
        if let Some(exc) = tc.exception() {
          let key = v8::String::new(tc, "__lastException").unwrap();
          cb_obj.set(tc, key.into(), exc);
        }
        tc.reset();
      }
      inner.got_exception = true;
      return -1;
    }
  }
  0
}

// ---- ReadInterceptor callback for consume() ----

/// Called by the stream's read callback when data arrives.
/// `ptr` points to the HTTPParser's Inner state.
unsafe fn consume_read_callback(
  ptr: *mut c_void,
  stream: *mut uv_stream_t,
  nread: isize,
  _buf: *const uv_buf_t,
) {
  let inner = unsafe { &mut *(ptr as *mut Inner) };

  // Clone what we need from inner before taking mutable borrows
  let isolate_ptr = inner.consume_isolate;
  if isolate_ptr.is_null() {
    return;
  }
  let Some(ref callbacks_global) = inner.consume_callbacks else {
    return;
  };
  let callbacks_global = callbacks_global.clone();

  // Get isolate and create scope
  let mut isolate = unsafe { v8::Isolate::from_raw_isolate_ptr(isolate_ptr) };
  let loop_ptr = unsafe { (*stream).loop_ };
  let context = unsafe { clone_context_from_uv_loop(&mut isolate, loop_ptr) };
  v8::scope!(let handle_scope, &mut isolate);
  let context = v8::Local::new(handle_scope, context);
  let scope = &mut v8::ContextScope::new(handle_scope, context);

  if nread <= 0 {
    // EOF or error - invoke kOnExecute callback with the nread value.
    // Use TryCatch to absorb exceptions from socket lifecycle errors
    // (hang up, reset, etc.) which are expected during connection close.
    let cb_obj = v8::Local::new(scope, &callbacks_global);
    if let Some(cb) = cb_obj.get_index(scope, K_ON_EXECUTE)
      && let Ok(func) = v8::Local::<v8::Function>::try_from(cb)
    {
      v8::tc_scope!(tc, scope);
      let nread_val = v8::Integer::new(tc, nread as i32);
      let _ = func.call(tc, cb_obj.into(), &[nread_val.into()]);
      // Absorb any exception - EOF errors are normal lifecycle events
      if tc.has_caught() {
        tc.reset();
      }
    }
    return;
  }

  let data = unsafe {
    std::slice::from_raw_parts((*_buf).base as *const u8, nread as usize)
  };

  // Execute the parser directly on the buffer
  inner.current_buffer_data = data.as_ptr();
  inner.current_buffer_len = data.len();
  inner.got_exception = false;

  let callbacks_local = v8::Local::new(scope, &callbacks_global);
  // SAFETY: ContextScope and PinScope both deref to HandleScope.
  // The ExecuteContext only accesses the scope via HandleScope methods.
  let scope_ptr = scope as *mut v8::ContextScope<v8::HandleScope> as *mut ();
  let callbacks_static: v8::Local<'static, v8::Object> =
    unsafe { std::mem::transmute(callbacks_local) };

  let mut ctx = ExecuteContext {
    inner: inner as *mut Inner,
    scope_ptr,
    callbacks: callbacks_static,
  };

  inner.parser.data = &mut ctx as *mut ExecuteContext as *mut std::ffi::c_void;

  let err = unsafe {
    sys::llhttp_execute(
      &mut inner.parser,
      data.as_ptr() as *const c_char,
      data.len(),
    )
  };

  inner.parser.data = std::ptr::null_mut();
  inner.current_buffer_data = std::ptr::null();
  inner.current_buffer_len = 0;

  let mut nread_result = data.len() as i32;
  if err != sys::HPE_OK {
    let error_pos = unsafe { sys::llhttp_get_error_pos(&inner.parser) };
    if !error_pos.is_null() {
      nread_result =
        unsafe { error_pos.offset_from(data.as_ptr() as *const c_char) as i32 };
    }
    if err == sys::HPE_PAUSED_UPGRADE {
      unsafe {
        sys::llhttp_resume_after_upgrade(&mut inner.parser);
      }
    }
  }

  if inner.pending_pause {
    inner.pending_pause = false;
    unsafe {
      sys::llhttp_pause(&mut inner.parser);
    }
  }

  // Invoke kOnExecute with the result (bytes parsed or error)
  let cb_obj = v8::Local::new(scope, &callbacks_global);
  if let Some(cb) = cb_obj.get_index(scope, K_ON_EXECUTE)
    && let Ok(func) = v8::Local::<v8::Function>::try_from(cb)
  {
    let result_val = if inner.got_exception
      || (inner.parser.upgrade == 0 && err != sys::HPE_OK)
    {
      v8::Integer::new(scope, -1)
    } else {
      v8::Integer::new(scope, nread_result)
    };
    let _ = func.call(scope, cb_obj.into(), &[result_val.into()]);
  }
}

// ---- Op implementations ----

#[op2]
impl HTTPParser {
  #[constructor]
  #[cppgc]
  fn new() -> HTTPParser {
    HTTPParser::create()
  }

  /// Initialize (or reinitialize) the parser.
  #[nofast]
  fn initialize(
    &self,
    #[smi] parser_type: i32,
    #[smi] max_header_size: i32,
    #[smi] lenient_flags: i32,
  ) {
    let inner = self.inner();
    inner.max_header_size = if max_header_size > 0 {
      max_header_size as u32
    } else {
      0
    };
    inner.init(parser_type, lenient_flags);
  }

  /// Execute the parser on a buffer. Returns bytes parsed or -1 on error.
  /// `callbacks` is the JS wrapper object with indexed callback properties.
  #[nofast]
  #[reentrant]
  fn execute(
    &self,
    scope: &mut v8::PinScope,
    callbacks: v8::Local<v8::Object>,
    #[buffer] data: &[u8],
  ) -> i32 {
    let inner = self.inner();
    if !inner.initialized {
      return -1;
    }

    inner.current_buffer_data = data.as_ptr();
    inner.current_buffer_len = data.len();
    inner.got_exception = false;

    let scope_ptr = scope as *mut v8::PinScope as *mut ();
    let callbacks_static: v8::Local<'static, v8::Object> =
      unsafe { std::mem::transmute(callbacks) };

    let mut ctx = ExecuteContext {
      inner: inner as *mut Inner,
      scope_ptr,
      callbacks: callbacks_static,
    };

    inner.parser.data =
      &mut ctx as *mut ExecuteContext as *mut std::ffi::c_void;

    let err = unsafe {
      sys::llhttp_execute(
        &mut inner.parser,
        data.as_ptr() as *const c_char,
        data.len(),
      )
    };

    inner.parser.data = std::ptr::null_mut();
    inner.current_buffer_data = std::ptr::null();
    inner.current_buffer_len = 0;

    let mut nread = data.len();
    if err != sys::HPE_OK {
      let error_pos = unsafe { sys::llhttp_get_error_pos(&inner.parser) };
      if !error_pos.is_null() {
        nread = unsafe {
          error_pos.offset_from(data.as_ptr() as *const c_char) as usize
        };
      }
      if err == sys::HPE_PAUSED_UPGRADE {
        unsafe {
          sys::llhttp_resume_after_upgrade(&mut inner.parser);
        }
      }
    }

    if inner.pending_pause {
      inner.pending_pause = false;
      unsafe {
        sys::llhttp_pause(&mut inner.parser);
      }
    }

    if inner.got_exception {
      return -1;
    }

    if inner.parser.upgrade == 0 && err != sys::HPE_OK {
      return -1;
    }

    nread as i32
  }

  /// Signal end of input. Like execute(), this can trigger llhttp callbacks
  /// (e.g. on_message_complete), so we must set up the ExecuteContext with
  /// a valid scope and callbacks object.
  #[nofast]
  #[reentrant]
  fn finish(
    &self,
    scope: &mut v8::PinScope,
    callbacks: v8::Local<v8::Object>,
  ) -> i32 {
    let inner = self.inner();
    if !inner.initialized {
      return -1;
    }

    let scope_ptr = scope as *mut v8::PinScope as *mut ();
    let callbacks_static: v8::Local<'static, v8::Object> =
      unsafe { std::mem::transmute(callbacks) };

    let mut ctx = ExecuteContext {
      inner: inner as *mut Inner,
      scope_ptr,
      callbacks: callbacks_static,
    };

    inner.parser.data =
      &mut ctx as *mut ExecuteContext as *mut std::ffi::c_void;

    let err = unsafe { sys::llhttp_finish(&mut inner.parser) };

    inner.parser.data = std::ptr::null_mut();

    if err != sys::HPE_OK { -1 } else { 0 }
  }

  #[fast]
  fn pause(&self) {
    let inner = self.inner();
    if inner.initialized {
      unsafe { sys::llhttp_pause(&mut inner.parser) }
    }
  }

  #[fast]
  fn resume(&self) {
    let inner = self.inner();
    if inner.initialized {
      unsafe { sys::llhttp_resume(&mut inner.parser) }
    }
  }

  #[fast]
  fn close(&self) {
    self.inner().initialized = false;
  }

  #[fast]
  fn free(&self) {
    self.inner().initialized = false;
  }

  #[fast]
  fn remove(&self) {}

  /// Check if the last parse error was caused by header overflow.
  #[fast]
  fn has_header_overflow(&self) -> bool {
    self.inner().header_overflow
  }

  /// Get the current buffer being parsed (for error reporting).
  #[buffer]
  fn get_current_buffer(&self) -> Box<[u8]> {
    let inner = self.inner();
    if inner.current_buffer_data.is_null() || inner.current_buffer_len == 0 {
      return Box::new([]);
    }
    let data = unsafe {
      std::slice::from_raw_parts(
        inner.current_buffer_data,
        inner.current_buffer_len,
      )
    };
    data.to_vec().into_boxed_slice()
  }

  /// Consume a stream handle: register a ReadInterceptor so data
  /// flows directly from the TCP handle into llhttp_execute,
  /// bypassing the JS readable stream layer.
  /// `callbacks` is the JS wrapper object with indexed callback properties.
  /// `handle` is the LibUvStreamWrap (e.g. TCPWrap) cppgc object.
  #[nofast]
  fn consume(
    &self,
    scope: &mut v8::PinScope,
    callbacks: v8::Local<v8::Object>,
    handle: v8::Local<v8::Object>,
  ) {
    let inner = self.inner();

    // Try to get the LibUvStreamWrap from the handle
    let handle_value: v8::Local<v8::Value> = handle.into();
    let Some(stream_wrap) = deno_core::cppgc::try_unwrap_cppgc_object::<
      LibUvStreamWrap,
    >(scope, handle_value) else {
      return;
    };

    let stream = stream_wrap.stream_ptr();
    if stream.is_null() {
      return;
    }

    // Store the callbacks and isolate for use in the interceptor
    inner.consume_callbacks = Some(v8::Global::new(scope, callbacks));
    inner.consume_isolate = unsafe { scope.as_raw_isolate_ptr() };
    inner.consumed_stream = Some(stream);

    // Register the read interceptor
    let interceptor = ReadInterceptor {
      ptr: inner as *mut Inner as *mut c_void,
      callback: consume_read_callback,
    };
    LibUvStreamWrap::set_read_interceptor_for_stream(stream, Some(interceptor));
  }

  /// Unconsume: remove the ReadInterceptor so data goes back
  /// through the normal JS readable stream path.
  #[fast]
  fn unconsume(&self) {
    let inner = self.inner();
    if let Some(stream) = inner.consumed_stream.take() {
      LibUvStreamWrap::set_read_interceptor_for_stream(stream, None);
    }
    inner.consume_callbacks = None;
    inner.consume_isolate = v8::UnsafeRawIsolatePtr::null();
  }
}
