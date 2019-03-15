// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use crate::js_errors::JSError;
use crate::libdeno;
use crate::libdeno::deno_buf;
use crate::libdeno::deno_mod;
use crate::shared_queue::SharedQueue;
use crate::shared_queue::RECOMMENDED_SIZE;
use futures::Async;
use futures::Future;
use futures::Poll;
use libc::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::{Once, ONCE_INIT};

pub type Buf = Box<[u8]>;
pub type Op = dyn Future<Item = Buf, Error = ()> + Send;

struct PendingOp {
  op: Box<Op>,
  polled_recently: bool,
  zero_copy_id: usize, // non-zero if associated zero-copy buffer.
}

impl Future for PendingOp {
  type Item = Buf;
  type Error = ();

  fn poll(&mut self) -> Poll<Buf, ()> {
    // Do not call poll on ops we've already polled this turn.
    if self.polled_recently {
      Ok(Async::NotReady)
    } else {
      self.polled_recently = true;
      let op = &mut self.op;
      op.poll().map_err(|()| {
        // Ops should not error. If an op experiences an error it needs to
        // encode that error into a buf, so it can be returned to JS.
        panic!("ops should not error")
      })
    }
  }
}

/// Defines the behavior of an Isolate.
pub trait Behavior {
  /// Called exactly once when an Isolate is created to retrieve the startup
  /// snapshot.
  fn startup_snapshot(&mut self) -> Option<deno_buf>;

  /// Called during mod_instantiate() to resolve imports.
  fn resolve(&mut self, specifier: &str, referrer: deno_mod) -> deno_mod;

  /// Called whenever libdeno.send() is called in JavaScript. zero_copy_buf
  /// corresponds to the second argument of libdeno.send().
  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy_buf: deno_buf,
  ) -> (bool, Box<Op>);
}

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM. An Isolate is a Future that can be used with
/// Tokio.  The Isolate future complete when there is an error or when all
/// pending ops have completed.
///
/// Ops are created in JavaScript by calling libdeno.send(), and in Rust by
/// implementing Behavior::dispatch. An Op corresponds exactly to a Promise in
/// JavaScript.
pub struct Isolate<B: Behavior> {
  libdeno_isolate: *const libdeno::isolate,
  behavior: B,
  shared: SharedQueue,
  pending_ops: Vec<PendingOp>,
  polled_recently: bool,
}

unsafe impl<B: Behavior> Send for Isolate<B> {}

impl<B: Behavior> Drop for Isolate<B> {
  fn drop(&mut self) {
    unsafe { libdeno::deno_delete(self.libdeno_isolate) }
  }
}

static DENO_INIT: Once = ONCE_INIT;

impl<B: Behavior> Isolate<B> {
  pub fn new(mut behavior: B) -> Self {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    let shared = SharedQueue::new(RECOMMENDED_SIZE);

    let config = libdeno::deno_config {
      will_snapshot: 0,
      load_snapshot: match behavior.startup_snapshot() {
        Some(s) => s,
        None => libdeno::deno_buf::empty(),
      },
      shared: shared.as_deno_buf(),
      recv_cb: Self::pre_dispatch,
    };
    let libdeno_isolate = unsafe { libdeno::deno_new(config) };

    Self {
      libdeno_isolate,
      behavior,
      shared,
      pending_ops: Vec::new(),
      polled_recently: false,
    }
  }

  /// Executes a bit of built-in JavaScript to provide Deno._sharedQueue.
  pub fn shared_init(&self) {
    js_check(self.execute("shared_queue.js", include_str!("shared_queue.js")));
  }

  extern "C" fn pre_dispatch(
    user_data: *mut c_void,
    control_argv0: deno_buf,
    zero_copy_buf: deno_buf,
  ) {
    let isolate = unsafe { Isolate::<B>::from_raw_ptr(user_data) };
    let zero_copy_id = zero_copy_buf.zero_copy_id;

    let control_shared = isolate.shared.shift();

    let (is_sync, op) = if control_argv0.len() > 0 {
      // The user called libdeno.send(control)
      isolate
        .behavior
        .dispatch(control_argv0.as_ref(), zero_copy_buf)
    } else if let Some(c) = control_shared {
      // The user called Deno._sharedQueue.push(control)
      isolate.behavior.dispatch(&c, zero_copy_buf)
    } else {
      // The sharedQueue is empty. The shouldn't happen usually, but it's also
      // not technically a failure.
      #[cfg(test)]
      unreachable!();
      #[cfg(not(test))]
      return;
    };

    // At this point the SharedQueue should be empty.
    assert_eq!(isolate.shared.size(), 0);

    if is_sync {
      let res_record = op.wait().unwrap();
      let push_success = isolate.shared.push(res_record);
      assert!(push_success);
      // TODO check that if JSError thrown during respond(), that it will be
      // picked up.
      let _ = isolate.respond();
    } else {
      isolate.pending_ops.push(PendingOp {
        op,
        polled_recently: false,
        zero_copy_id,
      });
      isolate.polled_recently = false;
    }
  }

  fn zero_copy_release(&self, zero_copy_id: usize) {
    unsafe {
      libdeno::deno_zero_copy_release(self.libdeno_isolate, zero_copy_id)
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

  fn last_exception(&self) -> Option<JSError> {
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

  /// Low-level module creation.
  /// You probably want to use IsolateState::mod_execute instead.
  pub fn mod_new(
    &self,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<deno_mod, JSError> {
    let name_ = CString::new(name.to_string()).unwrap();
    let name_ptr = name_.as_ptr() as *const libc::c_char;

    let source_ = CString::new(source.to_string()).unwrap();
    let source_ptr = source_.as_ptr() as *const libc::c_char;

    let id = unsafe {
      libdeno::deno_mod_new(self.libdeno_isolate, main, name_ptr, source_ptr)
    };
    if let Some(js_error) = self.last_exception() {
      assert_eq!(id, 0);
      return Err(js_error);
    }

    Ok(id)
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

  pub fn mod_instantiate(&self, id: deno_mod) -> Result<(), JSError> {
    unsafe {
      libdeno::deno_mod_instantiate(
        self.libdeno_isolate,
        self.as_raw_ptr(),
        id,
        Self::resolve_cb,
      )
    };
    if let Some(js_error) = self.last_exception() {
      return Err(js_error);
    }
    Ok(())
  }

  pub fn mod_evaluate(&self, id: deno_mod) -> Result<(), JSError> {
    unsafe {
      libdeno::deno_mod_evaluate(self.libdeno_isolate, self.as_raw_ptr(), id)
    };
    if let Some(js_error) = self.last_exception() {
      return Err(js_error);
    }
    Ok(())
  }

  /// Called during mod_instantiate() only.
  extern "C" fn resolve_cb(
    user_data: *mut libc::c_void,
    specifier_ptr: *const libc::c_char,
    referrer: deno_mod,
  ) -> deno_mod {
    let isolate = unsafe { Isolate::<B>::from_raw_ptr(user_data) };
    let specifier_c: &CStr = unsafe { CStr::from_ptr(specifier_ptr) };
    let specifier: &str = specifier_c.to_str().unwrap();
    isolate.behavior.resolve(specifier, referrer)
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

impl<B: Behavior> Future for Isolate<B> {
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
      let mut completed_count = 0;
      self.polled_recently = true;
      assert_eq!(self.shared.size(), 0);

      let mut i = 0;
      while i < self.pending_ops.len() {
        let pending = &mut self.pending_ops[i];
        match pending.poll() {
          Err(()) => panic!("unexpected error"),
          Ok(Async::NotReady) => {
            i += 1;
          }
          Ok(Async::Ready(buf)) => {
            let completed = self.pending_ops.remove(i);
            completed_count += 1;

            if completed.zero_copy_id > 0 {
              self.zero_copy_release(completed.zero_copy_id);
            }

            self.shared.push(buf);
          }
        }
      }

      if completed_count > 0 {
        self.respond()?;
        // The other side should have shifted off all the messages.
        assert_eq!(self.shared.size(), 0);
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

pub fn js_check(r: Result<(), JSError>) {
  if let Err(e) = r {
    panic!(e.to_string());
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_util::*;

  #[test]
  fn test_dispatch() {
    let behavior = TestBehavior::new();
    let isolate = Isolate::new(behavior);
    js_check(isolate.execute(
      "filename.js",
      r#"
        let control = new Uint8Array([42]);
        libdeno.send(control);
        async function main() {
          libdeno.send(control);
        }
        main();
        "#,
    ));
    assert_eq!(isolate.behavior.dispatch_count, 2);
  }

  #[test]
  fn test_mods() {
    let behavior = TestBehavior::new();
    let mut isolate = Isolate::new(behavior);
    let mod_a = isolate
      .mod_new(
        true,
        "a.js",
        r#"
        import { b } from 'b.js'
        if (b() != 'b') throw Error();
        let control = new Uint8Array([42]);
        libdeno.send(control);
      "#,
      ).unwrap();
    assert_eq!(isolate.behavior.dispatch_count, 0);
    assert_eq!(isolate.behavior.resolve_count, 0);

    let imports = isolate.mod_get_imports(mod_a);
    assert_eq!(imports, vec!["b.js".to_string()]);

    let mod_b = isolate
      .mod_new(false, "b.js", "export function b() { return 'b' }")
      .unwrap();
    let imports = isolate.mod_get_imports(mod_b);
    assert_eq!(imports.len(), 0);

    js_check(isolate.mod_instantiate(mod_b));
    assert_eq!(isolate.behavior.dispatch_count, 0);
    assert_eq!(isolate.behavior.resolve_count, 0);

    isolate.behavior.register("b.js", mod_b);
    js_check(isolate.mod_instantiate(mod_a));
    assert_eq!(isolate.behavior.dispatch_count, 0);
    assert_eq!(isolate.behavior.resolve_count, 1);

    js_check(isolate.mod_evaluate(mod_a));
    assert_eq!(isolate.behavior.dispatch_count, 1);
    assert_eq!(isolate.behavior.resolve_count, 1);
  }

  #[test]
  fn test_poll_async_immediate_ops() {
    let behavior = TestBehavior::new();
    let mut isolate = Isolate::new(behavior);

    isolate.shared_init();

    js_check(isolate.execute(
      "setup.js",
      r#"
        let nrecv = 0;
        Deno._setAsyncHandler((buf) => {
          nrecv++;
        });
        function assertEq(actual, expected) {
          if (expected != actual) {
            throw Error(`actual ${actual} expected ${expected} `);
          }
        }
        "#,
    ));
    assert_eq!(isolate.behavior.dispatch_count, 0);
    js_check(isolate.execute(
      "check1.js",
      r#"
        assertEq(nrecv, 0);
        let control = new Uint8Array([42]);
        libdeno.send(control);
        assertEq(nrecv, 0);
        "#,
    ));
    assert_eq!(isolate.behavior.dispatch_count, 1);
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
    assert_eq!(isolate.behavior.dispatch_count, 1);
    js_check(isolate.execute(
      "check2.js",
      r#"
        assertEq(nrecv, 1);
        libdeno.send(control);
        assertEq(nrecv, 1);
        "#,
    ));
    assert_eq!(isolate.behavior.dispatch_count, 2);
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
    js_check(isolate.execute("check3.js", "assertEq(nrecv, 2)"));
    assert_eq!(isolate.behavior.dispatch_count, 2);
    // We are idle, so the next poll should be the last.
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
  }

  #[test]
  fn test_shared() {
    let behavior = TestBehavior::new();
    let mut isolate = Isolate::new(behavior);

    isolate.shared_init();

    js_check(isolate.execute(
      "setup.js",
      r#"
        let nrecv = 0;
        Deno._setAsyncHandler((buf) => {
          assert(buf.byteLength === 1);
          assert(buf[0] === 43);
          nrecv++;
        });
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
    ));
    assert_eq!(isolate.behavior.dispatch_count, 0);

    js_check(isolate.execute(
      "send1.js",
      r#"
        let control = new Uint8Array([42]);
        Deno._sharedQueue.push(control);
        libdeno.send();
        assert(nrecv === 0);

        Deno._sharedQueue.push(control);
        libdeno.send();
        assert(nrecv === 0);
        "#,
    ));
    assert_eq!(isolate.behavior.dispatch_count, 2);
    assert_eq!(Ok(Async::Ready(())), isolate.poll());

    js_check(isolate.execute("send1.js", "assert(nrecv === 2);"));
  }

}
