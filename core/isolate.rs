// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use crate::js_errors::JSError;
use crate::libdeno;
use crate::libdeno::deno_buf;
use crate::libdeno::deno_mod;
use libc::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::{Once, ONCE_INIT};

use futures::Async;
use futures::Future;
use futures::Poll;

pub type Op<R> = dyn Future<Item = R, Error = ()> + Send;

struct PendingOp<R> {
  op: Box<Op<R>>,
  polled_recently: bool,
  zero_copy_id: usize, // non-zero if associated zero-copy buffer.
}

impl<R> Future for PendingOp<R> {
  type Item = R;
  type Error = ();

  fn poll(&mut self) -> Poll<R, ()> {
    // Do not call poll on ops we've already polled this turn.
    if self.polled_recently {
      Ok(Async::NotReady)
    } else {
      self.polled_recently = true;
      let op = &mut self.op;
      op.poll().map_err(|()| {
        // Ops should not error. If an op experiences an error it needs to
        // encode that error into the Buf, so it can be returned to JS.
        panic!("ops should not error")
      })
    }
  }
}

pub trait Behavior<R> {
  fn resolve(&mut self, specifier: &str, referrer: deno_mod) -> deno_mod;
  fn recv(&mut self, record: R, zero_copy_buf: deno_buf) -> (bool, Box<Op<R>>);
  fn records_reset(&mut self);
  fn records_push(&mut self, record: R) -> bool;
  fn records_pop(&mut self) -> Option<R>;
}

pub struct Isolate<R, B: Behavior<R>> {
  libdeno_isolate: *const libdeno::isolate,
  behavior: B,
  pending_ops: Vec<PendingOp<R>>,
  polled_recently: bool,
}

unsafe impl<R, B: Behavior<R>> Send for Isolate<R, B> {}

static DENO_INIT: Once = ONCE_INIT;

impl<R, B: Behavior<R>> Isolate<R, B> {
  pub fn new(
    behavior: B,
    shared: Option<deno_buf>,
    load_snapshot: Option<deno_buf>,
  ) -> Self {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    let config = libdeno::deno_config {
      will_snapshot: 0,
      load_snapshot: match load_snapshot {
        Some(s) => s,
        None => libdeno::deno_buf::empty(),
      },
      shared: match shared {
        Some(s) => s,
        None => libdeno::deno_buf::empty(),
      },
      recv_cb: Self::pre_dispatch,
    };
    let libdeno_isolate = unsafe { libdeno::deno_new(config) };

    Self {
      libdeno_isolate,
      behavior,
      pending_ops: Vec::new(),
      polled_recently: false,
    }
  }

  extern "C" fn pre_dispatch(
    user_data: *mut c_void,
    control_buf: deno_buf,
    zero_copy_buf: deno_buf,
  ) {
    let isolate = unsafe { Isolate::<R, B>::from_raw_ptr(user_data) };
    assert_eq!(control_buf.len(), 0);
    let zero_copy_id = zero_copy_buf.zero_copy_id;

    let req_record = isolate.behavior.records_pop().unwrap();

    isolate.behavior.records_reset();

    let (is_sync, op) = isolate.behavior.recv(req_record, zero_copy_buf);
    if is_sync {
      let res_record = op.wait().unwrap();
      let push_success = isolate.behavior.records_push(res_record);
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

  pub fn zero_copy_release(&self, zero_copy_id: usize) {
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

  pub fn check_promise_errors(&self) {
    unsafe {
      libdeno::deno_check_promise_errors(self.libdeno_isolate);
    }
  }

  pub fn respond(&mut self) -> Result<(), JSError> {
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

  pub fn mod_new(
    &mut self,
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

  pub fn mod_get_imports(&mut self, id: deno_mod) -> Vec<String> {
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

  extern "C" fn resolve_cb(
    user_data: *mut libc::c_void,
    specifier_ptr: *const libc::c_char,
    referrer: deno_mod,
  ) -> deno_mod {
    let isolate = unsafe { Isolate::<R, B>::from_raw_ptr(user_data) };
    let specifier_c: &CStr = unsafe { CStr::from_ptr(specifier_ptr) };
    let specifier: &str = specifier_c.to_str().unwrap();
    isolate.behavior.resolve(specifier, referrer)
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

impl<R, B: Behavior<R>> Future for Isolate<R, B> {
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

      debug!("poll loop");

      self.polled_recently = true;

      self.behavior.records_reset();

      let mut i = 0;
      while i != self.pending_ops.len() {
        let pending = &mut self.pending_ops[i];
        match pending.poll() {
          Err(()) => panic!("unexpectd error"),
          Ok(Async::NotReady) => {
            i += 1;
          }
          Ok(Async::Ready(record)) => {
            let completed = self.pending_ops.remove(i);
            completed_count += 1;

            if completed.zero_copy_id > 0 {
              self.zero_copy_release(completed.zero_copy_id);
            }

            self.behavior.records_push(record);
          }
        }
      }

      if completed_count > 0 {
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

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  fn js_check(r: Result<(), JSError>) {
    if let Err(e) = r {
      panic!(e.to_string());
    }
  }

  struct TestBehavior {
    recv_count: usize,
    resolve_count: usize,
    push_count: usize,
    pop_count: usize,
    reset_count: usize,
    mod_map: HashMap<String, deno_mod>,
  }

  impl TestBehavior {
    fn new() -> Self {
      Self {
        recv_count: 0,
        resolve_count: 0,
        push_count: 0,
        pop_count: 0,
        reset_count: 0,
        mod_map: HashMap::new(),
      }
    }

    fn register(&mut self, name: &str, id: deno_mod) {
      self.mod_map.insert(name.to_string(), id);
    }
  }

  impl Behavior<()> for TestBehavior {
    fn recv(
      &mut self,
      _record: (),
      _zero_copy_buf: deno_buf,
    ) -> (bool, Box<Op<()>>) {
      self.recv_count += 1;
      (false, Box::new(futures::future::ok(())))
    }

    fn resolve(&mut self, specifier: &str, _referrer: deno_mod) -> deno_mod {
      self.resolve_count += 1;
      match self.mod_map.get(specifier) {
        Some(id) => *id,
        None => 0,
      }
    }

    fn records_reset(&mut self) {
      self.reset_count += 1;
    }

    fn records_push(&mut self, _record: ()) -> bool {
      self.push_count += 1;
      true
    }

    fn records_pop(&mut self) -> Option<()> {
      self.pop_count += 1;
      Some(())
    }
  }

  #[test]
  fn test_recv() {
    let behavior = TestBehavior::new();
    let isolate = Isolate::new(behavior, None, None);
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
    assert_eq!(isolate.behavior.recv_count, 2);
  }

  #[test]
  fn test_mods() {
    let behavior = TestBehavior::new();
    let mut isolate = Isolate::new(behavior, None, None);
    let mod_a = isolate
      .mod_new(
        true,
        "a.js",
        r#"
        import { b } from 'b.js'
        if (b() != 'b') throw Error();
        libdeno.send();
      "#,
      ).unwrap();
    assert_eq!(isolate.behavior.recv_count, 0);
    assert_eq!(isolate.behavior.resolve_count, 0);

    let imports = isolate.mod_get_imports(mod_a);
    assert_eq!(imports, vec!["b.js".to_string()]);

    let mod_b = isolate
      .mod_new(false, "b.js", "export function b() { return 'b' }")
      .unwrap();
    let imports = isolate.mod_get_imports(mod_b);
    assert_eq!(imports.len(), 0);

    js_check(isolate.mod_instantiate(mod_b));
    assert_eq!(isolate.behavior.recv_count, 0);
    assert_eq!(isolate.behavior.resolve_count, 0);

    isolate.behavior.register("b.js", mod_b);
    js_check(isolate.mod_instantiate(mod_a));
    assert_eq!(isolate.behavior.recv_count, 0);
    assert_eq!(isolate.behavior.resolve_count, 1);

    js_check(isolate.mod_evaluate(mod_a));
    assert_eq!(isolate.behavior.recv_count, 1);
    assert_eq!(isolate.behavior.resolve_count, 1);
  }

  #[test]
  fn test_poll_async_immediate_ops() {
    let behavior = TestBehavior::new();
    let mut isolate = Isolate::new(behavior, None, None);

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
    assert_eq!(isolate.behavior.recv_count, 0);
    js_check(isolate.execute(
      "check1.js",
      r#"
        assertEq(nrecv, 0);
        libdeno.send();
        assertEq(nrecv, 0);
        "#,
    ));
    assert_eq!(isolate.behavior.recv_count, 1);
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
    assert_eq!(isolate.behavior.recv_count, 1);
    js_check(isolate.execute(
      "check2.js",
      r#"
        assertEq(nrecv, 1);
        libdeno.send();
        assertEq(nrecv, 1);
        "#,
    ));
    assert_eq!(isolate.behavior.recv_count, 2);
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
    js_check(isolate.execute("check3.js", "assertEq(nrecv, 2)"));
    assert_eq!(isolate.behavior.recv_count, 2);
    // We are idle, so the next poll should be the last.
    assert_eq!(Ok(Async::Ready(())), isolate.poll());
  }

}
