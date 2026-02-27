// Copyright 2018-2025 the Deno authors. MIT license.

use crate::error::CoreErrorKind;
use crate::modules::StaticModuleLoader;
use crate::runtime::tests::Mode;
use crate::runtime::tests::setup;
use crate::*;
use cooked_waker::IntoWaker;
use cooked_waker::Wake;
use cooked_waker::WakeRef;
use deno_error::JsErrorBox;
use parking_lot::Mutex;
use rstest::rstest;
use serde_json::Value;
use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::poll_fn;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI8;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;
use url::Url;

#[test]
fn icu() {
  // If this test fails, update core/runtime/icudtl.dat from
  // rusty_v8/third_party/icu/common/icudtl.dat
  let mut runtime = JsRuntime::new(Default::default());
  runtime
    .execute_script("a.js", "(new Date()).toLocaleString('ja-JP')")
    .unwrap();
}

#[test]
fn test_execute_script_return_value() {
  let mut runtime = JsRuntime::new(Default::default());
  let value_global = runtime.execute_script("a.js", "a = 1 + 2").unwrap();
  {
    deno_core::scope!(scope, runtime);
    let value = value_global.open(scope);
    assert_eq!(value.integer_value(scope).unwrap(), 3);
  }
  let value_global = runtime.execute_script("b.js", "b = 'foobar'").unwrap();
  {
    deno_core::scope!(scope, runtime);
    let value = value_global.open(scope);
    assert!(value.is_string());
    assert_eq!(
      value.to_string(scope).unwrap().to_rust_string_lossy(scope),
      "foobar"
    );
  }
}

#[derive(Default)]
struct LoggingWaker {
  woken: AtomicBool,
}

impl Wake for LoggingWaker {
  fn wake(self) {
    self.woken.store(true, Ordering::SeqCst);
  }
}

impl WakeRef for LoggingWaker {
  fn wake_by_ref(&self) {
    self.woken.store(true, Ordering::SeqCst);
  }
}

/// This is a reproduction for a very obscure bug where the Deno runtime locks up we end up polling
/// an empty JoinSet and attempt to resolve ops after-the-fact. There's a small footgun in the JoinSet
/// API where polling it while empty returns Ready(None), which means that it never holds on to the
/// waker. This means that if we aren't testing for this particular return value and don't stash the waker
/// ourselves for a future async op to eventually queue, we can end up losing the waker entirely and the
/// op wakes up, notifies tokio, which notifies the JoinSet, which then has nobody to notify )`:.
#[tokio::test]
async fn test_wakers_for_async_ops() {
  static STATE: AtomicI8 = AtomicI8::new(0);

  #[op2]
  async fn op_async_sleep() -> Result<(), JsErrorBox> {
    STATE.store(1, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    STATE.store(2, Ordering::SeqCst);
    Ok(())
  }

  STATE.store(0, Ordering::SeqCst);

  let logging_waker = Arc::new(LoggingWaker::default());
  let waker = logging_waker.clone().into_waker();

  deno_core::extension!(test_ext, ops = [op_async_sleep]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  // Drain events until we get to Ready
  loop {
    logging_waker.woken.store(false, Ordering::SeqCst);
    let res = runtime
      .poll_event_loop(&mut Context::from_waker(&waker), Default::default());
    let ready = matches!(res, Poll::Ready(Ok(())));
    assert!(ready || logging_waker.woken.load(Ordering::SeqCst));
    if ready {
      break;
    }
  }

  // Start the AIIFE
  runtime
    .execute_script(
      "",
      ascii_str!(
        "const { op_async_sleep } = Deno.core.ops; (async () => { await op_async_sleep(); })()"
      ),
    )
    .unwrap();

  // Wait for future to finish
  while STATE.load(Ordering::SeqCst) < 2 {
    tokio::time::sleep(Duration::from_millis(1)).await;
  }

  // This shouldn't take one minute, but if it does, things are definitely locked up
  for _ in 0..Duration::from_secs(60).as_millis() {
    if logging_waker.woken.load(Ordering::SeqCst) {
      // Success
      return;
    }
    tokio::time::sleep(Duration::from_millis(1)).await;
  }

  panic!("The waker was never woken after the future completed");
}

#[rstest]
#[case("Promise.resolve(1 + 2)", Ok(3))]
#[case("Promise.resolve(new Promise(resolve => resolve(2 + 2)))", Ok(4))]
#[case(
  "Promise.reject(new Error('fail'))",
  Err("Error: fail\n    at a.js:1:16")
)]
#[case(
  "new Promise(resolve => {})",
  Err(
    "Promise resolution is still pending but the event loop has already resolved"
  )
)]
#[tokio::test]
async fn test_resolve_promise(
  #[case] script: &'static str,
  #[case] result: Result<i32, &'static str>,
) {
  let mut runtime = JsRuntime::new(Default::default());
  let value_global = runtime.execute_script("a.js", script).unwrap();
  let resolve = runtime.resolve(value_global);
  let out = runtime
    .with_event_loop_promise(resolve, PollEventLoopOptions::default())
    .await;
  deno_core::scope!(scope, runtime);
  match result {
    Ok(value) => {
      let out = v8::Local::new(scope, out.expect("expected success"));
      assert_eq!(out.int32_value(scope).unwrap(), value);
    }
    Err(err) => assert_eq!(
      out.expect_err("expected error").to_string(),
      err.to_string()
    ),
  }
}

#[rstest]
#[case("script", "Promise.resolve(1 + 2)", Ok(Some(3)))]
#[case(
  "script",
  "Promise.resolve(new Promise(resolve => resolve(2 + 2)))",
  Ok(Some(4))
)]
#[case(
  "script",
  "Promise.reject(new Error('fail'))",
  Err("Uncaught (in promise) Error: fail")
)]
#[case("script", "new Promise(resolve => {})", Ok(None))]
#[case("call", "async () => 1 + 2", Ok(Some(3)))]
#[case(
  "call",
  "async () => { throw new Error('fail'); }",
  Err("Uncaught (in promise) Error: fail")
)]
#[case("call", "async () => new Promise(resolve => {})", Ok(None))]
#[case("call", "() => Promise.resolve(1 + 2)", Ok(Some(3)))]
#[case(
  "call",
  "() => Promise.resolve(new Promise(resolve => resolve(2 + 2)))",
  Ok(Some(4))
)]
#[case(
  "call",
  "() => Promise.reject(new Error('fail'))",
  Err("Uncaught (in promise) Error: fail")
)]
#[case("call", "() => new Promise(resolve => {})", Ok(None))]
#[case(
  "call",
  "() => { throw new Error('fail'); }",
  Err("Uncaught Error: fail")
)]
#[case(
  "call",
  "() => { Promise.reject(new Error('fail')); return 1; }",
  Ok(Some(1))
)]
// V8 will not terminate the runtime properly before this call returns. This test may fail
// in the future, but is being left as a form of change detection so we can see when this
// happens.
#[case(
  "call",
  "() => { Deno.core.reportUnhandledException(new Error('fail')); return 1; }",
  Ok(Some(1))
)]
#[case(
  "call",
  "() => { Deno.core.reportUnhandledException(new Error('fail')); willNotCall(); }",
  Err("Uncaught Error: fail")
)]
#[tokio::test]
async fn test_resolve_value(
  #[case] runner: &'static str,
  #[case] code: &'static str,
  #[case] output: Result<Option<u32>, &'static str>,
) {
  test_resolve_value_generic(runner, code, output).await
}

async fn test_resolve_value_generic(
  runner: &'static str,
  code: &'static str,
  output: Result<Option<u32>, &'static str>,
) {
  let mut runtime = JsRuntime::new(Default::default());
  let result_global = if runner == "script" {
    let value_global: v8::Global<v8::Value> =
      runtime.execute_script("a.js", code).unwrap();
    #[allow(deprecated)]
    runtime.resolve_value(value_global).await
  } else if runner == "call" {
    let value_global = runtime.execute_script("a.js", code).unwrap();
    let function: v8::Global<v8::Function> =
      unsafe { std::mem::transmute(value_global) };
    #[allow(deprecated)]
    runtime.call_and_await(&function).await
  } else {
    unreachable!()
  };
  deno_core::scope!(scope, runtime);

  match output {
    Ok(None) => {
      let error_string = result_global.unwrap_err().to_string();
      assert_eq!(
        "Promise resolution is still pending but the event loop has already resolved",
        error_string,
      );
    }
    Ok(Some(v)) => {
      let value = result_global.unwrap();
      let value = value.open(scope);
      assert_eq!(value.integer_value(scope).unwrap(), v as i64);
    }
    Err(e) => {
      let Err(err) = result_global else {
        let value = result_global.unwrap();
        let value = value.open(scope);
        panic!(
          "Expected an error, got {}",
          value.to_rust_string_lossy(scope)
        );
      };
      let CoreErrorKind::Js(js_err) = err.into_kind() else {
        unreachable!()
      };
      assert_eq!(e, js_err.exception_message);
    }
  }
}

#[test]
fn terminate_execution_webassembly() {
  let (mut runtime, _dispatch_count) = setup(Mode::Async);
  let v8_isolate_handle = runtime.v8_isolate().thread_safe_handle();

  // Run an infinite loop in WebAssembly code, which should be terminated.
  let promise = runtime.execute_script("infinite_wasm_loop.js",
                                       r#"
                               (async () => {
                                const wasmCode = new Uint8Array([
                                    0,    97,   115,  109,  1,    0,    0,    0,    1,   4,    1,
                                    96,   0,    0,    3,    2,    1,    0,    7,    17,  1,    13,
                                    105,  110,  102,  105,  110,  105,  116,  101,  95,  108,  111,
                                    111,  112,  0,    0,    10,   9,    1,    7,    0,   3,    64,
                                    12,   0,    11,   11,
                                ]);
                                const wasmModule = await WebAssembly.compile(wasmCode);
                                globalThis.wasmInstance = new WebAssembly.Instance(wasmModule);
                                })()
                                    "#).unwrap();
  #[allow(deprecated)]
  futures::executor::block_on(runtime.resolve_value(promise)).unwrap();
  let terminator_thread = std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // terminate execution
    let ok = v8_isolate_handle.terminate_execution();
    assert!(ok);
  });
  let err = runtime
    .execute_script(
      "infinite_wasm_loop2.js",
      "globalThis.wasmInstance.exports.infinite_loop();",
    )
    .unwrap_err();
  assert_eq!(err.to_string(), "Uncaught Error: execution terminated");
  // Cancel the execution-terminating exception in order to allow script
  // execution again.
  let ok = runtime.v8_isolate().cancel_terminate_execution();
  assert!(ok);

  // Verify that the isolate usable again.
  runtime
    .execute_script("simple.js", "1 + 1")
    .expect("execution should be possible again");

  terminator_thread.join().unwrap();
}

#[test]
fn terminate_execution() {
  let (mut isolate, _dispatch_count) = setup(Mode::Async);
  let v8_isolate_handle = isolate.v8_isolate().thread_safe_handle();

  let terminator_thread = std::thread::spawn(move || {
    // allow deno to boot and run
    std::thread::sleep(std::time::Duration::from_millis(100));

    // terminate execution
    let ok = v8_isolate_handle.terminate_execution();
    assert!(ok);
  });

  // Rn an infinite loop, which should be terminated.
  match isolate.execute_script("infinite_loop.js", "for(;;) {}") {
    Ok(_) => panic!("execution should be terminated"),
    Err(e) => {
      assert_eq!(e.to_string(), "Uncaught Error: execution terminated")
    }
  };

  // Cancel the execution-terminating exception in order to allow script
  // execution again.
  let ok = isolate.v8_isolate().cancel_terminate_execution();
  assert!(ok);

  // Verify that the isolate usable again.
  isolate
    .execute_script("simple.js", "1 + 1")
    .expect("execution should be possible again");

  terminator_thread.join().unwrap();
}

#[tokio::test]
async fn wasm_streaming_op_invocation_in_import() {
  let (mut runtime, _dispatch_count) = setup(Mode::Async);

  // Run an infinite loop in WebAssembly code, which should be terminated.
  runtime.execute_script("setup.js",
                         r#"
                                Deno.core.setWasmStreamingCallback((source, rid) => {
                                  Deno.core.ops.op_wasm_streaming_set_url(rid, "file:///foo.wasm");
                                  Deno.core.ops.op_wasm_streaming_feed(rid, source);
                                  Deno.core.close(rid);
                                });
                               "#).unwrap();

  let promise = runtime.execute_script("main.js",
                                       r#"
                             // (module (import "env" "data" (global i64)))
                             const bytes = new Uint8Array([0,97,115,109,1,0,0,0,2,13,1,3,101,110,118,4,100,97,116,97,3,126,0,0,8,4,110,97,109,101,2,1,0]);
                             WebAssembly.instantiateStreaming(bytes, {
                               env: {
                                 get data() {
                                   return new WebAssembly.Global({ value: "i64", mutable: false }, 42n);
                                 }
                               }
                             });
                            "#).unwrap();
  #[allow(deprecated)]
  let value = runtime.resolve_value(promise).await.unwrap();
  deno_core::scope!(scope, runtime);
  let val = value.open(scope);
  assert!(val.is_object());
}

#[test]
fn dangling_shared_isolate() {
  let v8_isolate_handle = {
    // isolate is dropped at the end of this block
    let (mut runtime, _dispatch_count) = setup(Mode::Async);
    runtime.v8_isolate().thread_safe_handle()
  };

  // this should not SEGFAULT
  v8_isolate_handle.terminate_execution();
}

/// Ensure that putting the inspector into OpState doesn't cause crashes. The only valid place we currently allow
/// the inspector to be stashed without cleanup is the OpState, and this should not actually cause crashes.
#[test]
fn inspector() {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    inspector: true,
    ..Default::default()
  });
  // This was causing a crash
  runtime.op_state().borrow_mut().put(runtime.inspector());
  runtime.execute_script("check.js", "null").unwrap();
}

#[rstest]
// https://github.com/denoland/deno/issues/29059
#[case(0.9999999999999999)]
#[case(31.245270191439438)]
#[case(117.63331139400017)]
#[tokio::test]
async fn test_preserve_float_precision_from_local_inspector_evaluate(
  #[case] input: f64,
) {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    inspector: true,
    ..Default::default()
  });

  let result = local_inspector_evaluate(&mut runtime, &format!("{}", input));

  assert_eq!(
    result["result"]["value"],
    Value::Number(serde_json::Number::from_f64(input).unwrap()),
  );
}

fn local_inspector_evaluate(
  runtime: &mut JsRuntime,
  expression: &str,
) -> Value {
  let kind = inspector::InspectorSessionKind::NonBlocking {
    wait_for_disconnect: false,
  };

  let inspector = runtime.inspector();
  let (tx, rx) = std::sync::mpsc::channel();
  let callback = Box::new(move |msg: InspectorMsg| {
    if matches!(msg.kind, InspectorMsgKind::Message(1)) {
      let value: serde_json::Value =
        serde_json::from_str(&msg.content).unwrap();
      let _ = tx.send(value["result"].clone());
    }
  });
  let mut local_inspector_session =
    JsRuntimeInspector::create_local_session(inspector, callback, kind);

  local_inspector_session.post_message(
    1,
    "Runtime.evaluate",
    Some(json!({
      "expression": expression,
    })),
  );

  rx.try_recv().unwrap()
}

#[test]
fn test_get_module_namespace() {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(NoopModuleLoader)),
    ..Default::default()
  });

  let specifier = crate::resolve_url("file:///main.js").unwrap();
  let source_code = r#"
    export const a = "b";
    export default 1 + 2;
  "#;

  let module_id = futures::executor::block_on(
    runtime.load_main_es_module_from_code(&specifier, source_code),
  )
  .unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(module_id);

  let module_namespace = runtime.get_module_namespace(module_id).unwrap();

  deno_core::scope!(scope, runtime);

  let module_namespace = v8::Local::<v8::Object>::new(scope, module_namespace);

  assert!(module_namespace.is_module_namespace_object());

  let unknown_export_name = v8::String::new(scope, "none").unwrap();
  let binding = module_namespace.get(scope, unknown_export_name.into());

  assert!(binding.is_some());
  assert!(binding.unwrap().is_undefined());

  let empty_export_name = v8::String::new(scope, "").unwrap();
  let binding = module_namespace.get(scope, empty_export_name.into());

  assert!(binding.is_some());
  assert!(binding.unwrap().is_undefined());

  let a_export_name = v8::String::new(scope, "a").unwrap();
  let binding = module_namespace.get(scope, a_export_name.into());

  assert!(binding.unwrap().is_string());
  assert_eq!(binding.unwrap(), v8::String::new(scope, "b").unwrap());

  let default_export_name = v8::String::new(scope, "default").unwrap();
  let binding = module_namespace.get(scope, default_export_name.into());

  assert!(binding.unwrap().is_number());
  assert_eq!(binding.unwrap(), v8::Number::new(scope, 3_f64));
}

#[test]
fn test_heap_limits() {
  let create_params =
    v8::Isolate::create_params().heap_limits(0, 5 * 1024 * 1024);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    create_params: Some(create_params),
    ..Default::default()
  });
  let cb_handle = runtime.v8_isolate().thread_safe_handle();

  let callback_invoke_count = Rc::new(AtomicUsize::new(0));
  let inner_invoke_count = Rc::clone(&callback_invoke_count);

  runtime.add_near_heap_limit_callback(move |current_limit, _initial_limit| {
    inner_invoke_count.fetch_add(1, Ordering::SeqCst);
    cb_handle.terminate_execution();
    current_limit * 2
  });
  let js_err = runtime
    .execute_script(
      "script name",
      r#"let s = ""; while(true) { s += "Hello"; }"#,
    )
    .expect_err("script should fail");
  assert_eq!(
    "Uncaught Error: execution terminated",
    js_err.exception_message
  );
  assert!(callback_invoke_count.load(Ordering::SeqCst) > 0)
}

#[test]
fn test_heap_limit_cb_remove() {
  let mut runtime = JsRuntime::new(Default::default());

  runtime.add_near_heap_limit_callback(|current_limit, _initial_limit| {
    current_limit * 2
  });
  runtime.remove_near_heap_limit_callback(3 * 1024 * 1024);
  assert!(runtime.allocations.near_heap_limit_callback_data.is_none());
}

#[test]
fn test_heap_limit_cb_multiple() {
  let create_params =
    v8::Isolate::create_params().heap_limits(0, 5 * 1024 * 1024);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    create_params: Some(create_params),
    ..Default::default()
  });
  let cb_handle = runtime.v8_isolate().thread_safe_handle();

  let callback_invoke_count_first = Rc::new(AtomicUsize::new(0));
  let inner_invoke_count_first = Rc::clone(&callback_invoke_count_first);
  runtime.add_near_heap_limit_callback(move |current_limit, _initial_limit| {
    inner_invoke_count_first.fetch_add(1, Ordering::SeqCst);
    current_limit * 2
  });

  let callback_invoke_count_second = Rc::new(AtomicUsize::new(0));
  let inner_invoke_count_second = Rc::clone(&callback_invoke_count_second);
  runtime.add_near_heap_limit_callback(move |current_limit, _initial_limit| {
    inner_invoke_count_second.fetch_add(1, Ordering::SeqCst);
    cb_handle.terminate_execution();
    current_limit * 2
  });

  let js_err = runtime
    .execute_script(
      "script name",
      r#"let s = ""; while(true) { s += "Hello"; }"#,
    )
    .expect_err("script should fail");
  assert_eq!(
    "Uncaught Error: execution terminated",
    js_err.exception_message
  );
  assert_eq!(0, callback_invoke_count_first.load(Ordering::SeqCst));
  assert!(callback_invoke_count_second.load(Ordering::SeqCst) > 0);
}

#[tokio::test]
async fn test_pump_message_loop() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  poll_fn(move |cx| {
    runtime
      .execute_script(
        "pump_message_loop.js",
        r#"
function assertEquals(a, b) {
if (a === b) return;
throw a + " does not equal " + b;
}
const sab = new SharedArrayBuffer(16);
const i32a = new Int32Array(sab);
globalThis.resolved = false;
(function() {
const result = Atomics.waitAsync(i32a, 0, 0);
result.value.then(
  (value) => { assertEquals("ok", value); globalThis.resolved = true; },
  () => { assertUnreachable();
});
})();
const notify_return_value = Atomics.notify(i32a, 0, 1);
assertEquals(1, notify_return_value);
"#,
      )
      .unwrap();

    match runtime.poll_event_loop(cx, Default::default()) {
      Poll::Ready(Ok(())) => {}
      _ => panic!(),
    };

    // noop script, will resolve promise from first script
    runtime
      .execute_script("pump_message_loop2.js", r#"assertEquals(1, 1);"#)
      .unwrap();

    // check that promise from `Atomics.waitAsync` has been resolved
    runtime
      .execute_script(
        "pump_message_loop3.js",
        r#"assertEquals(globalThis.resolved, true);"#,
      )
      .unwrap();
    Poll::Ready(())
  })
  .await;
}

#[test]
fn test_v8_platform() {
  let options = RuntimeOptions {
    v8_platform: Some(v8::new_default_platform(0, false).make_shared()),
    ..Default::default()
  };
  let mut runtime = JsRuntime::new(options);
  runtime.execute_script("<none>", "").unwrap();
}

#[ignore] // TODO(@littledivy): Fast API ops when snapshot is not loaded.
#[test]
fn test_is_proxy() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  let all_true: v8::Global<v8::Value> = runtime
    .execute_script(
      "is_proxy.js",
      r#"
    (function () {
      const o = { a: 1, b: 2};
      const p = new Proxy(o, {});
      return Deno.core.ops.op_is_proxy(p) && !Deno.core.ops.op_is_proxy(o) && !Deno.core.ops.op_is_proxy(42);
    })()
  "#,
    )
    .unwrap();
  deno_core::scope!(scope, runtime);
  let all_true = v8::Local::<v8::Value>::new(scope, &all_true);
  assert!(all_true.is_true());
}

#[tokio::test]
async fn test_set_macrotask_callback_set_next_tick_callback() {
  #[op2]
  async fn op_async_sleep() -> Result<(), JsErrorBox> {
    // Future must be Poll::Pending on first call
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    Ok(())
  }

  deno_core::extension!(test_ext, ops = [op_async_sleep]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  runtime
    .execute_script(
      "macrotasks_and_nextticks.js",
      r#"
      const { op_async_sleep } = Deno.core.ops;
      (async function () {
        const results = [];
        Deno.core.setMacrotaskCallback(() => {
          results.push("macrotask");
          return true;
        });
        Deno.core.setNextTickCallback(() => {
          results.push("nextTick");
          Deno.core.setHasTickScheduled(false);
        });
        Deno.core.setImmediateCallback(() => {
          results.push("immediate");
        });
        Deno.core.setHasTickScheduled(true);
        await op_async_sleep();
        if (results[0] != "nextTick") {
          throw new Error(`expected nextTick, got: ${results[0]}`);
        }
        if (results[1] != "macrotask") {
          throw new Error(`expected macrotask, got: ${results[1]}`);
        }
        // Manually trigger immediate callbacks to test they were registered
        Deno.core.runImmediateCallbacks();
        if (results[2] != "immediate") {
          throw new Error(`expected immediate, got: ${results[2]}`);
        }
      })();
      "#,
    )
    .unwrap();
  runtime.run_event_loop(Default::default()).await.unwrap();
}

#[test]
fn test_next_tick() {
  use futures::task::ArcWake;

  static MACROTASK: AtomicUsize = AtomicUsize::new(0);
  static NEXT_TICK: AtomicUsize = AtomicUsize::new(0);

  #[allow(clippy::unnecessary_wraps)]
  #[op2(fast)]
  fn op_macrotask() -> Result<(), JsErrorBox> {
    MACROTASK.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }

  #[allow(clippy::unnecessary_wraps)]
  #[op2(fast)]
  fn op_next_tick() -> Result<(), JsErrorBox> {
    NEXT_TICK.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }

  deno_core::extension!(test_ext, ops = [op_macrotask, op_next_tick]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  runtime
    .execute_script(
      "has_tick_scheduled.js",
      r#"
        Deno.core.setMacrotaskCallback(() => {
          Deno.core.ops.op_macrotask();
          return true; // We're done.
        });
        Deno.core.setNextTickCallback(() => Deno.core.ops.op_next_tick());
        Deno.core.setHasTickScheduled(true);
        "#,
    )
    .unwrap();

  struct ArcWakeImpl(Arc<AtomicUsize>);
  impl ArcWake for ArcWakeImpl {
    fn wake_by_ref(arc_self: &Arc<Self>) {
      arc_self.0.fetch_add(1, Ordering::Relaxed);
    }
  }

  let awoken_times = Arc::new(AtomicUsize::new(0));
  let waker = futures::task::waker(Arc::new(ArcWakeImpl(awoken_times.clone())));
  let cx = &mut Context::from_waker(&waker);

  assert!(matches!(
    runtime.poll_event_loop(cx, Default::default()),
    Poll::Pending
  ));
  assert_eq!(1, MACROTASK.load(Ordering::Relaxed));
  assert_eq!(1, NEXT_TICK.load(Ordering::Relaxed));
  assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
  assert!(matches!(
    runtime.poll_event_loop(cx, Default::default()),
    Poll::Pending
  ));
  assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
  assert!(matches!(
    runtime.poll_event_loop(cx, Default::default()),
    Poll::Pending
  ));
  assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
  assert!(matches!(
    runtime.poll_event_loop(cx, Default::default()),
    Poll::Pending
  ));
  assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);

  runtime
    .main_realm()
    .0
    .state()
    .has_next_tick_scheduled
    .take();
  assert!(matches!(
    runtime.poll_event_loop(cx, Default::default()),
    Poll::Ready(Ok(()))
  ));
  assert_eq!(awoken_times.load(Ordering::Relaxed), 0);
  assert!(matches!(
    runtime.poll_event_loop(cx, Default::default()),
    Poll::Ready(Ok(()))
  ));
  assert_eq!(awoken_times.load(Ordering::Relaxed), 0);
}

#[test]
fn terminate_during_module_eval() {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(NoopModuleLoader)),
    ..Default::default()
  });

  let specifier = crate::resolve_url("file:///main.js").unwrap();

  let module_id = futures::executor::block_on(
    runtime
      .load_main_es_module_from_code(&specifier, "Deno.core.print('hello\\n')"),
  )
  .unwrap();

  runtime.v8_isolate().terminate_execution();

  let mod_result =
    futures::executor::block_on(runtime.mod_evaluate(module_id)).unwrap_err();
  assert!(mod_result.to_string().contains("terminated"));
}

async fn test_promise_rejection_handler_generic(
  module: bool,
  case: &'static str,
  error: Option<&'static str>,
) {
  #[op2(fast)]
  fn op_breakpoint() {}

  deno_core::extension!(test_ext, ops = [op_breakpoint]);

  // We don't test throw_ cases in non-module mode since those don't reject
  if !module && case.starts_with("throw_") {
    return;
  }

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  let script = r#"
    let test = "__CASE__";
    function throwError() {
      throw new Error("boom");
    }
    const { op_void_async, op_void_async_deferred } = Deno.core.ops;
    if (test != "no_handler") {
      Deno.core.setUnhandledPromiseRejectionHandler((promise, rejection) => {
        if (test.startsWith("exception_")) {
          try {
            throwError();
          } catch (e) {
            Deno.core.reportUnhandledException(e);
          }
        }
        return test.endsWith("_true");
      });
    }
    if (test != "no_reject") {
      if (test.startsWith("async_op_eager_")) {
        op_void_async().then(() => { Deno.core.ops.op_breakpoint(); throw new Error("fail") });
      } else if (test.startsWith("async_op_deferred_")) {
        op_void_async_deferred().then(() => { Deno.core.ops.op_breakpoint(); throw new Error("fail") });
      } else if (test.startsWith("throw_")) {
        Deno.core.ops.op_breakpoint();
        throw new Error("fail");
      } else {
        Deno.core.ops.op_breakpoint();
        Promise.reject(new Error("fail"));
      }
    }
  "#
    .replace("__CASE__", case);

  let future = if module {
    let id = runtime
      .load_main_es_module_from_code(
        &Url::parse("file:///test.js").unwrap(),
        script,
      )
      .await
      .unwrap();
    Some(runtime.mod_evaluate(id))
  } else {
    runtime.execute_script("", script).unwrap();
    None
  };

  let res = runtime.run_event_loop(Default::default()).await;
  if let Some(error) = error {
    let err = res.expect_err("Expected a failure");
    let CoreErrorKind::Js(js_error) = err.into_kind() else {
      panic!("Expected a JsError");
    };
    assert_eq!(js_error.exception_message, error);
  } else {
    assert!(res.is_ok());
  }

  // Module evaluation will be successful in all cases except the one that throws at
  // the top level.
  if let Some(f) = future {
    f.await.expect("expected module resolution to succeed");
  }
}

#[rstest]
// Don't throw anything -- success
#[case::no_reject("no_reject", None)]
// Reject with no handler
#[case::no_handler("no_handler", Some("Uncaught (in promise) Error: fail"))]
// Exception thrown in unhandled rejection handler
#[case::exception_true("exception_true", Some("Uncaught Error: boom"))]
#[case::exception_false("exception_false", Some("Uncaught Error: boom"))]
// Standard promise rejection
#[case::return_true("return_true", None)]
#[case::return_false("return_false", Some("Uncaught (in promise) Error: fail"))]
// Top-level await throw
#[case::throw_true("throw_true", None)]
#[case::throw_false("throw_false", Some("Uncaught (in promise) Error: fail"))]
// Eager async op, throw from "then"
#[case::async_op_eager_true("async_op_eager_true", None)]
#[case::async_op_eager_false(
  "async_op_eager_false",
  Some("Uncaught (in promise) Error: fail")
)]
// Deferred async op, throw from "then"
#[case::async_op_deferred_true("async_op_deferred_true", None)]
#[case::async_op_deferred_false(
  "async_op_deferred_false",
  Some("Uncaught (in promise) Error: fail")
)]
#[tokio::test]
async fn test_promise_rejection_handler(
  #[case] case: &'static str,
  #[case] error: Option<&'static str>,
  #[values(true, false)] module: bool,
) {
  test_promise_rejection_handler_generic(module, case, error).await
}

// Verify that the async context (continuation-preserved embedder data) that
// was active at the time of a promise rejection is restored when the
// unhandled promise rejection handler is called. This is required for
// AsyncLocalStorage to work correctly inside unhandledRejection handlers
// (matching Node.js behavior). See https://github.com/denoland/deno/issues/30135
#[tokio::test]
async fn test_promise_rejection_handler_preserves_async_context() {
  let mut runtime = JsRuntime::new(Default::default());

  let script = r#"
    const v = new Deno.core.AsyncVariable();
    let capturedValue = undefined;

    Deno.core.setUnhandledPromiseRejectionHandler((promise, rejection) => {
      capturedValue = v.get();
      return true;
    });

    // Enter an async context with a known value, then reject a promise
    const prev = v.enter("my_context_data");
    Promise.reject(new Error("fail"));
    Deno.core.setAsyncContext(prev);

    // capturedValue will be checked after the event loop tick
  "#;

  runtime.execute_script("", script).unwrap();
  runtime
    .run_event_loop(Default::default())
    .await
    .expect("Event loop should complete without error");

  // Verify the handler saw the correct async context
  let result = runtime
    .execute_script(
      "",
      "if (capturedValue !== 'my_context_data') { throw new Error('expected my_context_data but got ' + capturedValue); }",
    )
    .unwrap();
  drop(result);
}

// Make sure that stalled top-level awaits (that is, top-level awaits that
// aren't tied to the progress of some op) are correctly reported, even in a
// realm other than the main one.
#[tokio::test]
async fn test_stalled_tla() {
  let loader = StaticModuleLoader::with(
    Url::parse("file:///test.js").unwrap(),
    "await new Promise(() => {});",
  );
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(loader)),
    ..Default::default()
  });
  let module_id = runtime
    .load_main_es_module(&crate::resolve_url("file:///test.js").unwrap())
    .await
    .unwrap();
  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(module_id);

  let error = runtime
    .run_event_loop(Default::default())
    .await
    .unwrap_err();
  let CoreErrorKind::Js(js_error) = error.into_kind() else {
    unreachable!()
  };
  assert_eq!(
    &js_error.exception_message,
    "Top-level await promise never resolved"
  );
  assert_eq!(js_error.frames.len(), 1);
  assert_eq!(
    js_error.frames[0].file_name.as_deref(),
    Some("file:///test.js")
  );
  assert_eq!(js_error.frames[0].line_number, Some(1));
  assert_eq!(js_error.frames[0].column_number, Some(1));
}

// Regression test for https://github.com/denoland/deno/issues/20034.
#[tokio::test]
async fn test_dynamic_import_module_error_stack() {
  #[op2]
  async fn op_async_error() -> Result<(), JsErrorBox> {
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    Err(deno_error::JsErrorBox::type_error("foo"))
  }
  deno_core::extension!(test_ext, ops = [op_async_error]);
  let loader = StaticModuleLoader::new([
    (
      Url::parse("file:///main.js").unwrap(),
      "await import(\"file:///import.js\");",
    ),
    (
      Url::parse("file:///import.js").unwrap(),
      "const { op_async_error } = Deno.core.ops; await op_async_error();",
    ),
  ]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    module_loader: Some(Rc::new(loader)),
    ..Default::default()
  });

  let module_id = runtime
    .load_main_es_module(&crate::resolve_url("file:///main.js").unwrap())
    .await
    .unwrap();
  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(module_id);

  let error = runtime
    .run_event_loop(Default::default())
    .await
    .unwrap_err();
  let CoreErrorKind::Js(js_error) = error.into_kind() else {
    unreachable!()
  };
  assert_eq!(
    js_error.to_string(),
    "TypeError: foo
    at async file:///import.js:1:43"
  );
}

#[tokio::test]
#[should_panic(
  expected = "Failed to initialize a JsRuntime: Top-level await is not allowed in synchronous evaluation"
)]
async fn tla_in_esm_extensions_panics() {
  #[op2]
  async fn op_wait(#[number] ms: usize) {
    tokio::time::sleep(Duration::from_millis(ms as u64)).await
  }

  deno_core::extension!(
    test_ext,
    ops = [op_wait],
    esm_entry_point = "mod:test",
    esm = [
      "mod:test" = { source = "import 'mod:tla';" },
      "mod:tla" = {
        source = r#"
          const { op_wait } = Deno.core.ops;
          await op_wait(0);
          export const TEST = "foo";
      "#
      }
    ],
  );

  // Panics
  let _runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(StaticModuleLoader::default())),
    extensions: vec![test_ext::init()],
    ..Default::default()
  });
}

#[tokio::test]
async fn generic_in_extension_middleware() {
  trait WelcomeWorld {
    fn hello(&self) -> String;
  }

  struct English;

  impl WelcomeWorld for English {
    fn hello(&self) -> String {
      "Hello World".to_string()
    }
  }

  #[op2]
  #[string]
  fn say_greeting<W: WelcomeWorld + 'static>(state: &mut OpState) -> String {
    let welcomer = state.borrow::<W>();

    welcomer.hello()
  }

  #[op2]
  #[string]
  pub fn say_goodbye() -> String {
    "Goodbye!".to_string()
  }

  deno_core::extension!(welcome_ext, parameters = [W: WelcomeWorld], ops = [say_greeting<W>, say_goodbye],
    middleware = |op| {
        match op.name {
            "say_goodbye" => say_greeting::<W>(),
            _ => op,
        }
    },

  );

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![welcome_ext::init::<English>()],
    ..Default::default()
  });

  {
    let op_state = runtime.op_state();
    let mut state = op_state.borrow_mut();

    state.put(English);
  }

  let value_global = runtime
    .execute_script(
      "greet.js",
      r#"
        const greet = Deno.core.ops.say_greeting();
        const bye = Deno.core.ops.say_goodbye();
        greet + " and " + bye;
      "#,
    )
    .unwrap();

  // Check the result
  deno_core::scope!(scope, &mut runtime);
  let value = value_global.open(scope);

  let result = value.to_rust_string_lossy(scope);
  assert_eq!(result, "Hello World and Hello World");
}
// TODO(mmastrac): This is only fired in debug mode
#[cfg(debug_assertions)]
#[tokio::test]
#[should_panic(
  expected = r#"Failed to initialize a JsRuntime: Error: This fails
    at a (mod:error:2:30)
    at mod:error:3:9"#
)]
async fn esm_extensions_throws() {
  #[op2]
  async fn op_wait(#[number] ms: usize) {
    tokio::time::sleep(Duration::from_millis(ms as u64)).await
  }

  deno_core::extension!(
    test_ext,
    ops = [op_wait],
    esm_entry_point = "mod:test",
    esm = [
      "mod:test" = { source = "import 'mod:error';" },
      "mod:error" = {
        source = r#"
        function a() { throw new Error("This fails") };
        a();
      "#
      }
    ],
  );

  // Panics
  let _runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(StaticModuleLoader::default())),
    extensions: vec![test_ext::init()],
    ..Default::default()
  });
}

fn create_spawner_runtime() -> JsRuntime {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    ..Default::default()
  });
  runtime
    .execute_script("main", ascii_str!("function f() { return 42; }"))
    .unwrap();
  runtime
}

fn call_i32_function(scope: &mut v8::PinScope) -> i32 {
  let ctx = scope.get_current_context();
  let global = ctx.global(scope);
  let key = v8::String::new_external_onebyte_static(scope, b"f")
    .unwrap()
    .into();
  let f: v8::Local<'_, v8::Function> =
    global.get(scope, key).unwrap().try_into().unwrap();
  let recv = v8::undefined(scope).into();
  let res: v8::Local<v8::Integer> =
    f.call(scope, recv, &[]).unwrap().try_into().unwrap();
  res.int32_value(scope).unwrap()
}

#[tokio::test]
async fn task_spawner() {
  let mut runtime = create_spawner_runtime();
  let value = Arc::new(AtomicUsize::new(0));
  let value_clone = value.clone();
  runtime
    .op_state()
    .borrow()
    .borrow::<V8TaskSpawner>()
    .spawn(move |scope| {
      let res = call_i32_function(scope);
      value_clone.store(res as _, Ordering::SeqCst);
    });
  poll_fn(|cx| runtime.poll_event_loop(cx, Default::default()))
    .await
    .unwrap();
  assert_eq!(value.load(Ordering::SeqCst), 42);
}

#[tokio::test]
async fn task_spawner_cross_thread() {
  let mut runtime = create_spawner_runtime();
  let value = Arc::new(AtomicUsize::new(0));
  let value_clone = value.clone();
  let spawner = runtime
    .op_state()
    .borrow()
    .borrow::<V8CrossThreadTaskSpawner>()
    .clone();

  let barrier = Arc::new(std::sync::Barrier::new(2));
  let barrier2 = barrier.clone();
  std::thread::spawn(move || {
    barrier2.wait();
    spawner.spawn(move |scope| {
      let res = call_i32_function(scope);
      value_clone.store(res as _, Ordering::SeqCst);
    });
  });
  barrier.wait();

  // Async spin while we wait for this to complete
  let start = Instant::now();
  while value.load(Ordering::SeqCst) != 42 {
    poll_fn(|cx| runtime.poll_event_loop(cx, Default::default()))
      .await
      .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(start.elapsed().as_secs() < 180);
  }
}

#[tokio::test]
async fn task_spawner_cross_thread_blocking() {
  let mut runtime = create_spawner_runtime();

  let value = Arc::new(AtomicUsize::new(0));
  let value_clone = value.clone();
  let spawner = runtime
    .op_state()
    .borrow()
    .borrow::<V8CrossThreadTaskSpawner>()
    .clone();

  let barrier = Arc::new(std::sync::Barrier::new(2));
  let barrier2 = barrier.clone();
  std::thread::spawn(move || {
    barrier2.wait();
    let res = spawner.spawn_blocking(call_i32_function);
    value_clone.store(res as _, Ordering::SeqCst);
  });
  barrier.wait();

  // Async spin while we wait for this to complete
  let start = Instant::now();
  while value.load(Ordering::SeqCst) != 42 {
    poll_fn(|cx| runtime.poll_event_loop(cx, Default::default()))
      .await
      .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(start.elapsed().as_secs() < 1800);
  }
}

#[tokio::test]
async fn terminate_execution_run_event_loop_js() {
  #[op2]
  async fn op_async_sleep() -> Result<(), JsErrorBox> {
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(())
  }
  deno_core::extension!(test_ext, ops = [op_async_sleep]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  // Start async task
  runtime.execute_script("sleep.js", "(async () => { while (true) { await Deno.core.ops.op_async_sleep(); } })()").unwrap();

  // Terminate execution after 1 second.
  let v8_isolate_handle = runtime.v8_isolate().thread_safe_handle();
  let barrier = Arc::new(std::sync::Barrier::new(2));
  let barrier2 = barrier.clone();
  let terminator_thread = std::thread::spawn(move || {
    barrier2.wait();
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let ok = v8_isolate_handle.terminate_execution();
    assert!(ok);
  });
  barrier.wait();

  let err = runtime
    .run_event_loop(Default::default())
    .await
    .unwrap_err();
  assert_eq!(err.to_string(), "Uncaught Error: execution terminated");

  // Cancel the execution-terminating exception in order to allow script
  // execution again.
  let ok = runtime.v8_isolate().cancel_terminate_execution();
  assert!(ok);

  // Verify that the isolate usable again.
  runtime
    .execute_script("simple.js", "1 + 1")
    .expect("execution should be possible again");

  terminator_thread.join().unwrap();
}

#[tokio::test]
async fn global_template_middleware() {
  use parking_lot::Mutex;
  use v8::MapFnTo;

  static CALLS: Mutex<Vec<String>> = Mutex::new(Vec::new());

  pub fn descriptor<'s>(
    _scope: &mut v8::PinScope<'s, '_>,
    _key: v8::Local<'s, v8::Name>,
    _args: v8::PropertyCallbackArguments<'s>,
    _rv: v8::ReturnValue,
  ) -> v8::Intercepted {
    CALLS.lock().push("descriptor".to_string());

    v8::Intercepted::kNo
  }

  pub fn setter<'s>(
    _scope: &mut v8::PinScope<'s, '_>,
    _key: v8::Local<'s, v8::Name>,
    _value: v8::Local<'s, v8::Value>,
    _args: v8::PropertyCallbackArguments<'s>,
    _rv: v8::ReturnValue<()>,
  ) -> v8::Intercepted {
    CALLS.lock().push("setter".to_string());
    v8::Intercepted::kNo
  }

  fn definer<'s>(
    _scope: &mut v8::PinScope<'s, '_>,
    _key: v8::Local<'s, v8::Name>,
    _descriptor: &v8::PropertyDescriptor,
    _args: v8::PropertyCallbackArguments<'s>,
    _rv: v8::ReturnValue<()>,
  ) -> v8::Intercepted {
    CALLS.lock().push("definer".to_string());
    v8::Intercepted::kNo
  }

  pub fn gt_middleware<'s>(
    _scope: &mut v8::PinScope<'s, '_, ()>,
    template: v8::Local<'s, v8::ObjectTemplate>,
  ) -> v8::Local<'s, v8::ObjectTemplate> {
    let mut config = v8::NamedPropertyHandlerConfiguration::new().flags(
      v8::PropertyHandlerFlags::NON_MASKING
        | v8::PropertyHandlerFlags::HAS_NO_SIDE_EFFECT,
    );

    config = config.descriptor_raw(descriptor.map_fn_to());
    config = config.setter_raw(setter.map_fn_to());
    config = config.definer_raw(definer.map_fn_to());

    template.set_named_property_handler(config);

    template
  }

  deno_core::extension!(test_ext, global_template_middleware = gt_middleware);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  // Create sleep function that waits for 2 seconds.
  runtime
    .execute_script(
      "check_global_template_middleware.js",
      r#"Object.defineProperty(globalThis, 'key', { value: 9, enumerable: true, configurable: true, writable: true })"#,
    )
    .unwrap();

  let calls_set = CALLS
    .lock()
    .clone()
    .into_iter()
    .collect::<HashSet<String>>();
  assert!(calls_set.contains("definer"));
  assert!(calls_set.contains("setter"));
  assert!(calls_set.contains("descriptor"));
}

#[test]
fn eval_context_with_code_cache() {
  let code_cache = {
    let updated_code_cache = Arc::new(Mutex::new(HashMap::new()));

    let get_code_cache_cb = Box::new(|_: &Url, source: &v8::String| {
      Ok(SourceCodeCacheInfo {
        data: None,
        hash: hash_source(source),
      })
    });

    let updated_code_cache_clone = updated_code_cache.clone();
    let set_code_cache_cb =
      Box::new(move |specifier: Url, _hash: u64, code_cache: &[u8]| {
        let mut c = updated_code_cache_clone.lock();
        c.insert(specifier, code_cache.to_vec());
      });

    let mut runtime = JsRuntime::new(RuntimeOptions {
      eval_context_code_cache_cbs: Some((get_code_cache_cb, set_code_cache_cb)),
      ..Default::default()
    });
    runtime
      .execute_script(
        "",
        ascii_str!("Deno.core.evalContext('const i = 10;', 'file:///foo.js');"),
      )
      .unwrap();

    let c = updated_code_cache.lock();
    let mut keys = c.keys().map(|s| s.as_str()).collect::<Vec<_>>();
    keys.sort();
    assert_eq!(keys, vec!["file:///foo.js",]);
    c.clone()
  };

  {
    // Create another runtime and try to use the code cache.
    let updated_code_cache = Arc::new(Mutex::new(HashMap::new()));

    let code_cache_clone = code_cache.clone();
    let get_code_cache_cb =
      Box::new(move |specifier: &Url, source: &v8::String| {
        Ok(SourceCodeCacheInfo {
          data: code_cache_clone
            .get(specifier)
            .map(|code_cache| Cow::Owned(code_cache.clone())),
          hash: hash_source(source),
        })
      });

    let updated_code_cache_clone = updated_code_cache.clone();
    let set_code_cache_cb =
      Box::new(move |specifier: Url, _hash: u64, code_cache: &[u8]| {
        let mut c = updated_code_cache_clone.lock();
        c.insert(specifier, code_cache.to_vec());
      });

    let mut runtime = JsRuntime::new(RuntimeOptions {
      eval_context_code_cache_cbs: Some((get_code_cache_cb, set_code_cache_cb)),
      ..Default::default()
    });
    runtime
      .execute_script(
        "",
        ascii_str!("Deno.core.evalContext('const i = 10;', 'file:///foo.js');"),
      )
      .unwrap();

    // Verify that code cache was not updated, which means that provided code cache was used.
    let c = updated_code_cache.lock();
    assert!(c.is_empty());
  }
}

fn hash_source(source: &v8::String) -> u64 {
  use std::hash::Hash;
  use std::hash::Hasher;
  let mut hasher = twox_hash::XxHash64::default();
  source.hash(&mut hasher);
  hasher.finish()
}
