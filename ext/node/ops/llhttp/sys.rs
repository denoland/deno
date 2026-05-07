// Copyright 2018-2026 the Deno authors. MIT license.

//! Raw FFI bindings for llhttp 9.3.1.
//! Generated from deps/llhttp/include/llhttp.h

use std::os::raw::c_char;
use std::os::raw::c_int;

// ---- Types ----

pub type llhttp_data_cb = Option<
  unsafe extern "C" fn(
    parser: *mut llhttp_t,
    at: *const c_char,
    length: usize,
  ) -> c_int,
>;
pub type llhttp_cb =
  Option<unsafe extern "C" fn(parser: *mut llhttp_t) -> c_int>;

// ---- Enums ----

pub type llhttp_errno_t = c_int;
pub const HPE_OK: llhttp_errno_t = 0;
pub const HPE_INTERNAL: llhttp_errno_t = 1;
pub const HPE_STRICT: llhttp_errno_t = 2;
pub const HPE_CR_EXPECTED: llhttp_errno_t = 25;
pub const HPE_LF_EXPECTED: llhttp_errno_t = 3;
pub const HPE_UNEXPECTED_CONTENT_LENGTH: llhttp_errno_t = 4;
pub const HPE_UNEXPECTED_SPACE: llhttp_errno_t = 30;
pub const HPE_CLOSED_CONNECTION: llhttp_errno_t = 5;
pub const HPE_INVALID_METHOD: llhttp_errno_t = 6;
pub const HPE_INVALID_URL: llhttp_errno_t = 7;
pub const HPE_INVALID_CONSTANT: llhttp_errno_t = 8;
pub const HPE_INVALID_VERSION: llhttp_errno_t = 9;
pub const HPE_INVALID_HEADER_TOKEN: llhttp_errno_t = 10;
pub const HPE_INVALID_CONTENT_LENGTH: llhttp_errno_t = 11;
pub const HPE_INVALID_CHUNK_SIZE: llhttp_errno_t = 12;
pub const HPE_INVALID_STATUS: llhttp_errno_t = 13;
pub const HPE_INVALID_EOF_STATE: llhttp_errno_t = 14;
pub const HPE_INVALID_TRANSFER_ENCODING: llhttp_errno_t = 15;
pub const HPE_CB_MESSAGE_BEGIN: llhttp_errno_t = 16;
pub const HPE_CB_HEADERS_COMPLETE: llhttp_errno_t = 17;
pub const HPE_CB_MESSAGE_COMPLETE: llhttp_errno_t = 18;
pub const HPE_CB_CHUNK_HEADER: llhttp_errno_t = 19;
pub const HPE_CB_CHUNK_COMPLETE: llhttp_errno_t = 20;
pub const HPE_PAUSED: llhttp_errno_t = 21;
pub const HPE_PAUSED_UPGRADE: llhttp_errno_t = 22;
pub const HPE_PAUSED_H2_UPGRADE: llhttp_errno_t = 23;
pub const HPE_USER: llhttp_errno_t = 24;

pub type llhttp_type_t = c_int;
pub const HTTP_BOTH: llhttp_type_t = 0;
pub const HTTP_REQUEST: llhttp_type_t = 1;
pub const HTTP_RESPONSE: llhttp_type_t = 2;

pub type llhttp_method_t = c_int;
pub const HTTP_DELETE: llhttp_method_t = 0;
pub const HTTP_GET: llhttp_method_t = 1;
pub const HTTP_HEAD: llhttp_method_t = 2;
pub const HTTP_POST: llhttp_method_t = 3;
pub const HTTP_PUT: llhttp_method_t = 4;
pub const HTTP_CONNECT: llhttp_method_t = 5;
pub const HTTP_OPTIONS: llhttp_method_t = 6;
pub const HTTP_TRACE: llhttp_method_t = 7;
pub const HTTP_COPY: llhttp_method_t = 8;
pub const HTTP_LOCK: llhttp_method_t = 9;
pub const HTTP_MKCOL: llhttp_method_t = 10;
pub const HTTP_MOVE: llhttp_method_t = 11;
pub const HTTP_PROPFIND: llhttp_method_t = 12;
pub const HTTP_PROPPATCH: llhttp_method_t = 13;
pub const HTTP_SEARCH: llhttp_method_t = 14;
pub const HTTP_UNLOCK: llhttp_method_t = 15;
pub const HTTP_BIND: llhttp_method_t = 16;
pub const HTTP_REBIND: llhttp_method_t = 17;
pub const HTTP_UNBIND: llhttp_method_t = 18;
pub const HTTP_ACL: llhttp_method_t = 19;
pub const HTTP_REPORT: llhttp_method_t = 20;
pub const HTTP_MKACTIVITY: llhttp_method_t = 21;
pub const HTTP_CHECKOUT: llhttp_method_t = 22;
pub const HTTP_MERGE: llhttp_method_t = 23;
pub const HTTP_MSEARCH: llhttp_method_t = 24;
pub const HTTP_NOTIFY: llhttp_method_t = 25;
pub const HTTP_SUBSCRIBE: llhttp_method_t = 26;
pub const HTTP_UNSUBSCRIBE: llhttp_method_t = 27;
pub const HTTP_PATCH: llhttp_method_t = 28;
pub const HTTP_PURGE: llhttp_method_t = 29;
pub const HTTP_MKCALENDAR: llhttp_method_t = 30;
pub const HTTP_LINK: llhttp_method_t = 31;
pub const HTTP_UNLINK: llhttp_method_t = 32;
pub const HTTP_SOURCE: llhttp_method_t = 33;
pub const HTTP_PRI: llhttp_method_t = 34;
pub const HTTP_DESCRIBE: llhttp_method_t = 35;
pub const HTTP_ANNOUNCE: llhttp_method_t = 36;
pub const HTTP_SETUP: llhttp_method_t = 37;
pub const HTTP_PLAY: llhttp_method_t = 38;
pub const HTTP_PAUSE: llhttp_method_t = 39;
pub const HTTP_TEARDOWN: llhttp_method_t = 40;
pub const HTTP_GET_PARAMETER: llhttp_method_t = 41;
pub const HTTP_SET_PARAMETER: llhttp_method_t = 42;
pub const HTTP_REDIRECT: llhttp_method_t = 43;
pub const HTTP_RECORD: llhttp_method_t = 44;
pub const HTTP_FLUSH: llhttp_method_t = 45;
pub const HTTP_QUERY: llhttp_method_t = 46;

pub type llhttp_finish_t = c_int;
pub const HTTP_FINISH_SAFE: llhttp_finish_t = 0;
pub const HTTP_FINISH_SAFE_WITH_CB: llhttp_finish_t = 1;
pub const HTTP_FINISH_UNSAFE: llhttp_finish_t = 2;

// Flags
pub const F_CONNECTION_KEEP_ALIVE: u16 = 0x1;
pub const F_CONNECTION_CLOSE: u16 = 0x2;
pub const F_CONNECTION_UPGRADE: u16 = 0x4;
pub const F_CHUNKED: u16 = 0x8;
pub const F_UPGRADE: u16 = 0x10;
pub const F_CONTENT_LENGTH: u16 = 0x20;
pub const F_SKIPBODY: u16 = 0x40;
pub const F_TRAILING: u16 = 0x80;
pub const F_TRANSFER_ENCODING: u16 = 0x200;

// Lenient flags
pub const LENIENT_HEADERS: u16 = 0x1;
pub const LENIENT_CHUNKED_LENGTH: u16 = 0x2;
pub const LENIENT_KEEP_ALIVE: u16 = 0x4;
pub const LENIENT_TRANSFER_ENCODING: u16 = 0x8;
pub const LENIENT_VERSION: u16 = 0x10;
pub const LENIENT_DATA_AFTER_CLOSE: u16 = 0x20;
pub const LENIENT_OPTIONAL_LF_AFTER_CR: u16 = 0x40;
pub const LENIENT_OPTIONAL_CRLF_AFTER_CHUNK: u16 = 0x80;
pub const LENIENT_OPTIONAL_CR_BEFORE_LF: u16 = 0x100;
pub const LENIENT_SPACES_AFTER_CHUNK_SIZE: u16 = 0x200;

// ---- Structs ----

/// The llhttp parser state. Matches `llhttp__internal_t` in the C header.
#[repr(C)]
pub struct llhttp_t {
  pub _index: i32,
  pub _span_pos0: *mut std::ffi::c_void,
  pub _span_cb0: *mut std::ffi::c_void,
  pub error: c_int,
  pub reason: *const c_char,
  pub error_pos: *const c_char,
  pub data: *mut std::ffi::c_void,
  pub _current: *mut std::ffi::c_void,
  pub content_length: u64,
  pub type_: u8,
  pub method: u8,
  pub http_major: u8,
  pub http_minor: u8,
  pub header_state: u8,
  pub lenient_flags: u16,
  pub upgrade: u8,
  pub finish: u8,
  pub flags: u16,
  pub status_code: u16,
  pub initial_message_completed: u8,
  pub settings: *mut std::ffi::c_void,
}

/// Parser callback settings. Field order must match the C struct exactly.
#[repr(C)]
pub struct llhttp_settings_t {
  pub on_message_begin: llhttp_cb,

  // Data callbacks (interleaved per the C header)
  pub on_protocol: llhttp_data_cb,
  pub on_url: llhttp_data_cb,
  pub on_status: llhttp_data_cb,
  pub on_method: llhttp_data_cb,
  pub on_version: llhttp_data_cb,
  pub on_header_field: llhttp_data_cb,
  pub on_header_value: llhttp_data_cb,
  pub on_chunk_extension_name: llhttp_data_cb,
  pub on_chunk_extension_value: llhttp_data_cb,

  pub on_headers_complete: llhttp_cb,

  pub on_body: llhttp_data_cb,

  pub on_message_complete: llhttp_cb,
  pub on_protocol_complete: llhttp_cb,
  pub on_url_complete: llhttp_cb,
  pub on_status_complete: llhttp_cb,
  pub on_method_complete: llhttp_cb,
  pub on_version_complete: llhttp_cb,
  pub on_header_field_complete: llhttp_cb,
  pub on_header_value_complete: llhttp_cb,
  pub on_chunk_extension_name_complete: llhttp_cb,
  pub on_chunk_extension_value_complete: llhttp_cb,
  pub on_chunk_header: llhttp_cb,
  pub on_chunk_complete: llhttp_cb,
  pub on_reset: llhttp_cb,
}

// ---- Functions ----

unsafe extern "C" {
  // Core
  pub fn llhttp_init(
    parser: *mut llhttp_t,
    type_: llhttp_type_t,
    settings: *const llhttp_settings_t,
  );
  pub fn llhttp_settings_init(settings: *mut llhttp_settings_t);
  pub fn llhttp_execute(
    parser: *mut llhttp_t,
    data: *const c_char,
    len: usize,
  ) -> llhttp_errno_t;
  pub fn llhttp_finish(parser: *mut llhttp_t) -> llhttp_errno_t;
  pub fn llhttp_reset(parser: *mut llhttp_t);

  // Pause/resume
  pub fn llhttp_pause(parser: *mut llhttp_t);
  pub fn llhttp_resume(parser: *mut llhttp_t);
  pub fn llhttp_resume_after_upgrade(parser: *mut llhttp_t);

  // Getters
  pub fn llhttp_get_type(parser: *mut llhttp_t) -> u8;
  pub fn llhttp_get_http_major(parser: *mut llhttp_t) -> u8;
  pub fn llhttp_get_http_minor(parser: *mut llhttp_t) -> u8;
  pub fn llhttp_get_method(parser: *mut llhttp_t) -> u8;
  pub fn llhttp_get_status_code(parser: *mut llhttp_t) -> c_int;
  pub fn llhttp_get_upgrade(parser: *mut llhttp_t) -> u8;
  pub fn llhttp_get_errno(parser: *const llhttp_t) -> llhttp_errno_t;
  pub fn llhttp_get_error_reason(parser: *const llhttp_t) -> *const c_char;
  pub fn llhttp_get_error_pos(parser: *const llhttp_t) -> *const c_char;
  pub fn llhttp_message_needs_eof(parser: *const llhttp_t) -> c_int;
  pub fn llhttp_should_keep_alive(parser: *const llhttp_t) -> c_int;

  // Error helpers
  pub fn llhttp_set_error_reason(parser: *mut llhttp_t, reason: *const c_char);
  pub fn llhttp_errno_name(err: llhttp_errno_t) -> *const c_char;
  pub fn llhttp_method_name(method: llhttp_method_t) -> *const c_char;
  pub fn llhttp_status_name(status: c_int) -> *const c_char;

  // Lenient mode setters
  pub fn llhttp_set_lenient_headers(parser: *mut llhttp_t, enabled: c_int);
  pub fn llhttp_set_lenient_chunked_length(
    parser: *mut llhttp_t,
    enabled: c_int,
  );
  pub fn llhttp_set_lenient_keep_alive(parser: *mut llhttp_t, enabled: c_int);
  pub fn llhttp_set_lenient_transfer_encoding(
    parser: *mut llhttp_t,
    enabled: c_int,
  );
  pub fn llhttp_set_lenient_version(parser: *mut llhttp_t, enabled: c_int);
  pub fn llhttp_set_lenient_data_after_close(
    parser: *mut llhttp_t,
    enabled: c_int,
  );
  pub fn llhttp_set_lenient_optional_lf_after_cr(
    parser: *mut llhttp_t,
    enabled: c_int,
  );
  pub fn llhttp_set_lenient_optional_crlf_after_chunk(
    parser: *mut llhttp_t,
    enabled: c_int,
  );
  pub fn llhttp_set_lenient_optional_cr_before_lf(
    parser: *mut llhttp_t,
    enabled: c_int,
  );
  pub fn llhttp_set_lenient_spaces_after_chunk_size(
    parser: *mut llhttp_t,
    enabled: c_int,
  );
}
