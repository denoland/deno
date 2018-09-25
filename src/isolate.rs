// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Do not use FlatBuffers in this module.
// TODO Currently this module uses Tokio, but it would be nice if they were
// decoupled.

use deno_dir;
use errors::DenoError;
use flags;
use libdeno;

use futures;
use futures::Future;
use libc::c_void;
use std;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::atomic::AtomicIsize;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
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
pub type Dispatch = fn(state: Arc<IsolateState>, buf: &[u8]) -> (bool, Box<Op>);

pub struct Isolate {
  ptr: *const libdeno::isolate,
  dispatch: Dispatch,
  rx: mpsc::Receiver<Buf>,
  ntasks: i32,
  pub state: Arc<IsolateState>,
}

// Isolate cannot be passed between threads but IsolateState can. So any state that
// needs to be accessed outside the main V8 thread should be inside IsolateState.
pub struct IsolateState {
  pub timeout: AtomicIsize,
  pub dir: deno_dir::DenoDir,
  pub timers: Mutex<HashMap<u32, futures::sync::oneshot::Sender<()>>>,
  pub argv: Vec<String>,
  pub flags: flags::DenoFlags,
  tx: Mutex<Option<mpsc::Sender<Buf>>>,
}

impl IsolateState {
  // Thread safe.
  fn send_to_js(&self, buf: Buf) {
    let mut g = self.tx.lock().unwrap();
    let maybe_tx = g.as_mut();
    assert!(maybe_tx.is_some(), "Expected tx to not be deleted.");
    let tx = maybe_tx.unwrap();
    tx.send(buf).expect("tx.send error");
  }
}

static DENO_INIT: std::sync::Once = std::sync::ONCE_INIT;

impl Isolate {
  pub fn new(argv: Vec<String>, dispatch: Dispatch) -> Box<Isolate> {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    let (flags, argv_rest) = flags::set_flags(argv);

    // This channel handles sending async messages back to the runtime.
    let (tx, rx) = mpsc::channel::<Buf>();

    let mut isolate = Box::new(Isolate {
      ptr: 0 as *const libdeno::isolate,
      dispatch,
      rx,
      ntasks: 0,
      state: Arc::new(IsolateState {
        timeout: AtomicIsize::new(-1),
        dir: deno_dir::DenoDir::new(flags.reload, None).unwrap(),
        timers: Mutex::new(HashMap::new()),
        argv: argv_rest,
        flags,
        tx: Mutex::new(Some(tx)),
      }),
    });

    (*isolate).ptr = unsafe {
      libdeno::deno_new(
        isolate.as_ref() as *const _ as *const c_void,
        pre_dispatch,
      )
    };

    isolate
  }

  pub fn from_c<'a>(d: *const libdeno::isolate) -> &'a mut Isolate {
    let ptr = unsafe { libdeno::deno_get_data(d) };
    let ptr = ptr as *mut Isolate;
    let isolate_box = unsafe { Box::from_raw(ptr) };
    Box::leak(isolate_box)
  }

  pub fn execute(
    &self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), DenoException> {
    let filename = CString::new(js_filename).unwrap();
    let source = CString::new(js_source).unwrap();
    let r = unsafe {
      libdeno::deno_execute(self.ptr, filename.as_ptr(), source.as_ptr())
    };
    if r == 0 {
      let ptr = unsafe { libdeno::deno_last_exception(self.ptr) };
      let cstr = unsafe { CStr::from_ptr(ptr) };
      return Err(cstr.to_str().unwrap());
    }
    Ok(())
  }

  pub fn set_response(&self, buf: Buf) {
    unsafe { libdeno::deno_set_response(self.ptr, buf.into()) }
  }

  pub fn send(&self, buf: Buf) {
    unsafe { libdeno::deno_send(self.ptr, buf.into()) };
  }

  // TODO Use Park abstraction? Note at time of writing Tokio default runtime
  // does not have new_with_park().
  pub fn event_loop(&mut self) {
    // Main thread event loop.
    while !self.is_idle() {
      let buf = self.rx.recv().unwrap();
      // Receiving a message on rx exactly corresponds to an async task
      // completing.
      self.ntasks_decrement();
      // Call into JS with the buf.
      self.send(buf);
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
    self.ntasks == 0
  }
}

impl Drop for Isolate {
  fn drop(&mut self) {
    unsafe { libdeno::deno_delete(self.ptr) }
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
extern "C" fn pre_dispatch(d: *const libdeno::isolate, buf: libdeno::deno_buf) {
  let bytes = unsafe { std::slice::from_raw_parts(buf.data_ptr, buf.data_len) };
  let isolate = Isolate::from_c(d);
  let dispatch = isolate.dispatch;
  let (is_sync, op) = dispatch(isolate.state.clone(), bytes);

  if is_sync {
    // Execute op synchronously.
    let buf = tokio_util::block_on(op).unwrap();
    if buf.len() != 0 {
      // Set the synchronous response, the value returned from isolate.send().
      isolate.set_response(buf);
    }
  } else {
    // Execute op asynchronously.
    let state = isolate.state.clone();

    // TODO Ideally Tokio would could tell us how many tasks are executing, but
    // it cannot currently. Therefore we track top-level promises/tasks
    // manually.
    isolate.ntasks_increment();

    let task = op
      .and_then(move |buf| {
        state.send_to_js(buf);
        Ok(())
      }).map_err(|_| ());
    tokio::spawn(task);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_c_to_rust() {
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    let isolate = Isolate::new(argv, unreachable_dispatch);
    let isolate2 = Isolate::from_c(isolate.ptr);
    assert_eq!(isolate.ptr, isolate2.ptr);
    assert_eq!(
      isolate.state.dir.root.join("gen"),
      isolate.state.dir.gen,
      "Sanity check"
    );
  }

  fn unreachable_dispatch(
    _state: Arc<IsolateState>,
    _buf: &[u8],
  ) -> (bool, Box<Op>) {
    unreachable!();
  }

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

  fn dispatch_sync(_state: Arc<IsolateState>, buf: &[u8]) -> (bool, Box<Op>) {
    assert_eq!(buf[0], 4);
    assert_eq!(buf[1], 5);
    assert_eq!(buf[2], 6);
    // Send back some sync response.
    let vec: Vec<u8> = vec![1, 2, 3];
    let buf = vec.into_boxed_slice();
    let op = Box::new(futures::future::ok(buf));
    (true, op)
  }
}
