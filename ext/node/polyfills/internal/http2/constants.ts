// Copyright 2018-2026 the Deno authors. MIT license.

// HTTP/2 constants inlined from nghttp2 headers. These are all compile-time
// constants that never change, so there's no reason to fetch them from Rust.

// Header categories
export const NGHTTP2_HCAT_REQUEST = 0;
export const NGHTTP2_HCAT_RESPONSE = 1;
export const NGHTTP2_HCAT_PUSH_RESPONSE = 2;
export const NGHTTP2_HCAT_HEADERS = 3;

// NV flags
export const NGHTTP2_NV_FLAG_NONE = 0;
export const NGHTTP2_NV_FLAG_NO_INDEX = 1;

// Session types
export const NGHTTP2_SESSION_SERVER = 0;
export const NGHTTP2_SESSION_CLIENT = 1;

// Error codes (negative)
export const NGHTTP2_ERR_DEFERRED = -508;
export const NGHTTP2_ERR_STREAM_ID_NOT_AVAILABLE = -509;
export const NGHTTP2_ERR_INVALID_ARGUMENT = -501;
export const NGHTTP2_ERR_STREAM_CLOSED = -510;
export const NGHTTP2_ERR_NOMEM = -901;
export const NGHTTP2_ERR_FRAME_SIZE_ERROR = -522;

// Stream states
export const NGHTTP2_STREAM_STATE_IDLE = 1;
export const NGHTTP2_STREAM_STATE_OPEN = 2;
export const NGHTTP2_STREAM_STATE_RESERVED_LOCAL = 3;
export const NGHTTP2_STREAM_STATE_RESERVED_REMOTE = 4;
export const NGHTTP2_STREAM_STATE_HALF_CLOSED_LOCAL = 5;
export const NGHTTP2_STREAM_STATE_HALF_CLOSED_REMOTE = 6;
export const NGHTTP2_STREAM_STATE_CLOSED = 7;

// Frame flags
export const NGHTTP2_FLAG_NONE = 0;
export const NGHTTP2_FLAG_END_STREAM = 1;
export const NGHTTP2_FLAG_END_HEADERS = 4;
export const NGHTTP2_FLAG_ACK = 1;
export const NGHTTP2_FLAG_PADDED = 8;
export const NGHTTP2_FLAG_PRIORITY = 32;

// Settings IDs
export const NGHTTP2_SETTINGS_HEADER_TABLE_SIZE = 1;
export const NGHTTP2_SETTINGS_ENABLE_PUSH = 2;
export const NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS = 3;
export const NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE = 4;
export const NGHTTP2_SETTINGS_MAX_FRAME_SIZE = 5;
export const NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE = 6;
export const NGHTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL = 8;

// Default settings (RFC 7540)
export const DEFAULT_SETTINGS_HEADER_TABLE_SIZE = 4096;
export const DEFAULT_SETTINGS_ENABLE_PUSH = 1;
export const DEFAULT_SETTINGS_MAX_CONCURRENT_STREAMS = 0xffffffff;
export const DEFAULT_SETTINGS_INITIAL_WINDOW_SIZE = 65535;
export const DEFAULT_SETTINGS_MAX_FRAME_SIZE = 16384;
export const DEFAULT_SETTINGS_MAX_HEADER_LIST_SIZE = 0xffffffff;
export const DEFAULT_SETTINGS_ENABLE_CONNECT_PROTOCOL = 0;

// Frame size limits
export const MAX_MAX_FRAME_SIZE = 16777215;
export const MIN_MAX_FRAME_SIZE = 16384;
export const MAX_INITIAL_WINDOW_SIZE = 2147483647;

// Padding strategies
export const PADDING_STRATEGY_NONE = 0;
export const PADDING_STRATEGY_ALIGNED = 1;
export const PADDING_STRATEGY_MAX = 2;
export const PADDING_STRATEGY_CALLBACK = 3;

// HTTP/2 error codes (for RST_STREAM/GOAWAY)
export const NGHTTP2_NO_ERROR = 0;
export const NGHTTP2_PROTOCOL_ERROR = 1;
export const NGHTTP2_INTERNAL_ERROR = 2;
export const NGHTTP2_FLOW_CONTROL_ERROR = 3;
export const NGHTTP2_SETTINGS_TIMEOUT = 4;
export const NGHTTP2_STREAM_CLOSED = 5;
export const NGHTTP2_FRAME_SIZE_ERROR = 6;
export const NGHTTP2_REFUSED_STREAM = 7;
export const NGHTTP2_CANCEL = 8;
export const NGHTTP2_COMPRESSION_ERROR = 9;
export const NGHTTP2_CONNECT_ERROR = 10;
export const NGHTTP2_ENHANCE_YOUR_CALM = 11;
export const NGHTTP2_INADEQUATE_SECURITY = 12;
export const NGHTTP2_HTTP_1_1_REQUIRED = 13;

// Settings field IDs (aliases used in updateSettingsBuffer)
export const HEADER_TABLE_SIZE = 1;
export const ENABLE_PUSH = 2;
export const MAX_CONCURRENT_STREAMS = 3;
export const INITIAL_WINDOW_SIZE = 4;
export const MAX_FRAME_SIZE = 5;
export const MAX_HEADER_LIST_SIZE = 6;
export const ENABLE_CONNECT_PROTOCOL = 8;

// Stream options
export const STREAM_OPTION_EMPTY_PAYLOAD = 0x1;
export const STREAM_OPTION_GET_TRAILERS = 0x2;

// Default weight
export const NGHTTP2_DEFAULT_WEIGHT = 16;

// HTTP/2 header names
export const HTTP2_HEADER_STATUS = ":status";
export const HTTP2_HEADER_METHOD = ":method";
export const HTTP2_HEADER_AUTHORITY = ":authority";
export const HTTP2_HEADER_SCHEME = ":scheme";
export const HTTP2_HEADER_PATH = ":path";
export const HTTP2_HEADER_PROTOCOL = ":protocol";
export const HTTP2_HEADER_ACCESS_CONTROL_ALLOW_CREDENTIALS =
  "access-control-allow-credentials";
export const HTTP2_HEADER_ACCESS_CONTROL_MAX_AGE = "access-control-max-age";
export const HTTP2_HEADER_ACCESS_CONTROL_REQUEST_METHOD =
  "access-control-request-method";
export const HTTP2_HEADER_AGE = "age";
export const HTTP2_HEADER_AUTHORIZATION = "authorization";
export const HTTP2_HEADER_CONTENT_ENCODING = "content-encoding";
export const HTTP2_HEADER_CONTENT_LANGUAGE = "content-language";
export const HTTP2_HEADER_CONTENT_LENGTH = "content-length";
export const HTTP2_HEADER_CONTENT_LOCATION = "content-location";
export const HTTP2_HEADER_CONTENT_MD5 = "content-md5";
export const HTTP2_HEADER_CONTENT_RANGE = "content-range";
export const HTTP2_HEADER_CONTENT_TYPE = "content-type";
export const HTTP2_HEADER_COOKIE = "cookie";
export const HTTP2_HEADER_DATE = "date";
export const HTTP2_HEADER_DNT = "dnt";
export const HTTP2_HEADER_ETAG = "etag";
export const HTTP2_HEADER_EXPIRES = "expires";
export const HTTP2_HEADER_FROM = "from";
export const HTTP2_HEADER_HOST = "host";
export const HTTP2_HEADER_IF_MATCH = "if-match";
export const HTTP2_HEADER_IF_NONE_MATCH = "if-none-match";
export const HTTP2_HEADER_IF_MODIFIED_SINCE = "if-modified-since";
export const HTTP2_HEADER_IF_RANGE = "if-range";
export const HTTP2_HEADER_IF_UNMODIFIED_SINCE = "if-unmodified-since";
export const HTTP2_HEADER_LAST_MODIFIED = "last-modified";
export const HTTP2_HEADER_LOCATION = "location";
export const HTTP2_HEADER_MAX_FORWARDS = "max-forwards";
export const HTTP2_HEADER_PROXY_AUTHORIZATION = "proxy-authorization";
export const HTTP2_HEADER_RANGE = "range";
export const HTTP2_HEADER_REFERER = "referer";
export const HTTP2_HEADER_RETRY_AFTER = "retry-after";
export const HTTP2_HEADER_SET_COOKIE = "set-cookie";
export const HTTP2_HEADER_TK = "tk";
export const HTTP2_HEADER_UPGRADE_INSECURE_REQUESTS =
  "upgrade-insecure-requests";
export const HTTP2_HEADER_USER_AGENT = "user-agent";
export const HTTP2_HEADER_X_CONTENT_TYPE_OPTIONS = "x-content-type-options";
export const HTTP2_HEADER_CONNECTION = "connection";
export const HTTP2_HEADER_UPGRADE = "upgrade";
export const HTTP2_HEADER_HTTP2_SETTINGS = "http2-settings";
export const HTTP2_HEADER_TE = "te";
export const HTTP2_HEADER_TRANSFER_ENCODING = "transfer-encoding";
export const HTTP2_HEADER_KEEP_ALIVE = "keep-alive";
export const HTTP2_HEADER_PROXY_CONNECTION = "proxy-connection";
export const HTTP2_HEADER_ACCEPT = "accept";
export const HTTP2_HEADER_ACCEPT_ENCODING = "accept-encoding";
export const HTTP2_HEADER_ACCEPT_LANGUAGE = "accept-language";
export const HTTP2_HEADER_ACCEPT_RANGES = "accept-ranges";

// HTTP methods
export const HTTP2_METHOD_CONNECT = "CONNECT";
export const HTTP2_METHOD_DELETE = "DELETE";
export const HTTP2_METHOD_GET = "GET";
export const HTTP2_METHOD_HEAD = "HEAD";

// HTTP status codes
export const HTTP_STATUS_CONTINUE = 100;
export const HTTP_STATUS_SWITCHING_PROTOCOLS = 101;
export const HTTP_STATUS_EARLY_HINTS = 103;
export const HTTP_STATUS_OK = 200;
export const HTTP_STATUS_NO_CONTENT = 204;
export const HTTP_STATUS_RESET_CONTENT = 205;
export const HTTP_STATUS_NOT_MODIFIED = 304;
export const HTTP_STATUS_METHOD_NOT_ALLOWED = 405;
export const HTTP_STATUS_EXPECTATION_FAILED = 417;
export const HTTP_STATUS_MISDIRECTED_REQUEST = 421;
