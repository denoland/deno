#[macro_use]
extern crate log;
extern crate futures;
extern crate libc;

mod js_errors;
mod libdeno;
mod shared;

pub use crate::js_errors::JSError;
pub use crate::libdeno::deno_buf;
pub use crate::shared::*;
use futures::Async;
use futures::Future;
use futures::Poll;
use libc::c_void;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::{Once, ONCE_INIT};

pub struct Isolate {
  libdeno_isolate: *const libdeno::isolate,
  pending_ops: HashMap<i32, PendingOp>, // promise_id -> op
  polled_recently: bool,
  recv_cb: RecvCallback,

  pub shared: Shared,
  pub test_send_counter: u32, // TODO only used for testing- REMOVE.
}

pub type RecvCallback = fn(isolate: &mut Isolate, zero_copy_buf: deno_buf);

pub const NUM_RECORDS: usize = 100;

// TODO rename to AsyncResult
pub struct AsyncResult {
  pub result: i32,
}

pub type Op = dyn Future<Item = AsyncResult, Error = std::io::Error> + Send;

struct PendingOp {
  op: Box<Op>,
  polled_recently: bool,
  zero_copy_id: usize, // non-zero if associated zero-copy buffer.
}

static DENO_INIT: Once = ONCE_INIT;

unsafe impl Send for Isolate {}

impl Isolate {
  pub fn new(recv_cb: RecvCallback) -> Self {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    // Allocate unmanaged memory for the shared buffer by creating a Vec<u8>,
    // grabbing the raw pointer, and then leaking the Vec so it is never freed.
    let mut shared = Shared::new();
    let shared_deno_buf = shared.as_deno_buf();

    let config = libdeno::deno_config {
      will_snapshot: 0,
      load_snapshot: deno_buf::empty(), // TODO
      shared: shared_deno_buf,
      recv_cb: pre_dispatch,
    };
    let libdeno_isolate = unsafe { libdeno::deno_new(config) };

    Self {
      pending_ops: HashMap::new(),
      polled_recently: false,
      libdeno_isolate,
      test_send_counter: 0,
      recv_cb,
      shared,
    }
  }

  fn zero_copy_release(&self, zero_copy_id: usize) {
    unsafe {
      libdeno::deno_zero_copy_release(self.libdeno_isolate, zero_copy_id)
    }
  }

  pub fn add_op(
    self: &mut Self,
    promise_id: i32,
    op: Box<Op>,
    zero_copy_id: usize,
  ) {
    debug!("add_op {}", zero_copy_id);
    self.pending_ops.insert(
      promise_id,
      PendingOp {
        op,
        polled_recently: false,
        zero_copy_id,
      },
    );
    self.polled_recently = false;
  }

  #[inline]
  pub unsafe fn from_raw_ptr<'a>(ptr: *const c_void) -> &'a mut Self {
    let ptr = ptr as *mut _;
    &mut *ptr
  }

  #[inline]
  pub fn as_raw_ptr(&self) -> *const c_void {
    self as *const _ as *const c_void
  }

  pub fn execute(
    &self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), JSError> {
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
    if let Some(err) = self.last_exception() {
      return Err(err);
    }
    Ok(())
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
      Some(js_error)
    }
  }

  fn check_promise_errors(&self) {
    unsafe {
      libdeno::deno_check_promise_errors(self.libdeno_isolate);
    }
  }

  fn respond(&mut self) -> Result<(), JSError> {
    let buf = deno_buf::empty();
    unsafe {
      libdeno::deno_respond(self.libdeno_isolate, self.as_raw_ptr(), buf)
    }
    if let Some(err) = self.last_exception() {
      Err(err)
    } else {
      Ok(())
    }
  }
}

struct LockerScope {
  libdeno_isolate: *const libdeno::isolate,
}

impl LockerScope {
  fn new(isolate: &Isolate) -> LockerScope {
    let libdeno_isolate = isolate.libdeno_isolate;
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
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Poll<(), JSError> {
    // Lock the current thread for V8.
    let _locker = LockerScope::new(self);

    // Clear
    self.polled_recently = false;
    for (_, pending) in self.pending_ops.iter_mut() {
      pending.polled_recently = false;
    }

    while !self.polled_recently {
      let mut complete = HashMap::<i32, AsyncResult>::new();

      self.polled_recently = true;
      for (promise_id, pending) in self.pending_ops.iter_mut() {
        // Do not call poll on futures we've already polled this turn.
        if pending.polled_recently {
          continue;
        }
        pending.polled_recently = true;

        let promise_id = *promise_id;
        let op = &mut pending.op;
        match op.poll() {
          Err(op_err) => {
            eprintln!("op err {:?}", op_err);
            complete.insert(promise_id, AsyncResult { result: -1 });
            debug!("pending op {} complete err", promise_id);
          }
          Ok(Async::Ready(async_result)) => {
            complete.insert(promise_id, async_result);
            debug!("pending op {} complete ready", promise_id);
          }
          Ok(Async::NotReady) => {
            debug!("pending op {} not ready", promise_id);
            continue;
          }
        }
      }

      self.shared.set_num_records(complete.len() as i32);
      if complete.len() > 0 {
        // self.zero_copy_release() and self.respond() need Locker.
        let mut i = 0;
        for (promise_id, async_result) in complete.iter_mut() {
          let pending = self.pending_ops.remove(promise_id).unwrap();

          if pending.zero_copy_id > 0 {
            self.zero_copy_release(pending.zero_copy_id);
          }

          self
            .shared
            .set_record(i, RECORD_OFFSET_PROMISE_ID, *promise_id);
          self
            .shared
            .set_record(i, RECORD_OFFSET_RESULT, async_result.result);
          i += 1;
        }
        self.respond()?;
      }
    }

    self.check_promise_errors();
    if let Some(err) = self.last_exception() {
      return Err(err);
    }

    // We're idle if pending_ops is empty.
    if self.pending_ops.is_empty() {
      Ok(futures::Async::Ready(()))
    } else {
      Ok(futures::Async::NotReady)
    }
  }
}

extern "C" fn pre_dispatch(
  user_data: *mut c_void,
  control_buf: deno_buf,
  zero_copy_buf: deno_buf,
) {
  let isolate = unsafe { Isolate::from_raw_ptr(user_data) };
  assert_eq!(control_buf.len(), 0);
  (isolate.recv_cb)(isolate, zero_copy_buf);
}

#[cfg(test)]
mod tests {
  use super::*;

  fn inc_counter(isolate: &mut Isolate, zero_copy_buf: deno_buf) {
    assert_eq!(zero_copy_buf.len(), 0);
    isolate.test_send_counter += 1; // TODO ideally store this in isolate.state?
  }

  fn js_check(r: Result<(), JSError>) {
    if let Err(e) = r {
      panic!(e.to_string());
    }
  }

  #[test]
  fn test_execute() {
    let isolate = Isolate::new(inc_counter);
    js_check(isolate.execute(
      "filename.js",
      r#"
        libdeno.send();
        async function main() {
          libdeno.send();
        }
        main();
        "#,
    ));
    // We expect that main is executed even tho we didn't poll.
    assert_eq!(isolate.test_send_counter, 2);
  }

  fn async_immediate(isolate: &mut Isolate, zero_copy_buf: deno_buf) {
    assert_eq!(zero_copy_buf.len(), 0);
    isolate.test_send_counter += 1; // TODO ideally store this in isolate.state?

    let promise_id = 0;
    let op = Box::new(futures::future::ok(AsyncResult { result: 0 }));
    isolate.add_op(promise_id, op, zero_copy_buf.zero_copy_id);
  }

  #[test]
  fn test_poll_async_immediate_ops() {
    let mut isolate = Isolate::new(async_immediate);
    js_check(isolate.execute(
      "setup.js",
      r#"
        let nrecv = 0;
        libdeno.recv(() => {
          nrecv++;
        });
        function assertEq(actual, expected) {
          if (expected != actual) {
            throw Error(`actual ${actual} expected ${expected} `);
          }
        }
        "#,
    ));
    assert_eq!(isolate.test_send_counter, 0);
    js_check(isolate.execute(
      "check1.js",
      r#"
        assertEq(nrecv, 0);
        libdeno.send();
        assertEq(nrecv, 0);
        "#,
    ));
    assert_eq!(isolate.test_send_counter, 1);
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
    assert_eq!(isolate.test_send_counter, 1);
    js_check(isolate.execute(
      "check2.js",
      r#"
        assertEq(nrecv, 1);
        libdeno.send();
        assertEq(nrecv, 1);
        "#,
    ));
    assert_eq!(isolate.test_send_counter, 2);
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
    js_check(isolate.execute("check3.js", "assertEq(nrecv, 2)"));
    assert_eq!(isolate.test_send_counter, 2);
    // We are idle, so the next poll should be the last.
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
  }
}
