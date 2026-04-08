// Copyright 2018-2026 the Deno authors. MIT license.

// The C FFI callbacks require extensive unsafe for pointer dereferences.
// All unsafe operations follow the same pattern: during execute(),
// llhttp_t.data points to a stack-allocated ExecuteContext which holds
// raw pointers to the Inner state and v8 PinScope, both of which are
// valid for the duration of the execute() call.
#![allow(clippy::undocumented_unsafe_blocks)]

//! CppGC-based HTTPParser binding for `internalBinding('http_parser')`.
//!
//! This exposes llhttp to JavaScript matching Node.js's native
//! `HTTPParser` class. JS callbacks are stored as indexed properties
//! on the parser object and invoked synchronously during `execute()`.

use std::cell::UnsafeCell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::os::raw::c_int;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;

use super::sys;

// JS callback indices — must match the constants in http_parser.ts
const K_ON_MESSAGE_BEGIN: u32 = 0;
const K_ON_HEADERS: u32 = 1;
const K_ON_HEADERS_COMPLETE: u32 = 2;
const K_ON_BODY: u32 = 3;
const K_ON_MESSAGE_COMPLETE: u32 = 4;
// const K_ON_EXECUTE: u32 = 5;
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

  url: Vec<u8>,
  status_message: Vec<u8>,

  current_buffer_data: *const u8,
  current_buffer_len: usize,

  got_exception: bool,
  pending_pause: bool,

  max_header_size: u32,
  header_nread: u32,
  initialized: bool,
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
        url: Vec::new(),
        status_message: Vec::new(),
        current_buffer_data: std::ptr::null(),
        current_buffer_len: 0,
        got_exception: false,
        pending_pause: false,
        max_header_size: 0,
        header_nread: 0,
        initialized: false,
      }),
    }
  }

  /// Get mutable access to inner state.
  /// SAFETY: only one caller at a time (single-threaded JS).
  #[inline]
  #[allow(clippy::mut_from_ref)]
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
  inner.header_nread = 0;

  let (scope, cb_obj) = unsafe { ctx.scope_and_callbacks() };
  let cb = cb_obj.get_index(scope, K_ON_MESSAGE_BEGIN);
  if let Some(cb) = cb
    && let Ok(func) = v8::Local::<v8::Function>::try_from(cb)
    && func.call(scope, cb_obj.into(), &[]).is_none()
  {
    inner.got_exception = true;
    return -1;
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
  0
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
  0
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
  0
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
  0
}

unsafe extern "C" fn on_header_value_complete(
  parser: *mut sys::llhttp_t,
) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  let field = std::mem::take(&mut inner.current_header_field);
  let value = std::mem::take(&mut inner.current_header_value);
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
  }
  0
}

unsafe extern "C" fn on_headers_complete(parser: *mut sys::llhttp_t) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let inner = unsafe { &mut *ctx.inner };
  let (scope, cb_obj) = unsafe { ctx.scope_and_callbacks() };

  let Some(cb) = cb_obj.get_index(scope, K_ON_HEADERS_COMPLETE) else {
    return 0;
  };
  let Ok(func) = v8::Local::<v8::Function>::try_from(cb) else {
    return 0;
  };

  let version_major =
    v8::Integer::new(scope, unsafe { (*parser).http_major } as i32);
  let version_minor =
    v8::Integer::new(scope, unsafe { (*parser).http_minor } as i32);
  let headers = if !inner.header_fields.is_empty() {
    Inner::create_headers_array(
      scope,
      &inner.header_fields,
      &inner.header_values,
    )
    .into()
  } else {
    v8::undefined(scope).into()
  };
  let method =
    v8::Integer::new_from_unsigned(scope, unsafe { (*parser).method } as u32);
  let url =
    v8::String::new_from_one_byte(scope, &inner.url, v8::NewStringType::Normal)
      .unwrap_or_else(|| v8::String::empty(scope));
  let status_code =
    v8::Integer::new(scope, unsafe { (*parser).status_code } as i32);
  let status_message = v8::String::new_from_one_byte(
    scope,
    &inner.status_message,
    v8::NewStringType::Normal,
  )
  .unwrap_or_else(|| v8::String::empty(scope));
  let upgrade = v8::Boolean::new(scope, unsafe { (*parser).upgrade } != 0);
  let should_keep_alive = v8::Boolean::new(
    scope,
    unsafe { sys::llhttp_should_keep_alive(parser) } != 0,
  );

  let args: [v8::Local<v8::Value>; 9] = [
    version_major.into(),
    version_minor.into(),
    headers,
    method.into(),
    url.into(),
    status_code.into(),
    status_message.into(),
    upgrade.into(),
    should_keep_alive.into(),
  ];

  let result = func.call(scope, cb_obj.into(), &args);
  inner.header_fields.clear();
  inner.header_values.clear();
  inner.url.clear();
  inner.status_message.clear();

  match result {
    None => {
      inner.got_exception = true;
      -1
    }
    Some(val) => val.int32_value(scope).unwrap_or(0),
  }
}

unsafe extern "C" fn on_body(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
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

  let result = func.call(scope, cb_obj.into(), &[buffer.into()]);
  if result.is_none() {
    let inner = unsafe { &mut *ctx.inner };
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

  if let Some(cb) = cb_obj.get_index(scope, K_ON_MESSAGE_COMPLETE)
    && let Ok(func) = v8::Local::<v8::Function>::try_from(cb)
    && func.call(scope, cb_obj.into(), &[]).is_none()
  {
    inner.got_exception = true;
    return -1;
  }
  0
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

  /// Signal end of input.
  #[fast]
  fn finish(&self) -> i32 {
    let inner = self.inner();
    if !inner.initialized {
      return -1;
    }
    let err = unsafe { sys::llhttp_finish(&mut inner.parser) };
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
}
