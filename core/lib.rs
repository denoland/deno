#[macro_use]
extern crate log;
extern crate futures;
extern crate libc;

mod js_errors;
mod libdeno;
mod shared;
mod shared_simple;

pub use crate::js_errors::*;
pub use crate::libdeno::deno_buf;
pub use crate::shared::*;
pub use crate::shared_simple::*;
use futures::Async;
use futures::Future;
use futures::Poll;
use libc::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::{Once, ONCE_INIT};

pub struct Isolate<R = SharedSimpleRecord, S = SharedSimple> {
  libdeno_isolate: *const libdeno::isolate,
  pending_ops: Vec<PendingOp<R>>,
  polled_recently: bool,
  recv_cb: RecvCallback<R, S>,

  pub shared: S,
  pub test_send_counter: u32, // TODO only used for testing- REMOVE.
}

pub type RecvCallback<R, S> =
  fn(isolate: &mut Isolate<R, S>, zero_copy_buf: deno_buf);

/// Buf represents a byte array returned from a "Op".
/// The message might be empty (which will be translated into a null object on
/// the javascript side) or it is a heap allocated opaque sequence of bytes.
/// Usually a flatbuffer message.
pub type Buf = Box<[u8]>;

/// JS promises in Deno map onto a specific Future
/// which yields a Buf object. Ops should never error.
/// Errors need to be encoded into the Buf and sent back to JS.
pub type Op<R> = dyn Future<Item = R, Error = ()> + Send;

struct PendingOp<R> {
  op: Box<Op<R>>,
  polled_recently: bool,
  zero_copy_id: usize, // non-zero if associated zero-copy buffer.
  result: Option<R>,
}

impl<R> PendingOp<R> {
  /// Calls poll() on a PendingOp. Returns true if it's complete.
  fn is_complete(self: &mut PendingOp<R>) -> bool {
    // TODO the name of this method isn't great, because it doesn't indicate
    // that the op is being polled.

    // Do not call poll on futures we've already polled this turn.
    if self.polled_recently {
      return false;
    }
    self.polled_recently = true;

    let op = &mut self.op;
    match op.poll() {
      Err(_) => {
        // Ops should not error. If an op experiences an error it needs to
        // encode that error into the Buf, so it can be returned to JS.
        panic!("ops should not error")
      }
      Ok(Async::Ready(buf)) => {
        self.result = Some(buf);
        true
      }
      Ok(Async::NotReady) => false,
    }
  }
}

static DENO_INIT: Once = ONCE_INIT;

unsafe impl<R, S: Shared<R>> Send for Isolate<R, S> {}

impl<R, S: Shared<R>> Isolate<R, S> {
  pub fn new(shared: S, recv_cb: RecvCallback<R, S>) -> Self {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    // Allocate unmanaged memory for the shared buffer by creating a Vec<u8>,
    // grabbing the raw pointer, and then leaking the Vec so it is never freed.
    let shared_deno_buf = shared.as_deno_buf();

    let config = libdeno::deno_config {
      will_snapshot: 0,
      load_snapshot: deno_buf::empty(), // TODO
      shared: shared_deno_buf,
      recv_cb: pre_dispatch::<R, S>,
    };
    let libdeno_isolate = unsafe { libdeno::deno_new(config) };

    Self {
      pending_ops: Vec::new(),
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

  pub fn add_op(self: &mut Self, op: Box<Op<R>>, zero_copy_id: usize) {
    debug!("add_op {}", zero_copy_id);
    self.pending_ops.push(PendingOp::<R> {
      op,
      polled_recently: false,
      zero_copy_id,
      result: None,
    });
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

impl<R, S: Shared<R>> Future for Isolate<R, S> {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Poll<(), JSError> {
    // Lock the current thread for V8.
    let _locker = LockerScope::new(self.libdeno_isolate);

    // Clear poll_recently state both on the Isolate itself and
    // on the pending ops.
    self.polled_recently = false;
    for pending in self.pending_ops.iter_mut() {
      pending.polled_recently = false;
    }

    while !self.polled_recently {
      let mut complete = Vec::<PendingOp<R>>::new();

      debug!("poll loop");

      self.polled_recently = true;

      let mut i = 0;
      while i != self.pending_ops.len() {
        let pending = &mut self.pending_ops[i];
        if pending.is_complete() {
          let pending = self.pending_ops.remove(i);
          complete.push(pending);
        } else {
          i += 1;
        }
      }

      self.shared.reset();
      if complete.len() > 0 {
        for completed_op in complete.iter_mut() {
          if completed_op.zero_copy_id > 0 {
            self.zero_copy_release(completed_op.zero_copy_id);
          }
          let result_record = &completed_op.result.take().unwrap();
          self.shared.push(result_record);
        }
        assert_eq!(self.shared.len(), complete.len());
        debug!("respond");
        self.respond()?;
        debug!("after respond");
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

extern "C" fn pre_dispatch<R, S: Shared<R>>(
  user_data: *mut c_void,
  control_buf: deno_buf,
  zero_copy_buf: deno_buf,
) {
  let isolate = unsafe { Isolate::<R, S>::from_raw_ptr(user_data) };
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
    let shared = SharedSimple::new();
    let isolate = Isolate::new(shared, inc_counter);
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
    let op = Box::new(futures::future::ok(SharedSimpleRecord {
      result: 42,
      ..Default::default()
    }));
    isolate.add_op(op, zero_copy_buf.zero_copy_id);
  }

  #[test]
  fn test_poll_async_immediate_ops() {
    let shared = SharedSimple::new();
    let mut isolate = Isolate::new(shared, async_immediate);
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
