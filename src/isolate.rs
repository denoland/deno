// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Do not use FlatBuffers in this module.
// TODO Currently this module uses Tokio, but it would be nice if they were
// decoupled.

use deno_dir;
use errors::DenoError;
use flags;
use libdeno;

use futures::Future;
use libc::c_void;
use std;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
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
  ntasks: i32,
  pub timeout_due: Option<Instant>,
  pub state: Arc<IsolateState>,
}

// Isolate cannot be passed between threads but IsolateState can. So any state that
// needs to be accessed outside the main V8 thread should be inside IsolateState.
pub struct IsolateState {
  pub dir: deno_dir::DenoDir,
  pub argv: Vec<String>,
  pub flags: flags::DenoFlags,
  tx: Mutex<Option<mpsc::Sender<(i32, Buf)>>>,
  pub metrics: Mutex<Metrics>,
}

impl IsolateState {
  // Thread safe.
  fn send_to_js(&self, req_id: i32, buf: Buf) {
    let mut g = self.tx.lock().unwrap();
    let maybe_tx = g.as_mut();
    assert!(maybe_tx.is_some(), "Expected tx to not be deleted.");
    let tx = maybe_tx.unwrap();
    tx.send((req_id, buf)).expect("tx.send error");
  }

  fn metrics_op_dispatched(
    &self,
    bytes_sent_control: u64,
    bytes_sent_data: u64,
  ) {
    let mut metrics = self.metrics.lock().unwrap();
    metrics.ops_dispatched += 1;
    metrics.bytes_sent_control += bytes_sent_control;
    metrics.bytes_sent_data += bytes_sent_data;
  }

  fn metrics_op_completed(&self, bytes_received: u64) {
    let mut metrics = self.metrics.lock().unwrap();
    metrics.ops_completed += 1;
    metrics.bytes_received += bytes_received;
  }
}

#[derive(Default)]
pub struct Metrics {
  pub ops_dispatched: u64,
  pub ops_completed: u64,
  pub bytes_sent_control: u64,
  pub bytes_sent_data: u64,
  pub bytes_received: u64,
}

static DENO_INIT: std::sync::Once = std::sync::ONCE_INIT;

impl Isolate {
  pub fn new(argv: Vec<String>, dispatch: Dispatch) -> Isolate {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    let (flags, argv_rest) = flags::set_flags(argv);
    let libdeno_isolate = unsafe { libdeno::deno_new(pre_dispatch) };
    // This channel handles sending async messages back to the runtime.
    let (tx, rx) = mpsc::channel::<(i32, Buf)>();

    Isolate {
      libdeno_isolate,
      dispatch,
      rx,
      ntasks: 0,
      timeout_due: None,
      state: Arc::new(IsolateState {
        dir: deno_dir::DenoDir::new(flags.reload, None).unwrap(),
        argv: argv_rest,
        flags,
        tx: Mutex::new(Some(tx)),
        metrics: Mutex::new(Metrics::default()),
      }),
    }
  }

  pub fn as_void_ptr(&mut self) -> *mut c_void {
    self as *mut _ as *mut c_void
  }

  pub fn from_void_ptr<'a>(ptr: *mut c_void) -> &'a mut Isolate {
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
    self.state.metrics_op_completed(buf.len() as u64);
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
      alloc_ptr: 0 as *mut u8,
      alloc_len: 0,
      data_ptr: 0 as *mut u8,
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
    }
  }

  fn ntasks_increment(&mut self) {
    assert!(self.ntasks >= 0);
    self.ntasks = self.ntasks + 1;
  }

  fn ntasks_decrement(&mut self) {
    self.ntasks = self.ntasks - 1;
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

/// Converts Rust Buf to libdeno deno_buf.
impl From<Buf> for libdeno::deno_buf {
  fn from(x: Buf) -> libdeno::deno_buf {
    let len = x.len();
    let ptr = Box::into_raw(x);
    libdeno::deno_buf {
      alloc_ptr: 0 as *mut u8,
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
  let bytes_sent_control = control_buf.data_len as u64;
  let bytes_sent_data = data_buf.data_len as u64;

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
    if buf_size != 0 {
      // Set the synchronous response, the value returned from isolate.send().
      isolate.respond(req_id, buf);
    }
  } else {
    // Execute op asynchronously.
    let state = Arc::clone(&isolate.state);

    // TODO Ideally Tokio would could tell us how many tasks are executing, but
    // it cannot currently. Therefore we track top-level promises/tasks
    // manually.
    isolate.ntasks_increment();

    let task = op
      .and_then(move |buf| {
        state.send_to_js(req_id, buf);
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
    let mut isolate = Isolate::new(argv, dispatch_sync);
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
    let mut isolate = Isolate::new(argv, metrics_dispatch_sync);
    tokio_util::init(|| {
      // Verify that metrics have been properly initialized.
      {
        let metrics = isolate.state.metrics.lock().unwrap();
        assert_eq!(metrics.ops_dispatched, 0);
        assert_eq!(metrics.ops_completed, 0);
        assert_eq!(metrics.bytes_sent_control, 0);
        assert_eq!(metrics.bytes_sent_data, 0);
        assert_eq!(metrics.bytes_received, 0);
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
      let metrics = isolate.state.metrics.lock().unwrap();
      assert_eq!(metrics.ops_dispatched, 1);
      assert_eq!(metrics.ops_completed, 1);
      assert_eq!(metrics.bytes_sent_control, 3);
      assert_eq!(metrics.bytes_sent_data, 5);
      assert_eq!(metrics.bytes_received, 4);
    });
  }

  #[test]
  fn test_metrics_async() {
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    let mut isolate = Isolate::new(argv, metrics_dispatch_async);
    tokio_util::init(|| {
      // Verify that metrics have been properly initialized.
      {
        let metrics = isolate.state.metrics.lock().unwrap();
        assert_eq!(metrics.ops_dispatched, 0);
        assert_eq!(metrics.ops_completed, 0);
        assert_eq!(metrics.bytes_sent_control, 0);
        assert_eq!(metrics.bytes_sent_data, 0);
        assert_eq!(metrics.bytes_received, 0);
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
        let metrics = isolate.state.metrics.lock().unwrap();
        assert_eq!(metrics.ops_dispatched, 1);
        assert_eq!(metrics.bytes_sent_control, 3);
        assert_eq!(metrics.bytes_sent_data, 5);
        // Note we cannot check ops_completed nor bytes_received because that
        // would be a race condition. It might be nice to have use a oneshot
        // with metrics_dispatch_async() to properly validate them.
      }

      isolate.event_loop();

      // Make sure relevant metrics are updated after task is executed.
      {
        let metrics = isolate.state.metrics.lock().unwrap();
        assert_eq!(metrics.ops_dispatched, 1);
        assert_eq!(metrics.ops_completed, 1);
        assert_eq!(metrics.bytes_sent_control, 3);
        assert_eq!(metrics.bytes_sent_data, 5);
        assert_eq!(metrics.bytes_received, 4);
      }
    });
  }

  fn metrics_dispatch_sync(
    _isolate: &mut Isolate,
    _control: &[u8],
    _data: &'static mut [u8],
  ) -> (bool, Box<Op>) {
    // Send back some sync response
    let vec: Vec<u8> = vec![1, 2, 3, 4];
    let control = vec.into_boxed_slice();
    let op = Box::new(futures::future::ok(control));
    (true, op)
  }

  fn metrics_dispatch_async(
    _isolate: &mut Isolate,
    _control: &[u8],
    _data: &'static mut [u8],
  ) -> (bool, Box<Op>) {
    // Send back some sync response
    let vec: Vec<u8> = vec![1, 2, 3, 4];
    let control = vec.into_boxed_slice();
    let op = Box::new(futures::future::ok(control));
    (false, op)
  }
}
