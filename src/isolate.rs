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
use std::sync::atomic;
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
  ptr: *const libdeno::isolate,
  dispatch: Dispatch,
  rx: mpsc::Receiver<(i32, Buf)>,
  // Although Isolate is only accessed on the main thread, we use an atomic
  // variable here to workaround an issue probably caused by our poor usage
  // of Box::leak in Isolate::from_c()
  // https://github.com/denoland/deno/issues/919
  // ntasks ought to be i32.
  ntasks: atomic::AtomicIsize,
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
}

static DENO_INIT: std::sync::Once = std::sync::ONCE_INIT;

impl Isolate {
  pub fn new(argv: Vec<String>, dispatch: Dispatch) -> Box<Isolate> {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    let (flags, argv_rest) = flags::set_flags(argv);

    // This channel handles sending async messages back to the runtime.
    let (tx, rx) = mpsc::channel::<(i32, Buf)>();

    let mut isolate = Box::new(Isolate {
      ptr: 0 as *const libdeno::isolate,
      dispatch,
      rx,
      ntasks: atomic::AtomicIsize::new(0),
      timeout_due: None,
      state: Arc::new(IsolateState {
        dir: deno_dir::DenoDir::new(flags.reload, None).unwrap(),
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

  pub fn respond(&self, req_id: i32, buf: Buf) {
    // TODO(zero-copy) Use Buf::leak(buf) to leak the heap allocated buf. And
    // don't do the memcpy in ImportBuf() (in libdeno/binding.cc)
    unsafe { libdeno::deno_respond(self.ptr, req_id, buf.into()) }
  }

  fn complete_op(&mut self, req_id: i32, buf: Buf) {
    // Receiving a message on rx exactly corresponds to an async task
    // completing.
    self.ntasks_decrement();
    // Call into JS with the buf.
    self.respond(req_id, buf);
  }

  fn timeout(&self) {
    let dummy_buf = libdeno::deno_buf {
      alloc_ptr: 0 as *mut u8,
      alloc_len: 0,
      data_ptr: 0 as *mut u8,
      data_len: 0,
    };
    unsafe { libdeno::deno_respond(self.ptr, -1, dummy_buf) }
  }

  // TODO Use Park abstraction? Note at time of writing Tokio default runtime
  // does not have new_with_park().
  pub fn event_loop(&mut self) {
    // Main thread event loop.
    while !self.is_idle() {
      // Ideally, mpsc::Receiver would have a receive method that takes a optional
      // timeout. But it doesn't so we need all this duplicate code.
      match self.timeout_due {
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
          match self.rx.recv_timeout(timeout) {
            Ok((req_id, buf)) => self.complete_op(req_id, buf),
            Err(mpsc::RecvTimeoutError::Timeout) => self.timeout(),
            Err(e) => panic!("mpsc::Receiver::recv_timeout() failed: {:?}", e),
          }
        }
        None => match self.rx.recv() {
          Ok((req_id, buf)) => self.complete_op(req_id, buf),
          Err(e) => panic!("mpsc::Receiver::recv() failed: {:?}", e),
        },
      };
    }
  }

  fn ntasks_increment(&mut self) {
    let previous_ntasks = self.ntasks.fetch_add(1, atomic::Ordering::SeqCst);
    assert!(previous_ntasks >= 0);
  }

  fn ntasks_decrement(&mut self) {
    let previous_ntasks = self.ntasks.fetch_sub(1, atomic::Ordering::SeqCst);
    assert!(previous_ntasks >= 1);
  }

  fn is_idle(&self) -> bool {
    let n = self.ntasks.load(atomic::Ordering::SeqCst);
    n == 0 && self.timeout_due.is_none()
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
extern "C" fn pre_dispatch(
  d: *const libdeno::isolate,
  req_id: i32,
  control_buf: libdeno::deno_buf,
  data_buf: libdeno::deno_buf,
) {
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

  let isolate = Isolate::from_c(d);
  let dispatch = isolate.dispatch;
  let (is_sync, op) = dispatch(isolate, control_slice, data_slice);

  if is_sync {
    // Execute op synchronously.
    let buf = tokio_util::block_on(op).unwrap();
    if buf.len() != 0 {
      // Set the synchronous response, the value returned from isolate.send().
      isolate.respond(req_id, buf);
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
        state.send_to_js(req_id, buf);
        Ok(())
      }).map_err(|_| ());
    tokio::spawn(task);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures;

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
    _isolate: &mut Isolate,
    _control: &[u8],
    _data: &'static mut [u8],
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
}
