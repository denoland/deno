// Copyright 2018-2026 the Deno authors. MIT license.

//! CppGC-based HTTPParser binding for `internalBinding('http_parser')`.
//!
//! This exposes llhttp to JavaScript matching Node.js's native
//! `HTTPParser` class. JS callbacks are stored as indexed properties
//! on the parser object and invoked synchronously during `execute()`.

use std::cell::Cell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::os::raw::c_int;

use deno_core::op2;
use deno_core::GarbageCollected;

use super::sys;

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

/// The CppGC-managed HTTP parser object.
///
/// Owns the llhttp parser state and settings. During `execute()`, a
/// pointer to an `ExecuteContext` is stored in `llhttp_t.data` so that
/// C callbacks can reach JS.
pub struct HTTPParser {
  parser: sys::llhttp_t,
  settings: sys::llhttp_settings_t,

  // Header accumulation buffer (field, value pairs as byte vecs)
  header_fields: Vec<Vec<u8>>,
  header_values: Vec<Vec<u8>>,
  // Current header field/value being accumulated across callbacks
  current_header_field: Vec<u8>,
  current_header_value: Vec<u8>,
  in_header_value: bool,

  // URL and status message accumulation
  url: Vec<u8>,
  status_message: Vec<u8>,

  // The data buffer currently being parsed (set during execute)
  current_buffer_data: *const u8,
  current_buffer_len: usize,

  // Track if a JS exception occurred in a callback
  got_exception: Cell<bool>,

  // Whether to pause after the current headers_complete callback
  pending_pause: bool,

  // Max header size (0 = default)
  max_header_size: u32,
  header_nread: u32,

  initialized: bool,
}

// SAFETY: HTTPParser is only accessed from the main thread.
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
      got_exception: Cell::new(false),
      pending_pause: false,
      max_header_size: 0,
      header_nread: 0,
      initialized: false,
    }
  }

  fn init(&mut self, parser_type: i32, lenient_flags: i32) {
    // Install C callbacks that bridge to JS
    self.settings.on_message_begin = Some(on_message_begin);
    self.settings.on_url = Some(on_url);
    self.settings.on_status = Some(on_status);
    self.settings.on_header_field = Some(on_header_field);
    self.settings.on_header_value = Some(on_header_value);
    self.settings.on_headers_complete = Some(on_headers_complete);
    self.settings.on_body = Some(on_body);
    self.settings.on_message_complete = Some(on_message_complete);
    self.settings.on_url_complete = Some(on_url_complete);
    self.settings.on_header_field_complete = Some(on_header_field_complete);
    self.settings.on_header_value_complete = Some(on_header_value_complete);

    unsafe {
      sys::llhttp_init(&mut self.parser, parser_type, &self.settings);
    }

    // Apply lenient flags
    if lenient_flags != 0 {
      unsafe {
        if lenient_flags & 1 != 0 {
          sys::llhttp_set_lenient_headers(&mut self.parser, 1);
        }
        if lenient_flags & 2 != 0 {
          sys::llhttp_set_lenient_chunked_length(&mut self.parser, 1);
        }
        if lenient_flags & 4 != 0 {
          sys::llhttp_set_lenient_keep_alive(&mut self.parser, 1);
        }
        if lenient_flags & 8 != 0 {
          sys::llhttp_set_lenient_transfer_encoding(&mut self.parser, 1);
        }
        if lenient_flags & 16 != 0 {
          sys::llhttp_set_lenient_version(&mut self.parser, 1);
        }
        if lenient_flags & 32 != 0 {
          sys::llhttp_set_lenient_data_after_close(&mut self.parser, 1);
        }
        if lenient_flags & 64 != 0 {
          sys::llhttp_set_lenient_optional_lf_after_cr(&mut self.parser, 1);
        }
        if lenient_flags & 128 != 0 {
          sys::llhttp_set_lenient_optional_crlf_after_chunk(&mut self.parser, 1);
        }
        if lenient_flags & 256 != 0 {
          sys::llhttp_set_lenient_optional_cr_before_lf(&mut self.parser, 1);
        }
        if lenient_flags & 512 != 0 {
          sys::llhttp_set_lenient_spaces_after_chunk_size(&mut self.parser, 1);
        }
      }
    }

    // Reset accumulation state
    self.header_fields.clear();
    self.header_values.clear();
    self.current_header_field.clear();
    self.current_header_value.clear();
    self.in_header_value = false;
    self.url.clear();
    self.status_message.clear();
    self.got_exception.set(false);
    self.pending_pause = false;
    self.header_nread = 0;
    self.initialized = true;
  }

  /// Flush accumulated headers to JS via the kOnHeaders callback.
  fn flush_headers(
    &mut self,
    scope: &mut v8::PinScope<'_, '_>,
    this: v8::Local<v8::Object>,
  ) {
    if self.header_fields.is_empty() {
      return;
    }

    let headers = Self::create_headers_array(
      scope,
      &self.header_fields,
      &self.header_values,
    );
    let url = v8::String::new_from_one_byte(
      scope,
      &self.url,
      v8::NewStringType::Normal,
    )
    .unwrap_or_else(|| v8::String::empty(scope));

    let cb = this.get_index(scope, K_ON_HEADERS);
    if let Some(cb) = cb {
      if let Ok(func) = v8::Local::<v8::Function>::try_from(cb) {
        let args: [v8::Local<v8::Value>; 2] =
          [headers.into(), url.into()];
        let _ = func.call(scope, this.into(), &args);
      }
    }

    self.header_fields.clear();
    self.header_values.clear();
    self.url.clear();
  }

  /// Create a flat JS array from header field/value pairs:
  /// [field1, value1, field2, value2, ...]
  fn create_headers_array<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    fields: &[Vec<u8>],
    values: &[Vec<u8>],
  ) -> v8::Local<'a, v8::Array> {
    let len = fields.len() * 2;
    let arr = v8::Array::new(scope, len as i32);

    for (i, (field, value)) in fields.iter().zip(values.iter()).enumerate() {
      let f = v8::String::new_from_one_byte(
        scope,
        field,
        v8::NewStringType::Normal,
      )
      .unwrap_or_else(|| v8::String::empty(scope));
      let v = v8::String::new_from_one_byte(
        scope,
        value,
        v8::NewStringType::Normal,
      )
      .unwrap_or_else(|| v8::String::empty(scope));
      arr.set_index(scope, (i * 2) as u32, f.into());
      arr.set_index(scope, (i * 2 + 1) as u32, v.into());
    }

    arr
  }
}

// ---- Context passed through llhttp_t.data during execute() ----

/// Stored in `llhttp_t.data` during `execute()` so C callbacks can
/// reach the JS scope and the parser's JS object.
struct ExecuteContext {
  /// Raw pointer to the HTTPParser (valid for the duration of execute)
  parser: *mut HTTPParser,
  /// Type-erased pointer to the v8::PinScope. Only valid during execute().
  scope_ptr: *mut (),
  /// The JS `this` object (the HTTPParser instance)
  this: v8::Local<'static, v8::Object>,
}

impl ExecuteContext {
  /// Get a mutable reference to the PinScope.
  ///
  /// # Safety
  /// Only valid during execute() while the PinScope is alive on the stack.
  unsafe fn scope(&mut self) -> &mut v8::PinScope<'static, 'static> {
    unsafe { &mut *(self.scope_ptr as *mut v8::PinScope<'static, 'static>) }
  }
}

/// Helper to get the ExecuteContext from an llhttp_t during a callback.
///
/// # Safety
/// Only valid during `execute()` when `parser.data` points to a live
/// `ExecuteContext`.
unsafe fn get_execute_ctx<'a>(
  parser: *mut sys::llhttp_t,
) -> &'a mut ExecuteContext {
  unsafe { &mut *((*parser).data as *mut ExecuteContext) }
}

// ---- llhttp C callbacks ----

unsafe extern "C" fn on_message_begin(
  parser: *mut sys::llhttp_t,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &mut *ctx.parser };

  // Reset accumulation state for new message
  http_parser.url.clear();
  http_parser.status_message.clear();
  http_parser.header_fields.clear();
  http_parser.header_values.clear();
  http_parser.current_header_field.clear();
  http_parser.current_header_value.clear();
  http_parser.in_header_value = false;
  http_parser.header_nread = 0;

  let scope = unsafe { ctx.scope() };
  let cb = ctx.this.get_index(scope, K_ON_MESSAGE_BEGIN);
  if let Some(cb) = cb {
    if let Ok(func) = v8::Local::<v8::Function>::try_from(cb) {
      let result = func.call(scope, ctx.this.into(), &[]);
      if result.is_none() {
        http_parser.got_exception.set(true);
        return -1;
      }
    }
  }
  0
}

unsafe extern "C" fn on_url(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &mut *ctx.parser };
  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };
  http_parser.url.extend_from_slice(data);
  http_parser.header_nread += length as u32;
  0
}

unsafe extern "C" fn on_url_complete(
  _parser: *mut sys::llhttp_t,
) -> c_int {
  0
}

unsafe extern "C" fn on_status(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &mut *ctx.parser };
  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };
  http_parser.status_message.extend_from_slice(data);
  http_parser.header_nread += length as u32;
  0
}

unsafe extern "C" fn on_header_field(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &mut *ctx.parser };
  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };

  if http_parser.in_header_value {
    // Transitioning from value to a new field
    http_parser.in_header_value = false;
  }

  http_parser.current_header_field.extend_from_slice(data);
  http_parser.header_nread += length as u32;
  0
}

unsafe extern "C" fn on_header_field_complete(
  _parser: *mut sys::llhttp_t,
) -> c_int {
  0
}

unsafe extern "C" fn on_header_value(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &mut *ctx.parser };
  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };

  if !http_parser.in_header_value {
    http_parser.in_header_value = true;
  }

  http_parser.current_header_value.extend_from_slice(data);
  http_parser.header_nread += length as u32;
  0
}

unsafe extern "C" fn on_header_value_complete(
  parser: *mut sys::llhttp_t,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &mut *ctx.parser };

  // Save completed header pair
  let field = std::mem::take(&mut http_parser.current_header_field);
  let value = std::mem::take(&mut http_parser.current_header_value);
  http_parser.header_fields.push(field);
  http_parser.header_values.push(value);
  http_parser.in_header_value = false;

  // Flush if we've accumulated enough headers
  if http_parser.header_fields.len() >= MAX_HEADER_PAIRS {
    let scope = unsafe { ctx.scope() };
    http_parser.flush_headers(scope, ctx.this);
  }

  0
}

unsafe extern "C" fn on_headers_complete(
  parser: *mut sys::llhttp_t,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &mut *ctx.parser };
  let scope = unsafe { ctx.scope() };

  let cb = ctx.this.get_index(scope, K_ON_HEADERS_COMPLETE);
  let cb = match cb {
    Some(cb) => match v8::Local::<v8::Function>::try_from(cb) {
      Ok(f) => f,
      Err(_) => return 0,
    },
    None => return 0,
  };

  // Build the arguments array matching Node.js:
  // [versionMajor, versionMinor, headers, method, url,
  //  statusCode, statusMessage, upgrade, shouldKeepAlive]

  let version_major =
    v8::Integer::new(scope, unsafe { (*parser).http_major } as i32);
  let version_minor =
    v8::Integer::new(scope, unsafe { (*parser).http_minor } as i32);

  // Headers: create flat array or undefined if already flushed
  let headers = if !http_parser.header_fields.is_empty() {
    HTTPParser::create_headers_array(
      scope,
      &http_parser.header_fields,
      &http_parser.header_values,
    )
    .into()
  } else {
    v8::undefined(scope).into()
  };

  let method = v8::Integer::new_from_unsigned(
    scope,
    unsafe { (*parser).method } as u32,
  );
  let url = v8::String::new_from_one_byte(
    scope,
    &http_parser.url,
    v8::NewStringType::Normal,
  )
  .unwrap_or_else(|| v8::String::empty(scope));
  let status_code =
    v8::Integer::new(scope, unsafe { (*parser).status_code } as i32);
  let status_message = v8::String::new_from_one_byte(
    scope,
    &http_parser.status_message,
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

  let result = cb.call(scope, ctx.this.into(), &args);

  // Clear accumulated headers
  http_parser.header_fields.clear();
  http_parser.header_values.clear();
  http_parser.url.clear();
  http_parser.status_message.clear();

  match result {
    None => {
      http_parser.got_exception.set(true);
      -1
    }
    Some(val) => {
      // Return value: 0 = continue, 1 = skip body, 2 = skip body + upgrade
      val.int32_value(scope).unwrap_or(0)
    }
  }
}

unsafe extern "C" fn on_body(
  parser: *mut sys::llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &*ctx.parser };
  let scope = unsafe { ctx.scope() };

  let cb = ctx.this.get_index(scope, K_ON_BODY);
  let cb = match cb {
    Some(cb) => match v8::Local::<v8::Function>::try_from(cb) {
      Ok(f) => f,
      Err(_) => return 0,
    },
    None => return 0,
  };

  let data = unsafe { std::slice::from_raw_parts(at as *const u8, length) };
  let bytes: Vec<u8> = data.to_vec();
  let store =
    v8::ArrayBuffer::new_backing_store_from_bytes(bytes.into_boxed_slice())
      .make_shared();
  let buffer = v8::ArrayBuffer::with_backing_store(scope, &store);
  let buffer = v8::Uint8Array::new(scope, buffer, 0, length).unwrap();

  let result = cb.call(scope, ctx.this.into(), &[buffer.into()]);
  if result.is_none() {
    http_parser.got_exception.set(true);
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

unsafe extern "C" fn on_message_complete(
  parser: *mut sys::llhttp_t,
) -> c_int {
  let ctx = unsafe { get_execute_ctx(parser) };
  let http_parser = unsafe { &*ctx.parser };
  let scope = unsafe { ctx.scope() };

  let cb = ctx.this.get_index(scope, K_ON_MESSAGE_COMPLETE);
  if let Some(cb) = cb {
    if let Ok(func) = v8::Local::<v8::Function>::try_from(cb) {
      let result = func.call(scope, ctx.this.into(), &[]);
      if result.is_none() {
        http_parser.got_exception.set(true);
        return -1;
      }
    }
  }
  0
}

// ---- Op implementations ----

use deno_core::v8;

#[op2]
impl HTTPParser {
  /// Create a new uninitialized HTTPParser.
  #[constructor]
  #[cppgc]
  fn new() -> HTTPParser {
    HTTPParser::create()
  }

  /// Initialize (or reinitialize) the parser.
  /// Args: type (REQUEST=1, RESPONSE=2), maxHeaderSize, lenientFlags
  #[nofast]
  fn initialize(
    &mut self,
    #[smi] parser_type: i32,
    #[smi] max_header_size: i32,
    #[smi] lenient_flags: i32,
  ) {
    self.max_header_size = if max_header_size > 0 {
      max_header_size as u32
    } else {
      0
    };
    self.init(parser_type, lenient_flags);
  }

  /// Execute the parser on a buffer. Returns bytes parsed or throws.
  ///
  /// During execution, llhttp callbacks invoke JS functions stored as
  /// indexed properties on this parser object (kOnHeaders, kOnBody, etc.).
  #[nofast]
  #[reentrant]
  fn execute(
    &mut self,
    scope: &mut v8::PinScope,
    this: v8::Local<v8::Object>,
    #[buffer] data: &[u8],
  ) -> i32 {
    if !self.initialized {
      return -1;
    }

    self.current_buffer_data = data.as_ptr();
    self.current_buffer_len = data.len();
    self.got_exception.set(false);

    // Store scope pointer for the duration of execute(). The
    // ExecuteContext lives on the stack and is only valid during this call.
    let scope_ptr = scope as *mut v8::PinScope as *mut ();
    // SAFETY: this Local is only accessed during execute() via callbacks.
    let this_static: v8::Local<'static, v8::Object> =
      unsafe { std::mem::transmute(this) };

    let mut ctx = ExecuteContext {
      parser: self as *mut HTTPParser,
      scope_ptr,
      this: this_static,
    };

    self.parser.data =
      &mut ctx as *mut ExecuteContext as *mut std::ffi::c_void;

    let err = unsafe {
      sys::llhttp_execute(
        &mut self.parser,
        data.as_ptr() as *const c_char,
        data.len(),
      )
    };

    // Clear context pointer
    self.parser.data = std::ptr::null_mut();
    self.current_buffer_data = std::ptr::null();
    self.current_buffer_len = 0;

    // Calculate bytes parsed
    let mut nread = data.len();
    if err != sys::HPE_OK {
      let error_pos = unsafe { sys::llhttp_get_error_pos(&self.parser) };
      if !error_pos.is_null() {
        nread = unsafe {
          error_pos.offset_from(data.as_ptr() as *const c_char) as usize
        };
      }

      if err == sys::HPE_PAUSED_UPGRADE {
        unsafe {
          sys::llhttp_resume_after_upgrade(&mut self.parser);
        }
      }
    }

    // Apply pending pause
    if self.pending_pause {
      self.pending_pause = false;
      unsafe {
        sys::llhttp_pause(&mut self.parser);
      }
    }

    if self.got_exception.get() {
      return -1;
    }

    // If there was a parse error (not upgrade), return error
    if self.parser.upgrade == 0 && err != sys::HPE_OK {
      // TODO: create proper error object with code/reason
      return -1;
    }

    nread as i32
  }

  /// Signal end of input (for responses that use EOF to signal end).
  #[nofast]
  fn finish(&mut self) -> i32 {
    if !self.initialized {
      return -1;
    }
    let err = unsafe { sys::llhttp_finish(&mut self.parser) };
    if err != sys::HPE_OK {
      -1
    } else {
      0
    }
  }

  /// Pause the parser.
  #[fast]
  fn pause(&mut self) {
    if self.initialized {
      unsafe {
        sys::llhttp_pause(&mut self.parser);
      }
    }
  }

  /// Resume the parser.
  #[fast]
  fn resume(&mut self) {
    if self.initialized {
      unsafe {
        sys::llhttp_resume(&mut self.parser);
      }
    }
  }

  /// Close the parser and free resources.
  #[fast]
  fn close(&mut self) {
    self.initialized = false;
  }

  /// Free the parser (alias for close, matching Node API).
  #[fast]
  fn free(&mut self) {
    self.initialized = false;
  }

  /// Remove from connection tracking (no-op for now).
  #[fast]
  fn remove(&self) {
    // ConnectionsList tracking not yet implemented
  }

  /// Get the current buffer being parsed (for error reporting).
  fn get_current_buffer(
    &self,
    scope: &mut v8::PinScope,
  ) -> v8::Local<'_, v8::Value> {
    if self.current_buffer_data.is_null() || self.current_buffer_len == 0 {
      return v8::undefined(scope).into();
    }

    let data = unsafe {
      std::slice::from_raw_parts(
        self.current_buffer_data,
        self.current_buffer_len,
      )
    };
    let store =
      v8::ArrayBuffer::new_backing_store_from_bytes(data.to_vec().into());
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &store);
    v8::Uint8Array::new(scope, buffer, 0, self.current_buffer_len)
      .unwrap()
      .into()
  }
}
