// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Do not add any dependency to modules.rs!
// modules.rs is complex and should remain decoupled from isolate.rs to keep the
// Isolate struct from becoming too bloating for users who do not need
// asynchronous module loading.

use crate::any_error::ErrBox;
use crate::js_errors::CoreJSError;
use crate::js_errors::V8Exception;
use crate::libdeno;
use crate::libdeno::deno_buf;
use crate::libdeno::deno_dyn_import_id;
use crate::libdeno::deno_mod;
use crate::libdeno::deno_pinned_buf;
use crate::libdeno::PinnedBuf;
use crate::libdeno::Snapshot1;
use crate::libdeno::Snapshot2;
use crate::ops::*;
use crate::shared_queue::SharedQueue;
use crate::shared_queue::RECOMMENDED_SIZE;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::IntoStream;
use futures::stream::Stream;
use futures::stream::StreamExt;
use futures::stream::StreamFuture;
use futures::stream::TryStream;
use futures::stream::TryStreamExt;
use futures::task::AtomicWaker;
use libc::c_char;
use libc::c_void;
use libc::strdup;
use std::ffi::CStr;
use std::ffi::CString;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::ptr::null;
use std::sync::{Arc, Mutex, Once};
use std::task::Context;
use std::task::Poll;

/// Stores a script used to initalize a Isolate
pub struct Script<'a> {
  pub source: &'a str,
  pub filename: &'a str,
}

/// Represent result of fetching the source code of a module. Found module URL
/// might be different from specified URL used for loading due to redirections
/// (like HTTP 303). E.G. Both https://example.com/a.ts and
/// https://example.com/b.ts may point to https://example.com/c.ts
/// By keeping track of specified and found URL we can alias modules and avoid
/// recompiling the same code 3 times.
#[derive(Debug, Eq, PartialEq)]
pub struct SourceCodeInfo {
  pub code: String,
  pub module_url_specified: String,
  pub module_url_found: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RecursiveLoadEvent {
  Fetch(SourceCodeInfo),
  Instantiate(deno_mod),
}

pub trait ImportStream: TryStream {
  fn register(
    &mut self,
    source_code_info: SourceCodeInfo,
    isolate: &mut Isolate,
  ) -> Result<(), ErrBox>;
}

type DynImportStream = Box<
  dyn ImportStream<
      Ok = RecursiveLoadEvent,
      Error = ErrBox,
      Item = Result<RecursiveLoadEvent, ErrBox>,
    > + Send
    + Unpin,
>;

type DynImportFn = dyn Fn(deno_dyn_import_id, &str, &str) -> DynImportStream;

/// Wraps DynImportStream to include the deno_dyn_import_id, so that it doesn't
/// need to be exposed.
#[derive(Debug)]
struct DynImport {
  pub id: deno_dyn_import_id,
  pub inner: DynImportStream,
}

impl fmt::Debug for DynImportStream {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "DynImportStream(..)")
  }
}

impl Stream for DynImport {
  type Item = Result<
    (deno_dyn_import_id, RecursiveLoadEvent),
    (deno_dyn_import_id, ErrBox),
  >;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let self_inner = self.get_mut();
    match self_inner.inner.try_poll_next_unpin(cx) {
      Poll::Ready(Some(Ok(event))) => {
        Poll::Ready(Some(Ok((self_inner.id, event))))
      }
      Poll::Ready(None) => unreachable!(),
      Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err((self_inner.id, e)))),
      Poll::Pending => Poll::Pending,
    }
  }
}

impl ImportStream for DynImport {
  fn register(
    &mut self,
    source_code_info: SourceCodeInfo,
    isolate: &mut Isolate,
  ) -> Result<(), ErrBox> {
    self.inner.register(source_code_info, isolate)
  }
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
  Snapshot(&'a [u8]),
  LibdenoSnapshot(Snapshot1<'a>),
  None,
}

type JSErrorCreateFn = dyn Fn(V8Exception) -> ErrBox;
type IsolateErrorHandleFn = dyn FnMut(ErrBox) -> Result<(), ErrBox>;

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM. An Isolate is a Future that can be used with
/// Tokio.  The Isolate future complete when there is an error or when all
/// pending ops have completed.
///
/// Ops are created in JavaScript by calling Deno.core.dispatch(), and in Rust
/// by implementing dispatcher function that takes control buffer and optional zero copy buffer
/// as arguments. An async Op corresponds exactly to a Promise in JavaScript.
pub struct Isolate {
  libdeno_isolate: *const libdeno::isolate,
  shared_libdeno_isolate: Arc<Mutex<Option<*const libdeno::isolate>>>,
  dyn_import: Option<Arc<DynImportFn>>,
  js_error_create: Arc<JSErrorCreateFn>,
  needs_init: bool,
  shared: SharedQueue,
  pending_ops: FuturesUnordered<PendingOpFuture>,
  pending_dyn_imports: FuturesUnordered<StreamFuture<IntoStream<DynImport>>>,
  have_unpolled_ops: bool,
  startup_script: Option<OwnedScript>,
  pub op_registry: Arc<OpRegistry>,
  waker: AtomicWaker,
  error_handler: Option<Box<IsolateErrorHandleFn>>,
}

unsafe impl Send for Isolate {}

impl Drop for Isolate {
  fn drop(&mut self) {
    // remove shared_libdeno_isolate reference
    *self.shared_libdeno_isolate.lock().unwrap() = None;

    unsafe { libdeno::deno_delete(self.libdeno_isolate) }
  }
}

static DENO_INIT: Once = Once::new();

impl Isolate {
  /// startup_data defines the snapshot or script used at startup to initialize
  /// the isolate.
  pub fn new(startup_data: StartupData, will_snapshot: bool) -> Self {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    let shared = SharedQueue::new(RECOMMENDED_SIZE);

    let needs_init = true;

    let mut libdeno_config = libdeno::deno_config {
      will_snapshot: will_snapshot.into(),
      load_snapshot: Snapshot2::empty(),
      shared: shared.as_deno_buf(),
      recv_cb: Self::pre_dispatch,
      dyn_import_cb: Self::dyn_import,
    };

    let mut startup_script: Option<OwnedScript> = None;

    // Separate into Option values for each startup type
    match startup_data {
      StartupData::Script(d) => {
        startup_script = Some(d.into());
      }
      StartupData::Snapshot(d) => {
        libdeno_config.load_snapshot = d.into();
      }
      StartupData::LibdenoSnapshot(d) => {
        libdeno_config.load_snapshot = d;
      }
      StartupData::None => {}
    };

    let libdeno_isolate = unsafe { libdeno::deno_new(libdeno_config) };

    Self {
      libdeno_isolate,
      shared_libdeno_isolate: Arc::new(Mutex::new(Some(libdeno_isolate))),
      dyn_import: None,
      js_error_create: Arc::new(CoreJSError::from_v8_exception),
      shared,
      needs_init,
      pending_ops: FuturesUnordered::new(),
      have_unpolled_ops: false,
      pending_dyn_imports: FuturesUnordered::new(),
      startup_script,
      op_registry: Arc::new(OpRegistry::new()),
      waker: AtomicWaker::new(),
      error_handler: None,
    }
  }

  pub fn set_error_handler(&mut self, handler: Box<IsolateErrorHandleFn>) {
    self.error_handler = Some(handler);
  }

  /// Defines the how Deno.core.dispatch() acts.
  /// Called whenever Deno.core.dispatch() is called in JavaScript. zero_copy_buf
  /// corresponds to the second argument of Deno.core.dispatch().
  ///
  /// Requires runtime to explicitly ask for op ids before using any of the ops.
  pub fn register_op<F>(&self, name: &str, op: F) -> OpId
  where
    F: Fn(&[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static,
  {
    self.op_registry.register(name, op)
  }

  pub fn set_dyn_import<F>(&mut self, f: F)
  where
    F: Fn(deno_dyn_import_id, &str, &str) -> DynImportStream
      + Send
      + Sync
      + 'static,
  {
    self.dyn_import = Some(Arc::new(f));
  }

  /// Allows a callback to be set whenever a V8 exception is made. This allows
  /// the caller to wrap the V8Exception into an error. By default this callback
  /// is set to CoreJSError::from_v8_exception.
  pub fn set_js_error_create<F>(&mut self, f: F)
  where
    F: Fn(V8Exception) -> ErrBox + 'static,
  {
    self.js_error_create = Arc::new(f);
  }

  /// Get a thread safe handle on the isolate.
  pub fn shared_isolate_handle(&mut self) -> IsolateHandle {
    IsolateHandle {
      shared_libdeno_isolate: self.shared_libdeno_isolate.clone(),
    }
  }

  /// Executes a bit of built-in JavaScript to provide Deno.sharedQueue.
  fn shared_init(&mut self) {
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

  extern "C" fn dyn_import(
    user_data: *mut c_void,
    specifier: *const c_char,
    referrer: *const c_char,
    id: deno_dyn_import_id,
  ) {
    assert_ne!(user_data, std::ptr::null_mut());
    let isolate = unsafe { Isolate::from_raw_ptr(user_data) };
    let specifier = unsafe { CStr::from_ptr(specifier).to_str().unwrap() };
    let referrer = unsafe { CStr::from_ptr(referrer).to_str().unwrap() };
    debug!("dyn_import specifier {} referrer {} ", specifier, referrer);

    if let Some(ref f) = isolate.dyn_import {
      let inner = f(id, specifier, referrer);
      let stream = DynImport { inner, id };
      isolate.waker.wake();
      isolate
        .pending_dyn_imports
        .push(stream.into_stream().into_future());
    } else {
      panic!("dyn_import callback not set")
    }
  }

  extern "C" fn pre_dispatch(
    user_data: *mut c_void,
    op_id: OpId,
    control_buf: deno_buf,
    zero_copy_buf: deno_pinned_buf,
  ) {
    let isolate = unsafe { Isolate::from_raw_ptr(user_data) };

    let maybe_op = isolate.op_registry.call(
      op_id,
      control_buf.as_ref(),
      PinnedBuf::new(zero_copy_buf),
    );

    let op = match maybe_op {
      Some(op) => op,
      None => {
        return isolate.throw_exception(&format!("Unknown op id: {}", op_id))
      }
    };

    debug_assert_eq!(isolate.shared.size(), 0);
    match op {
      Op::Sync(buf) => {
        // For sync messages, we always return the response via Deno.core.send's
        // return value. Sync messages ignore the op_id.
        let op_id = 0;
        isolate
          .respond(Some((op_id, &buf)))
          // Because this is a sync op, deno_respond() does not actually call
          // into JavaScript. We should not get an error here.
          .expect("unexpected error");
      }
      Op::Async(fut) => {
        let fut2 = fut.map_ok(move |buf| (op_id, buf));
        isolate.pending_ops.push(fut2.boxed());
        isolate.have_unpolled_ops = true;
      }
    }
  }

  #[inline]
  unsafe fn from_raw_ptr<'a>(ptr: *const c_void) -> &'a mut Self {
    let ptr = ptr as *mut _;
    &mut *ptr
  }

  #[inline]
  fn as_raw_ptr(&self) -> *const c_void {
    self as *const _ as *const c_void
  }

  /// Executes traditional JavaScript code (traditional = not ES modules)
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn execute(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), ErrBox> {
    self.shared_init();
    let filename = CString::new(js_filename).unwrap();
    let source = CString::new(js_source).unwrap();
    unsafe {
      libdeno::deno_execute(
        self.libdeno_isolate,
        self.as_raw_ptr(),
        filename.as_ptr(),
        source.as_ptr(),
      )
    };
    self.check_last_exception()
  }

  fn check_last_exception(&mut self) -> Result<(), ErrBox> {
    let ptr = unsafe { libdeno::deno_last_exception(self.libdeno_isolate) };
    if ptr.is_null() {
      Ok(())
    } else {
      let js_error_create = &*self.js_error_create;
      let cstr = unsafe { CStr::from_ptr(ptr) };
      if self.error_handler.is_some() {
        // We duplicate the string and assert ownership.
        // This is due to we want the user to safely clone the error.
        let cstring = unsafe { CString::from_raw(strdup(ptr)) };
        // We need to clear last exception to avoid double handling.
        unsafe { libdeno::deno_clear_last_exception(self.libdeno_isolate) };
        let json_string = cstring.into_string().unwrap();
        let v8_exception = V8Exception::from_json(&json_string).unwrap();
        let js_error = js_error_create(v8_exception);
        let handler = self.error_handler.as_mut().unwrap();
        handler(js_error)
      } else {
        let json_str = cstr.to_str().unwrap();
        let v8_exception = V8Exception::from_json(json_str).unwrap();
        let js_error = js_error_create(v8_exception);
        Err(js_error)
      }
    }
  }

  fn check_promise_errors(&self) {
    unsafe {
      libdeno::deno_check_promise_errors(self.libdeno_isolate);
    }
  }

  fn throw_exception(&mut self, exception_text: &str) {
    let text = CString::new(exception_text).unwrap();
    unsafe {
      libdeno::deno_throw_exception(self.libdeno_isolate, text.as_ptr())
    }
  }

  fn respond(
    &mut self,
    maybe_buf: Option<(OpId, &[u8])>,
  ) -> Result<(), ErrBox> {
    let (op_id, buf) = match maybe_buf {
      None => (0, deno_buf::empty()),
      Some((op_id, r)) => (op_id, deno_buf::from(r)),
    };
    unsafe {
      libdeno::deno_respond(self.libdeno_isolate, self.as_raw_ptr(), op_id, buf)
    }
    self.check_last_exception()
  }

  /// Low-level module creation.
  pub fn mod_new(
    &mut self,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<deno_mod, ErrBox> {
    let name_ = CString::new(name.to_string()).unwrap();
    let name_ptr = name_.as_ptr() as *const libc::c_char;

    let source_ = CString::new(source.to_string()).unwrap();
    let source_ptr = source_.as_ptr() as *const libc::c_char;

    let id = unsafe {
      libdeno::deno_mod_new(self.libdeno_isolate, main, name_ptr, source_ptr)
    };

    self.check_last_exception().map(|_| id)
  }

  pub fn mod_get_imports(&self, id: deno_mod) -> Vec<String> {
    let len =
      unsafe { libdeno::deno_mod_imports_len(self.libdeno_isolate, id) };
    let mut out = Vec::new();
    for i in 0..len {
      let specifier_ptr =
        unsafe { libdeno::deno_mod_imports_get(self.libdeno_isolate, id, i) };
      let specifier_c: &CStr = unsafe { CStr::from_ptr(specifier_ptr) };
      let specifier: &str = specifier_c.to_str().unwrap();

      out.push(specifier.to_string());
    }
    out
  }

  /// Takes a snapshot. The isolate should have been created with will_snapshot
  /// set to true.
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn snapshot(&mut self) -> Result<Snapshot1<'static>, ErrBox> {
    let snapshot = unsafe { libdeno::deno_snapshot_new(self.libdeno_isolate) };
    match self.check_last_exception() {
      Ok(..) => Ok(snapshot),
      Err(err) => {
        assert_eq!(snapshot.data_ptr, null());
        assert_eq!(snapshot.data_len, 0);
        Err(err)
      }
    }
  }

  fn dyn_import_done(
    &mut self,
    id: libdeno::deno_dyn_import_id,
    result: Result<deno_mod, Option<String>>,
  ) -> Result<(), ErrBox> {
    debug!("dyn_import_done {} {:?}", id, result);
    let (mod_id, maybe_err_str) = match result {
      Ok(mod_id) => (mod_id, None),
      Err(None) => (0, None),
      Err(Some(err_str)) => (0, Some(CString::new(err_str).unwrap())),
    };
    let err_str_ptr = match maybe_err_str {
      Some(ref err_str) => err_str.as_ptr(),
      None => std::ptr::null(),
    };
    unsafe {
      libdeno::deno_dyn_import_done(
        self.libdeno_isolate,
        self.as_raw_ptr(),
        id,
        mod_id,
        err_str_ptr,
      )
    };
    self.check_last_exception()
  }

  fn poll_dyn_imports(&mut self, cx: &mut Context) -> Poll<Result<(), ErrBox>> {
    use RecursiveLoadEvent::*;
    loop {
      match self.pending_dyn_imports.poll_next_unpin(cx) {
        Poll::Pending | Poll::Ready(None) => {
          // There are no active dynamic import loaders, or none are ready.
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Some((
          Some(Ok((dyn_import_id, Fetch(source_code_info)))),
          mut stream,
        ))) => {
          // A module (not necessarily the one dynamically imported) has been
          // fetched. Create and register it, and if successful, poll for the
          // next recursive-load event related to this dynamic import.
          match stream.get_mut().register(source_code_info, self) {
            Ok(()) => self.pending_dyn_imports.push(stream.into_future()),
            Err(err) => {
              self.dyn_import_done(dyn_import_id, Err(Some(err.to_string())))?
            }
          }
        }
        Poll::Ready(Some((
          Some(Ok((dyn_import_id, Instantiate(module_id)))),
          _,
        ))) => {
          // The top-level module from a dynamic import has been instantiated.
          match self.mod_evaluate(module_id) {
            Ok(()) => self.dyn_import_done(dyn_import_id, Ok(module_id))?,
            Err(..) => self.dyn_import_done(dyn_import_id, Err(None))?,
          }
        }
        Poll::Ready(Some((Some(Err((dyn_import_id, err))), _))) => {
          // A non-javascript error occurred; this could be due to a an invalid
          // module specifier, or a problem with the source map, or a failure
          // to fetch the module source code.
          self.dyn_import_done(dyn_import_id, Err(Some(err.to_string())))?
        }
        Poll::Ready(Some((None, _))) => unreachable!(),
      }
    }
  }
}

/// Called during mod_instantiate() to resolve imports.
type ResolveFn<'a> = dyn FnMut(&str, deno_mod) -> deno_mod + 'a;

/// Used internally by Isolate::mod_instantiate to wrap ResolveFn and
/// encapsulate pointer casts.
struct ResolveContext<'a> {
  resolve_fn: &'a mut ResolveFn<'a>,
}

impl<'a> ResolveContext<'a> {
  #[inline]
  fn as_raw_ptr(&mut self) -> *mut c_void {
    self as *mut _ as *mut c_void
  }

  #[inline]
  unsafe fn from_raw_ptr(ptr: *mut c_void) -> &'a mut Self {
    &mut *(ptr as *mut _)
  }
}

impl Isolate {
  /// Instanciates a ES module
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn mod_instantiate(
    &mut self,
    id: deno_mod,
    resolve_fn: &mut ResolveFn,
  ) -> Result<(), ErrBox> {
    let libdeno_isolate = self.libdeno_isolate;
    let mut ctx = ResolveContext { resolve_fn };
    unsafe {
      libdeno::deno_mod_instantiate(
        libdeno_isolate,
        ctx.as_raw_ptr(),
        id,
        Self::resolve_cb,
      )
    };
    self.check_last_exception()
  }

  /// Called during mod_instantiate() only.
  extern "C" fn resolve_cb(
    user_data: *mut libc::c_void,
    specifier_ptr: *const libc::c_char,
    referrer: deno_mod,
  ) -> deno_mod {
    let ResolveContext { resolve_fn } =
      unsafe { ResolveContext::from_raw_ptr(user_data) };
    let specifier_c: &CStr = unsafe { CStr::from_ptr(specifier_ptr) };
    let specifier: &str = specifier_c.to_str().unwrap();

    resolve_fn(specifier, referrer)
  }

  /// Evaluates an already instantiated ES module.
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn mod_evaluate(&mut self, id: deno_mod) -> Result<(), ErrBox> {
    self.shared_init();
    unsafe {
      libdeno::deno_mod_evaluate(self.libdeno_isolate, self.as_raw_ptr(), id)
    };
    self.check_last_exception()
  }
}

struct LockerScope {
  libdeno_isolate: *const libdeno::isolate,
}

impl LockerScope {
  fn new(libdeno_isolate: *const libdeno::isolate) -> LockerScope {
    unsafe { libdeno::deno_lock(libdeno_isolate) }
    LockerScope { libdeno_isolate }
  }
}

impl Drop for LockerScope {
  fn drop(&mut self) {
    unsafe { libdeno::deno_unlock(self.libdeno_isolate) }
  }
}

impl Future for Isolate {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();

    inner.waker.register(cx.waker());

    inner.shared_init();

    let mut overflow_response: Option<(OpId, Buf)> = None;

    loop {
      // If there are any pending dyn_import futures, do those first.
      if !inner.pending_dyn_imports.is_empty() {
        let poll_imports = inner.poll_dyn_imports(cx)?;
        assert!(poll_imports.is_ready());
      }

      // Now handle actual ops.
      inner.have_unpolled_ops = false;
      #[allow(clippy::match_wild_err_arm)]
      match inner.pending_ops.poll_next_unpin(cx) {
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
      // Lock the current thread for V8.
      let locker = LockerScope::new(inner.libdeno_isolate);
      inner.respond(None)?;
      // The other side should have shifted off all the messages.
      assert_eq!(inner.shared.size(), 0);
      drop(locker);
    }

    if overflow_response.is_some() {
      // Lock the current thread for V8.
      let locker = LockerScope::new(inner.libdeno_isolate);
      let (op_id, buf) = overflow_response.take().unwrap();
      inner.respond(Some((op_id, &buf)))?;
      drop(locker);
    }

    inner.check_promise_errors();
    inner.check_last_exception()?;

    // We're idle if pending_ops is empty.
    if inner.pending_ops.is_empty() && inner.pending_dyn_imports.is_empty() {
      Poll::Ready(Ok(()))
    } else {
      if inner.have_unpolled_ops {
        inner.waker.wake();
      }
      Poll::Pending
    }
  }
}

/// IsolateHandle is a thread safe handle on an Isolate. It exposed thread safe V8 functions.
#[derive(Clone)]
pub struct IsolateHandle {
  shared_libdeno_isolate: Arc<Mutex<Option<*const libdeno::isolate>>>,
}

unsafe impl Send for IsolateHandle {}

impl IsolateHandle {
  /// Terminate the execution of any currently running javascript.
  /// After terminating execution it is probably not wise to continue using
  /// the isolate.
  pub fn terminate_execution(&self) {
    unsafe {
      if let Some(isolate) = *self.shared_libdeno_isolate.lock().unwrap() {
        libdeno::deno_terminate_execution(isolate)
      }
    }
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
  use std::io;
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
    OverflowReqSync,
    OverflowResSync,
    OverflowReqAsync,
    OverflowResAsync,
  }

  pub fn setup(mode: Mode) -> (Isolate, Arc<AtomicUsize>) {
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let mut isolate = Isolate::new(StartupData::None, false);

    let dispatcher =
      move |control: &[u8], _zero_copy: Option<PinnedBuf>| -> CoreOp {
        dispatch_count_.fetch_add(1, Ordering::Relaxed);
        match mode {
          Mode::Async => {
            assert_eq!(control.len(), 1);
            assert_eq!(control[0], 42);
            let buf = vec![43u8, 0, 0, 0].into_boxed_slice();
            Op::Async(futures::future::ok(buf).boxed())
          }
          Mode::OverflowReqSync => {
            assert_eq!(control.len(), 100 * 1024 * 1024);
            let buf = vec![43u8, 0, 0, 0].into_boxed_slice();
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
            let buf = vec![43u8, 0, 0, 0].into_boxed_slice();
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
  fn test_mods() {
    let (mut isolate, dispatch_count) = setup(Mode::Async);
    let mod_a = isolate
      .mod_new(
        true,
        "a.js",
        r#"
        import { b } from 'b.js'
        if (b() != 'b') throw Error();
        let control = new Uint8Array([42]);
        Deno.core.send(1, control);
      "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let imports = isolate.mod_get_imports(mod_a);
    assert_eq!(imports, vec!["b.js".to_string()]);

    let mod_b = isolate
      .mod_new(false, "b.js", "export function b() { return 'b' }")
      .unwrap();
    let imports = isolate.mod_get_imports(mod_b);
    assert_eq!(imports.len(), 0);

    let resolve_count = Arc::new(AtomicUsize::new(0));
    let resolve_count_ = resolve_count.clone();

    let mut resolve = move |specifier: &str, _referrer: deno_mod| -> deno_mod {
      resolve_count_.fetch_add(1, Ordering::SeqCst);
      assert_eq!(specifier, "b.js");
      mod_b
    };

    js_check(isolate.mod_instantiate(mod_b, &mut resolve));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 0);

    js_check(isolate.mod_instantiate(mod_a, &mut resolve));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    js_check(isolate.mod_evaluate(mod_a));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn test_poll_async_delayed_ops() {
    run_in_task(|cx| {
      let (mut isolate, dispatch_count) = setup(Mode::Async);

      js_check(isolate.execute(
        "setup2.js",
        r#"
         let nrecv = 0;
         Deno.core.setAsyncHandler((opId, buf) => {
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

  struct MockImportStream(Vec<Result<RecursiveLoadEvent, ErrBox>>);

  impl Stream for MockImportStream {
    type Item = Result<RecursiveLoadEvent, ErrBox>;

    fn poll_next(
      self: Pin<&mut Self>,
      _cx: &mut Context,
    ) -> Poll<Option<Self::Item>> {
      let inner = self.get_mut();
      let event = if inner.0.is_empty() {
        None
      } else {
        Some(inner.0.remove(0))
      };
      Poll::Ready(event)
    }
  }

  impl ImportStream for MockImportStream {
    fn register(
      &mut self,
      module_data: SourceCodeInfo,
      isolate: &mut Isolate,
    ) -> Result<(), ErrBox> {
      let id = isolate.mod_new(
        false,
        &module_data.module_url_found,
        &module_data.code,
      )?;
      println!(
        "MockImportStream register {} {}",
        id, module_data.module_url_found
      );
      Ok(())
    }
  }

  #[test]
  fn dyn_import_err() {
    // Test an erroneous dynamic import where the specified module isn't found.
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();
      let mut isolate = Isolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_, specifier, referrer| {
        count_.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "foo.js");
        assert_eq!(referrer, "dyn_import2.js");
        let err = io::Error::from(io::ErrorKind::NotFound);
        let stream = MockImportStream(vec![Err(err.into())]);
        Box::new(stream)
      });
      js_check(isolate.execute(
        "dyn_import2.js",
        r#"
        (async () => {
          await import("foo.js");
        })();
        "#,
      ));
      assert_eq!(count.load(Ordering::Relaxed), 1);

      // We should get an error here.
      let result = isolate.poll_unpin(cx);
      if let Poll::Ready(Ok(_)) = result {
        unreachable!();
      }
    })
  }

  #[test]
  fn dyn_import_err2() {
    use std::convert::TryInto;
    // Import multiple modules to demonstrate that after failed dynamic import
    // another dynamic import can still be run
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();
      let mut isolate = Isolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_, specifier, referrer| {
        let c = count_.fetch_add(1, Ordering::Relaxed);
        match c {
          0 => assert_eq!(specifier, "foo1.js"),
          1 => assert_eq!(specifier, "foo2.js"),
          2 => assert_eq!(specifier, "foo3.js"),
          _ => unreachable!(),
        }
        assert_eq!(referrer, "dyn_import_error.js");

        let source_code_info = SourceCodeInfo {
          module_url_specified: specifier.to_owned(),
          module_url_found: specifier.to_owned(),
          code: "# not valid JS".to_owned(),
        };
        let stream = MockImportStream(vec![
          Ok(RecursiveLoadEvent::Fetch(source_code_info)),
          Ok(RecursiveLoadEvent::Instantiate(c.try_into().unwrap())),
        ]);
        Box::new(stream)
      });

      js_check(isolate.execute(
        "dyn_import_error.js",
        r#"
        (async () => {
          await import("foo1.js");
        })();
        (async () => {
          await import("foo2.js");
        })();
        (async () => {
          await import("foo3.js");
        })();
        "#,
      ));

      assert_eq!(count.load(Ordering::Relaxed), 3);
      // Now each poll should return error
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
    })
  }

  #[test]
  fn dyn_import_ok() {
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();

      // Sometimes Rust is really annoying.
      let mod_b = Arc::new(Mutex::new(0));
      let mod_b2 = mod_b.clone();

      let mut isolate = Isolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_id, specifier, referrer| {
        let c = count_.fetch_add(1, Ordering::Relaxed);
        match c {
          0 => assert_eq!(specifier, "foo1.js"),
          1 => assert_eq!(specifier, "foo2.js"),
          _ => unreachable!(),
        }
        assert_eq!(referrer, "dyn_import3.js");
        let mod_id = *mod_b2.lock().unwrap();
        let source_code_info = SourceCodeInfo {
          module_url_specified: "foo.js".to_owned(),
          module_url_found: "foo.js".to_owned(),
          code: "".to_owned(),
        };
        let stream = MockImportStream(vec![
          Ok(RecursiveLoadEvent::Fetch(source_code_info)),
          Ok(RecursiveLoadEvent::Instantiate(mod_id)),
        ]);
        Box::new(stream)
      });

      // Instantiate mod_b
      {
        let mut mod_id = mod_b.lock().unwrap();
        *mod_id = isolate
          .mod_new(false, "b.js", "export function b() { return 'b' }")
          .unwrap();
        let mut resolve = move |_specifier: &str,
                                _referrer: deno_mod|
              -> deno_mod { unreachable!() };
        js_check(isolate.mod_instantiate(*mod_id, &mut resolve));
      }
      // Dynamically import mod_b
      js_check(isolate.execute(
        "dyn_import3.js",
        r#"
          (async () => {
            let mod = await import("foo1.js");
            if (mod.b() !== 'b') {
              throw Error("bad1");
            }
            // And again!
            mod = await import("foo2.js");
            if (mod.b() !== 'b') {
              throw Error("bad2");
            }
          })();
          "#,
      ));

      assert_eq!(count.load(Ordering::Relaxed), 1);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(count.load(Ordering::Relaxed), 2);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(count.load(Ordering::Relaxed), 2);
    })
  }

  #[test]
  fn terminate_execution() {
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let tx_clone = tx.clone();

    let (mut isolate, _dispatch_count) = setup(Mode::Async);
    let shared = isolate.shared_isolate_handle();

    let t1 = std::thread::spawn(move || {
      // allow deno to boot and run
      std::thread::sleep(std::time::Duration::from_millis(100));

      // terminate execution
      shared.terminate_execution();

      // allow shutdown
      std::thread::sleep(std::time::Duration::from_millis(200));

      // unless reported otherwise the test should fail after this point
      tx_clone.send(false).ok();
    });

    let t2 = std::thread::spawn(move || {
      // run an infinite loop
      let res = isolate.execute(
        "infinite_loop.js",
        r#"
          let i = 0;
          while (true) { i++; }
        "#,
      );

      // execute() terminated, which means terminate_execution() was successful.
      tx.send(true).ok();

      if let Err(e) = res {
        assert_eq!(e.to_string(), "Uncaught Error: execution terminated");
      } else {
        panic!("should return an error");
      }

      // make sure the isolate is still unusable
      let res = isolate.execute("simple.js", "1+1;");
      if let Err(e) = res {
        assert_eq!(e.to_string(), "Uncaught Error: execution terminated");
      } else {
        panic!("should return an error");
      }
    });

    if !rx.recv().unwrap() {
      panic!("should have terminated")
    }

    t1.join().unwrap();
    t2.join().unwrap();
  }

  #[test]
  fn dangling_shared_isolate() {
    let shared = {
      // isolate is dropped at the end of this block
      let (mut isolate, _dispatch_count) = setup(Mode::Async);
      isolate.shared_isolate_handle()
    };

    // this should not SEGFAULT
    shared.terminate_execution();
  }

  #[test]
  fn overflow_req_sync() {
    let (mut isolate, dispatch_count) = setup(Mode::OverflowReqSync);
    js_check(isolate.execute(
      "overflow_req_sync.js",
      r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler((opId, buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array(100 * 1024 * 1024);
        let response = Deno.core.dispatch(1, control);
        assert(response instanceof Uint8Array);
        assert(response.length == 4);
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
        Deno.core.setAsyncHandler((opId, buf) => { asyncRecv++ });
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
         Deno.core.setAsyncHandler((opId, buf) => {
           assert(opId == 1);
           assert(buf.byteLength === 4);
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
         Deno.core.setAsyncHandler((opId, buf) => {
           assert(opId == 1);
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
         Deno.core.setAsyncHandler((opId, buf) => {
           assert(opId === 1);
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
          assert(thrown == "Unknown op id: 100");
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
  fn will_snapshot() {
    let snapshot = {
      let mut isolate = Isolate::new(StartupData::None, true);
      js_check(isolate.execute("a.js", "a = 1 + 2"));
      let s = isolate.snapshot().unwrap();
      drop(isolate);
      s
    };

    let startup_data = StartupData::LibdenoSnapshot(snapshot);
    let mut isolate2 = Isolate::new(startup_data, false);
    js_check(isolate2.execute("check.js", "if (a != 3) throw Error('x')"));
  }
}
