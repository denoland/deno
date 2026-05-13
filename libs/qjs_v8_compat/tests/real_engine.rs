// Copyright 2018-2026 the Deno authors. MIT license.
//
// Smoke test that exercises the *real* QuickJS-ng C library through our FFI
// declarations. Gated on `--features link_quickjs` so it only runs when the
// vendored engine is built and linked in.
//
// What we prove here:
//
// 1. Our FFI declarations are ABI-compatible with the linked libquickjs:
//    if any signature were wrong, the calls below would crash or corrupt
//    memory.
//
// 2. Our `JSValue` layout (the `JSValueUnion`/`tag` pair we declare in
//    `ffi.rs`) matches what QuickJS-ng returns, so we can decode result
//    tags and read out integer / float / string contents correctly.
//
// 3. The integer path through `JS_Eval` works end-to-end and refcounts
//    balance under `JS_FreeValue`. This is the foundation of the
//    "hello world" path described in [[architecture-integration-plan]].
//
// Each test creates its own `JSRuntime` so they're independent and any
// memory leak caught by the runtime's leak check fires per-test.

#![cfg(feature = "link_quickjs")]

use std::ffi::CString;

use qjs_v8_compat::ffi;

fn eval_int(src: &str) -> i32 {
  let src_c = CString::new(src).unwrap();
  let fname_c = CString::new("<smoke>").unwrap();
  unsafe {
    let rt = ffi::JS_NewRuntime();
    assert!(!rt.is_null(), "JS_NewRuntime returned null");
    let ctx = ffi::JS_NewContext(rt);
    assert!(!ctx.is_null(), "JS_NewContext returned null");

    let val = ffi::JS_Eval(
      ctx,
      src_c.as_ptr(),
      src.len(),
      fname_c.as_ptr(),
      ffi::JS_EVAL_TYPE_GLOBAL,
    );
    assert_ne!(
      val.tag,
      ffi::JS_TAG_EXCEPTION,
      "JS_Eval returned an exception for src={src:?}"
    );
    assert_eq!(
      val.tag,
      ffi::JS_TAG_INT,
      "expected JS_TAG_INT result for src={src:?}, got tag={}",
      val.tag
    );
    let n = val.u.int32;

    ffi::JS_FreeValue(ctx, val);
    ffi::JS_FreeContext(ctx);
    ffi::JS_FreeRuntime(rt);
    n
  }
}

#[test]
fn eval_arithmetic() {
  assert_eq!(eval_int("1 + 1"), 2);
  assert_eq!(eval_int("6 * 7"), 42);
  assert_eq!(eval_int("(function () { return 100 - 1; })()"), 99);
}

#[test]
fn eval_with_let() {
  // A multi-statement program. The last expression's value is returned.
  let src = "let x = 0; for (let i = 1; i <= 10; i++) x += i; x";
  assert_eq!(eval_int(src), 55);
}

#[test]
fn eval_returning_string_via_length() {
  // Avoid taking on the full string-decoding surface here; just compute the
  // length of "hello world" using JS-side `.length` and assert it as int.
  // This proves the eval path can drive an arbitrary JS expression, not
  // just numeric arithmetic.
  assert_eq!(eval_int("'hello world'.length"), 11);
}

#[test]
fn runtime_lifecycle_does_not_leak() {
  // A loop creating and dropping runtimes. If `JS_FreeRuntime` were
  // misdeclared (e.g. wrong calling convention), this would corrupt the
  // heap and crash long before the loop finishes.
  for _ in 0..32 {
    unsafe {
      let rt = ffi::JS_NewRuntime();
      let ctx = ffi::JS_NewContext(rt);
      ffi::JS_FreeContext(ctx);
      ffi::JS_FreeRuntime(rt);
    }
  }
}

// ---- hello world: register a Rust callback as console.log ---------------
//
// This is the smallest end-to-end demonstration of host -> guest -> host
// control flow. JS calls into Rust, Rust collects the strings, and the test
// asserts on what came back. It's the path that every Deno op walks and
// the precondition for any future deno_core integration.
//
// Each test thread has its own collector. The C callback reads from the
// thread-local set up by the test that installed the function. Tests run
// in parallel by default; the thread-local guarantees they don't stomp
// each other.

use std::cell::RefCell;

thread_local! {
  static COLLECTOR: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "C" fn console_log_callback(
  ctx: *mut ffi::JSContext,
  _this: ffi::JSValue,
  argc: std::ffi::c_int,
  argv: *mut ffi::JSValue,
) -> ffi::JSValue {
  let mut parts = Vec::with_capacity(argc as usize);
  for i in 0..argc as isize {
    unsafe {
      let arg = *argv.offset(i);
      let p = ffi::JS_ToCString(ctx, arg);
      if !p.is_null() {
        let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
        ffi::JS_FreeCString(ctx, p);
        parts.push(s);
      }
    }
  }
  let line = parts.join(" ");
  COLLECTOR.with(|c| c.borrow_mut().push(line));
  ffi::jsv_undefined()
}

fn run_with_console_log(src: &str) -> Vec<String> {
  COLLECTOR.with(|c| c.borrow_mut().clear());

  let src_c = CString::new(src).unwrap();
  let fname_c = CString::new("<hello>").unwrap();
  let log_name = CString::new("log").unwrap();
  let console_name = CString::new("console").unwrap();

  unsafe {
    let rt = ffi::JS_NewRuntime();
    let ctx = ffi::JS_NewContext(rt);

    let global = ffi::JS_GetGlobalObject(ctx);
    let console = ffi::JS_NewObject(ctx);
    let log_fn =
      ffi::JS_NewCFunction(ctx, console_log_callback, log_name.as_ptr(), 1);
    ffi::JS_SetPropertyStr(ctx, console, log_name.as_ptr(), log_fn);
    ffi::JS_SetPropertyStr(ctx, global, console_name.as_ptr(), console);
    ffi::JS_FreeValue(ctx, global);

    let val = ffi::JS_Eval(
      ctx,
      src_c.as_ptr(),
      src.len(),
      fname_c.as_ptr(),
      ffi::JS_EVAL_TYPE_GLOBAL,
    );
    assert_ne!(val.tag, ffi::JS_TAG_EXCEPTION, "JS threw for src={src:?}");
    ffi::JS_FreeValue(ctx, val);

    ffi::JS_FreeContext(ctx);
    ffi::JS_FreeRuntime(rt);
  }

  COLLECTOR.with(|c| c.borrow().clone())
}

#[test]
fn hello_world_via_console_log() {
  let out = run_with_console_log("console.log('hello, world')");
  assert_eq!(out, vec!["hello, world".to_string()]);
}

#[test]
fn console_log_multiple_args() {
  let out = run_with_console_log("console.log('a', 'b', 1 + 2)");
  assert_eq!(out, vec!["a b 3".to_string()]);
}

#[test]
fn console_log_called_in_loop() {
  let out =
    run_with_console_log("for (let i = 0; i < 5; i++) console.log('n=' + i)");
  assert_eq!(
    out,
    vec![
      "n=0".to_string(),
      "n=1".to_string(),
      "n=2".to_string(),
      "n=3".to_string(),
      "n=4".to_string(),
    ]
  );
}

// ---- promises + microtask draining --------------------------------------
//
// QuickJS-ng uses a job queue (`JS_ExecutePendingJob`) for microtasks
// (Promise callbacks, queueMicrotask). Eval-ing `Promise.resolve(42).then(...)`
// only schedules the callback; the callback fires when we drain pending jobs.
// This is the central fact every async integration has to model: the host
// pumps `JS_ExecutePendingJob` until `JS_IsJobPending` returns 0 (or until
// the host wants to suspend the loop and wait for an external event).
//
// These tests prove the drain loop works against the real engine, which
// is the foundation for a tokio-driven event loop in any future deno_core
// integration.

fn drain_jobs(rt: *mut ffi::JSRuntime) {
  unsafe {
    while ffi::JS_IsJobPending(rt) > 0 {
      let mut pctx: *mut ffi::JSContext = std::ptr::null_mut();
      let r = ffi::JS_ExecutePendingJob(rt, &mut pctx);
      if r <= 0 {
        // 0 = no more jobs, <0 = job threw. Stop in either case; if it
        // threw, the test will detect it via the captured collector.
        break;
      }
    }
  }
}

#[test]
fn promise_resolve_then_drains_to_callback() {
  // Promise.resolve(42).then(v => Rust callback) — fires after drain.
  let out = COLLECTOR.with(|c| {
    c.borrow_mut().clear();
    let src =
      "Promise.resolve(42).then(v => collect('p=' + (v + 1))); 'scheduled'";
    let src_c = CString::new(src).unwrap();
    let fname_c = CString::new("<promise>").unwrap();
    let collect_name = CString::new("collect").unwrap();

    unsafe {
      let rt = ffi::JS_NewRuntime();
      let ctx = ffi::JS_NewContext(rt);
      let global = ffi::JS_GetGlobalObject(ctx);
      let collect_fn = ffi::JS_NewCFunction(
        ctx,
        console_log_callback,
        collect_name.as_ptr(),
        1,
      );
      ffi::JS_SetPropertyStr(ctx, global, collect_name.as_ptr(), collect_fn);
      ffi::JS_FreeValue(ctx, global);

      let val = ffi::JS_Eval(
        ctx,
        src_c.as_ptr(),
        src.len(),
        fname_c.as_ptr(),
        ffi::JS_EVAL_TYPE_GLOBAL,
      );
      assert_ne!(val.tag, ffi::JS_TAG_EXCEPTION, "promise eval threw: {src}");
      ffi::JS_FreeValue(ctx, val);

      // Before draining, no callback has fired yet.
      let pre = c.borrow().clone();
      assert!(pre.is_empty(), "callback fired synchronously: {:?}", pre);

      drain_jobs(rt);

      ffi::JS_FreeContext(ctx);
      ffi::JS_FreeRuntime(rt);
    }

    c.borrow().clone()
  });

  assert_eq!(out, vec!["p=43".to_string()]);
}

#[test]
fn promise_chain_drains_in_order() {
  // Multiple .then steps and a queueMicrotask all interleave correctly
  // when we keep pumping pending jobs.
  let out = COLLECTOR.with(|c| {
    c.borrow_mut().clear();
    let src = r#"
      Promise.resolve(1)
        .then(v => { collect('a=' + v); return v + 1; })
        .then(v => { collect('b=' + v); return v + 1; })
        .then(v => { collect('c=' + v); });
      queueMicrotask(() => collect('mt'));
      'scheduled'
    "#;
    let src_c = CString::new(src).unwrap();
    let fname_c = CString::new("<chain>").unwrap();
    let collect_name = CString::new("collect").unwrap();

    unsafe {
      let rt = ffi::JS_NewRuntime();
      let ctx = ffi::JS_NewContext(rt);
      let global = ffi::JS_GetGlobalObject(ctx);
      let collect_fn = ffi::JS_NewCFunction(
        ctx,
        console_log_callback,
        collect_name.as_ptr(),
        1,
      );
      ffi::JS_SetPropertyStr(ctx, global, collect_name.as_ptr(), collect_fn);
      ffi::JS_FreeValue(ctx, global);

      let val = ffi::JS_Eval(
        ctx,
        src_c.as_ptr(),
        src.len(),
        fname_c.as_ptr(),
        ffi::JS_EVAL_TYPE_GLOBAL,
      );
      assert_ne!(val.tag, ffi::JS_TAG_EXCEPTION);
      ffi::JS_FreeValue(ctx, val);

      drain_jobs(rt);

      ffi::JS_FreeContext(ctx);
      ffi::JS_FreeRuntime(rt);
    }

    c.borrow().clone()
  });

  // a=1 fires first (queued before mt), then mt (its own microtask),
  // then b=2 (chained off a's resolution), then c=3.
  // QuickJS's queueing puts the .then callback in the job queue at the
  // moment its parent settles; queueMicrotask puts the cb at top-level
  // immediately. Exact interleaving is "all four fired and the chain
  // is in order" — assert that, not the precise position of mt.
  assert!(out.contains(&"a=1".to_string()));
  assert!(out.contains(&"b=2".to_string()));
  assert!(out.contains(&"c=3".to_string()));
  assert!(out.contains(&"mt".to_string()));
  assert_eq!(out.len(), 4);

  let a = out.iter().position(|s| s == "a=1").unwrap();
  let b = out.iter().position(|s| s == "b=2").unwrap();
  let c = out.iter().position(|s| s == "c=3").unwrap();
  assert!(a < b && b < c, "chain out of order: {out:?}");
}
