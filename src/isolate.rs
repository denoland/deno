// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Do not use FlatBuffers in this module.
// TODO Currently this module uses Tokio, but it would be nice if they were
// decoupled.

use deno_dir;
use errors::DenoError;
use errors::DenoResult;
use flags;
use libdeno;
use permissions::DenoPermissions;

use futures::Future;
use libc::c_void;
use std;
use std::env;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio;
use tokio_util;

type DenoException<'a> = &'a str;

// Buf represents a byte array returned from a "Op".
// The message might be empty (which will be translated into a null object on
// the javascript side) or it is a heap allocated opaque sequence of bytes.
// Usually a flatbuffer message.
pub type Buf = Box<[u8]>;

// JS promises in Deno map onto a specific Future
// which yields either a DenoError or a byte array.
pub type Op = Future<Item = Buf, Error = DenoError> + Send;

// Returns (is_sync, op)
pub type Dispatch =
  fn(isolate: &mut Isolate, buf: &[u8], data_buf: &'static mut [u8])
    -> (bool, Box<Op>);

pub struct Isolate {
  libdeno_isolate: *const libdeno::isolate,
  dispatch: Dispatch,
  rx: mpsc::Receiver<(i32, Buf)>,
  tx: mpsc::Sender<(i32, Buf)>,
  ntasks: i32,
  pub timeout_due: Option<Instant>,
  pub state: Arc<IsolateState>,
}

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
}

impl IsolateState {
  pub fn new(flags: flags::DenoFlags, argv_rest: Vec<String>) -> Self {
    let custom_root = env::var("DENO_DIR").map(|s| s.into()).ok();
    Self {
      dir: deno_dir::DenoDir::new(flags.reload, custom_root).unwrap(),
      argv: argv_rest,
      permissions: DenoPermissions::new(&flags),
      flags,
      metrics: Metrics::default(),
    }
  }

  pub fn check_write(&self, filename: &str) -> DenoResult<()> {
    self.permissions.check_write(filename)
  }

  pub fn check_env(&self) -> DenoResult<()> {
    self.permissions.check_env()
  }

  pub fn check_net(&self, filename: &str) -> DenoResult<()> {
    self.permissions.check_net(filename)
  }

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
    let shared = libdeno::deno_buf::empty(); // TODO Use shared for message passing.
    let libdeno_isolate =
      unsafe { libdeno::deno_new(snapshot, shared, pre_dispatch) };
    // This channel handles sending async messages back to the runtime.
    let (tx, rx) = mpsc::channel::<(i32, Buf)>();

    Self {
      libdeno_isolate,
      dispatch,
      rx,
      tx,
      ntasks: 0,
      timeout_due: None,
      state,
    }
  }

  pub fn as_void_ptr(&mut self) -> *mut c_void {
    self as *mut _ as *mut c_void
  }

  pub fn from_void_ptr<'a>(ptr: *mut c_void) -> &'a mut Self {
    let ptr = ptr as *mut _;
    unsafe { &mut *ptr }
  }

  pub fn execute(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), DenoException> {
    let filename = CString::new(js_filename).unwrap();
    let source = CString::new(js_source).unwrap();
    let r = unsafe {
      libdeno::deno_execute(
        self.libdeno_isolate,
        self.as_void_ptr(),
        filename.as_ptr(),
        source.as_ptr(),
      )
    };
    if r == 0 {
      let ptr = unsafe { libdeno::deno_last_exception(self.libdeno_isolate) };
      let cstr = unsafe { CStr::from_ptr(ptr) };
      return Err(cstr.to_str().unwrap());
    }
    Ok(())
  }

  pub fn respond(&mut self, req_id: i32, buf: Buf) {
    self.state.metrics_op_completed(buf.len());

    // TODO(zero-copy) Use Buf::leak(buf) to leak the heap allocated buf. And
    // don't do the memcpy in ImportBuf() (in libdeno/binding.cc)
    unsafe {
      libdeno::deno_respond(
        self.libdeno_isolate,
        self.as_void_ptr(),
        req_id,
        buf.into(),
      )
    }
  }

  fn complete_op(&mut self, req_id: i32, buf: Buf) {
    // Receiving a message on rx exactly corresponds to an async task
    // completing.
    self.ntasks_decrement();
    // Call into JS with the buf.
    self.respond(req_id, buf);
  }

  fn timeout(&mut self) {
    let dummy_buf = libdeno::deno_buf {
      alloc_ptr: std::ptr::null_mut(),
      alloc_len: 0,
      data_ptr: std::ptr::null_mut(),
      data_len: 0,
    };
    unsafe {
      libdeno::deno_respond(
        self.libdeno_isolate,
        self.as_void_ptr(),
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
  pub fn event_loop(&mut self) {
    // Main thread event loop.
    while !self.is_idle() {
      match recv_deadline(&self.rx, self.timeout_due) {
        Ok((req_id, buf)) => self.complete_op(req_id, buf),
        Err(mpsc::RecvTimeoutError::Timeout) => self.timeout(),
        Err(e) => panic!("recv_deadline() failed: {:?}", e),
      }
      self.check_promise_errors();
    }
    // Check on done
    self.check_promise_errors();
  }

  fn ntasks_increment(&mut self) {
    assert!(self.ntasks >= 0);
    self.ntasks += 1;
  }

  fn ntasks_decrement(&mut self) {
    self.ntasks -= 1;
    assert!(self.ntasks >= 0);
  }

  fn is_idle(&self) -> bool {
    self.ntasks == 0 && self.timeout_due.is_none()
  }
}

impl Drop for Isolate {
  fn drop(&mut self) {
    unsafe { libdeno::deno_delete(self.libdeno_isolate) }
  }
}

/// Converts Rust Buf to libdeno `deno_buf`.
impl From<Buf> for libdeno::deno_buf {
  fn from(x: Buf) -> Self {
    let len = x.len();
    let ptr = Box::into_raw(x);
    Self {
      alloc_ptr: std::ptr::null_mut(),
      alloc_len: 0,
      data_ptr: ptr as *mut u8,
      data_len: len,
    }
  }
}

// Dereferences the C pointer into the Rust Isolate object.
extern "C" fn pre_dispatch(
  user_data: *mut c_void,
  req_id: i32,
  control_buf: libdeno::deno_buf,
  data_buf: libdeno::deno_buf,
) {
  // for metrics
  let bytes_sent_control = control_buf.data_len;
  let bytes_sent_data = data_buf.data_len;

  // control_buf is only valid for the lifetime of this call, thus is
  // interpretted as a slice.
  let control_slice = unsafe {
    std::slice::from_raw_parts(control_buf.data_ptr, control_buf.data_len)
  };

  // data_buf is valid for the lifetime of the promise, thus a mutable buf with
  // static lifetime.
  let data_slice = unsafe {
    std::slice::from_raw_parts_mut::<'static>(
      data_buf.data_ptr,
      data_buf.data_len,
    )
  };

  let isolate = Isolate::from_void_ptr(user_data);
  let dispatch = isolate.dispatch;
  let (is_sync, op) = dispatch(isolate, control_slice, data_slice);

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
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();

    let state = Arc::new(IsolateState::new(flags, rest_argv));
    let snapshot = libdeno::deno_buf::empty();
    let mut isolate = Isolate::new(snapshot, state, dispatch_sync);
    tokio_util::init(|| {
      isolate
        .execute(
          "y.js",
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
      isolate.event_loop();
    });
  }

  fn dispatch_sync(
    _isolate: &mut Isolate,
    control: &[u8],
    data: &'static mut [u8],
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
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();
    let state = Arc::new(IsolateState::new(flags, rest_argv));
    let snapshot = libdeno::deno_buf::empty();
    let mut isolate = Isolate::new(snapshot, state, metrics_dispatch_sync);
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
          "y.js",
          r#"
          const control = new Uint8Array([4, 5, 6]);
          const data = new Uint8Array([42, 43, 44, 45, 46]);
          libdeno.send(control, data);
        "#,
        ).expect("execute error");
      isolate.event_loop();
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
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();
    let state = Arc::new(IsolateState::new(flags, rest_argv));
    let snapshot = libdeno::deno_buf::empty();
    let mut isolate = Isolate::new(snapshot, state, metrics_dispatch_async);
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
          "y.js",
          r#"
          const control = new Uint8Array([4, 5, 6]);
          const data = new Uint8Array([42, 43, 44, 45, 46]);
          let r = libdeno.send(control, data);
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

      isolate.event_loop();

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
    _isolate: &mut Isolate,
    _control: &[u8],
    _data: &'static mut [u8],
  ) -> (bool, Box<Op>) {
    // Send back some sync response
    let vec: Box<[u8]> = vec![1, 2, 3, 4].into_boxed_slice();
    let op = Box::new(futures::future::ok(vec));
    (true, op)
  }

  fn metrics_dispatch_async(
    _isolate: &mut Isolate,
    _control: &[u8],
    _data: &'static mut [u8],
  ) -> (bool, Box<Op>) {
    // Send back some sync response
    let vec: Box<[u8]> = vec![1, 2, 3, 4].into_boxed_slice();
    let op = Box::new(futures::future::ok(vec));
    (false, op)
  }
}
