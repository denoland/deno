// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Do not use FlatBuffers in this module.
// TODO Currently this module uses Tokio, but it would be nice if they were
// decoupled.

use crate::compiler::compile_sync;
use crate::compiler::CodeFetchOutput;
use crate::deno_dir;
use crate::errors::DenoError;
use crate::errors::DenoResult;
use crate::flags;
use crate::js_errors::JSError;
use crate::libdeno;
use crate::msg;
use crate::permissions::DenoPermissions;
use crate::tokio_util;

use futures::sync::mpsc as async_mpsc;
use futures::Future;
use libc::c_char;
use libc::c_void;
use std;
use std::cell::Cell;
use std::env;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use tokio;

// Buf represents a byte array returned from a "Op".
// The message might be empty (which will be translated into a null object on
// the javascript side) or it is a heap allocated opaque sequence of bytes.
// Usually a flatbuffer message.
pub type Buf = Box<[u8]>;

// JS promises in Deno map onto a specific Future
// which yields either a DenoError or a byte array.
pub type Op = dyn Future<Item = Buf, Error = DenoError> + Send;

// Returns (is_sync, op)
pub type Dispatch =
  fn(isolate: &Isolate, buf: libdeno::deno_buf, data_buf: libdeno::deno_buf)
    -> (bool, Box<Op>);

pub struct Isolate {
  libdeno_isolate: *const libdeno::isolate,
  dispatch: Dispatch,
  rx: mpsc::Receiver<(i32, Buf)>,
  tx: mpsc::Sender<(i32, Buf)>,
  ntasks: Cell<i32>,
  timeout_due: Cell<Option<Instant>>,
  pub state: Arc<IsolateState>,
}

pub type WorkerSender = async_mpsc::Sender<Buf>;
pub type WorkerReceiver = async_mpsc::Receiver<Buf>;
pub type WorkerChannels = (WorkerSender, WorkerReceiver);

// Isolate cannot be passed between threads but IsolateState can.
// IsolateState satisfies Send and Sync.
// So any state that needs to be accessed outside the main V8 thread should be
// inside IsolateState.
#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct IsolateState {
  pub dir: deno_dir::DenoDir,
  pub argv: Vec<String>,
  pub permissions: DenoPermissions,
  pub flags: flags::DenoFlags,
  pub metrics: Metrics,
  pub worker_channels: Option<Mutex<WorkerChannels>>,
}

impl IsolateState {
  pub fn new(
    flags: flags::DenoFlags,
    argv_rest: Vec<String>,
    worker_channels: Option<WorkerChannels>,
  ) -> Self {
    let custom_root = env::var("DENO_DIR").map(|s| s.into()).ok();

    Self {
      dir: deno_dir::DenoDir::new(flags.reload, custom_root).unwrap(),
      argv: argv_rest,
      permissions: DenoPermissions::new(&flags),
      flags,
      metrics: Metrics::default(),
      worker_channels: worker_channels.map(Mutex::new),
    }
  }

  #[cfg(test)]
  pub fn mock() -> Arc<IsolateState> {
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    // For debugging: argv.push_back(String::from("-D"));
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();
    Arc::new(IsolateState::new(flags, rest_argv, None))
  }

  #[inline]
  pub fn check_write(&self, filename: &str) -> DenoResult<()> {
    self.permissions.check_write(filename)
  }

  #[inline]
  pub fn check_env(&self) -> DenoResult<()> {
    self.permissions.check_env()
  }

  #[inline]
  pub fn check_net(&self, filename: &str) -> DenoResult<()> {
    self.permissions.check_net(filename)
  }

  #[inline]
  pub fn check_run(&self) -> DenoResult<()> {
    self.permissions.check_run()
  }

  fn metrics_op_dispatched(
    &self,
    bytes_sent_control: usize,
    bytes_sent_data: usize,
  ) {
    self.metrics.ops_dispatched.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_sent_control
      .fetch_add(bytes_sent_control, Ordering::SeqCst);
    self
      .metrics
      .bytes_sent_data
      .fetch_add(bytes_sent_data, Ordering::SeqCst);
  }

  fn metrics_op_completed(&self, bytes_received: usize) {
    self.metrics.ops_completed.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_received
      .fetch_add(bytes_received, Ordering::SeqCst);
  }
}

// AtomicU64 is currently unstable
#[derive(Default)]
pub struct Metrics {
  pub ops_dispatched: AtomicUsize,
  pub ops_completed: AtomicUsize,
  pub bytes_sent_control: AtomicUsize,
  pub bytes_sent_data: AtomicUsize,
  pub bytes_received: AtomicUsize,
}

static DENO_INIT: std::sync::Once = std::sync::ONCE_INIT;

impl Isolate {
  pub fn new(
    snapshot: libdeno::deno_buf,
    state: Arc<IsolateState>,
    dispatch: Dispatch,
  ) -> Self {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });
    let config = libdeno::deno_config {
      will_snapshot: 0,
      load_snapshot: snapshot,
      shared: libdeno::deno_buf::empty(), // TODO Use for message passing.
      recv_cb: pre_dispatch,
      resolve_cb,
    };
    let libdeno_isolate = unsafe { libdeno::deno_new(config) };
    // This channel handles sending async messages back to the runtime.
    let (tx, rx) = mpsc::channel::<(i32, Buf)>();

    Self {
      libdeno_isolate,
      dispatch,
      rx,
      tx,
      ntasks: Cell::new(0),
      timeout_due: Cell::new(None),
      state,
    }
  }

  #[inline]
  pub fn as_raw_ptr(&self) -> *const c_void {
    self as *const _ as *const c_void
  }

  #[inline]
  pub unsafe fn from_raw_ptr<'a>(ptr: *const c_void) -> &'a Self {
    let ptr = ptr as *const _;
    &*ptr
  }

  #[inline]
  pub fn get_timeout_due(&self) -> Option<Instant> {
    self.timeout_due.clone().into_inner()
  }

  #[inline]
  pub fn set_timeout_due(&self, inst: Option<Instant>) {
    self.timeout_due.set(inst);
  }

  pub fn last_exception(&self) -> Option<JSError> {
    let ptr = unsafe { libdeno::deno_last_exception(self.libdeno_isolate) };
    if ptr.is_null() {
      None
    } else {
      let cstr = unsafe { CStr::from_ptr(ptr) };
      let v8_exception = cstr.to_str().unwrap();
      debug!("v8_exception\n{}\n", v8_exception);
      let js_error = JSError::from_v8_exception(v8_exception).unwrap();
      let js_error_mapped = js_error.apply_source_map(&self.state.dir);
      Some(js_error_mapped)
    }
  }

  /// Same as execute2() but the filename defaults to "<anonymous>".
  pub fn execute(&self, js_source: &str) -> Result<(), JSError> {
    self.execute2("<anonymous>", js_source)
  }

  /// Executes the provided JavaScript source code. The js_filename argument is
  /// provided only for debugging purposes.
  pub fn execute2(
    &self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), JSError> {
    let filename = CString::new(js_filename).unwrap();
    let source = CString::new(js_source).unwrap();
    let r = unsafe {
      libdeno::deno_execute(
        self.libdeno_isolate,
        self.as_raw_ptr(),
        filename.as_ptr(),
        source.as_ptr(),
      )
    };
    if r == 0 {
      let js_error = self.last_exception().unwrap();
      return Err(js_error);
    }
    Ok(())
  }

  /// Executes the provided JavaScript module.
  pub fn execute_mod(
    &self,
    js_filename: &str,
    is_prefetch: bool,
  ) -> Result<(), JSError> {
    let out =
      code_fetch_and_maybe_compile(&self.state, js_filename, ".").unwrap();

    let filename = CString::new(out.filename.clone()).unwrap();
    let filename_ptr = filename.as_ptr() as *const i8;

    let js_source = CString::new(out.js_source().clone()).unwrap();
    let js_source = CString::new(js_source).unwrap();
    let js_source_ptr = js_source.as_ptr() as *const i8;

    let r = unsafe {
      libdeno::deno_execute_mod(
        self.libdeno_isolate,
        self.as_raw_ptr(),
        filename_ptr,
        js_source_ptr,
        if is_prefetch { 1 } else { 0 },
      )
    };
    if r == 0 {
      let js_error = self.last_exception().unwrap();
      return Err(js_error);
    }
    Ok(())
  }

  pub fn respond(&self, req_id: i32, buf: Buf) {
    self.state.metrics_op_completed(buf.len());
    // deno_respond will memcpy the buf into V8's heap,
    // so borrowing a reference here is sufficient.
    unsafe {
      libdeno::deno_respond(
        self.libdeno_isolate,
        self.as_raw_ptr(),
        req_id,
        buf.as_ref().into(),
      )
    }
  }

  fn complete_op(&self, req_id: i32, buf: Buf) {
    // Receiving a message on rx exactly corresponds to an async task
    // completing.
    self.ntasks_decrement();
    // Call into JS with the buf.
    self.respond(req_id, buf);
  }

  fn timeout(&self) {
    let dummy_buf = libdeno::deno_buf::empty();
    unsafe {
      libdeno::deno_respond(
        self.libdeno_isolate,
        self.as_raw_ptr(),
        -1,
        dummy_buf,
      )
    }
  }

  fn check_promise_errors(&self) {
    unsafe {
      libdeno::deno_check_promise_errors(self.libdeno_isolate);
    }
  }

  // TODO Use Park abstraction? Note at time of writing Tokio default runtime
  // does not have new_with_park().
  pub fn event_loop(&self) -> Result<(), JSError> {
    // Main thread event loop.
    while !self.is_idle() {
      match recv_deadline(&self.rx, self.get_timeout_due()) {
        Ok((req_id, buf)) => self.complete_op(req_id, buf),
        Err(mpsc::RecvTimeoutError::Timeout) => self.timeout(),
        Err(e) => panic!("recv_deadline() failed: {:?}", e),
      }
      self.check_promise_errors();
      if let Some(err) = self.last_exception() {
        return Err(err);
      }
    }
    // Check on done
    self.check_promise_errors();
    if let Some(err) = self.last_exception() {
      return Err(err);
    }
    Ok(())
  }

  #[inline]
  fn ntasks_increment(&self) {
    assert!(self.ntasks.get() >= 0);
    self.ntasks.set(self.ntasks.get() + 1);
  }

  #[inline]
  fn ntasks_decrement(&self) {
    self.ntasks.set(self.ntasks.get() - 1);
    assert!(self.ntasks.get() >= 0);
  }

  #[inline]
  fn is_idle(&self) -> bool {
    self.ntasks.get() == 0 && self.get_timeout_due().is_none()
  }
}

impl Drop for Isolate {
  fn drop(&mut self) {
    unsafe { libdeno::deno_delete(self.libdeno_isolate) }
  }
}

fn code_fetch_and_maybe_compile(
  state: &Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
) -> Result<CodeFetchOutput, DenoError> {
  let mut out = state.dir.code_fetch(specifier, referrer)?;
  if (out.media_type == msg::MediaType::TypeScript
    && out.maybe_output_code.is_none())
    || state.flags.recompile
  {
    debug!(">>>>> compile_sync START");
    out = compile_sync(state, specifier, &referrer).unwrap();
    debug!(">>>>> compile_sync END");
  }
  Ok(out)
}

extern "C" fn resolve_cb(
  user_data: *mut c_void,
  specifier_ptr: *const c_char,
  referrer_ptr: *const c_char,
) {
  let specifier_c: &CStr = unsafe { CStr::from_ptr(specifier_ptr) };
  let specifier: &str = specifier_c.to_str().unwrap();

  let referrer_c: &CStr = unsafe { CStr::from_ptr(referrer_ptr) };
  let referrer: &str = referrer_c.to_str().unwrap();

  debug!("module_resolve callback {} {}", specifier, referrer);
  let isolate = unsafe { Isolate::from_raw_ptr(user_data) };

  let maybe_out =
    code_fetch_and_maybe_compile(&isolate.state, specifier, referrer);

  if maybe_out.is_err() {
    // Resolution failure
    return;
  }

  let out = maybe_out.unwrap();

  let filename = CString::new(out.filename.clone()).unwrap();
  let filename_ptr = filename.as_ptr() as *const i8;

  let js_source = CString::new(out.js_source().clone()).unwrap();
  let js_source_ptr = js_source.as_ptr() as *const i8;

  unsafe {
    libdeno::deno_resolve_ok(
      isolate.libdeno_isolate,
      filename_ptr,
      js_source_ptr,
    )
  };
}

// Dereferences the C pointer into the Rust Isolate object.
extern "C" fn pre_dispatch(
  user_data: *mut c_void,
  req_id: i32,
  control_buf: libdeno::deno_buf,
  data_buf: libdeno::deno_buf,
) {
  // for metrics
  let bytes_sent_control = control_buf.len();
  let bytes_sent_data = data_buf.len();

  // We should ensure that there is no other `&mut Isolate` exists.
  // And also, it should be in the same thread with other `&Isolate`s.
  let isolate = unsafe { Isolate::from_raw_ptr(user_data) };
  let dispatch = isolate.dispatch;
  let (is_sync, op) = dispatch(isolate, control_buf, data_buf);

  isolate
    .state
    .metrics_op_dispatched(bytes_sent_control, bytes_sent_data);

  if is_sync {
    // Execute op synchronously.
    let buf = tokio_util::block_on(op).unwrap();
    let buf_size = buf.len();

    if buf_size == 0 {
      // FIXME
      isolate.state.metrics_op_completed(buf.len());
    } else {
      // Set the synchronous response, the value returned from isolate.send().
      isolate.respond(req_id, buf);
    }
  } else {
    // Execute op asynchronously.
    let tx = isolate.tx.clone();

    // TODO Ideally Tokio would could tell us how many tasks are executing, but
    // it cannot currently. Therefore we track top-level promises/tasks
    // manually.
    isolate.ntasks_increment();

    let task = op
      .and_then(move |buf| {
        let sender = tx; // tx is moved to new thread
        sender.send((req_id, buf)).expect("tx.send error");
        Ok(())
      }).map_err(|_| ());
    tokio::spawn(task);
  }
}

fn recv_deadline<T>(
  rx: &mpsc::Receiver<T>,
  maybe_due: Option<Instant>,
) -> Result<T, mpsc::RecvTimeoutError> {
  match maybe_due {
    None => rx.recv().map_err(|e| e.into()),
    Some(due) => {
      // Subtracting two Instants causes a panic if the resulting duration
      // would become negative. Avoid this.
      let now = Instant::now();
      let timeout = if due > now {
        due - now
      } else {
        Duration::new(0, 0)
      };
      // TODO: use recv_deadline() instead of recv_timeout() when this
      // feature becomes stable/available.
      rx.recv_timeout(timeout)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures;

  #[test]
  fn test_dispatch_sync() {
    let state = IsolateState::mock();
    let snapshot = libdeno::deno_buf::empty();
    let isolate = Isolate::new(snapshot, state, dispatch_sync);
    tokio_util::init(|| {
      isolate
        .execute(
          r#"
          const m = new Uint8Array([4, 5, 6]);
          let n = libdeno.send(m);
          if (!(n.byteLength === 3 &&
                n[0] === 1 &&
                n[1] === 2 &&
                n[2] === 3)) {
            throw Error("assert error");
          }
        "#,
        ).expect("execute error");
      isolate.event_loop().ok();
    });
  }

  fn dispatch_sync(
    _isolate: &Isolate,
    control: libdeno::deno_buf,
    data: libdeno::deno_buf,
  ) -> (bool, Box<Op>) {
    assert_eq!(control[0], 4);
    assert_eq!(control[1], 5);
    assert_eq!(control[2], 6);
    assert_eq!(data.len(), 0);
    // Send back some sync response.
    let vec: Vec<u8> = vec![1, 2, 3];
    let control = vec.into_boxed_slice();
    let op = Box::new(futures::future::ok(control));
    (true, op)
  }

  #[test]
  fn test_metrics_sync() {
    let state = IsolateState::mock();
    let snapshot = libdeno::deno_buf::empty();
    let isolate = Isolate::new(snapshot, state, metrics_dispatch_sync);
    tokio_util::init(|| {
      // Verify that metrics have been properly initialized.
      {
        let metrics = &isolate.state.metrics;
        assert_eq!(metrics.ops_dispatched.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.ops_completed.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.bytes_sent_control.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.bytes_sent_data.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.bytes_received.load(Ordering::SeqCst), 0);
      }

      isolate
        .execute(
          r#"
          const control = new Uint8Array([4, 5, 6]);
          const data = new Uint8Array([42, 43, 44, 45, 46]);
          libdeno.send(control, data);
        "#,
        ).expect("execute error");;
      isolate.event_loop().unwrap();
      let metrics = &isolate.state.metrics;
      assert_eq!(metrics.ops_dispatched.load(Ordering::SeqCst), 1);
      assert_eq!(metrics.ops_completed.load(Ordering::SeqCst), 1);
      assert_eq!(metrics.bytes_sent_control.load(Ordering::SeqCst), 3);
      assert_eq!(metrics.bytes_sent_data.load(Ordering::SeqCst), 5);
      assert_eq!(metrics.bytes_received.load(Ordering::SeqCst), 4);
    });
  }

  #[test]
  fn test_metrics_async() {
    let state = IsolateState::mock();
    let snapshot = libdeno::deno_buf::empty();
    let isolate = Isolate::new(snapshot, state, metrics_dispatch_async);
    tokio_util::init(|| {
      // Verify that metrics have been properly initialized.
      {
        let metrics = &isolate.state.metrics;
        assert_eq!(metrics.ops_dispatched.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.ops_completed.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.bytes_sent_control.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.bytes_sent_data.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.bytes_received.load(Ordering::SeqCst), 0);
      }

      isolate
        .execute(
          r#"
          const control = new Uint8Array([4, 5, 6]);
          const data = new Uint8Array([42, 43, 44, 45, 46]);
          let r = libdeno.send(control, data);
          libdeno.recv(() => {});
          if (r != null) throw Error("expected null");
        "#,
        ).expect("execute error");

      // Make sure relevant metrics are updated before task is executed.
      {
        let metrics = &isolate.state.metrics;
        assert_eq!(metrics.ops_dispatched.load(Ordering::SeqCst), 1);
        assert_eq!(metrics.bytes_sent_control.load(Ordering::SeqCst), 3);
        assert_eq!(metrics.bytes_sent_data.load(Ordering::SeqCst), 5);
        // Note we cannot check ops_completed nor bytes_received because that
        // would be a race condition. It might be nice to have use a oneshot
        // with metrics_dispatch_async() to properly validate them.
      }

      isolate.event_loop().unwrap();

      // Make sure relevant metrics are updated after task is executed.
      {
        let metrics = &isolate.state.metrics;
        assert_eq!(metrics.ops_dispatched.load(Ordering::SeqCst), 1);
        assert_eq!(metrics.ops_completed.load(Ordering::SeqCst), 1);
        assert_eq!(metrics.bytes_sent_control.load(Ordering::SeqCst), 3);
        assert_eq!(metrics.bytes_sent_data.load(Ordering::SeqCst), 5);
        assert_eq!(metrics.bytes_received.load(Ordering::SeqCst), 4);
      }
    });
  }

  fn metrics_dispatch_sync(
    _isolate: &Isolate,
    _control: libdeno::deno_buf,
    _data: libdeno::deno_buf,
  ) -> (bool, Box<Op>) {
    // Send back some sync response
    let vec: Box<[u8]> = vec![1, 2, 3, 4].into_boxed_slice();
    let op = Box::new(futures::future::ok(vec));
    (true, op)
  }

  fn metrics_dispatch_async(
    _isolate: &Isolate,
    _control: libdeno::deno_buf,
    _data: libdeno::deno_buf,
  ) -> (bool, Box<Op>) {
    // Send back some sync response
    let vec: Box<[u8]> = vec![1, 2, 3, 4].into_boxed_slice();
    let op = Box::new(futures::future::ok(vec));
    (false, op)
  }

  #[test]
  fn thread_safety() {
    fn is_thread_safe<T: Sync + Send>() {}
    is_thread_safe::<IsolateState>();
  }

  #[test]
  fn execute_mod() {
    let filename = std::env::current_dir()
      .unwrap()
      .join("tests/esm_imports_a.js");
    let filename = filename.to_str().unwrap();

    let argv = vec![String::from("./deno"), String::from(filename)];
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();

    let state = Arc::new(IsolateState::new(flags, rest_argv, None));
    let snapshot = libdeno::deno_buf::empty();
    let isolate = Isolate::new(snapshot, state, dispatch_sync);
    tokio_util::init(|| {
      isolate
        .execute_mod(filename, false)
        .expect("execute_mod error");
      isolate.event_loop().ok();
    });
  }
}
