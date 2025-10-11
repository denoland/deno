// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::op2;
use deno_core::serde_v8;
use deno_core::v8;
use libnghttp2_sys as ffi;
use serde::Serialize;

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

// Stream is not going to have any DATA frames
const STREAM_OPTION_EMPTY_PAYLOAD: i32 = 0x1;
// Stream might have trailing headers
const STREAM_OPTION_GET_TRAILERS: i32 = 0x2;

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

// number of max additional settings, thus settings not implemented by nghttp2
const MAX_ADDITIONAL_SETTINGS: usize = 10;

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
struct Http2State<'a> {
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

impl<'a> Http2State<'a> {
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
) -> Http2State<'a> {
  Http2State::create(scope)
}

pub struct Http2Session {}

unsafe impl deno_core::GarbageCollected for Http2Session {
  fn trace(&self, _: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Http2Session"
  }
}

#[op2]
impl Http2Session {
  #[constructor]
  #[cppgc]
  fn new(_: bool) -> Http2Session {
    Http2Session {}
  }

  #[fast]
  fn settings(&self) -> bool {
    true
  }

  #[fast]
  fn local_settings(&self) {}

  #[fast]
  fn refresh_state(&self) {}
}
