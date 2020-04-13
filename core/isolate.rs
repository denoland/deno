// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Do not add any dependency to modules.rs!
// modules.rs is complex and should remain decoupled from isolate.rs to keep the
// Isolate struct from becoming too bloating for users who do not need
// asynchronous module loading.

use rusty_v8 as v8;

use crate::any_error::ErrBox;
use crate::bindings;
use crate::js_errors::JSError;
use crate::ops::*;
use crate::shared_queue::SharedQueue;
use crate::shared_queue::RECOMMENDED_SIZE;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::stream::select;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures::task::AtomicWaker;
use futures::Future;
use libc::c_void;
use std::collections::HashMap;
use std::convert::From;
use std::error::Error;
use std::fmt;
use std::mem::forget;
use std::ops::{Deref, DerefMut};
use std::option::Option;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Once};
use std::task::Context;
use std::task::Poll;

/// A ZeroCopyBuf encapsulates a slice that's been borrowed from a JavaScript
/// ArrayBuffer object. JavaScript objects can normally be garbage collected,
/// but the existence of a ZeroCopyBuf inhibits this until it is dropped. It
/// behaves much like an Arc<[u8]>, although a ZeroCopyBuf currently can't be
/// cloned.
pub struct ZeroCopyBuf {
  backing_store: v8::SharedRef<v8::BackingStore>,
  byte_offset: usize,
  byte_length: usize,
}

unsafe impl Send for ZeroCopyBuf {}

impl ZeroCopyBuf {
  pub fn new(view: v8::Local<v8::ArrayBufferView>) -> Self {
    let backing_store = view.buffer().unwrap().get_backing_store();
    let byte_offset = view.byte_offset();
    let byte_length = view.byte_length();
    Self {
      backing_store,
      byte_offset,
      byte_length,
    }
  }
}

impl Deref for ZeroCopyBuf {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    let buf = unsafe { &**self.backing_store.get() };
    &buf[self.byte_offset..self.byte_offset + self.byte_length]
  }
}

impl DerefMut for ZeroCopyBuf {
  fn deref_mut(&mut self) -> &mut [u8] {
    let buf = unsafe { &mut **self.backing_store.get() };
    &mut buf[self.byte_offset..self.byte_offset + self.byte_length]
  }
}

impl AsRef<[u8]> for ZeroCopyBuf {
  fn as_ref(&self) -> &[u8] {
    &*self
  }
}

impl AsMut<[u8]> for ZeroCopyBuf {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut *self
  }
}

pub enum SnapshotConfig {
  Borrowed(v8::StartupData<'static>),
  Owned(v8::OwnedStartupData),
}

impl From<&'static [u8]> for SnapshotConfig {
  fn from(sd: &'static [u8]) -> Self {
    Self::Borrowed(v8::StartupData::new(sd))
  }
}

impl From<v8::OwnedStartupData> for SnapshotConfig {
  fn from(sd: v8::OwnedStartupData) -> Self {
    Self::Owned(sd)
  }
}

impl Deref for SnapshotConfig {
  type Target = v8::StartupData<'static>;
  fn deref(&self) -> &Self::Target {
    match self {
      Self::Borrowed(sd) => sd,
      Self::Owned(sd) => &*sd,
    }
  }
}

/// Stores a script used to initalize a Isolate
pub struct Script<'a> {
  pub source: &'a str,
  pub filename: &'a str,
}

// TODO(ry) It's ugly that we have both Script and OwnedScript. Ideally we
// wouldn't expose such twiddly complexity.
struct OwnedScript {
  pub source: String,
  pub filename: String,
}

impl From<Script<'_>> for OwnedScript {
  fn from(s: Script) -> OwnedScript {
    OwnedScript {
      source: s.source.to_string(),
      filename: s.filename.to_string(),
    }
  }
}

/// Represents data used to initialize isolate at startup
/// either a binary snapshot or a javascript source file
/// in the form of the StartupScript struct.
pub enum StartupData<'a> {
  Script(Script<'a>),
  Snapshot(&'static [u8]),
  OwnedSnapshot(v8::OwnedStartupData),
  None,
}

type JSErrorCreateFn = dyn Fn(JSError) -> ErrBox;
type IsolateErrorHandleFn = dyn FnMut(ErrBox) -> Result<(), ErrBox>;

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM. An Isolate is a Future that can be used with
/// Tokio.  The Isolate future complete when there is an error or when all
/// pending ops have completed.
///
/// Ops are created in JavaScript by calling Deno.core.dispatch(), and in Rust
/// by implementing dispatcher function that takes control buffer and optional zero copy buffer
/// as arguments. An async Op corresponds exactly to a Promise in JavaScript.
#[allow(unused)]
pub struct Isolate {
  pub v8_isolate: Option<v8::OwnedIsolate>,
  snapshot_creator: Option<v8::SnapshotCreator>,
  has_snapshotted: bool,
  snapshot: Option<SnapshotConfig>,
  pub global_context: v8::Global<v8::Context>,
  pub(crate) shared_ab: v8::Global<v8::SharedArrayBuffer>,
  pub(crate) js_recv_cb: v8::Global<v8::Function>,
  pub(crate) js_macrotask_cb: v8::Global<v8::Function>,
  pub(crate) pending_promise_exceptions: HashMap<i32, v8::Global<v8::Value>>,
  shared_isolate_handle: Arc<Mutex<Option<*mut v8::Isolate>>>,
  pub(crate) js_error_create_fn: Box<JSErrorCreateFn>,
  needs_init: bool,
  pub(crate) shared: SharedQueue,
  pending_ops: FuturesUnordered<PendingOpFuture>,
  pending_unref_ops: FuturesUnordered<PendingOpFuture>,
  have_unpolled_ops: bool,
  startup_script: Option<OwnedScript>,
  pub op_registry: Rc<OpRegistry>,
  waker: AtomicWaker,
  error_handler: Option<Box<IsolateErrorHandleFn>>,
}

impl Drop for Isolate {
  fn drop(&mut self) {
    if let Some(creator) = self.snapshot_creator.take() {
      // TODO(ry): in rusty_v8, `SnapShotCreator::get_owned_isolate()` returns
      // a `struct OwnedIsolate` which is not actually owned, hence the need
      // here to leak the `OwnedIsolate` in order to avoid a double free and
      // the segfault that it causes.
      let v8_isolate = self.v8_isolate.take().unwrap();
      forget(v8_isolate);

      // TODO(ry) V8 has a strange assert which prevents a SnapshotCreator from
      // being deallocated if it hasn't created a snapshot yet.
      // https://github.com/v8/v8/blob/73212783fbd534fac76cc4b66aac899c13f71fc8/src/api.cc#L603
      // If that assert is removed, this if guard could be removed.
      // WARNING: There may be false positive LSAN errors here.
      if self.has_snapshotted {
        drop(creator);
      }
    }
  }
}

static DENO_INIT: Once = Once::new();

#[allow(clippy::missing_safety_doc)]
pub unsafe fn v8_init() {
  let platform = v8::new_default_platform();
  v8::V8::initialize_platform(platform);
  v8::V8::initialize();
  // TODO(ry) This makes WASM compile synchronously. Eventually we should
  // remove this to make it work asynchronously too. But that requires getting
  // PumpMessageLoop and RunMicrotasks setup correctly.
  // See https://github.com/denoland/deno/issues/2544
  let argv = vec![
    "".to_string(),
    "--no-wasm-async-compilation".to_string(),
    "--harmony-top-level-await".to_string(),
  ];
  v8::V8::set_flags_from_command_line(argv);
}

impl Isolate {
  /// startup_data defines the snapshot or script used at startup to initialize
  /// the isolate.
  pub fn new(startup_data: StartupData, will_snapshot: bool) -> Box<Self> {
    DENO_INIT.call_once(|| {
      unsafe { v8_init() };
    });

    let mut load_snapshot: Option<SnapshotConfig> = None;
    let mut startup_script: Option<OwnedScript> = None;

    // Separate into Option values for each startup type
    match startup_data {
      StartupData::Script(d) => {
        startup_script = Some(d.into());
      }
      StartupData::Snapshot(d) => {
        load_snapshot = Some(d.into());
      }
      StartupData::OwnedSnapshot(d) => {
        load_snapshot = Some(d.into());
      }
      StartupData::None => {}
    };

    let mut global_context = v8::Global::<v8::Context>::new();
    let (mut isolate, maybe_snapshot_creator) = if will_snapshot {
      // TODO(ry) Support loading snapshots before snapshotting.
      assert!(load_snapshot.is_none());
      let mut creator =
        v8::SnapshotCreator::new(Some(&bindings::EXTERNAL_REFERENCES));
      let isolate = unsafe { creator.get_owned_isolate() };
      let mut isolate = Isolate::setup_isolate(isolate);

      let mut hs = v8::HandleScope::new(&mut isolate);
      let scope = hs.enter();

      let context = bindings::initialize_context(scope);
      global_context.set(scope, context);
      creator.set_default_context(context);

      (isolate, Some(creator))
    } else {
      let mut params = v8::Isolate::create_params();
      params.set_array_buffer_allocator(v8::new_default_allocator());
      params.set_external_references(&bindings::EXTERNAL_REFERENCES);
      if let Some(ref mut snapshot) = load_snapshot {
        params.set_snapshot_blob(snapshot);
      }

      let isolate = v8::Isolate::new(params);
      let mut isolate = Isolate::setup_isolate(isolate);

      let mut hs = v8::HandleScope::new(&mut isolate);
      let scope = hs.enter();

      let context = match load_snapshot {
        Some(_) => v8::Context::new(scope),
        None => {
          // If no snapshot is provided, we initialize the context with empty
          // main source code and source maps.
          bindings::initialize_context(scope)
        }
      };
      global_context.set(scope, context);

      (isolate, None)
    };

    let shared = SharedQueue::new(RECOMMENDED_SIZE);
    let needs_init = true;

    let core_isolate = Self {
      v8_isolate: None,
      global_context,
      pending_promise_exceptions: HashMap::new(),
      shared_ab: v8::Global::<v8::SharedArrayBuffer>::new(),
      js_recv_cb: v8::Global::<v8::Function>::new(),
      js_macrotask_cb: v8::Global::<v8::Function>::new(),
      snapshot_creator: maybe_snapshot_creator,
      snapshot: load_snapshot,
      has_snapshotted: false,
      shared_isolate_handle: Arc::new(Mutex::new(None)),
      js_error_create_fn: Box::new(JSError::create),
      shared,
      needs_init,
      pending_ops: FuturesUnordered::new(),
      pending_unref_ops: FuturesUnordered::new(),
      have_unpolled_ops: false,
      startup_script,
      op_registry: Rc::new(OpRegistry::new()),
      waker: AtomicWaker::new(),
      error_handler: None,
    };

    let mut boxed_isolate = Box::new(core_isolate);
    {
      let core_isolate_ptr: *mut Self = Box::into_raw(boxed_isolate);
      unsafe { isolate.set_data(0, core_isolate_ptr as *mut c_void) };
      boxed_isolate = unsafe { Box::from_raw(core_isolate_ptr) };
      let shared_handle_ptr = &mut *isolate;
      *boxed_isolate.shared_isolate_handle.lock().unwrap() =
        Some(shared_handle_ptr);
      boxed_isolate.v8_isolate = Some(isolate);
    }

    boxed_isolate
  }

  pub fn setup_isolate(mut isolate: v8::OwnedIsolate) -> v8::OwnedIsolate {
    isolate.set_capture_stack_trace_for_uncaught_exceptions(true, 10);
    isolate.set_promise_reject_callback(bindings::promise_reject_callback);
    isolate
  }

  /// Defines the how Deno.core.dispatch() acts.
  /// Called whenever Deno.core.dispatch() is called in JavaScript. zero_copy_buf
  /// corresponds to the second argument of Deno.core.dispatch().
  ///
  /// Requires runtime to explicitly ask for op ids before using any of the ops.
  pub fn register_op<F>(&self, name: &str, op: F) -> OpId
  where
    F: Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp + 'static,
  {
    self.op_registry.register(name, op)
  }

  /// Allows a callback to be set whenever a V8 exception is made. This allows
  /// the caller to wrap the JSError into an error. By default this callback
  /// is set to JSError::create.
  pub fn set_js_error_create_fn(
    &mut self,
    f: impl Fn(JSError) -> ErrBox + 'static,
  ) {
    self.js_error_create_fn = Box::new(f);
  }

  /// Executes a bit of built-in JavaScript to provide Deno.sharedQueue.
  pub(crate) fn shared_init(&mut self) {
    if self.needs_init {
      self.needs_init = false;
      js_check(
        self.execute("shared_queue.js", include_str!("shared_queue.js")),
      );
      // Maybe execute the startup script.
      if let Some(s) = self.startup_script.take() {
        self.execute(&s.filename, &s.source).unwrap()
      }
    }
  }

  pub fn dispatch_op<'s>(
    &mut self,
    scope: &mut impl v8::ToLocal<'s>,
    op_id: OpId,
    control_buf: &[u8],
    zero_copy_buf: Option<ZeroCopyBuf>,
  ) -> Option<(OpId, Box<[u8]>)> {
    let maybe_op = self.op_registry.call(op_id, control_buf, zero_copy_buf);

    let op = match maybe_op {
      Some(op) => op,
      None => {
        let message =
          v8::String::new(scope, &format!("Unknown op id: {}", op_id)).unwrap();
        let exception = v8::Exception::type_error(scope, message);
        scope.isolate().throw_exception(exception);
        return None;
      }
    };

    debug_assert_eq!(self.shared.size(), 0);
    match op {
      Op::Sync(buf) => {
        // For sync messages, we always return the response via Deno.core.send's
        // return value. Sync messages ignore the op_id.
        let op_id = 0;
        Some((op_id, buf))
      }
      Op::Async(fut) => {
        let fut2 = fut.map_ok(move |buf| (op_id, buf));
        self.pending_ops.push(fut2.boxed_local());
        self.have_unpolled_ops = true;
        None
      }
      Op::AsyncUnref(fut) => {
        let fut2 = fut.map_ok(move |buf| (op_id, buf));
        self.pending_unref_ops.push(fut2.boxed_local());
        self.have_unpolled_ops = true;
        None
      }
    }
  }

  /// Executes traditional JavaScript code (traditional = not ES modules)
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is JSError, however it may be a
  /// different type if Isolate::set_js_error_create_fn() has been used.
  pub fn execute(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), ErrBox> {
    self.shared_init();

    let js_error_create_fn = &*self.js_error_create_fn;
    let v8_isolate = self.v8_isolate.as_mut().unwrap();

    let mut hs = v8::HandleScope::new(v8_isolate);
    let scope = hs.enter();
    assert!(!self.global_context.is_empty());
    let context = self.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let source = v8::String::new(scope, js_source).unwrap();
    let name = v8::String::new(scope, js_filename).unwrap();
    let origin = bindings::script_origin(scope, name);

    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let mut script =
      v8::Script::compile(scope, context, source, Some(&origin)).unwrap();
    match script.run(scope, context) {
      Some(_) => Ok(()),
      None => {
        assert!(tc.has_caught());
        let exception = tc.exception().unwrap();
        exception_to_err_result(scope, exception, js_error_create_fn)
      }
    }
  }

  /// Takes a snapshot. The isolate should have been created with will_snapshot
  /// set to true.
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is JSError, however it may be a
  /// different type if Isolate::set_js_error_create_fn() has been used.
  pub fn snapshot(&mut self) -> v8::OwnedStartupData {
    assert!(self.snapshot_creator.is_some());

    // Note: create_blob() method must not be called from within a HandleScope.
    // The HandleScope created here is exited at the end of the block.
    // TODO(piscisaureus): The rusty_v8 type system should enforce this.
    {
      let v8_isolate = self.v8_isolate.as_mut().unwrap();
      let mut hs = v8::HandleScope::new(v8_isolate);
      let scope = hs.enter();
      self.global_context.reset(scope);
    }

    let snapshot_creator = self.snapshot_creator.as_mut().unwrap();
    let snapshot = snapshot_creator
      .create_blob(v8::FunctionCodeHandling::Keep)
      .unwrap();
    self.has_snapshotted = true;

    snapshot
  }
}

impl Future for Isolate {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    inner.waker.register(cx.waker());
    inner.shared_init();

    let v8_isolate = inner.v8_isolate.as_mut().unwrap();
    let js_error_create_fn = &*inner.js_error_create_fn;
    let js_recv_cb = &inner.js_recv_cb;
    let js_macrotask_cb = &inner.js_macrotask_cb;
    let pending_promise_exceptions = &mut inner.pending_promise_exceptions;

    let mut hs = v8::HandleScope::new(v8_isolate);
    let scope = hs.enter();
    let context = inner.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    check_promise_exceptions(
      scope,
      pending_promise_exceptions,
      js_error_create_fn,
    )?;

    let mut overflow_response: Option<(OpId, Buf)> = None;

    loop {
      // Now handle actual ops.
      inner.have_unpolled_ops = false;
      #[allow(clippy::match_wild_err_arm)]
      match select(&mut inner.pending_ops, &mut inner.pending_unref_ops)
        .poll_next_unpin(cx)
      {
        Poll::Ready(Some(Err(_))) => panic!("unexpected op error"),
        Poll::Ready(None) => break,
        Poll::Pending => break,
        Poll::Ready(Some(Ok((op_id, buf)))) => {
          let successful_push = inner.shared.push(op_id, &buf);
          if !successful_push {
            // If we couldn't push the response to the shared queue, because
            // there wasn't enough size, we will return the buffer via the
            // legacy route, using the argument of deno_respond.
            overflow_response = Some((op_id, buf));
            break;
          }
        }
      }
    }

    if inner.shared.size() > 0 {
      async_op_response(scope, None, js_recv_cb, js_error_create_fn)?;
      // The other side should have shifted off all the messages.
      assert_eq!(inner.shared.size(), 0);
    }

    if overflow_response.is_some() {
      let (op_id, buf) = overflow_response.take().unwrap();
      async_op_response(
        scope,
        Some((op_id, buf)),
        js_recv_cb,
        js_error_create_fn,
      )?;
    }

    drain_macrotasks(scope, js_macrotask_cb, js_error_create_fn)?;

    check_promise_exceptions(
      scope,
      pending_promise_exceptions,
      js_error_create_fn,
    )?;

    // We're idle if pending_ops is empty.
    if inner.pending_ops.is_empty() {
      Poll::Ready(Ok(()))
    } else {
      if inner.have_unpolled_ops {
        inner.waker.wake();
      }
      Poll::Pending
    }
  }
}

fn async_op_response<'s>(
  scope: &mut impl v8::ToLocal<'s>,
  maybe_buf: Option<(OpId, Box<[u8]>)>,
  js_recv_cb: &v8::Global<v8::Function>,
  js_error_create_fn: &JSErrorCreateFn,
) -> Result<(), ErrBox> {
  let context = scope.get_current_context().unwrap();
  let global: v8::Local<v8::Value> = context.global(scope).into();
  let js_recv_cb = js_recv_cb
    .get(scope)
    .expect("Deno.core.recv has not been called.");

  // TODO(piscisaureus): properly integrate TryCatch in the scope chain.
  let mut try_catch = v8::TryCatch::new(scope);
  let tc = try_catch.enter();

  match maybe_buf {
    Some((op_id, buf)) => {
      let op_id: v8::Local<v8::Value> =
        v8::Integer::new(scope, op_id as i32).into();
      let ui8: v8::Local<v8::Value> =
        bindings::boxed_slice_to_uint8array(scope, buf).into();
      js_recv_cb.call(scope, context, global, &[op_id, ui8])
    }
    None => js_recv_cb.call(scope, context, global, &[]),
  };

  match tc.exception() {
    None => Ok(()),
    Some(exception) => {
      exception_to_err_result(scope, exception, js_error_create_fn)
    }
  }
}

fn drain_macrotasks<'s>(
  scope: &mut impl v8::ToLocal<'s>,
  js_macrotask_cb: &v8::Global<v8::Function>,
  js_error_create_fn: &JSErrorCreateFn,
) -> Result<(), ErrBox> {
  let context = scope.get_current_context().unwrap();
  let global: v8::Local<v8::Value> = context.global(scope).into();
  let js_macrotask_cb = js_macrotask_cb.get(scope);
  if js_macrotask_cb.is_none() {
    return Ok(());
  }
  let js_macrotask_cb = js_macrotask_cb.unwrap();

  // Repeatedly invoke macrotask callback until it returns true (done),
  // such that ready microtasks would be automatically run before
  // next macrotask is processed.
  loop {
    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let is_done = js_macrotask_cb.call(scope, context, global, &[]);

    if let Some(exception) = tc.exception() {
      return exception_to_err_result(scope, exception, js_error_create_fn);
    }

    let is_done = is_done.unwrap();
    if is_done.is_true() {
      break;
    }
  }

  Ok(())
}

pub(crate) fn attach_handle_to_error(
  scope: &mut impl v8::InIsolate,
  err: ErrBox,
  handle: v8::Local<v8::Value>,
) -> ErrBox {
  ErrWithV8Handle::new(scope, err, handle).into()
}

pub(crate) fn exception_to_err_result<'s, T>(
  scope: &mut impl v8::ToLocal<'s>,
  exception: v8::Local<v8::Value>,
  js_error_create_fn: &JSErrorCreateFn,
) -> Result<T, ErrBox> {
  // TODO(piscisaureus): in rusty_v8, `is_execution_terminating()` should
  // also be implemented on `struct Isolate`.
  let is_terminating_exception = scope
    .isolate()
    .thread_safe_handle()
    .is_execution_terminating();
  let mut exception = exception;

  if is_terminating_exception {
    // TerminateExecution was called. Cancel exception termination so that the
    // exception can be created..
    // TODO(piscisaureus): in rusty_v8, `cancel_terminate_execution()` should
    // also be implemented on `struct Isolate`.
    scope
      .isolate()
      .thread_safe_handle()
      .cancel_terminate_execution();

    // Maybe make a new exception object.
    if exception.is_null_or_undefined() {
      let message = v8::String::new(scope, "execution terminated").unwrap();
      exception = v8::Exception::error(scope, message);
    }
  }

  let js_error = JSError::from_v8_exception(scope, exception);
  let js_error = (js_error_create_fn)(js_error);

  if is_terminating_exception {
    // Re-enable exception termination.
    // TODO(piscisaureus): in rusty_v8, `terminate_execution()` should also
    // be implemented on `struct Isolate`.
    scope.isolate().thread_safe_handle().terminate_execution();
  }

  Err(js_error)
}

fn check_promise_exceptions<'s>(
  scope: &mut impl v8::ToLocal<'s>,
  pending_promise_exceptions: &mut HashMap<i32, v8::Global<v8::Value>>,
  js_error_create_fn: &JSErrorCreateFn,
) -> Result<(), ErrBox> {
  if let Some(&key) = pending_promise_exceptions.keys().next() {
    let handle = pending_promise_exceptions.remove(&key).unwrap();
    let exception = handle.get(scope).expect("empty error handle");
    exception_to_err_result(scope, exception, js_error_create_fn)
  } else {
    Ok(())
  }
}

pub fn js_check<T>(r: Result<T, ErrBox>) -> T {
  if let Err(e) = r {
    panic!(e.to_string());
  }
  r.unwrap()
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use futures::future::lazy;
  use std::ops::FnOnce;
  use std::sync::atomic::{AtomicUsize, Ordering};

  pub fn run_in_task<F>(f: F)
  where
    F: FnOnce(&mut Context) + Send + 'static,
  {
    futures::executor::block_on(lazy(move |cx| f(cx)));
  }

  fn poll_until_ready<F>(future: &mut F, max_poll_count: usize) -> F::Output
  where
    F: Future + Unpin,
  {
    let mut cx = Context::from_waker(futures::task::noop_waker_ref());
    for _ in 0..max_poll_count {
      match future.poll_unpin(&mut cx) {
        Poll::Pending => continue,
        Poll::Ready(val) => return val,
      }
    }
    panic!(
      "Isolate still not ready after polling {} times.",
      max_poll_count
    )
  }

  pub enum Mode {
    Async,
    AsyncUnref,
    OverflowReqSync,
    OverflowResSync,
    OverflowReqAsync,
    OverflowResAsync,
  }

  pub fn setup(mode: Mode) -> (Box<Isolate>, Arc<AtomicUsize>) {
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let mut isolate = Isolate::new(StartupData::None, false);

    let dispatcher =
      move |control: &[u8], _zero_copy: Option<ZeroCopyBuf>| -> CoreOp {
        dispatch_count_.fetch_add(1, Ordering::Relaxed);
        match mode {
          Mode::Async => {
            assert_eq!(control.len(), 1);
            assert_eq!(control[0], 42);
            let buf = vec![43u8].into_boxed_slice();
            Op::Async(futures::future::ok(buf).boxed())
          }
          Mode::AsyncUnref => {
            assert_eq!(control.len(), 1);
            assert_eq!(control[0], 42);
            let fut = async {
              // This future never finish.
              futures::future::pending::<()>().await;
              let buf = vec![43u8].into_boxed_slice();
              Ok(buf)
            };
            Op::AsyncUnref(fut.boxed())
          }
          Mode::OverflowReqSync => {
            assert_eq!(control.len(), 100 * 1024 * 1024);
            let buf = vec![43u8].into_boxed_slice();
            Op::Sync(buf)
          }
          Mode::OverflowResSync => {
            assert_eq!(control.len(), 1);
            assert_eq!(control[0], 42);
            let mut vec = Vec::<u8>::new();
            vec.resize(100 * 1024 * 1024, 0);
            vec[0] = 99;
            let buf = vec.into_boxed_slice();
            Op::Sync(buf)
          }
          Mode::OverflowReqAsync => {
            assert_eq!(control.len(), 100 * 1024 * 1024);
            let buf = vec![43u8].into_boxed_slice();
            Op::Async(futures::future::ok(buf).boxed())
          }
          Mode::OverflowResAsync => {
            assert_eq!(control.len(), 1);
            assert_eq!(control[0], 42);
            let mut vec = Vec::<u8>::new();
            vec.resize(100 * 1024 * 1024, 0);
            vec[0] = 4;
            let buf = vec.into_boxed_slice();
            Op::Async(futures::future::ok(buf).boxed())
          }
        }
      };

    isolate.register_op("test", dispatcher);

    js_check(isolate.execute(
      "setup.js",
      r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    (isolate, dispatch_count)
  }

  #[test]
  fn test_dispatch() {
    let (mut isolate, dispatch_count) = setup(Mode::Async);
    js_check(isolate.execute(
      "filename.js",
      r#"
        let control = new Uint8Array([42]);
        Deno.core.send(1, control);
        async function main() {
          Deno.core.send(1, control);
        }
        main();
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
  }

  #[test]
  fn test_poll_async_delayed_ops() {
    run_in_task(|cx| {
      let (mut isolate, dispatch_count) = setup(Mode::Async);

      js_check(isolate.execute(
        "setup2.js",
        r#"
         let nrecv = 0;
         Deno.core.setAsyncHandler(1, (buf) => {
           nrecv++;
         });
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
      js_check(isolate.execute(
        "check1.js",
        r#"
         assert(nrecv == 0);
         let control = new Uint8Array([42]);
         Deno.core.send(1, control);
         assert(nrecv == 0);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      js_check(isolate.execute(
        "check2.js",
        r#"
         assert(nrecv == 1);
         Deno.core.send(1, control);
         assert(nrecv == 1);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      js_check(isolate.execute("check3.js", "assert(nrecv == 2)"));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      // We are idle, so the next poll should be the last.
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
    });
  }

  #[test]
  fn test_poll_async_optional_ops() {
    run_in_task(|cx| {
      let (mut isolate, dispatch_count) = setup(Mode::AsyncUnref);
      js_check(isolate.execute(
        "check1.js",
        r#"
          Deno.core.setAsyncHandler(1, (buf) => {
            // This handler will never be called
            assert(false);
          });
          let control = new Uint8Array([42]);
          Deno.core.send(1, control);
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      // The above op never finish, but isolate can finish
      // because the op is an unreffed async op.
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
    })
  }

  #[test]
  fn terminate_execution() {
    let (mut isolate, _dispatch_count) = setup(Mode::Async);
    // TODO(piscisaureus): in rusty_v8, the `thread_safe_handle()` method
    // should not require a mutable reference to `struct rusty_v8::Isolate`.
    let v8_isolate_handle =
      isolate.v8_isolate.as_mut().unwrap().thread_safe_handle();

    let terminator_thread = std::thread::spawn(move || {
      // allow deno to boot and run
      std::thread::sleep(std::time::Duration::from_millis(100));

      // terminate execution
      let ok = v8_isolate_handle.terminate_execution();
      assert!(ok);
    });

    // Rn an infinite loop, which should be terminated.
    match isolate.execute("infinite_loop.js", "for(;;) {}") {
      Ok(_) => panic!("execution should be terminated"),
      Err(e) => {
        assert_eq!(e.to_string(), "Uncaught Error: execution terminated")
      }
    };

    // Cancel the execution-terminating exception in order to allow script
    // execution again.
    // TODO(piscisaureus): in rusty_v8, `cancel_terminate_execution()` should
    // also be implemented on `struct Isolate`.
    let ok = isolate
      .v8_isolate
      .as_mut()
      .unwrap()
      .thread_safe_handle()
      .cancel_terminate_execution();
    assert!(ok);

    // Verify that the isolate usable again.
    isolate
      .execute("simple.js", "1 + 1")
      .expect("execution should be possible again");

    terminator_thread.join().unwrap();
  }

  #[test]
  fn dangling_shared_isolate() {
    let v8_isolate_handle = {
      // isolate is dropped at the end of this block
      let (mut isolate, _dispatch_count) = setup(Mode::Async);
      // TODO(piscisaureus): in rusty_v8, the `thread_safe_handle()` method
      // should not require a mutable reference to `struct rusty_v8::Isolate`.
      isolate.v8_isolate.as_mut().unwrap().thread_safe_handle()
    };

    // this should not SEGFAULT
    v8_isolate_handle.terminate_execution();
  }

  #[test]
  fn overflow_req_sync() {
    let (mut isolate, dispatch_count) = setup(Mode::OverflowReqSync);
    js_check(isolate.execute(
      "overflow_req_sync.js",
      r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler(1, (buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array(100 * 1024 * 1024);
        let response = Deno.core.dispatch(1, control);
        assert(response instanceof Uint8Array);
        assert(response.length == 1);
        assert(response[0] == 43);
        assert(asyncRecv == 0);
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn overflow_res_sync() {
    // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
    // should optimize this.
    let (mut isolate, dispatch_count) = setup(Mode::OverflowResSync);
    js_check(isolate.execute(
      "overflow_res_sync.js",
      r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler(1, (buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array([42]);
        let response = Deno.core.dispatch(1, control);
        assert(response instanceof Uint8Array);
        assert(response.length == 100 * 1024 * 1024);
        assert(response[0] == 99);
        assert(asyncRecv == 0);
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn overflow_req_async() {
    run_in_task(|cx| {
      let (mut isolate, dispatch_count) = setup(Mode::OverflowReqAsync);
      js_check(isolate.execute(
        "overflow_req_async.js",
        r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler(1, (buf) => {
           assert(buf.byteLength === 1);
           assert(buf[0] === 43);
           asyncRecv++;
         });
         // Large message that will overflow the shared space.
         let control = new Uint8Array(100 * 1024 * 1024);
         let response = Deno.core.dispatch(1, control);
         // Async messages always have null response.
         assert(response == null);
         assert(asyncRecv == 0);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      js_check(isolate.execute("check.js", "assert(asyncRecv == 1);"));
    });
  }

  #[test]
  fn overflow_res_async() {
    run_in_task(|_cx| {
      // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
      // should optimize this.
      let (mut isolate, dispatch_count) = setup(Mode::OverflowResAsync);
      js_check(isolate.execute(
        "overflow_res_async.js",
        r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler(1, (buf) => {
           assert(buf.byteLength === 100 * 1024 * 1024);
           assert(buf[0] === 4);
           asyncRecv++;
         });
         // Large message that will overflow the shared space.
         let control = new Uint8Array([42]);
         let response = Deno.core.dispatch(1, control);
         assert(response == null);
         assert(asyncRecv == 0);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      poll_until_ready(&mut isolate, 3).unwrap();
      js_check(isolate.execute("check.js", "assert(asyncRecv == 1);"));
    });
  }

  #[test]
  fn overflow_res_multiple_dispatch_async() {
    // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
    // should optimize this.
    run_in_task(|_cx| {
      let (mut isolate, dispatch_count) = setup(Mode::OverflowResAsync);
      js_check(isolate.execute(
        "overflow_res_multiple_dispatch_async.js",
        r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler(1, (buf) => {
           assert(buf.byteLength === 100 * 1024 * 1024);
           assert(buf[0] === 4);
           asyncRecv++;
         });
         // Large message that will overflow the shared space.
         let control = new Uint8Array([42]);
         let response = Deno.core.dispatch(1, control);
         assert(response == null);
         assert(asyncRecv == 0);
         // Dispatch another message to verify that pending ops
         // are done even if shared space overflows
         Deno.core.dispatch(1, control);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      poll_until_ready(&mut isolate, 3).unwrap();
      js_check(isolate.execute("check.js", "assert(asyncRecv == 2);"));
    });
  }

  #[test]
  fn test_pre_dispatch() {
    run_in_task(|mut cx| {
      let (mut isolate, _dispatch_count) = setup(Mode::OverflowResAsync);
      js_check(isolate.execute(
        "bad_op_id.js",
        r#"
          let thrown;
          try {
            Deno.core.dispatch(100, []);
          } catch (e) {
            thrown = e;
          }
          assert(String(thrown) === "TypeError: Unknown op id: 100");
         "#,
      ));
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_js() {
    run_in_task(|mut cx| {
      let (mut isolate, _dispatch_count) = setup(Mode::Async);
      js_check(
        isolate.execute(
          "shared_queue_test.js",
          include_str!("shared_queue_test.js"),
        ),
      );
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_encode_decode() {
    run_in_task(|mut cx| {
      let (mut isolate, _dispatch_count) = setup(Mode::Async);
      js_check(isolate.execute(
        "encode_decode_test.js",
        include_str!("encode_decode_test.js"),
      ));
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn will_snapshot() {
    let snapshot = {
      let mut isolate = Isolate::new(StartupData::None, true);
      js_check(isolate.execute("a.js", "a = 1 + 2"));
      isolate.snapshot()
    };

    let startup_data = StartupData::OwnedSnapshot(snapshot);
    let mut isolate2 = Isolate::new(startup_data, false);
    js_check(isolate2.execute("check.js", "if (a != 3) throw Error('x')"));
  }
}

// TODO(piscisaureus): rusty_v8 should implement the Error trait on
// values of type v8::Global<T>.
pub struct ErrWithV8Handle {
  err: ErrBox,
  handle: v8::Global<v8::Value>,
}

impl ErrWithV8Handle {
  pub fn new(
    scope: &mut impl v8::InIsolate,
    err: ErrBox,
    handle: v8::Local<v8::Value>,
  ) -> Self {
    let handle = v8::Global::new_from(scope, handle);
    Self { err, handle }
  }

  pub fn get_handle(&self) -> &v8::Global<v8::Value> {
    &self.handle
  }
}

unsafe impl Send for ErrWithV8Handle {}
unsafe impl Sync for ErrWithV8Handle {}

impl Error for ErrWithV8Handle {}

impl fmt::Display for ErrWithV8Handle {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.err.fmt(f)
  }
}

impl fmt::Debug for ErrWithV8Handle {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.err.fmt(f)
  }
}
