// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::ascii_str;
use crate::error::custom_error;
use crate::error::generic_error;
use crate::error::AnyError;
use crate::error::JsError;
use crate::extensions::OpDecl;
use crate::include_ascii_string;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::AssertedModuleType;
use crate::modules::ModuleCode;
use crate::modules::ModuleInfo;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleLoader;
use crate::modules::ModuleSource;
use crate::modules::ModuleSourceFuture;
use crate::modules::ModuleType;
use crate::modules::ResolutionKind;
use crate::modules::SymbolicModule;
use crate::Extension;
use crate::JsBuffer;
use crate::*;
use anyhow::Error;
use deno_ops::op;
use futures::future::poll_fn;
use futures::future::Future;
use futures::FutureExt;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

// deno_ops macros generate code assuming deno_core in scope.
mod deno_core {
  pub use crate::*;
}

#[derive(Copy, Clone)]
pub enum Mode {
  Async,
  AsyncDeferred,
  AsyncZeroCopy(bool),
}

struct TestState {
  mode: Mode,
  dispatch_count: Arc<AtomicUsize>,
}

#[op]
async fn op_test(
  rc_op_state: Rc<RefCell<OpState>>,
  control: u8,
  buf: Option<JsBuffer>,
) -> Result<u8, AnyError> {
  #![allow(clippy::await_holding_refcell_ref)] // False positive.
  let op_state_ = rc_op_state.borrow();
  let test_state = op_state_.borrow::<TestState>();
  test_state.dispatch_count.fetch_add(1, Ordering::Relaxed);
  let mode = test_state.mode;
  drop(op_state_);
  match mode {
    Mode::Async => {
      assert_eq!(control, 42);
      Ok(43)
    }
    Mode::AsyncDeferred => {
      tokio::task::yield_now().await;
      assert_eq!(control, 42);
      Ok(43)
    }
    Mode::AsyncZeroCopy(has_buffer) => {
      assert_eq!(buf.is_some(), has_buffer);
      if let Some(buf) = buf {
        assert_eq!(buf.len(), 1);
      }
      Ok(43)
    }
  }
}

fn setup(mode: Mode) -> (JsRuntime, Arc<AtomicUsize>) {
  let dispatch_count = Arc::new(AtomicUsize::new(0));
  deno_core::extension!(
    test_ext,
    ops = [op_test],
    options = {
      mode: Mode,
      dispatch_count: Arc<AtomicUsize>,
    },
    state = |state, options| {
      state.put(TestState {
        mode: options.mode,
        dispatch_count: options.dispatch_count
      })
    }
  );
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops(mode, dispatch_count.clone())],
    get_error_class_fn: Some(&|error| {
      crate::error::get_custom_error_class(error).unwrap()
    }),
    ..Default::default()
  });

  runtime
    .execute_script_static(
      "setup.js",
      r#"
      function assert(cond) {
        if (!cond) {
          throw Error("assert");
        }
      }
      "#,
    )
    .unwrap();
  assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
  (runtime, dispatch_count)
}

#[tokio::test]
async fn test_ref_unref_ops() {
  let (mut runtime, _dispatch_count) = setup(Mode::AsyncDeferred);
  runtime
    .execute_script_static(
      "filename.js",
      r#"

      var promiseIdSymbol = Symbol.for("Deno.core.internalPromiseId");
      var p1 = Deno.core.opAsync("op_test", 42);
      var p2 = Deno.core.opAsync("op_test", 42);
      "#,
    )
    .unwrap();
  {
    let realm = runtime.global_realm();
    assert_eq!(realm.num_pending_ops(), 2);
    assert_eq!(realm.num_unrefed_ops(), 0);
  }
  runtime
    .execute_script_static(
      "filename.js",
      r#"
      Deno.core.ops.op_unref_op(p1[promiseIdSymbol]);
      Deno.core.ops.op_unref_op(p2[promiseIdSymbol]);
      "#,
    )
    .unwrap();
  {
    let realm = runtime.global_realm();
    assert_eq!(realm.num_pending_ops(), 2);
    assert_eq!(realm.num_unrefed_ops(), 2);
  }
  runtime
    .execute_script_static(
      "filename.js",
      r#"
      Deno.core.ops.op_ref_op(p1[promiseIdSymbol]);
      Deno.core.ops.op_ref_op(p2[promiseIdSymbol]);
      "#,
    )
    .unwrap();
  {
    let realm = runtime.global_realm();
    assert_eq!(realm.num_pending_ops(), 2);
    assert_eq!(realm.num_unrefed_ops(), 0);
  }
}

#[test]
fn test_dispatch() {
  let (mut runtime, dispatch_count) = setup(Mode::Async);
  runtime
    .execute_script_static(
      "filename.js",
      r#"
      let control = 42;

      Deno.core.opAsync("op_test", control);
      async function main() {
        Deno.core.opAsync("op_test", control);
      }
      main();
      "#,
    )
    .unwrap();
  assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
}

#[test]
fn test_op_async_promise_id() {
  let (mut runtime, _dispatch_count) = setup(Mode::Async);
  runtime
    .execute_script_static(
      "filename.js",
      r#"

      const p = Deno.core.opAsync("op_test", 42);
      if (p[Symbol.for("Deno.core.internalPromiseId")] == undefined) {
        throw new Error("missing id on returned promise");
      }
      "#,
    )
    .unwrap();
}

#[test]
fn test_dispatch_no_zero_copy_buf() {
  let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(false));
  runtime
    .execute_script_static(
      "filename.js",
      r#"

      Deno.core.opAsync("op_test", 0);
      "#,
    )
    .unwrap();
  assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
}

#[test]
fn test_dispatch_stack_zero_copy_bufs() {
  let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(true));
  runtime
    .execute_script_static(
      "filename.js",
      r#"
      const { op_test } = Deno.core.ensureFastOps();
      let zero_copy_a = new Uint8Array([0]);
      op_test(0, zero_copy_a);
      "#,
    )
    .unwrap();
  assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
}

#[test]
fn test_execute_script_return_value() {
  let mut runtime = JsRuntime::new(Default::default());
  let value_global =
    runtime.execute_script_static("a.js", "a = 1 + 2").unwrap();
  {
    let scope = &mut runtime.handle_scope();
    let value = value_global.open(scope);
    assert_eq!(value.integer_value(scope).unwrap(), 3);
  }
  let value_global = runtime
    .execute_script_static("b.js", "b = 'foobar'")
    .unwrap();
  {
    let scope = &mut runtime.handle_scope();
    let value = value_global.open(scope);
    assert!(value.is_string());
    assert_eq!(
      value.to_string(scope).unwrap().to_rust_string_lossy(scope),
      "foobar"
    );
  }
}

#[tokio::test]
async fn test_poll_value() {
  let mut runtime = JsRuntime::new(Default::default());
  poll_fn(move |cx| {
    let value_global = runtime
      .execute_script_static("a.js", "Promise.resolve(1 + 2)")
      .unwrap();
    let v = runtime.poll_value(&value_global, cx);
    {
      let scope = &mut runtime.handle_scope();
      assert!(
        matches!(v, Poll::Ready(Ok(v)) if v.open(scope).integer_value(scope).unwrap() == 3)
      );
    }

    let value_global = runtime
      .execute_script_static(
        "a.js",
        "Promise.resolve(new Promise(resolve => resolve(2 + 2)))",
      )
      .unwrap();
    let v = runtime.poll_value(&value_global, cx);
    {
      let scope = &mut runtime.handle_scope();
      assert!(
        matches!(v, Poll::Ready(Ok(v)) if v.open(scope).integer_value(scope).unwrap() == 4)
      );
    }

    let value_global = runtime
      .execute_script_static("a.js", "Promise.reject(new Error('fail'))")
      .unwrap();
    let v = runtime.poll_value(&value_global, cx);
    assert!(
      matches!(v, Poll::Ready(Err(e)) if e.downcast_ref::<JsError>().unwrap().exception_message == "Uncaught Error: fail")
    );

    let value_global = runtime
      .execute_script_static("a.js", "new Promise(resolve => {})")
      .unwrap();
    let v = runtime.poll_value(&value_global, cx);
    matches!(v, Poll::Ready(Err(e)) if e.to_string() == "Promise resolution is still pending but the event loop has already resolved.");
    Poll::Ready(())
  }).await;
}

#[tokio::test]
async fn test_resolve_value() {
  let mut runtime = JsRuntime::new(Default::default());
  let value_global = runtime
    .execute_script_static("a.js", "Promise.resolve(1 + 2)")
    .unwrap();
  let result_global = runtime.resolve_value(value_global).await.unwrap();
  {
    let scope = &mut runtime.handle_scope();
    let value = result_global.open(scope);
    assert_eq!(value.integer_value(scope).unwrap(), 3);
  }

  let value_global = runtime
    .execute_script_static(
      "a.js",
      "Promise.resolve(new Promise(resolve => resolve(2 + 2)))",
    )
    .unwrap();
  let result_global = runtime.resolve_value(value_global).await.unwrap();
  {
    let scope = &mut runtime.handle_scope();
    let value = result_global.open(scope);
    assert_eq!(value.integer_value(scope).unwrap(), 4);
  }

  let value_global = runtime
    .execute_script_static("a.js", "Promise.reject(new Error('fail'))")
    .unwrap();
  let err = runtime.resolve_value(value_global).await.unwrap_err();
  assert_eq!(
    "Uncaught Error: fail",
    err.downcast::<JsError>().unwrap().exception_message
  );

  let value_global = runtime
    .execute_script_static("a.js", "new Promise(resolve => {})")
    .unwrap();
  let error_string = runtime
    .resolve_value(value_global)
    .await
    .unwrap_err()
    .to_string();
  assert_eq!(
    "Promise resolution is still pending but the event loop has already resolved.",
    error_string,
  );
}

#[test]
fn terminate_execution_webassembly() {
  let (mut runtime, _dispatch_count) = setup(Mode::Async);
  let v8_isolate_handle = runtime.v8_isolate().thread_safe_handle();

  // Run an infinite loop in WebAssembly code, which should be terminated.
  let promise = runtime.execute_script_static("infinite_wasm_loop.js",
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
  futures::executor::block_on(runtime.resolve_value(promise)).unwrap();
  let terminator_thread = std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // terminate execution
    let ok = v8_isolate_handle.terminate_execution();
    assert!(ok);
  });
  let err = runtime
    .execute_script_static(
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
    .execute_script_static("simple.js", "1 + 1")
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
  match isolate.execute_script_static("infinite_loop.js", "for(;;) {}") {
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
    .execute_script_static("simple.js", "1 + 1")
    .expect("execution should be possible again");

  terminator_thread.join().unwrap();
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

#[test]
fn syntax_error() {
  let mut runtime = JsRuntime::new(Default::default());
  let src = "hocuspocus(";
  let r = runtime.execute_script_static("i.js", src);
  let e = r.unwrap_err();
  let js_error = e.downcast::<JsError>().unwrap();
  let frame = js_error.frames.first().unwrap();
  assert_eq!(frame.column_number, Some(12));
}

#[tokio::test]
async fn test_encode_decode() {
  let (mut runtime, _dispatch_count) = setup(Mode::Async);
  poll_fn(move |cx| {
    runtime
      .execute_script(
        "encode_decode_test.js",
        // Note: We make this to_owned because it contains non-ASCII chars
        include_str!("encode_decode_test.js").to_owned().into(),
      )
      .unwrap();
    if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
      unreachable!();
    }
    Poll::Ready(())
  })
  .await;
}

#[tokio::test]
async fn test_serialize_deserialize() {
  let (mut runtime, _dispatch_count) = setup(Mode::Async);
  poll_fn(move |cx| {
    runtime
      .execute_script(
        "serialize_deserialize_test.js",
        include_ascii_string!("serialize_deserialize_test.js"),
      )
      .unwrap();
    if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
      unreachable!();
    }
    Poll::Ready(())
  })
  .await;
}

#[tokio::test]
async fn test_error_builder() {
  #[op]
  fn op_err() -> Result<(), Error> {
    Err(custom_error("DOMExceptionOperationError", "abc"))
  }

  pub fn get_error_class_name(_: &Error) -> &'static str {
    "DOMExceptionOperationError"
  }

  deno_core::extension!(test_ext, ops = [op_err]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    get_error_class_fn: Some(&get_error_class_name),
    ..Default::default()
  });
  poll_fn(move |cx| {
    runtime
      .execute_script_static(
        "error_builder_test.js",
        include_str!("error_builder_test.js"),
      )
      .unwrap();
    if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
      unreachable!();
    }
    Poll::Ready(())
  })
  .await;
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
  runtime.execute_script_static("check.js", "null").unwrap();
}

#[test]
fn will_snapshot() {
  let snapshot = {
    let mut runtime =
      JsRuntimeForSnapshot::new(Default::default(), Default::default());
    runtime.execute_script_static("a.js", "a = 1 + 2").unwrap();
    runtime.snapshot()
  };

  let snapshot = Snapshot::JustCreated(snapshot);
  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  runtime2
    .execute_script_static("check.js", "if (a != 3) throw Error('x')")
    .unwrap();
}

#[test]
fn will_snapshot2() {
  let startup_data = {
    let mut runtime =
      JsRuntimeForSnapshot::new(Default::default(), Default::default());
    runtime
      .execute_script_static("a.js", "let a = 1 + 2")
      .unwrap();
    runtime.snapshot()
  };

  let snapshot = Snapshot::JustCreated(startup_data);
  let mut runtime = JsRuntimeForSnapshot::new(
    RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    },
    Default::default(),
  );

  let startup_data = {
    runtime
      .execute_script_static("check_a.js", "if (a != 3) throw Error('x')")
      .unwrap();
    runtime.execute_script_static("b.js", "b = 2 + 3").unwrap();
    runtime.snapshot()
  };

  let snapshot = Snapshot::JustCreated(startup_data);
  {
    let mut runtime = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime
      .execute_script_static("check_b.js", "if (b != 5) throw Error('x')")
      .unwrap();
    runtime
      .execute_script_static("check2.js", "if (!Deno.core) throw Error('x')")
      .unwrap();
  }
}

#[test]
fn test_snapshot_callbacks() {
  let snapshot = {
    let mut runtime =
      JsRuntimeForSnapshot::new(Default::default(), Default::default());
    runtime
      .execute_script_static(
        "a.js",
        r#"
        Deno.core.setMacrotaskCallback(() => {
          return true;
        });
        Deno.core.ops.op_set_format_exception_callback(()=> {
          return null;
        })
        Deno.core.setPromiseRejectCallback(() => {
          return false;
        });
        a = 1 + 2;
    "#,
      )
      .unwrap();
    runtime.snapshot()
  };

  let snapshot = Snapshot::JustCreated(snapshot);
  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  runtime2
    .execute_script_static("check.js", "if (a != 3) throw Error('x')")
    .unwrap();
}

#[test]
fn test_from_boxed_snapshot() {
  let snapshot = {
    let mut runtime =
      JsRuntimeForSnapshot::new(Default::default(), Default::default());
    runtime.execute_script_static("a.js", "a = 1 + 2").unwrap();
    let snap: &[u8] = &runtime.snapshot();
    Vec::from(snap).into_boxed_slice()
  };

  let snapshot = Snapshot::Boxed(snapshot);
  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  runtime2
    .execute_script_static("check.js", "if (a != 3) throw Error('x')")
    .unwrap();
}

#[test]
fn test_get_module_namespace() {
  #[derive(Default)]
  struct ModsLoader;

  impl ModuleLoader for ModsLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, Error> {
      assert_eq!(specifier, "file:///main.js");
      assert_eq!(referrer, ".");
      let s = crate::resolve_import(specifier, referrer).unwrap();
      Ok(s)
    }

    fn load(
      &self,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<&ModuleSpecifier>,
      _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
      async { Err(generic_error("Module loading is not supported")) }
        .boxed_local()
    }
  }

  let loader = std::rc::Rc::new(ModsLoader::default());
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let specifier = crate::resolve_url("file:///main.js").unwrap();
  let source_code = ascii_str!(
    r#"
    export const a = "b";
    export default 1 + 2;
    "#
  );

  let module_id = futures::executor::block_on(
    runtime.load_main_module(&specifier, Some(source_code)),
  )
  .unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(module_id);

  let module_namespace = runtime.get_module_namespace(module_id).unwrap();

  let scope = &mut runtime.handle_scope();

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
  let err = runtime
    .execute_script_static(
      "script name",
      r#"let s = ""; while(true) { s += "Hello"; }"#,
    )
    .expect_err("script should fail");
  assert_eq!(
    "Uncaught Error: execution terminated",
    err.downcast::<JsError>().unwrap().exception_message
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

  let err = runtime
    .execute_script_static(
      "script name",
      r#"let s = ""; while(true) { s += "Hello"; }"#,
    )
    .expect_err("script should fail");
  assert_eq!(
    "Uncaught Error: execution terminated",
    err.downcast::<JsError>().unwrap().exception_message
  );
  assert_eq!(0, callback_invoke_count_first.load(Ordering::SeqCst));
  assert!(callback_invoke_count_second.load(Ordering::SeqCst) > 0);
}

#[test]
fn es_snapshot() {
  #[derive(Default)]
  struct ModsLoader;

  impl ModuleLoader for ModsLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, Error> {
      let s = crate::resolve_import(specifier, referrer).unwrap();
      Ok(s)
    }

    fn load(
      &self,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<&ModuleSpecifier>,
      _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
      eprintln!("load() should not be called");
      unreachable!()
    }
  }

  fn create_module(
    runtime: &mut JsRuntime,
    i: usize,
    main: bool,
  ) -> ModuleInfo {
    let specifier = crate::resolve_url(&format!("file:///{i}.js")).unwrap();
    let prev = i - 1;
    let source_code = format!(
      r#"
      import {{ f{prev} }} from "file:///{prev}.js";
      export function f{i}() {{ return f{prev}() }}
      "#
    )
    .into();

    let id = if main {
      futures::executor::block_on(
        runtime.load_main_module(&specifier, Some(source_code)),
      )
      .unwrap()
    } else {
      futures::executor::block_on(
        runtime.load_side_module(&specifier, Some(source_code)),
      )
      .unwrap()
    };
    assert_eq!(i, id);

    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(id);
    futures::executor::block_on(runtime.run_event_loop(false)).unwrap();

    ModuleInfo {
      id,
      main,
      name: specifier.into(),
      requests: vec![crate::modules::ModuleRequest {
        specifier: format!("file:///{prev}.js"),
        asserted_module_type: AssertedModuleType::JavaScriptOrWasm,
      }],
      module_type: ModuleType::JavaScript,
    }
  }

  fn assert_module_map(runtime: &mut JsRuntime, modules: &Vec<ModuleInfo>) {
    let module_map = runtime.module_map.borrow();
    assert_eq!(module_map.handles.len(), modules.len());
    assert_eq!(module_map.info.len(), modules.len());
    assert_eq!(
      module_map.by_name(AssertedModuleType::Json).len()
        + module_map
          .by_name(AssertedModuleType::JavaScriptOrWasm)
          .len(),
      modules.len()
    );

    assert_eq!(module_map.next_load_id, (modules.len() + 1) as ModuleLoadId);

    for info in modules {
      assert!(module_map.handles.get(info.id).is_some());
      assert_eq!(module_map.info.get(info.id).unwrap(), info);
      assert_eq!(
        module_map
          .by_name(AssertedModuleType::JavaScriptOrWasm)
          .get(&info.name)
          .unwrap(),
        &SymbolicModule::Mod(info.id)
      );
    }
  }

  #[op]
  fn op_test() -> Result<String, Error> {
    Ok(String::from("test"))
  }

  let loader = Rc::new(ModsLoader::default());
  let mut runtime = JsRuntimeForSnapshot::new(
    RuntimeOptions {
      module_loader: Some(loader.clone()),
      extensions: vec![Extension::builder("text_ext")
        .ops(vec![op_test::decl()])
        .build()],
      ..Default::default()
    },
    Default::default(),
  );

  let specifier = crate::resolve_url("file:///0.js").unwrap();
  let source_code =
    ascii_str!(r#"export function f0() { return "hello world" }"#);
  let id = futures::executor::block_on(
    runtime.load_side_module(&specifier, Some(source_code)),
  )
  .unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(id);
  futures::executor::block_on(runtime.run_event_loop(false)).unwrap();

  let mut modules = vec![];
  modules.push(ModuleInfo {
    id,
    main: false,
    name: specifier.into(),
    requests: vec![],
    module_type: ModuleType::JavaScript,
  });

  modules.extend((1..200).map(|i| create_module(&mut runtime, i, false)));

  assert_module_map(&mut runtime, &modules);

  let snapshot = runtime.snapshot();

  let mut runtime2 = JsRuntimeForSnapshot::new(
    RuntimeOptions {
      module_loader: Some(loader.clone()),
      startup_snapshot: Some(Snapshot::JustCreated(snapshot)),
      extensions: vec![Extension::builder("text_ext")
        .ops(vec![op_test::decl()])
        .build()],
      ..Default::default()
    },
    Default::default(),
  );

  assert_module_map(&mut runtime2, &modules);

  modules.extend((200..400).map(|i| create_module(&mut runtime2, i, false)));
  modules.push(create_module(&mut runtime2, 400, true));

  assert_module_map(&mut runtime2, &modules);

  let snapshot2 = runtime2.snapshot();

  let mut runtime3 = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    startup_snapshot: Some(Snapshot::JustCreated(snapshot2)),
    extensions: vec![Extension::builder("text_ext")
      .ops(vec![op_test::decl()])
      .build()],
    ..Default::default()
  });

  assert_module_map(&mut runtime3, &modules);

  let source_code = r#"(async () => {
    const mod = await import("file:///400.js");
    return mod.f400() + " " + Deno.core.ops.op_test();
  })();"#;
  let val = runtime3.execute_script_static(".", source_code).unwrap();
  let val = futures::executor::block_on(runtime3.resolve_value(val)).unwrap();
  {
    let scope = &mut runtime3.handle_scope();
    let value = v8::Local::new(scope, val);
    let str_ = value.to_string(scope).unwrap().to_rust_string_lossy(scope);
    assert_eq!(str_, "hello world test");
  }
}

#[test]
fn test_error_without_stack() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  // SyntaxError
  let result = runtime.execute_script_static(
    "error_without_stack.js",
    r#"
function main() {
  console.log("asdf);
}
main();
"#,
  );
  let expected_error = r#"Uncaught SyntaxError: Invalid or unexpected token
    at error_without_stack.js:3:15"#;
  assert_eq!(result.unwrap_err().to_string(), expected_error);
}

#[test]
fn test_error_stack() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  let result = runtime.execute_script_static(
    "error_stack.js",
    r#"
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}
function main() {
  assert(false);
}
main();
      "#,
  );
  let expected_error = r#"Error: assert
    at assert (error_stack.js:4:11)
    at main (error_stack.js:8:3)
    at error_stack.js:10:1"#;
  assert_eq!(result.unwrap_err().to_string(), expected_error);
}

#[tokio::test]
async fn test_error_async_stack() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  poll_fn(move |cx| {
    runtime
      .execute_script_static(
        "error_async_stack.js",
        r#"
  (async () => {
  const p = (async () => {
    await Promise.resolve().then(() => {
      throw new Error("async");
    });
  })();
  try {
    await p;
  } catch (error) {
    console.log(error.stack);
    throw error;
  }
  })();"#,
      )
      .unwrap();
    let expected_error = r#"Error: async
    at error_async_stack.js:5:13
    at async error_async_stack.js:4:5
    at async error_async_stack.js:9:5"#;

    match runtime.poll_event_loop(cx, false) {
      Poll::Ready(Err(e)) => {
        assert_eq!(e.to_string(), expected_error);
      }
      _ => panic!(),
    };
    Poll::Ready(())
  })
  .await;
}

#[tokio::test]
async fn test_error_context() {
  use anyhow::anyhow;

  #[op]
  fn op_err_sync() -> Result<(), Error> {
    Err(anyhow!("original sync error").context("higher-level sync error"))
  }

  #[op]
  async fn op_err_async() -> Result<(), Error> {
    Err(anyhow!("original async error").context("higher-level async error"))
  }

  deno_core::extension!(test_ext, ops = [op_err_sync, op_err_async]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  poll_fn(move |cx| {
    runtime
      .execute_script_static(
        "test_error_context_sync.js",
        r#"
let errMessage;
try {
  Deno.core.ops.op_err_sync();
} catch (err) {
  errMessage = err.message;
}
if (errMessage !== "higher-level sync error: original sync error") {
  throw new Error("unexpected error message from op_err_sync: " + errMessage);
}
"#,
      )
      .unwrap();

    let promise = runtime
      .execute_script_static(
        "test_error_context_async.js",
        r#"

(async () => {
let errMessage;
try {
  await Deno.core.opAsync("op_err_async");
} catch (err) {
  errMessage = err.message;
}
if (errMessage !== "higher-level async error: original async error") {
  throw new Error("unexpected error message from op_err_async: " + errMessage);
}
})()
"#,
      )
      .unwrap();

    match runtime.poll_value(&promise, cx) {
      Poll::Ready(Ok(_)) => {}
      Poll::Ready(Err(err)) => panic!("{err:?}"),
      _ => panic!(),
    }
    Poll::Ready(())
  })
  .await;
}

#[tokio::test]
async fn test_pump_message_loop() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  poll_fn(move |cx| {
    runtime
      .execute_script_static(
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

    match runtime.poll_event_loop(cx, false) {
      Poll::Ready(Ok(())) => {}
      _ => panic!(),
    };

    // noop script, will resolve promise from first script
    runtime
      .execute_script_static("pump_message_loop2.js", r#"assertEquals(1, 1);"#)
      .unwrap();

    // check that promise from `Atomics.waitAsync` has been resolved
    runtime
      .execute_script_static(
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
  runtime.execute_script_static("<none>", "").unwrap();
}

#[ignore] // TODO(@littledivy): Fast API ops when snapshot is not loaded.
#[test]
fn test_is_proxy() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  let all_true: v8::Global<v8::Value> = runtime
    .execute_script_static(
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
  let mut scope = runtime.handle_scope();
  let all_true = v8::Local::<v8::Value>::new(&mut scope, &all_true);
  assert!(all_true.is_true());
}

#[tokio::test]
async fn test_async_opstate_borrow() {
  struct InnerState(u64);

  #[op]
  async fn op_async_borrow(
    op_state: Rc<RefCell<OpState>>,
  ) -> Result<(), Error> {
    let n = {
      let op_state = op_state.borrow();
      let inner_state = op_state.borrow::<InnerState>();
      inner_state.0
    };
    // Future must be Poll::Pending on first call
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    if n != 42 {
      unreachable!();
    }
    Ok(())
  }

  deno_core::extension!(
    test_ext,
    ops = [op_async_borrow],
    state = |state| state.put(InnerState(42))
  );
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  runtime
    .execute_script_static(
      "op_async_borrow.js",
      "Deno.core.opAsync(\"op_async_borrow\")",
    )
    .unwrap();
  runtime.run_event_loop(false).await.unwrap();
}

#[tokio::test]
async fn test_sync_op_serialize_object_with_numbers_as_keys() {
  #[op]
  fn op_sync_serialize_object_with_numbers_as_keys(
    value: serde_json::Value,
  ) -> Result<(), Error> {
    assert_eq!(
      value.to_string(),
      r#"{"lines":{"100":{"unit":"m"},"200":{"unit":"cm"}}}"#
    );
    Ok(())
  }

  deno_core::extension!(
    test_ext,
    ops = [op_sync_serialize_object_with_numbers_as_keys]
  );
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  runtime
    .execute_script_static(
      "op_sync_serialize_object_with_numbers_as_keys.js",
      r#"
Deno.core.ops.op_sync_serialize_object_with_numbers_as_keys({
lines: {
  100: {
    unit: "m"
  },
  200: {
    unit: "cm"
  }
}
})
"#,
    )
    .unwrap();
  runtime.run_event_loop(false).await.unwrap();
}

#[tokio::test]
async fn test_async_op_serialize_object_with_numbers_as_keys() {
  #[op]
  async fn op_async_serialize_object_with_numbers_as_keys(
    value: serde_json::Value,
  ) -> Result<(), Error> {
    assert_eq!(
      value.to_string(),
      r#"{"lines":{"100":{"unit":"m"},"200":{"unit":"cm"}}}"#
    );
    Ok(())
  }

  deno_core::extension!(
    test_ext,
    ops = [op_async_serialize_object_with_numbers_as_keys]
  );
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  runtime
    .execute_script_static(
      "op_async_serialize_object_with_numbers_as_keys.js",
      r#"

Deno.core.opAsync("op_async_serialize_object_with_numbers_as_keys", {
lines: {
  100: {
    unit: "m"
  },
  200: {
    unit: "cm"
  }
}
})
"#,
    )
    .unwrap();
  runtime.run_event_loop(false).await.unwrap();
}

#[tokio::test]
async fn test_set_macrotask_callback_set_next_tick_callback() {
  #[op]
  async fn op_async_sleep() -> Result<(), Error> {
    // Future must be Poll::Pending on first call
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    Ok(())
  }

  deno_core::extension!(test_ext, ops = [op_async_sleep]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  runtime
    .execute_script_static(
      "macrotasks_and_nextticks.js",
      r#"

      (async function () {
        const results = [];
        Deno.core.setMacrotaskCallback(() => {
          results.push("macrotask");
          return true;
        });
        Deno.core.setNextTickCallback(() => {
          results.push("nextTick");
          Deno.core.ops.op_set_has_tick_scheduled(false);
        });
        Deno.core.ops.op_set_has_tick_scheduled(true);
        await Deno.core.opAsync('op_async_sleep');
        if (results[0] != "nextTick") {
          throw new Error(`expected nextTick, got: ${results[0]}`);
        }
        if (results[1] != "macrotask") {
          throw new Error(`expected macrotask, got: ${results[1]}`);
        }
      })();
      "#,
    )
    .unwrap();
  runtime.run_event_loop(false).await.unwrap();
}

#[test]
fn test_has_tick_scheduled() {
  use futures::task::ArcWake;

  static MACROTASK: AtomicUsize = AtomicUsize::new(0);
  static NEXT_TICK: AtomicUsize = AtomicUsize::new(0);

  #[op]
  fn op_macrotask() -> Result<(), AnyError> {
    MACROTASK.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }

  #[op]
  fn op_next_tick() -> Result<(), AnyError> {
    NEXT_TICK.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }

  deno_core::extension!(test_ext, ops = [op_macrotask, op_next_tick]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  runtime
    .execute_script_static(
      "has_tick_scheduled.js",
      r#"
        Deno.core.setMacrotaskCallback(() => {
          Deno.core.ops.op_macrotask();
          return true; // We're done.
        });
        Deno.core.setNextTickCallback(() => Deno.core.ops.op_next_tick());
        Deno.core.ops.op_set_has_tick_scheduled(true);
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

  assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
  assert_eq!(1, MACROTASK.load(Ordering::Relaxed));
  assert_eq!(1, NEXT_TICK.load(Ordering::Relaxed));
  assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
  assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
  assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
  assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
  assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
  assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
  assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);

  runtime.inner.state.borrow_mut().has_tick_scheduled = false;
  assert!(matches!(
    runtime.poll_event_loop(cx, false),
    Poll::Ready(Ok(()))
  ));
  assert_eq!(awoken_times.load(Ordering::Relaxed), 0);
  assert!(matches!(
    runtime.poll_event_loop(cx, false),
    Poll::Ready(Ok(()))
  ));
  assert_eq!(awoken_times.load(Ordering::Relaxed), 0);
}

#[test]
fn terminate_during_module_eval() {
  #[derive(Default)]
  struct ModsLoader;

  impl ModuleLoader for ModsLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, Error> {
      assert_eq!(specifier, "file:///main.js");
      assert_eq!(referrer, ".");
      let s = crate::resolve_import(specifier, referrer).unwrap();
      Ok(s)
    }

    fn load(
      &self,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<&ModuleSpecifier>,
      _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
      async move {
        Ok(ModuleSource::for_test(
          "console.log('hello world');",
          "file:///main.js",
        ))
      }
      .boxed_local()
    }
  }

  let loader = std::rc::Rc::new(ModsLoader::default());
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader),
    ..Default::default()
  });

  let specifier = crate::resolve_url("file:///main.js").unwrap();
  let source_code = ascii_str!("Deno.core.print('hello\\n')");

  let module_id = futures::executor::block_on(
    runtime.load_main_module(&specifier, Some(source_code)),
  )
  .unwrap();

  runtime.v8_isolate().terminate_execution();

  let mod_result =
    futures::executor::block_on(runtime.mod_evaluate(module_id)).unwrap();
  assert!(mod_result
    .unwrap_err()
    .to_string()
    .contains("JavaScript execution has been terminated"));
}

#[tokio::test]
async fn test_unhandled_rejection_order() {
  let mut runtime = JsRuntime::new(Default::default());
  runtime
    .execute_script_static(
      "",
      r#"
      for (let i = 0; i < 100; i++) {
        Promise.reject(i);
      }
      "#,
    )
    .unwrap();
  let err = runtime.run_event_loop(false).await.unwrap_err();
  assert_eq!(err.to_string(), "Uncaught (in promise) 0");
}

#[tokio::test]
async fn test_set_promise_reject_callback() {
  static PROMISE_REJECT: AtomicUsize = AtomicUsize::new(0);

  #[op]
  fn op_promise_reject() -> Result<(), AnyError> {
    PROMISE_REJECT.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }

  deno_core::extension!(test_ext, ops = [op_promise_reject]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  runtime
    .execute_script_static(
      "promise_reject_callback.js",
      r#"
      // Note: |promise| is not the promise created below, it's a child.
      Deno.core.ops.op_set_promise_reject_callback((type, promise, reason) => {
        if (type !== /* PromiseRejectWithNoHandler */ 0) {
          throw Error("unexpected type: " + type);
        }
        if (reason.message !== "reject") {
          throw Error("unexpected reason: " + reason);
        }
        Deno.core.ops.op_store_pending_promise_rejection(promise);
        Deno.core.ops.op_promise_reject();
      });
      new Promise((_, reject) => reject(Error("reject")));
      "#,
    )
    .unwrap();
  runtime.run_event_loop(false).await.unwrap_err();

  assert_eq!(1, PROMISE_REJECT.load(Ordering::Relaxed));

  runtime
    .execute_script_static(
      "promise_reject_callback.js",
      r#"
      {
        const prev = Deno.core.ops.op_set_promise_reject_callback((...args) => {
          prev(...args);
        });
      }
      new Promise((_, reject) => reject(Error("reject")));
      "#,
    )
    .unwrap();
  runtime.run_event_loop(false).await.unwrap_err();

  assert_eq!(2, PROMISE_REJECT.load(Ordering::Relaxed));
}

#[tokio::test]
async fn test_set_promise_reject_callback_realms() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  let global_realm = runtime.global_realm();
  let realm1 = runtime.create_realm().unwrap();
  let realm2 = runtime.create_realm().unwrap();

  let realm_expectations = &[
    (&global_realm, "global_realm", 42),
    (&realm1, "realm1", 140),
    (&realm2, "realm2", 720),
  ];

  // Set up promise reject callbacks.
  for (realm, realm_name, number) in realm_expectations {
    realm
      .execute_script(
        runtime.v8_isolate(),
        "",
        format!(
          r#"

            globalThis.rejectValue = undefined;
            Deno.core.setPromiseRejectCallback((_type, _promise, reason) => {{
              globalThis.rejectValue = `{realm_name}/${{reason}}`;
            }});
            Deno.core.opAsync("op_void_async").then(() => Promise.reject({number}));
          "#
        ).into()
      )
      .unwrap();
  }

  runtime.run_event_loop(false).await.unwrap();

  for (realm, realm_name, number) in realm_expectations {
    let reject_value = realm
      .execute_script_static(runtime.v8_isolate(), "", "globalThis.rejectValue")
      .unwrap();
    let scope = &mut realm.handle_scope(runtime.v8_isolate());
    let reject_value = v8::Local::new(scope, reject_value);
    assert!(reject_value.is_string());
    let reject_value_string = reject_value.to_rust_string_lossy(scope);
    assert_eq!(reject_value_string, format!("{realm_name}/{number}"));
  }
}

#[tokio::test]
async fn test_set_promise_reject_callback_top_level_await() {
  static PROMISE_REJECT: AtomicUsize = AtomicUsize::new(0);

  #[op]
  fn op_promise_reject() -> Result<(), AnyError> {
    PROMISE_REJECT.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }

  deno_core::extension!(test_ext, ops = [op_promise_reject]);

  #[derive(Default)]
  struct ModsLoader;

  impl ModuleLoader for ModsLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, Error> {
      assert_eq!(specifier, "file:///main.js");
      assert_eq!(referrer, ".");
      let s = crate::resolve_import(specifier, referrer).unwrap();
      Ok(s)
    }

    fn load(
      &self,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<&ModuleSpecifier>,
      _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
      let code = r#"
      Deno.core.ops.op_set_promise_reject_callback((type, promise, reason) => {
        Deno.core.ops.op_promise_reject();
      });
      throw new Error('top level throw');
      "#;

      async move { Ok(ModuleSource::for_test(code, "file:///main.js")) }
        .boxed_local()
    }
  }

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    module_loader: Some(Rc::new(ModsLoader)),
    ..Default::default()
  });

  let id = runtime
    .load_main_module(&crate::resolve_url("file:///main.js").unwrap(), None)
    .await
    .unwrap();
  let receiver = runtime.mod_evaluate(id);
  runtime.run_event_loop(false).await.unwrap();
  receiver.await.unwrap().unwrap_err();

  assert_eq!(1, PROMISE_REJECT.load(Ordering::Relaxed));
}

#[test]
fn test_op_return_serde_v8_error() {
  #[op]
  fn op_err() -> Result<std::collections::BTreeMap<u64, u64>, anyhow::Error> {
    Ok([(1, 2), (3, 4)].into_iter().collect()) // Maps can't have non-string keys in serde_v8
  }

  deno_core::extension!(test_ext, ops = [op_err]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });
  assert!(runtime
    .execute_script_static(
      "test_op_return_serde_v8_error.js",
      "Deno.core.ops.op_err()"
    )
    .is_err());
}

#[test]
fn test_op_high_arity() {
  #[op]
  fn op_add_4(
    x1: i64,
    x2: i64,
    x3: i64,
    x4: i64,
  ) -> Result<i64, anyhow::Error> {
    Ok(x1 + x2 + x3 + x4)
  }

  deno_core::extension!(test_ext, ops = [op_add_4]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });
  let r = runtime
    .execute_script_static("test.js", "Deno.core.ops.op_add_4(1, 2, 3, 4)")
    .unwrap();
  let scope = &mut runtime.handle_scope();
  assert_eq!(r.open(scope).integer_value(scope), Some(10));
}

#[test]
fn test_op_disabled() {
  #[op]
  fn op_foo() -> Result<i64, anyhow::Error> {
    Ok(42)
  }

  fn ops() -> Vec<OpDecl> {
    vec![op_foo::decl().disable()]
  }

  deno_core::extension!(test_ext, ops_fn = ops);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });
  let err = runtime
    .execute_script_static("test.js", "Deno.core.ops.op_foo()")
    .unwrap_err();
  assert!(err
    .to_string()
    .contains("TypeError: Deno.core.ops.op_foo is not a function"));
}

#[test]
fn test_op_detached_buffer() {
  use serde_v8::DetachedBuffer;

  #[op]
  fn op_sum_take(b: DetachedBuffer) -> Result<u64, anyhow::Error> {
    Ok(b.as_ref().iter().clone().map(|x| *x as u64).sum())
  }

  #[op]
  fn op_boomerang(b: DetachedBuffer) -> Result<DetachedBuffer, anyhow::Error> {
    Ok(b)
  }

  deno_core::extension!(test_ext, ops = [op_sum_take, op_boomerang]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  runtime
    .execute_script_static(
      "test.js",
      r#"
      const a1 = new Uint8Array([1,2,3]);
      const a1b = a1.subarray(0, 3);
      const a2 = new Uint8Array([5,10,15]);
      const a2b = a2.subarray(0, 3);
      if (!(a1.length > 0 && a1b.length > 0)) {
        throw new Error("a1 & a1b should have a length");
      }
      let sum = Deno.core.ops.op_sum_take(a1b);
      if (sum !== 6) {
        throw new Error(`Bad sum: ${sum}`);
      }
      if (a1.length > 0 || a1b.length > 0) {
        throw new Error("expecting a1 & a1b to be detached");
      }
      const a3 = Deno.core.ops.op_boomerang(a2b);
      if (a3.byteLength != 3) {
        throw new Error(`Expected a3.byteLength === 3, got ${a3.byteLength}`);
      }
      if (a3[0] !== 5 || a3[1] !== 10) {
        throw new Error(`Invalid a3: ${a3[0]}, ${a3[1]}`);
      }
      if (a2.byteLength > 0 || a2b.byteLength > 0) {
        throw new Error("expecting a2 & a2b to be detached, a3 re-attached");
      }
      const wmem = new WebAssembly.Memory({ initial: 1, maximum: 2 });
      const w32 = new Uint32Array(wmem.buffer);
      w32[0] = 1; w32[1] = 2; w32[2] = 3;
      const assertWasmThrow = (() => {
        try {
          let sum = Deno.core.ops.op_sum_take(w32.subarray(0, 2));
          return false;
        } catch(e) {
          return e.message.includes('invalid type; expected: detachable');
        }
      });
      if (!assertWasmThrow()) {
        throw new Error("expected wasm mem to not be detachable");
      }
    "#,
    )
    .unwrap();
}

#[test]
fn test_op_unstable_disabling() {
  #[op]
  fn op_foo() -> Result<i64, anyhow::Error> {
    Ok(42)
  }

  #[op(unstable)]
  fn op_bar() -> Result<i64, anyhow::Error> {
    Ok(42)
  }

  deno_core::extension!(
    test_ext,
    ops = [op_foo, op_bar],
    middleware = |op| if op.is_unstable { op.disable() } else { op }
  );
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });
  runtime
    .execute_script_static(
      "test.js",
      r#"
      if (Deno.core.ops.op_foo() !== 42) {
        throw new Error("Expected op_foo() === 42");
      }
      if (typeof Deno.core.ops.op_bar !== "undefined") {
        throw new Error("Expected op_bar to be disabled")
      }
    "#,
    )
    .unwrap();
}

#[test]
fn js_realm_simple() {
  let mut runtime = JsRuntime::new(Default::default());
  let main_context = runtime.global_context();
  let main_global = {
    let scope = &mut runtime.handle_scope();
    let local_global = main_context.open(scope).global(scope);
    v8::Global::new(scope, local_global)
  };

  let realm = runtime.create_realm().unwrap();
  assert_ne!(realm.context(), &main_context);
  assert_ne!(realm.global_object(runtime.v8_isolate()), main_global);

  let main_object = runtime.execute_script_static("", "Object").unwrap();
  let realm_object = realm
    .execute_script_static(runtime.v8_isolate(), "", "Object")
    .unwrap();
  assert_ne!(main_object, realm_object);
}

#[test]
fn js_realm_init() {
  #[op]
  fn op_test() -> Result<String, Error> {
    Ok(String::from("Test"))
  }

  deno_core::extension!(test_ext, ops = [op_test]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });
  let realm = runtime.create_realm().unwrap();
  let ret = realm
    .execute_script_static(runtime.v8_isolate(), "", "Deno.core.ops.op_test()")
    .unwrap();

  let scope = &mut realm.handle_scope(runtime.v8_isolate());
  assert_eq!(ret, serde_v8::to_v8(scope, "Test").unwrap());
}

#[test]
fn js_realm_init_snapshot() {
  let snapshot = {
    let runtime =
      JsRuntimeForSnapshot::new(Default::default(), Default::default());
    let snap: &[u8] = &runtime.snapshot();
    Vec::from(snap).into_boxed_slice()
  };

  #[op]
  fn op_test() -> Result<String, Error> {
    Ok(String::from("Test"))
  }

  deno_core::extension!(test_ext, ops = [op_test]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(Snapshot::Boxed(snapshot)),
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });
  let realm = runtime.create_realm().unwrap();
  let ret = realm
    .execute_script_static(runtime.v8_isolate(), "", "Deno.core.ops.op_test()")
    .unwrap();

  let scope = &mut realm.handle_scope(runtime.v8_isolate());
  assert_eq!(ret, serde_v8::to_v8(scope, "Test").unwrap());
}

#[test]
fn js_realm_sync_ops() {
  // Test that returning a RustToV8Buf and throwing an exception from a sync
  // op result in objects with prototypes from the right realm. Note that we
  // don't test the result of returning structs, because they will be
  // serialized to objects with null prototype.

  #[op]
  fn op_test(fail: bool) -> Result<ToJsBuffer, Error> {
    if !fail {
      Ok(ToJsBuffer::empty())
    } else {
      Err(crate::error::type_error("Test"))
    }
  }

  deno_core::extension!(test_ext, ops = [op_test]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    get_error_class_fn: Some(&|error| {
      crate::error::get_custom_error_class(error).unwrap()
    }),
    ..Default::default()
  });
  let new_realm = runtime.create_realm().unwrap();

  // Test in both realms
  for realm in [runtime.global_realm(), new_realm].into_iter() {
    let ret = realm
      .execute_script_static(
        runtime.v8_isolate(),
        "",
        r#"
          const buf = Deno.core.ops.op_test(false);
          try {
            Deno.core.ops.op_test(true);
          } catch(e) {
            err = e;
          }
          buf instanceof Uint8Array && buf.byteLength === 0 &&
          err instanceof TypeError && err.message === "Test"
        "#,
      )
      .unwrap();
    assert!(ret.open(runtime.v8_isolate()).is_true());
  }
}

#[tokio::test]
async fn js_realm_async_ops() {
  // Test that returning a RustToV8Buf and throwing an exception from a async
  // op result in objects with prototypes from the right realm. Note that we
  // don't test the result of returning structs, because they will be
  // serialized to objects with null prototype.

  #[op]
  async fn op_test(fail: bool) -> Result<ToJsBuffer, Error> {
    if !fail {
      Ok(ToJsBuffer::empty())
    } else {
      Err(crate::error::type_error("Test"))
    }
  }

  deno_core::extension!(test_ext, ops = [op_test]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    get_error_class_fn: Some(&|error| {
      crate::error::get_custom_error_class(error).unwrap()
    }),
    ..Default::default()
  });

  let global_realm = runtime.global_realm();
  let new_realm = runtime.create_realm().unwrap();

  let mut rets = vec![];

  // Test in both realms
  for realm in [global_realm, new_realm].into_iter() {
    let ret = realm
      .execute_script_static(
        runtime.v8_isolate(),
        "",
        r#"

          (async function () {
            const buf = await Deno.core.opAsync("op_test", false);
            let err;
            try {
              await Deno.core.opAsync("op_test", true);
            } catch(e) {
              err = e;
            }
            return buf instanceof Uint8Array && buf.byteLength === 0 &&
                    err instanceof TypeError && err.message === "Test" ;
          })();
        "#,
      )
      .unwrap();
    rets.push((realm, ret));
  }

  runtime.run_event_loop(false).await.unwrap();

  for ret in rets {
    let scope = &mut ret.0.handle_scope(runtime.v8_isolate());
    let value = v8::Local::new(scope, ret.1);
    let promise = v8::Local::<v8::Promise>::try_from(value).unwrap();
    let result = promise.result(scope);

    assert!(result.is_boolean() && result.is_true());
  }
}

#[ignore]
#[tokio::test]
async fn js_realm_gc() {
  static INVOKE_COUNT: AtomicUsize = AtomicUsize::new(0);
  struct PendingFuture {}

  impl Future for PendingFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
      Poll::Pending
    }
  }

  impl Drop for PendingFuture {
    fn drop(&mut self) {
      assert_eq!(INVOKE_COUNT.fetch_sub(1, Ordering::SeqCst), 1);
    }
  }

  // Never resolves.
  #[op]
  async fn op_pending() {
    assert_eq!(INVOKE_COUNT.fetch_add(1, Ordering::SeqCst), 0);
    PendingFuture {}.await
  }

  deno_core::extension!(test_ext, ops = [op_pending]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  // Detect a drop in OpState
  let opstate_drop_detect = Rc::new(());
  runtime
    .op_state()
    .borrow_mut()
    .put(opstate_drop_detect.clone());
  assert_eq!(Rc::strong_count(&opstate_drop_detect), 2);

  let other_realm = runtime.create_realm().unwrap();
  other_realm
    .execute_script(
      runtime.v8_isolate(),
      "future",
      ModuleCode::from_static("Deno.core.opAsync('op_pending')"),
    )
    .unwrap();
  while INVOKE_COUNT.load(Ordering::SeqCst) == 0 {
    poll_fn(|cx: &mut Context| runtime.poll_event_loop(cx, false))
      .await
      .unwrap();
  }
  drop(other_realm);
  while INVOKE_COUNT.load(Ordering::SeqCst) == 1 {
    poll_fn(|cx| runtime.poll_event_loop(cx, false))
      .await
      .unwrap();
  }
  drop(runtime);

  // Make sure the OpState was dropped properly when the runtime dropped
  assert_eq!(Rc::strong_count(&opstate_drop_detect), 1);
}

#[tokio::test]
async fn js_realm_ref_unref_ops() {
  // Never resolves.
  #[op]
  async fn op_pending() {
    futures::future::pending().await
  }

  deno_core::extension!(test_ext, ops = [op_pending]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  poll_fn(move |cx| {
    let main_realm = runtime.global_realm();
    let other_realm = runtime.create_realm().unwrap();

    main_realm
      .execute_script_static(
        runtime.v8_isolate(),
        "",
        r#"

          var promise = Deno.core.opAsync("op_pending");
        "#,
      )
      .unwrap();
    other_realm
      .execute_script_static(
        runtime.v8_isolate(),
        "",
        r#"

          var promise = Deno.core.opAsync("op_pending");
        "#,
      )
      .unwrap();
    assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));

    main_realm
      .execute_script_static(
        runtime.v8_isolate(),
        "",
        r#"
          let promiseIdSymbol = Symbol.for("Deno.core.internalPromiseId");
          Deno.core.unrefOp(promise[promiseIdSymbol]);
        "#,
      )
      .unwrap();
    assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));

    other_realm
      .execute_script_static(
        runtime.v8_isolate(),
        "",
        r#"
          let promiseIdSymbol = Symbol.for("Deno.core.internalPromiseId");
          Deno.core.unrefOp(promise[promiseIdSymbol]);
        "#,
      )
      .unwrap();
    assert!(matches!(
      runtime.poll_event_loop(cx, false),
      Poll::Ready(Ok(()))
    ));
    Poll::Ready(())
  })
  .await;
}

#[test]
fn test_array_by_copy() {
  // Verify that "array by copy" proposal is enabled (https://github.com/tc39/proposal-change-array-by-copy)
  let mut runtime = JsRuntime::new(Default::default());
  assert!(runtime
    .execute_script_static(
      "test_array_by_copy.js",
      "const a = [1, 2, 3];
      const b = a.toReversed();
      if (!(a[0] === 1 && a[1] === 2 && a[2] === 3)) {
        throw new Error('Expected a to be intact');
      }
      if (!(b[0] === 3 && b[1] === 2 && b[2] === 1)) {
        throw new Error('Expected b to be reversed');
      }",
    )
    .is_ok());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "Found ops with duplicate names:")]
fn duplicate_op_names() {
  mod a {
    use super::*;

    #[op]
    fn op_test() -> Result<String, Error> {
      Ok(String::from("Test"))
    }
  }

  #[op]
  fn op_test() -> Result<String, Error> {
    Ok(String::from("Test"))
  }

  deno_core::extension!(test_ext, ops = [a::op_test, op_test]);
  JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });
}

#[test]
fn ops_in_js_have_proper_names() {
  #[op]
  fn op_test_sync() -> Result<String, Error> {
    Ok(String::from("Test"))
  }

  #[op]
  async fn op_test_async() -> Result<String, Error> {
    Ok(String::from("Test"))
  }

  deno_core::extension!(test_ext, ops = [op_test_sync, op_test_async]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init_ops()],
    ..Default::default()
  });

  let src = r#"
  if (Deno.core.ops.op_test_sync.name !== "op_test_sync") {
    throw new Error();
  }

  if (Deno.core.ops.op_test_async.name !== "op_test_async") {
    throw new Error();
  }

  const { op_test_async } = Deno.core.ensureFastOps();
  if (op_test_async.name !== "op_test_async") {
    throw new Error();
  }
  "#;
  runtime.execute_script_static("test", src).unwrap();
}
