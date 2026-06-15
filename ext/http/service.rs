// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::ffi::c_void;
use std::future::Future;
use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::task::ready;

use deno_core::BufView;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::futures::task::AtomicWaker;
use deno_core::serde_v8;
use deno_core::v8;
use deno_error::JsErrorBox;
use http::request::Parts;
use hyper::body::Body;
use hyper::body::Frame;
use hyper::body::Incoming;
use hyper::body::SizeHint;
use hyper::header::HeaderMap;
use hyper::upgrade::OnUpgrade;
use scopeguard::ScopeGuard;
use scopeguard::guard;
use tokio::sync::oneshot;

use crate::OtelInfo;
use crate::OtelInfoAttributes;
use crate::request_body::BufferedIncoming;
use crate::request_properties::HttpConnectionProperties;
use crate::response_body::ResponseBytesInner;
use crate::response_body::ResponseStreamResult;
use crate::v8_util::v8_string_to_utf8_bytes;

pub type Request = hyper::Request<Incoming>;
pub type Response = hyper::Response<HttpRecordResponse>;

#[cfg(feature = "__http_tracing")]
pub static RECORD_COUNT: std::sync::atomic::AtomicUsize =
  std::sync::atomic::AtomicUsize::new(0);

macro_rules! http_general_trace {
  ($($args:expr),*) => {
    #[cfg(feature = "__http_tracing")]
    {
      let count = $crate::service::RECORD_COUNT
        .load(std::sync::atomic::Ordering::SeqCst);

      println!(
        "HTTP [+{count}]: {}",
        format!($($args),*),
      );
    }
  };
}

macro_rules! http_trace {
  ($record:expr $(, $args:expr)*) => {
    #[cfg(feature = "__http_tracing")]
    {
      let count = $crate::service::RECORD_COUNT
        .load(std::sync::atomic::Ordering::SeqCst);

      println!(
        "HTTP [+{count}] id={:p} strong={}: {}",
        $record,
        std::rc::Rc::strong_count(&$record),
        format!($($args),*),
      );
    }
  };
}

pub(crate) use http_general_trace;
#[cfg(feature = "__http_tracing")]
pub(crate) use http_trace;

pub(crate) struct HttpServerStateInner {
  pool: Vec<(Rc<HttpRecord>, HeaderMap)>,
}

/// A signalling version of `Rc` that allows one to poll for when all other references
/// to the `Rc` have been dropped.
#[repr(transparent)]
pub(crate) struct SignallingRc<T>(Rc<(T, Cell<Option<Waker>>)>);

impl<T> SignallingRc<T> {
  #[inline]
  pub fn new(t: T) -> Self {
    Self(Rc::new((t, Default::default())))
  }

  #[inline]
  pub fn strong_count(&self) -> usize {
    Rc::strong_count(&self.0)
  }

  /// Resolves when this is the only remaining reference.
  #[inline]
  pub fn poll_complete(&self, cx: &mut Context<'_>) -> Poll<()> {
    if Rc::strong_count(&self.0) == 1 {
      Poll::Ready(())
    } else {
      self.0.1.set(Some(cx.waker().clone()));
      Poll::Pending
    }
  }
}

impl<T> Clone for SignallingRc<T> {
  #[inline]
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> Drop for SignallingRc<T> {
  #[inline]
  fn drop(&mut self) {
    // Trigger the waker iff the refcount is about to become 1.
    if Rc::strong_count(&self.0) == 2
      && let Some(waker) = self.0.1.take()
    {
      waker.wake();
    }
  }
}

impl<T> std::ops::Deref for SignallingRc<T> {
  type Target = T;
  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.0.0
  }
}

pub(crate) struct HttpServerState(RefCell<HttpServerStateInner>);

impl HttpServerState {
  pub fn new() -> SignallingRc<Self> {
    SignallingRc::new(Self(RefCell::new(HttpServerStateInner {
      pool: Vec::new(),
    })))
  }
}

impl std::ops::Deref for HttpServerState {
  type Target = RefCell<HttpServerStateInner>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// Holds the v8 callback registered by the JavaScript side via
/// `op_http_serve` along with the isolate/context required to invoke
/// it. This lets the hyper service synchronously call back into
/// JavaScript when a new request arrives, instead of routing every
/// request through an mpsc channel that the JS side has to drain in
/// a separate task.
pub struct ServerCallback {
  isolate_ptr: v8::UnsafeRawIsolatePtr,
  context: v8::Global<v8::Context>,
  callback: v8::Global<v8::Function>,
  native_callback: v8::Global<v8::Function>,
  serve_native_response_key: v8::Global<v8::Value>,
  serve_fast_status_key: v8::Global<v8::Value>,
  serve_fast_body_key: v8::Global<v8::Value>,
  serve_fast_header_kind_key: v8::Global<v8::Value>,
  serve_fast_content_type_key: v8::Global<v8::Value>,
  serve_fast_consumed_key: v8::Global<v8::Value>,
  raw_no_request: bool,
  runtime_waker: Arc<AtomicWaker>,
}

pub enum DirectResponseBody {
  Empty,
  Bytes(BufView),
}

pub enum DirectResponseHeaders {
  None,
  DefaultText,
  ContentType(Vec<u8>),
  List(Vec<(Vec<u8>, Vec<u8>)>),
}

pub struct DirectResponse {
  pub status: u16,
  pub headers: DirectResponseHeaders,
  pub body: DirectResponseBody,
}

pub struct NativeResponseCell(RefCell<Option<DirectResponse>>);

impl NativeResponseCell {
  #[allow(
    clippy::new_ret_no_self,
    reason = "returns an opaque pointer consumed by JS finalizers"
  )]
  pub fn new(response: DirectResponse) -> *const c_void {
    Rc::into_raw(Rc::new(Self(RefCell::new(Some(response))))) as *const c_void
  }

  /// # Safety
  ///
  /// `ptr` must have been returned by [`NativeResponseCell::new`], and the
  /// associated JS finalizer must still own one `Rc` strong reference.
  pub unsafe fn take(ptr: *const c_void) -> Option<DirectResponse> {
    if ptr.is_null() {
      return None;
    }
    // SAFETY: caller guarantees this is a valid NativeResponseCell pointer.
    let rc = unsafe { Rc::from_raw(ptr as *const Self) };
    let response = rc.0.borrow_mut().take();
    let _ = Rc::into_raw(rc);
    response
  }

  /// # Safety
  ///
  /// `ptr` must have been returned by [`NativeResponseCell::new`] and must be
  /// dropped exactly once for the JS-owned strong reference.
  pub unsafe fn drop(ptr: *const c_void) {
    if ptr.is_null() {
      return;
    }
    // SAFETY: caller guarantees this is the JS-owned strong reference.
    drop(unsafe { Rc::from_raw(ptr as *const Self) });
  }
}

impl ServerCallback {
  #[allow(
    clippy::too_many_arguments,
    reason = "captures JS callback and fast-response symbols in one place"
  )]
  pub fn new(
    scope: &mut v8::PinScope<'_, '_>,
    isolate: &mut v8::Isolate,
    callback: v8::Global<v8::Function>,
    native_callback: v8::Global<v8::Function>,
    serve_native_response_key: v8::Global<v8::Value>,
    serve_fast_status_key: v8::Global<v8::Value>,
    serve_fast_body_key: v8::Global<v8::Value>,
    serve_fast_header_kind_key: v8::Global<v8::Value>,
    serve_fast_content_type_key: v8::Global<v8::Value>,
    serve_fast_consumed_key: v8::Global<v8::Value>,
    raw_no_request: bool,
    runtime_waker: Arc<AtomicWaker>,
  ) -> Self {
    let ctx = scope.get_current_context();
    let context = v8::Global::new(scope, ctx);
    // SAFETY: isolate is a valid live isolate.
    let isolate_ptr = unsafe { isolate.as_raw_isolate_ptr() };
    Self {
      isolate_ptr,
      context,
      callback,
      native_callback,
      serve_native_response_key,
      serve_fast_status_key,
      serve_fast_body_key,
      serve_fast_header_kind_key,
      serve_fast_content_type_key,
      serve_fast_consumed_key,
      raw_no_request,
      runtime_waker,
    }
  }

  pub fn raw_no_request(&self) -> bool {
    self.raw_no_request
  }

  fn direct_response_body_from_v8<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    body_value: v8::Local<'s, v8::Value>,
  ) -> Option<DirectResponseBody> {
    if body_value.is_null_or_undefined() {
      Some(DirectResponseBody::Empty)
    } else if let Ok(text) = v8::Local::<v8::String>::try_from(body_value) {
      Some(DirectResponseBody::Bytes(BufView::from(
        v8_string_to_utf8_bytes(scope, text),
      )))
    } else {
      let buffer = serde_v8::from_v8::<JsBuffer>(scope, body_value).ok()?;
      Some(DirectResponseBody::Bytes(BufView::from(buffer)))
    }
  }

  fn direct_response_from_response_v8<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    value: v8::Local<'s, v8::Value>,
  ) -> Option<DirectResponse> {
    let response = v8::Local::<v8::Object>::try_from(value).ok()?;

    let native_key = v8::Local::new(scope, &self.serve_native_response_key);
    let native = response.get(scope, native_key)?;
    if let Ok(native) = v8::Local::<v8::External>::try_from(native) {
      // SAFETY: this external is created by op_http_new_response_native_*.
      let response_value = unsafe { NativeResponseCell::take(native.value()) };
      let null = v8::null(scope);
      let _ = response.set(scope, native_key, null.into());
      if response_value.is_some() {
        let consumed_key = v8::Local::new(scope, &self.serve_fast_consumed_key);
        let consumed = v8::Boolean::new(scope, true);
        let _ = response.set(scope, consumed_key, consumed.into());
        return response_value;
      }
    }

    let consumed_key = v8::Local::new(scope, &self.serve_fast_consumed_key);
    let consumed = response.get(scope, consumed_key)?;
    if consumed.boolean_value(scope) {
      return None;
    }

    let status_key = v8::Local::new(scope, &self.serve_fast_status_key);
    let status = response.get(scope, status_key)?.uint32_value(scope)? as u16;
    if status == 0 {
      return None;
    }

    let body_key = v8::Local::new(scope, &self.serve_fast_body_key);
    let body = Self::direct_response_body_from_v8(
      scope,
      response.get(scope, body_key)?,
    )?;

    let header_kind_key =
      v8::Local::new(scope, &self.serve_fast_header_kind_key);
    let header_kind =
      response.get(scope, header_kind_key)?.uint32_value(scope)?;
    let headers = match header_kind {
      0 => DirectResponseHeaders::None,
      1 => DirectResponseHeaders::DefaultText,
      2 => {
        let content_type_key =
          v8::Local::new(scope, &self.serve_fast_content_type_key);
        let content_type = response.get(scope, content_type_key)?;
        let content_type = v8::Local::<v8::String>::try_from(content_type)
          .ok()
          .map(|text| v8_string_to_utf8_bytes(scope, text))?;
        DirectResponseHeaders::ContentType(content_type)
      }
      _ => return None,
    };

    let consumed = v8::Boolean::new(scope, true);
    if !response
      .set(scope, consumed_key, consumed.into())
      .unwrap_or(false)
    {
      return None;
    }

    Some(DirectResponse {
      status,
      headers,
      body,
    })
  }

  /// Invoke the JavaScript native-response callback. The callback either
  /// returns a Response object that can be extracted natively, or completes the
  /// record through the regular JS response ops and returns undefined.
  pub unsafe fn dispatch_native_response(
    &self,
    record_ptr: *mut std::ffi::c_void,
  ) -> Option<DirectResponse> {
    // SAFETY: caller upholds isolate validity.
    unsafe {
      let mut isolate = v8::Isolate::from_raw_isolate_ptr(self.isolate_ptr);
      v8::scope!(let handle_scope, &mut isolate);
      let context = v8::Local::new(handle_scope, &self.context);
      let scope = &mut v8::ContextScope::new(handle_scope, context);
      let pin_scope: &mut v8::PinScope = scope;

      let response = {
        v8::tc_scope!(tc, pin_scope);
        let cb = v8::Local::new(tc, &self.native_callback);
        let arg = v8::External::new(tc, record_ptr);
        let recv = v8::undefined(tc);
        let value = cb.call(tc, recv.into(), &[arg.into()]);
        if tc.has_caught() {
          tc.reset();
          None
        } else if let Some(mut value) = value {
          let mut pending_or_rejected = false;
          if let Ok(promise) = v8::Local::<v8::Promise>::try_from(value) {
            tc.perform_microtask_checkpoint();
            match promise.state() {
              v8::PromiseState::Fulfilled => {
                value = promise.result(tc);
              }
              v8::PromiseState::Pending | v8::PromiseState::Rejected => {
                pending_or_rejected = true;
              }
            }
          }
          if pending_or_rejected || value.is_undefined() || value.is_null() {
            None
          } else {
            self.direct_response_from_response_v8(tc, value)
          }
        } else {
          None
        }
      };
      // The native callback can complete the response synchronously and return
      // undefined. Drain reactions queued by user code in the handler before
      // returning to the raw h1 task; otherwise tests/user code awaiting a
      // promise resolved inside the handler can sleep until unrelated JS work
      // enters V8.
      pin_scope.perform_microtask_checkpoint();
      self.runtime_waker.wake();
      response
    }
  }

  /// Invoke the JavaScript callback with the given record-pointer
  /// argument. The pointer is wrapped as a v8 External so the JS side
  /// can pass it back to ops that take `*const c_void` parameters.
  ///
  /// # Safety
  /// `record_ptr` must be a valid `ExternalPointer<RcHttpRecord>` raw
  /// pointer that the JS side will eventually consume (or that will
  /// otherwise be reclaimed). The isolate pointed to by
  /// `self.isolate_ptr` must still be live (this is upheld because the
  /// `HttpJoinHandle` holding this `ServerCallback` is dropped as part
  /// of resource teardown which happens before the isolate dies).
  pub unsafe fn dispatch(&self, record_ptr: *mut std::ffi::c_void) {
    // SAFETY: caller upholds isolate validity.
    unsafe {
      let mut isolate = v8::Isolate::from_raw_isolate_ptr(self.isolate_ptr);
      v8::scope!(let handle_scope, &mut isolate);
      let context = v8::Local::new(handle_scope, &self.context);
      let scope = &mut v8::ContextScope::new(handle_scope, context);
      let pin_scope: &mut v8::PinScope = scope;

      {
        v8::tc_scope!(tc, pin_scope);
        let cb = v8::Local::new(tc, &self.callback);
        let arg = v8::External::new(tc, record_ptr);
        let recv = v8::undefined(tc);
        let _ = cb.call(tc, recv.into(), &[arg.into()]);
        // The JS side is responsible for catching its own errors via
        // the wrapper installed in 00_serve.ts. Any exception that
        // escapes is swallowed here -- otherwise we would leave the
        // isolate in an exception-pending state and the next v8 call
        // would crash.
        if tc.has_caught() {
          tc.reset();
        }
      }
      // Drain microtasks queued by the JS callback. Because the
      // hyper service runs as a free-standing tokio task (not
      // registered with the JsRuntime's op driver), the runtime is
      // not woken by request arrival, so any `await`-style microtask
      // continuations in the handler chain would otherwise sit
      // un-drained until some unrelated op happened to fire.
      // Draining here makes the dispatch effectively synchronous up
      // to the first real async-op boundary in user code -- matching
      // how Bun's onRequest invokes its handler.
      pin_scope.perform_microtask_checkpoint();

      // Request arrival is handled by a standalone hyper task, not by
      // an op future owned by JsRuntime. If the handler reached an
      // async boundary above, the full event loop must run at least
      // once to drive timers, ops, and uv-compatible Node I/O.
      self.runtime_waker.wake();
    }
  }
}

enum RequestBodyState {
  Incoming(BufferedIncoming),
  Resource(
    #[allow(dead_code, reason = "prevent drop until variant is dropped")]
    HttpRequestBodyAutocloser,
  ),
}

pub enum FlatResponseBody {
  Empty,
  Bytes(BufView),
}

impl From<BufferedIncoming> for RequestBodyState {
  fn from(value: BufferedIncoming) -> Self {
    RequestBodyState::Incoming(value)
  }
}

/// Ensures that the request body closes itself when no longer needed.
pub struct HttpRequestBodyAutocloser(ResourceId, Rc<RefCell<OpState>>);

impl HttpRequestBodyAutocloser {
  pub fn new(res: ResourceId, op_state: Rc<RefCell<OpState>>) -> Self {
    Self(res, op_state)
  }
}

impl Drop for HttpRequestBodyAutocloser {
  fn drop(&mut self) {
    if let Ok(res) = self.1.borrow_mut().resource_table.take_any(self.0) {
      res.close();
    }
  }
}

#[allow(clippy::collapsible_if, reason = "for logic clarity")]
fn validate_request(req: &Request) -> bool {
  if req.uri() == "*" {
    if req.method() != http::Method::OPTIONS {
      return false;
    }
  } else if req.uri().path().is_empty() {
    if req.method() != http::Method::CONNECT {
      return false;
    }
  }

  if req.method() == http::Method::CONNECT && req.uri().authority().is_none() {
    return false;
  }

  true
}

pub(crate) async fn handle_request<F>(
  request: Request,
  request_info: HttpConnectionProperties,
  server_state: SignallingRc<HttpServerState>, // Keep server alive for duration of this future.
  dispatch: F,
  legacy_abort: bool,
  automatic_compression: bool,
) -> Result<Response, hyper::Error>
where
  F: FnOnce(Rc<HttpRecord>),
{
  if !validate_request(&request) {
    let mut response = Response::new(HttpRecordResponse::empty());
    *response.version_mut() = request.version();
    *response.status_mut() = http::StatusCode::BAD_REQUEST;
    return Ok(response);
  }

  let otel_info = if let Some(otel) = deno_telemetry::OTEL_GLOBALS
    .get()
    .filter(|o| o.has_metrics())
  {
    let instant = std::time::Instant::now();
    let size_hint = request.size_hint();
    Some(OtelInfo::new(
      otel,
      instant,
      size_hint.upper().unwrap_or(size_hint.lower()),
      OtelInfoAttributes {
        http_request_method: OtelInfoAttributes::method(request.method()),
        url_scheme: request
          .uri()
          .scheme_str()
          .map(|s| Cow::Owned(s.to_string()))
          .unwrap_or_else(|| Cow::Borrowed("http")),
        network_protocol_version: OtelInfoAttributes::version(
          request.version(),
        ),
        server_address: request.uri().host().map(|host| host.to_string()),
        server_port: request.uri().port_u16().map(|port| port as i64),
        error_type: Default::default(),
        http_route: None,
        http_response_status_code: Default::default(),
      },
    ))
  } else {
    None
  };

  // If the underlying TCP connection is closed, this future will be dropped
  // and execution could stop at any await point.
  // The HttpRecord must live until JavaScript is done processing so is wrapped
  // in an Rc. The guard ensures unneeded resources are freed at cancellation.
  let guarded_record = guard(
    HttpRecord::new(
      request,
      request_info,
      server_state,
      otel_info,
      legacy_abort,
      automatic_compression,
    ),
    HttpRecord::cancel,
  );

  // Synchronously dispatch to the registered JavaScript callback.
  // The dispatch closure handles wrapping the record as a V8 external
  // and invoking the user-supplied function under a HandleScope. The
  // JS side is expected to call op_http_set_response_* eventually,
  // which fires `record.complete()` and wakes the response_ready
  // future below.
  dispatch(guarded_record.clone());

  // Wait for JavaScript handler to return request.
  http_trace!(*guarded_record, "handle_request response_ready.await");
  guarded_record.response_ready().await;

  // Defuse the guard. Must not await after this point.
  let record = ScopeGuard::into_inner(guarded_record);
  http_trace!(record, "handle_request complete");
  let response = record.into_response();
  Ok(response)
}

#[derive(Debug, thiserror::Error)]
#[error("upgrade unavailable")]
pub struct UpgradeUnavailableError;

struct HttpRecordInner {
  server_state: SignallingRc<HttpServerState>,
  closed_channel: Option<oneshot::Sender<()>>,
  request_info: HttpConnectionProperties,
  request_parts: http::request::Parts,
  request_body: Option<RequestBodyState>,
  response_parts: Option<http::response::Parts>,
  response_ready: bool,
  response_waker: Option<Waker>,
  response_body: ResponseBytesInner,
  flat_response_body: Option<FlatResponseBody>,
  response_body_finished: bool,
  response_body_waker: Option<Waker>,
  trailers: Option<HeaderMap>,
  been_dropped: bool,
  finished: bool,
  needs_close_after_finish: bool,
  legacy_abort: bool,
  automatic_compression: bool,
  otel_info: Option<OtelInfo>,
  client_addr: Option<http::HeaderValue>,
}

pub struct HttpRecord(RefCell<Option<HttpRecordInner>>);

#[cfg(feature = "__http_tracing")]
impl Drop for HttpRecord {
  fn drop(&mut self) {
    RECORD_COUNT
      .fetch_sub(1, std::sync::atomic::Ordering::SeqCst)
      .checked_sub(1)
      .expect("Count went below zero");
    http_general_trace!("HttpRecord::drop");
  }
}

pub(crate) fn trust_proxy_headers() -> bool {
  static TRUST_PROXY_HEADERS: OnceLock<bool> = OnceLock::new();

  static VAR_NAME: &str = "DENO_TRUST_PROXY_HEADERS";

  *TRUST_PROXY_HEADERS.get_or_init(|| {
    if let Some(v) = std::env::var_os(VAR_NAME) {
      // SAFETY: called once during single-threaded init via OnceLock
      unsafe { std::env::remove_var(VAR_NAME) };
      v == "1"
    } else {
      false
    }
  })
}

impl HttpRecord {
  fn new(
    request: Request,
    request_info: HttpConnectionProperties,
    server_state: SignallingRc<HttpServerState>,
    otel_info: Option<OtelInfo>,
    legacy_abort: bool,
    automatic_compression: bool,
  ) -> Rc<Self> {
    let (mut request_parts, request_body) = request.into_parts();
    let client_addr = if trust_proxy_headers() {
      request_parts.headers.remove("x-deno-client-address")
    } else {
      None
    };
    let request_body = Some(BufferedIncoming::new(request_body).into());

    let (mut response_parts, _) = http::Response::new(()).into_parts();
    let record = match server_state.borrow_mut().pool.pop() {
      Some((record, headers)) => {
        response_parts.headers = headers;
        http_trace!(record, "HttpRecord::reuse");
        record
      }
      _ => {
        #[cfg(feature = "__http_tracing")]
        {
          RECORD_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        #[allow(clippy::let_and_return, reason = "depends on cfg")]
        let record = Rc::new(Self(RefCell::new(None)));
        http_trace!(record, "HttpRecord::new");
        record
      }
    };
    *record.0.borrow_mut() = Some(HttpRecordInner {
      server_state,
      request_info,
      request_parts,
      request_body,
      response_parts: Some(response_parts),
      response_ready: false,
      response_waker: None,
      response_body: ResponseBytesInner::Empty,
      flat_response_body: None,
      response_body_finished: false,
      response_body_waker: None,
      trailers: None,
      closed_channel: None,
      been_dropped: false,
      finished: false,
      legacy_abort,
      automatic_compression,
      needs_close_after_finish: false,
      otel_info,
      client_addr,
    });
    record
  }

  fn finish(self: Rc<Self>) {
    http_trace!(self, "HttpRecord::finish");
    let mut inner = self.self_mut();
    inner.response_body_finished = true;
    let response_body_waker = inner.response_body_waker.take();
    let needs_close_after_finish = inner.needs_close_after_finish;
    drop(inner);
    if let Some(waker) = response_body_waker {
      waker.wake();
    }
    if !needs_close_after_finish {
      self.recycle();
    }
  }

  pub fn close_after_finish(self: Rc<Self>) {
    debug_assert!(self.self_ref().needs_close_after_finish);
    let mut inner = self.self_mut();
    inner.needs_close_after_finish = false;
    if !inner.finished {
      drop(inner);
      self.recycle();
    }
  }

  pub fn needs_close_after_finish(&self) -> RefMut<'_, bool> {
    RefMut::map(self.self_mut(), |inner| &mut inner.needs_close_after_finish)
  }

  pub fn on_cancel(&self, sender: oneshot::Sender<()>) {
    self.self_mut().closed_channel = Some(sender);
  }

  fn recycle(self: Rc<Self>) {
    assert!(
      Rc::strong_count(&self) == 1,
      "HTTP state error: Expected to be last strong reference"
    );
    let HttpRecordInner {
      server_state,
      request_parts: Parts { mut headers, .. },
      ..
    } = self.0.borrow_mut().take().unwrap();

    let inflight = server_state.strong_count();
    http_trace!(self, "HttpRecord::recycle inflight={}", inflight);

    // Keep a buffer of allocations on hand to be reused by incoming requests.
    // Estimated target size is 16 + 1/8 the number of inflight requests.
    let target = 16 + (inflight >> 3);
    let pool = &mut server_state.borrow_mut().pool;
    if target > pool.len() {
      headers.clear();
      pool.push((self, headers));
    } else if target < pool.len() - 8 {
      pool.truncate(target);
    }
  }

  fn self_ref(&self) -> Ref<'_, HttpRecordInner> {
    Ref::map(self.0.borrow(), |option| option.as_ref().unwrap())
  }

  fn self_mut(&self) -> RefMut<'_, HttpRecordInner> {
    RefMut::map(self.0.borrow_mut(), |option| option.as_mut().unwrap())
  }

  /// Perform the Hyper upgrade on this record.
  pub fn upgrade(&self) -> Result<OnUpgrade, UpgradeUnavailableError> {
    // Manually perform the upgrade. We're peeking into hyper's underlying machinery here a bit
    self
      .self_mut()
      .request_parts
      .extensions
      .remove::<OnUpgrade>()
      .ok_or(UpgradeUnavailableError)
  }

  /// Take the Hyper body from this record.
  pub fn take_request_body(&self) -> Option<BufferedIncoming> {
    let body_holder = &mut self.self_mut().request_body;
    let body = body_holder.take();
    match body {
      Some(RequestBodyState::Incoming(body)) => Some(body),
      x => {
        *body_holder = x;
        None
      }
    }
  }

  /// Try to drain the request body in one shot without ever
  /// blocking. Returns `Some(bytes)` iff the entire body is
  /// already buffered in hyper at the time of the call. On
  /// `None` the body is left intact for the streaming path
  /// (any frames that were polled out are kept in the wrapper
  /// and replayed transparently before further reads).
  pub fn try_take_full_request_body(&self) -> Option<Vec<u8>> {
    let body_holder = &mut self.self_mut().request_body;
    if let Some(RequestBodyState::Incoming(body)) = body_holder.as_mut() {
      body.try_take_full()
    } else {
      None
    }
  }

  /// Replace the request body with a resource ID and the OpState we'll need to shut it down.
  /// We cannot keep just the resource itself, as JS code might be reading from the resource ID
  /// to generate the response data (requiring us to keep it in the resource table).
  pub fn put_resource(&self, res: HttpRequestBodyAutocloser) {
    self.self_mut().request_body = Some(RequestBodyState::Resource(res));
  }

  /// Cleanup resources not needed after the future is dropped.
  fn cancel(self: Rc<Self>) {
    http_trace!(self, "HttpRecord::cancel");
    let mut inner = self.self_mut();
    if inner.response_ready {
      // Future dropped between wake() and async fn resuming.
      drop(inner);
      self.finish();
      return;
    }
    inner.been_dropped = true;
    // The request body might include actual resources.
    inner.request_body.take();

    if (inner.legacy_abort || !inner.response_body_finished)
      && let Some(closed_channel) = inner.closed_channel.take()
    {
      let _ = closed_channel.send(());
    }
  }

  /// Complete this record, potentially expunging it if it is fully complete (ie: cancelled as well).
  pub fn complete(self: Rc<Self>) {
    http_trace!(self, "HttpRecord::complete");
    let mut inner = self.self_mut();
    assert!(
      !inner.response_ready,
      "HTTP state error: Entry has already been completed"
    );
    if inner.been_dropped {
      drop(inner);
      self.finish();
      return;
    }
    inner.response_ready = true;
    if let Some(waker) = inner.response_waker.take() {
      drop(inner);
      waker.wake();
    }
  }

  fn take_response_body(&self) -> ResponseBytesInner {
    let mut inner = self.self_mut();
    debug_assert!(
      !matches!(inner.response_body, ResponseBytesInner::Done),
      "HTTP state error: response body already complete"
    );
    std::mem::replace(&mut inner.response_body, ResponseBytesInner::Done)
  }

  /// Has the future for this record been dropped? ie, has the underlying TCP connection
  /// been closed?
  pub fn cancelled(&self) -> bool {
    self.self_ref().been_dropped
  }

  /// Get a mutable reference to the response status and headers.
  pub fn response_parts(&self) -> RefMut<'_, http::response::Parts> {
    RefMut::map(self.self_mut(), |inner| {
      inner.response_parts.as_mut().unwrap()
    })
  }

  /// Get a mutable reference to the trailers.
  pub fn trailers(&self) -> RefMut<'_, Option<HeaderMap>> {
    RefMut::map(self.self_mut(), |inner| &mut inner.trailers)
  }

  pub fn set_response_body(&self, response_body: ResponseBytesInner) {
    let mut inner = self.self_mut();
    debug_assert!(matches!(inner.response_body, ResponseBytesInner::Empty));
    debug_assert!(inner.flat_response_body.is_none());
    inner.response_body = response_body;
  }

  pub fn set_flat_response_body(&self, response_body: FlatResponseBody) {
    let mut inner = self.self_mut();
    debug_assert!(matches!(inner.response_body, ResponseBytesInner::Empty));
    debug_assert!(inner.flat_response_body.is_none());
    inner.flat_response_body = Some(response_body);
  }

  /// Take the response.
  fn into_response(self: Rc<Self>) -> Response {
    let mut inner = self.self_mut();
    let parts = inner.response_parts.take().unwrap();
    let flat_response_body = inner.flat_response_body.take();
    drop(inner);
    let body = match flat_response_body {
      Some(body) => HttpRecordResponse::Flat {
        record: Some(ManuallyDrop::new(self)),
        body,
      },
      None => HttpRecordResponse::Record(Some(ManuallyDrop::new(self))),
    };
    Response::from_parts(parts, body)
  }

  /// Get a reference to the connection properties.
  pub fn request_info(&self) -> Ref<'_, HttpConnectionProperties> {
    Ref::map(self.self_ref(), |inner| &inner.request_info)
  }

  /// Get a reference to the request parts.
  pub fn request_parts(&self) -> Ref<'_, Parts> {
    Ref::map(self.self_ref(), |inner| &inner.request_parts)
  }

  pub fn automatic_compression(&self) -> bool {
    self.self_ref().automatic_compression
  }

  /// Resolves when response head is ready.
  fn response_ready(&self) -> impl Future<Output = ()> + '_ {
    struct HttpRecordReady<'a>(&'a HttpRecord);

    impl Future for HttpRecordReady<'_> {
      type Output = ();

      fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
      ) -> Poll<Self::Output> {
        let mut mut_self = self.0.self_mut();
        if mut_self.response_ready {
          mut_self.otel_info.take();
          return Poll::Ready(());
        }
        mut_self.response_waker = Some(cx.waker().clone());
        Poll::Pending
      }
    }

    HttpRecordReady(self)
  }

  /// Resolves when response body has finished streaming. Returns true if the
  /// response completed.
  pub fn response_body_finished(&self) -> impl Future<Output = bool> + '_ {
    struct HttpRecordFinished<'a>(&'a HttpRecord);

    impl Future for HttpRecordFinished<'_> {
      type Output = bool;

      fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
      ) -> Poll<Self::Output> {
        let mut mut_self = self.0.self_mut();
        if mut_self.response_body_finished {
          // If we sent the response body and the trailers, this body completed successfully
          return Poll::Ready(
            mut_self.response_body.is_complete() && mut_self.trailers.is_none(),
          );
        }
        mut_self.response_body_waker = Some(cx.waker().clone());
        Poll::Pending
      }
    }

    HttpRecordFinished(self)
  }

  pub fn otel_info_set_status(&self, status: u16) {
    let mut inner = self.self_mut();
    if let Some(info) = inner.otel_info.as_mut() {
      info.attributes.http_response_status_code = Some(status as _);
      info.handle_duration_and_request_size();
    }
  }

  pub fn otel_info_set_error(&self, error: &'static str) {
    let mut inner = self.self_mut();
    if let Some(info) = inner.otel_info.as_mut() {
      info.attributes.error_type = Some(error);
      info.handle_duration_and_request_size();
    }
  }

  /// Copy relevant attributes (like `http.route`) from a span to OtelInfo
  /// for metrics.
  pub fn copy_span_to_otel_info(&self, span: &deno_telemetry::OtelSpan) {
    let mut inner = self.self_mut();
    let span_state = span.0.borrow();
    if let deno_telemetry::OtelSpanState::Recording(data) = &**span_state
      && let Some(info) = inner.otel_info.as_mut()
    {
      for attr in &data.attributes {
        if attr.key.as_str() == "http.route" {
          info.attributes.http_route = Some(attr.value.to_string());
        }
      }
    }
  }

  pub fn client_addr(&self) -> Ref<'_, Option<http::HeaderValue>> {
    Ref::map(self.self_ref(), |inner| &inner.client_addr)
  }
}

// `None` variant used when no body is present, for example
// when we want to return a synthetic 400 for invalid requests.
pub enum HttpRecordResponse {
  Empty,
  Record(Option<ManuallyDrop<Rc<HttpRecord>>>),
  Flat {
    record: Option<ManuallyDrop<Rc<HttpRecord>>>,
    body: FlatResponseBody,
  },
}

impl HttpRecordResponse {
  pub fn empty() -> Self {
    Self::Empty
  }
}

impl Body for HttpRecordResponse {
  type Data = BufView;
  type Error = JsErrorBox;

  fn poll_frame(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
    use crate::response_body::PollFrame;
    match self.get_mut() {
      Self::Empty => Poll::Ready(None),
      Self::Flat { record, body } => {
        match std::mem::replace(body, FlatResponseBody::Empty) {
          FlatResponseBody::Empty => Poll::Ready(None),
          FlatResponseBody::Bytes(buf) => {
            if let Some(record) = record {
              let mut http = record.0.borrow_mut();
              if let Some(otel_info) = &mut http.as_mut().unwrap().otel_info
                && let Some(response_size) = &mut otel_info.response_size
              {
                *response_size += buf.len() as u64;
              }
            }
            Poll::Ready(Some(Ok(Frame::data(buf))))
          }
        }
      }
      Self::Record(record) => {
        let Some(record) = record else {
          return Poll::Ready(None);
        };

        let res = loop {
          let mut inner = record.self_mut();
          let res = match &mut inner.response_body {
            ResponseBytesInner::Done | ResponseBytesInner::Empty => {
              if let Some(trailers) = inner.trailers.take() {
                return Poll::Ready(Some(Ok(Frame::trailers(trailers))));
              }
              unreachable!()
            }
            ResponseBytesInner::Bytes(..) => {
              drop(inner);
              let ResponseBytesInner::Bytes(data) = record.take_response_body()
              else {
                unreachable!();
              };
              return Poll::Ready(Some(Ok(Frame::data(data))));
            }
            ResponseBytesInner::UncompressedStream(stm) => {
              ready!(Pin::new(stm).poll_frame(cx))
            }
            ResponseBytesInner::GZipStream(stm) => {
              ready!(Pin::new(stm.as_mut()).poll_frame(cx))
            }
            ResponseBytesInner::BrotliStream(stm) => {
              ready!(Pin::new(stm.as_mut()).poll_frame(cx))
            }
          };
          // This is where we retry the NoData response
          if matches!(res, ResponseStreamResult::NoData) {
            continue;
          }
          break res;
        };

        if matches!(res, ResponseStreamResult::EndOfStream) {
          if let Some(trailers) = record.self_mut().trailers.take() {
            return Poll::Ready(Some(Ok(Frame::trailers(trailers))));
          }
          record.take_response_body();
        }

        if let ResponseStreamResult::NonEmptyBuf(buf) = &res {
          let mut http = record.0.borrow_mut();
          if let Some(otel_info) = &mut http.as_mut().unwrap().otel_info
            && let Some(response_size) = &mut otel_info.response_size
          {
            *response_size += buf.len() as u64;
          }
        }

        Poll::Ready(res.into())
      }
    }
  }

  fn is_end_stream(&self) -> bool {
    match self {
      Self::Empty => true,
      Self::Flat { body, .. } => matches!(body, FlatResponseBody::Empty),
      Self::Record(Some(record)) => {
        let inner = record.self_ref();
        matches!(
          inner.response_body,
          ResponseBytesInner::Done | ResponseBytesInner::Empty
        ) && inner.trailers.is_none()
      }
      Self::Record(None) => true,
    }
  }

  fn size_hint(&self) -> SizeHint {
    match self {
      Self::Empty => SizeHint::with_exact(0),
      Self::Flat { body, .. } => match body {
        FlatResponseBody::Empty => SizeHint::with_exact(0),
        FlatResponseBody::Bytes(buf) => SizeHint::with_exact(buf.len() as u64),
      },
      Self::Record(Some(record)) => {
        // The size hint currently only used in the case where it is exact bounds in hyper, but we'll pass it through
        // anyways just in case hyper needs it.
        record.self_ref().response_body.size_hint()
      }
      Self::Record(None) => SizeHint::with_exact(0),
    }
  }
}

impl Drop for HttpRecordResponse {
  fn drop(&mut self) {
    let record = match self {
      Self::Empty => None,
      Self::Record(record) => record.as_mut(),
      Self::Flat { record, .. } => record.as_mut(),
    };
    let Some(record) = record else {
      return;
    };
    // SAFETY: this ManuallyDrop is not used again.
    let record = unsafe { ManuallyDrop::take(record) };
    http_trace!(record, "HttpRecordResponse::drop");
    record.finish();
  }
}

#[cfg(test)]
mod tests {
  use std::error::Error as StdError;

  use bytes::Buf;
  use deno_net::raw::NetworkStreamType;
  use hyper::body::Body;
  use hyper::service::HttpService;
  use hyper::service::service_fn;
  use hyper_util::rt::TokioIo;

  use super::*;
  use crate::response_body::Compression;
  use crate::response_body::ResponseBytesInner;

  /// Execute client request on service and concurrently map the response.
  async fn serve_request<B, S, T, F>(
    req: http::Request<B>,
    service: S,
    map_response: impl FnOnce(hyper::Response<Incoming>) -> F,
  ) -> hyper::Result<T>
  where
    B: Body + Send + 'static, // Send bound due to DuplexStream
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    S: HttpService<Incoming>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    S::ResBody: 'static,
    <S::ResBody as Body>::Error: Into<Box<dyn StdError + Send + Sync>>,
    F: std::future::Future<Output = hyper::Result<T>>,
  {
    use hyper::client::conn::http1::handshake;
    use hyper::server::conn::http1::Builder;
    let (stream_client, stream_server) = tokio::io::duplex(16 * 1024);
    let conn_server =
      Builder::new().serve_connection(TokioIo::new(stream_server), service);
    let (mut sender, conn_client) =
      handshake(TokioIo::new(stream_client)).await?;

    let (res, _, _) = tokio::try_join!(
      async move {
        let res = sender.send_request(req).await?;
        map_response(res).await
      },
      conn_server,
      conn_client,
    )?;
    Ok(res)
  }

  #[tokio::test]
  async fn test_handle_request() -> Result<(), deno_core::error::AnyError> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let server_state = HttpServerState::new();
    let server_state_check = server_state.clone();
    let request_info = HttpConnectionProperties {
      peer_address: "".into(),
      peer_port: None,
      local_port: None,
      stream_type: NetworkStreamType::Tcp,
      scheme: "http://",
      fallback_host: "localhost".into(),
    };
    let svc = service_fn(move |req: hyper::Request<Incoming>| {
      let tx = tx.clone();
      handle_request(
        req,
        request_info.clone(),
        server_state.clone(),
        move |record| {
          tx.try_send(record).unwrap();
        },
        true,
        true,
      )
    });

    let client_req = http::Request::builder().uri("/").body("".to_string())?;

    // Response produced by concurrent tasks
    tokio::try_join!(
      async move {
        // JavaScript handler produces response
        let record = rx.recv().await.unwrap();
        record.set_response_body(ResponseBytesInner::from_vec(
          Compression::None,
          b"hello world".to_vec(),
        ));
        record.complete();
        Ok(())
      },
      // Server connection executes service
      async move {
        serve_request(client_req, svc, |res| async {
          // Client reads the response
          use http_body_util::BodyExt;
          assert_eq!(res.status(), 200);
          let body = res.collect().await?.to_bytes();
          assert_eq!(body.chunk(), b"hello world");
          Ok(())
        })
        .await
      },
    )?;
    assert_eq!(server_state_check.strong_count(), 1);
    Ok(())
  }
}
