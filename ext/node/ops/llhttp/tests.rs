// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::os::raw::c_int;

use super::*;

/// Helper: parse a complete HTTP request and collect callbacks.
struct ParseResult {
  method: u8,
  http_major: u8,
  http_minor: u8,
  url: String,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
  message_complete: bool,
  keep_alive: bool,
}

struct ParseContext {
  current_header_field: String,
  current_header_value: String,
  result: ParseResult,
}

unsafe fn get_ctx<'a>(parser: *mut llhttp_t) -> &'a mut ParseContext {
  unsafe { &mut *((*parser).data as *mut ParseContext) }
}

unsafe fn get_slice<'a>(at: *const c_char, length: usize) -> &'a [u8] {
  unsafe { std::slice::from_raw_parts(at as *const u8, length) }
}

// C callbacks that store results in ParseContext via parser.data
unsafe extern "C" fn on_message_begin(parser: *mut llhttp_t) -> c_int {
  unsafe { get_ctx(parser) }.result.message_complete = false;
  0
}

unsafe extern "C" fn on_url(
  parser: *mut llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let s = &String::from_utf8_lossy(unsafe { get_slice(at, length) });
  unsafe { get_ctx(parser) }.result.url.push_str(s);
  0
}

unsafe extern "C" fn on_header_field(
  parser: *mut llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let s = &String::from_utf8_lossy(unsafe { get_slice(at, length) });
  unsafe { get_ctx(parser) }.current_header_field.push_str(s);
  0
}

unsafe extern "C" fn on_header_value(
  parser: *mut llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let s = &String::from_utf8_lossy(unsafe { get_slice(at, length) });
  unsafe { get_ctx(parser) }.current_header_value.push_str(s);
  0
}

unsafe extern "C" fn on_header_value_complete(parser: *mut llhttp_t) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  let field = std::mem::take(&mut ctx.current_header_field);
  let value = std::mem::take(&mut ctx.current_header_value);
  ctx.result.headers.push((field, value));
  0
}

unsafe extern "C" fn on_headers_complete(parser: *mut llhttp_t) -> c_int {
  let ctx = unsafe { get_ctx(parser) };
  unsafe {
    ctx.result.method = (*parser).method;
    ctx.result.http_major = (*parser).http_major;
    ctx.result.http_minor = (*parser).http_minor;
    ctx.result.keep_alive = llhttp_should_keep_alive(parser) != 0;
  }
  0
}

unsafe extern "C" fn on_body(
  parser: *mut llhttp_t,
  at: *const c_char,
  length: usize,
) -> c_int {
  let slice = unsafe { get_slice(at, length) };
  unsafe { get_ctx(parser) }
    .result
    .body
    .extend_from_slice(slice);
  0
}

unsafe extern "C" fn on_message_complete(parser: *mut llhttp_t) -> c_int {
  unsafe { get_ctx(parser) }.result.message_complete = true;
  0
}

fn make_settings() -> llhttp_settings_t {
  unsafe {
    let mut settings = std::mem::MaybeUninit::<llhttp_settings_t>::uninit();
    llhttp_settings_init(settings.as_mut_ptr());
    let mut s = settings.assume_init();
    s.on_message_begin = Some(on_message_begin);
    s.on_url = Some(on_url);
    s.on_header_field = Some(on_header_field);
    s.on_header_value = Some(on_header_value);
    s.on_header_value_complete = Some(on_header_value_complete);
    s.on_headers_complete = Some(on_headers_complete);
    s.on_body = Some(on_body);
    s.on_message_complete = Some(on_message_complete);
    s
  }
}

fn new_context() -> ParseContext {
  ParseContext {
    current_header_field: String::new(),
    current_header_value: String::new(),
    result: ParseResult {
      method: 0,
      http_major: 0,
      http_minor: 0,
      url: String::new(),
      headers: Vec::new(),
      body: Vec::new(),
      message_complete: false,
      keep_alive: false,
    },
  }
}

fn parse_request(data: &[u8]) -> ParseResult {
  let settings = make_settings();
  let mut ctx = new_context();

  unsafe {
    let mut parser = std::mem::MaybeUninit::<llhttp_t>::uninit();
    llhttp_init(parser.as_mut_ptr(), HTTP_REQUEST, &settings);
    let parser = parser.assume_init_mut();
    parser.data = &mut ctx as *mut ParseContext as *mut std::ffi::c_void;

    let err =
      llhttp_execute(parser, data.as_ptr() as *const c_char, data.len());
    assert_eq!(err, HPE_OK, "parse error: {err}");
  }

  ctx.result
}

#[test]
fn test_simple_get() {
  let result =
    parse_request(b"GET /hello HTTP/1.1\r\nHost: example.com\r\n\r\n");

  assert_eq!(result.method, HTTP_GET as u8);
  assert_eq!(result.url, "/hello");
  assert_eq!(result.http_major, 1);
  assert_eq!(result.http_minor, 1);
  assert_eq!(result.headers.len(), 1);
  assert_eq!(result.headers[0].0, "Host");
  assert_eq!(result.headers[0].1, "example.com");
  assert!(result.message_complete);
  assert!(result.body.is_empty());
  assert!(result.keep_alive);
}

#[test]
fn test_post_with_body() {
  let result = parse_request(
    b"POST /submit HTTP/1.1\r\nContent-Length: 13\r\n\r\nHello, World!",
  );

  assert_eq!(result.method, HTTP_POST as u8);
  assert_eq!(result.url, "/submit");
  assert_eq!(result.body, b"Hello, World!");
  assert!(result.message_complete);
}

#[test]
fn test_chunked_encoding() {
  let result = parse_request(
    b"POST /data HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nHello\r\n6\r\n World\r\n0\r\n\r\n",
  );

  assert_eq!(result.method, HTTP_POST as u8);
  assert_eq!(result.body, b"Hello World");
  assert!(result.message_complete);
}

#[test]
fn test_response_parsing() {
  let settings = make_settings();
  let mut ctx = new_context();

  let data = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
  unsafe {
    let mut parser = std::mem::MaybeUninit::<llhttp_t>::uninit();
    llhttp_init(parser.as_mut_ptr(), HTTP_RESPONSE, &settings);
    let parser = parser.assume_init_mut();
    parser.data = &mut ctx as *mut ParseContext as *mut std::ffi::c_void;

    let err =
      llhttp_execute(parser, data.as_ptr() as *const c_char, data.len());
    assert_eq!(err, HPE_OK);
    assert_eq!(parser.status_code, 200);
  }

  assert_eq!(ctx.result.body, b"OK");
  assert!(ctx.result.message_complete);
}

#[test]
fn test_method_name() {
  unsafe {
    let name = CStr::from_ptr(llhttp_method_name(HTTP_GET));
    assert_eq!(name.to_str().unwrap(), "GET");

    let name = CStr::from_ptr(llhttp_method_name(HTTP_POST));
    assert_eq!(name.to_str().unwrap(), "POST");
  }
}

#[test]
fn test_errno_name() {
  unsafe {
    let name = CStr::from_ptr(llhttp_errno_name(HPE_OK));
    assert_eq!(name.to_str().unwrap(), "HPE_OK");

    let name = CStr::from_ptr(llhttp_errno_name(HPE_INVALID_METHOD));
    assert_eq!(name.to_str().unwrap(), "HPE_INVALID_METHOD");
  }
}

#[test]
fn test_incremental_parsing() {
  let settings = make_settings();
  let mut ctx = new_context();

  let chunks: &[&[u8]] = &[
    b"GET /pa",
    b"th HTTP/1",
    b".1\r\nHost: ex",
    b"ample.com\r\n\r\n",
  ];

  unsafe {
    let mut parser = std::mem::MaybeUninit::<llhttp_t>::uninit();
    llhttp_init(parser.as_mut_ptr(), HTTP_REQUEST, &settings);
    let parser = parser.assume_init_mut();
    parser.data = &mut ctx as *mut ParseContext as *mut std::ffi::c_void;

    for chunk in chunks {
      let err =
        llhttp_execute(parser, chunk.as_ptr() as *const c_char, chunk.len());
      assert_eq!(err, HPE_OK, "parse error on chunk");
    }
  }

  assert_eq!(ctx.result.url, "/path");
  assert!(ctx.result.message_complete);
}

#[test]
fn test_keep_alive_http10() {
  let result = parse_request(b"GET / HTTP/1.0\r\n\r\n");
  assert!(!result.keep_alive);
}

#[test]
fn test_keep_alive_http11() {
  let result = parse_request(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n");
  assert!(result.keep_alive);
}

#[test]
fn test_multiple_headers() {
  let result = parse_request(
    b"GET / HTTP/1.1\r\nHost: x\r\nAccept: text/html\r\nX-Custom: foo\r\n\r\n",
  );
  assert_eq!(result.headers.len(), 3);
  assert_eq!(result.headers[0], ("Host".into(), "x".into()));
  assert_eq!(result.headers[1], ("Accept".into(), "text/html".into()));
  assert_eq!(result.headers[2], ("X-Custom".into(), "foo".into()));
}
