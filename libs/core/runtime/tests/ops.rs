// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unused_async)]

use crate::extensions::OpDecl;
use crate::modules::StaticModuleLoader;
use crate::runtime::tests::Mode;
use crate::runtime::tests::setup;
use crate::*;
use deno_error::JsErrorBox;
use pretty_assertions::assert_eq;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use url::Url;

#[tokio::test]
async fn test_async_opstate_borrow() {
  struct InnerState(u64);

  #[op2]
  async fn op_async_borrow(
    op_state: Rc<RefCell<OpState>>,
  ) -> Result<(), JsErrorBox> {
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
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  runtime
    .execute_script(
      "op_async_borrow.js",
      "const { op_async_borrow } = Deno.core.ops; op_async_borrow();",
    )
    .unwrap();
  runtime.run_event_loop(Default::default()).await.unwrap();
}

#[tokio::test]
async fn test_sync_op_serialize_object_with_numbers_as_keys() {
  #[allow(clippy::unnecessary_wraps)]
  #[op2]
  fn op_sync_serialize_object_with_numbers_as_keys(
    #[serde] value: serde_json::Value,
  ) -> Result<(), JsErrorBox> {
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
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  runtime
    .execute_script(
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
  runtime.run_event_loop(Default::default()).await.unwrap();
}

#[tokio::test]
async fn test_async_op_serialize_object_with_numbers_as_keys() {
  #[op2]
  async fn op_async_serialize_object_with_numbers_as_keys(
    #[serde] value: serde_json::Value,
  ) -> Result<(), JsErrorBox> {
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
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  runtime
    .execute_script(
      "op_async_serialize_object_with_numbers_as_keys.js",
      r#"
        const { op_async_serialize_object_with_numbers_as_keys } = Deno.core.ops;
        op_async_serialize_object_with_numbers_as_keys({
          lines: {
            100: {
              unit: "m"
            },
            200: {
              unit: "cm"
            }
          }
        });
      "#,
    )
    .unwrap();
  runtime.run_event_loop(Default::default()).await.unwrap();
}

#[test]
fn test_op_return_serde_v8_error() {
  #[allow(clippy::unnecessary_wraps)]
  #[op2]
  #[serde]
  fn op_err() -> Result<std::collections::BTreeMap<u64, u64>, JsErrorBox> {
    Ok([(1, 2), (3, 4)].into_iter().collect()) // Maps can't have non-string keys in serde_v8
  }

  deno_core::extension!(test_ext, ops = [op_err]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });
  assert!(
    runtime
      .execute_script(
        "test_op_return_serde_v8_error.js",
        "Deno.core.ops.op_err()"
      )
      .is_err()
  );
}

#[test]
fn test_op_high_arity() {
  #[allow(clippy::unnecessary_wraps)]
  #[op2(fast)]
  #[number]
  fn op_add_4(
    #[number] x1: i64,
    #[number] x2: i64,
    #[number] x3: i64,
    #[number] x4: i64,
  ) -> Result<i64, JsErrorBox> {
    Ok(x1 + x2 + x3 + x4)
  }

  deno_core::extension!(test_ext, ops = [op_add_4]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });
  let r = runtime
    .execute_script("test.js", "Deno.core.ops.op_add_4(1, 2, 3, 4)")
    .unwrap();
  deno_core::scope!(scope, runtime);
  assert_eq!(r.open(scope).integer_value(scope), Some(10));
}

#[test]
fn test_op_disabled() {
  #[allow(clippy::unnecessary_wraps)]
  #[op2(fast)]
  #[number]
  fn op_foo() -> Result<i64, JsErrorBox> {
    Ok(42)
  }

  fn ops() -> Vec<OpDecl> {
    vec![op_foo().disable()]
  }

  deno_core::extension!(test_ext, ops_fn = ops);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });
  // Disabled op should be replaced with a function that throws.
  let err = runtime
    .execute_script("test.js", "Deno.core.ops.op_foo()")
    .unwrap_err();
  assert!(err.to_string().contains("op is disabled"));
}

#[test]
fn test_op_detached_buffer() {
  #[allow(clippy::unnecessary_wraps)]
  #[op2]
  fn op_sum_take(#[buffer(detach)] b: JsBuffer) -> Result<u32, JsErrorBox> {
    Ok(b.as_ref().iter().clone().map(|x| *x as u32).sum())
  }

  #[allow(clippy::unnecessary_wraps)]
  #[op2]
  #[buffer]
  fn op_boomerang(
    #[buffer(detach)] b: JsBuffer,
  ) -> Result<JsBuffer, JsErrorBox> {
    Ok(b)
  }

  deno_core::extension!(test_ext, ops = [op_sum_take, op_boomerang]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  runtime
    .execute_script(
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
      "#,
    )
    .unwrap();

  runtime
    .execute_script(
      "test.js",
      r#"
      const wmem = new WebAssembly.Memory({ initial: 1, maximum: 2 });
      const w32 = new Uint32Array(wmem.buffer);
      w32[0] = 1; w32[1] = 2; w32[2] = 3;
      const assertWasmThrow = (() => {
        try {
          let sum = Deno.core.ops.op_sum_take(w32.subarray(0, 2));
          return false;
        } catch(e) {
          return e.message.includes('expected');
        }
      });
      if (!assertWasmThrow()) {
        throw new Error("expected wasm mem to not be detachable");
      }
    "#,
    )
    .unwrap();
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "Found ops with duplicate names:")]
fn duplicate_op_names() {
  mod a {
    use super::*;

    #[allow(clippy::unnecessary_wraps)]
    #[op2]
    #[string]
    pub fn op_test() -> Result<String, JsErrorBox> {
      Ok(String::from("Test"))
    }
  }

  #[op2]
  #[string]
  #[allow(clippy::unnecessary_wraps)]
  pub fn op_test() -> Result<String, JsErrorBox> {
    Ok(String::from("Test"))
  }

  deno_core::extension!(test_ext, ops = [a::op_test, op_test]);
  JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });
}

#[test]
fn ops_in_js_have_proper_names() {
  #[allow(clippy::unnecessary_wraps)]
  #[op2]
  #[string]
  fn op_test_sync() -> Result<String, JsErrorBox> {
    Ok(String::from("Test"))
  }

  #[op2]
  #[string]
  async fn op_test_async() -> Result<String, JsErrorBox> {
    Ok(String::from("Test"))
  }

  deno_core::extension!(test_ext, ops = [op_test_sync, op_test_async]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });

  let src = r#"
  if (Deno.core.ops.op_test_sync.name !== "op_test_sync") {
    throw new Error();
  }

  if (Deno.core.ops.op_test_async.name !== "op_test_async") {
    throw new Error();
  }

  const { op_test_async } = Deno.core.ops;
  if (op_test_async.name !== "op_test_async") {
    throw new Error();
  }
  "#;
  runtime.execute_script("test", src).unwrap();
}

#[tokio::test]
async fn test_ref_unref_ops() {
  let (mut runtime, _dispatch_count) = setup(Mode::AsyncDeferred);
  runtime
    .execute_script(
      "filename.js",
      r#"
      const { op_test } = Deno.core.ops;
      var p1 = op_test(42);
      var p2 = op_test(42);
      "#,
    )
    .unwrap();
  {
    let realm = runtime.main_realm();
    assert_eq!(realm.num_pending_ops(), 2);
    assert_eq!(realm.num_unrefed_ops(), 0);
  }
  runtime
    .execute_script(
      "filename.js",
      r#"
      Deno.core.unrefOpPromise(p1);
      Deno.core.unrefOpPromise(p2);
      "#,
    )
    .unwrap();
  {
    let realm = runtime.main_realm();
    assert_eq!(realm.num_pending_ops(), 2);
    assert_eq!(realm.num_unrefed_ops(), 2);
  }
  runtime
    .execute_script(
      "filename.js",
      r#"
      Deno.core.refOpPromise(p1);
      Deno.core.refOpPromise(p2);
      "#,
    )
    .unwrap();
  {
    let realm = runtime.main_realm();
    assert_eq!(realm.num_pending_ops(), 2);
    assert_eq!(realm.num_unrefed_ops(), 0);
  }
}

#[test]
fn test_dispatch() {
  let (mut runtime, dispatch_count) = setup(Mode::Async);
  runtime
    .execute_script(
      "filename.js",
      r#"
      let control = 42;
      const { op_test } = Deno.core.ops;
      op_test(control);
      async function main() {
        op_test(control);
      }
      main();
      "#,
    )
    .unwrap();
  assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
}

#[test]
fn test_dispatch_no_zero_copy_buf() {
  let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(false));
  runtime
    .execute_script(
      "filename.js",
      r#"
      const { op_test } = Deno.core.ops;
      op_test(0);
      "#,
    )
    .unwrap();
  assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
}

#[test]
fn test_dispatch_stack_zero_copy_bufs() {
  let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(true));
  runtime
    .execute_script(
      "filename.js",
      r#"
      const { op_test } = Deno.core.ops;
      let zero_copy_a = new Uint8Array([0]);
      op_test(0, zero_copy_a);
      "#,
    )
    .unwrap();
  assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
}

#[test]
fn test_call_site() {
  let (mut runtime, _) = setup(Mode::Async);
  runtime
    .execute_script(
      "file:///filename.js",
      r#"
      const cs = Deno.core.currentUserCallSite();
      assert(cs.fileName === "file:///filename.js");
      assert(cs.lineNumber === 2);
      assert(cs.columnNumber === 28);
    "#,
    )
    .unwrap();
}

/// Test that long-running ops do not block dynamic imports from loading.
// https://github.com/denoland/deno/issues/19903
// https://github.com/denoland/deno/issues/19455
#[tokio::test]
pub async fn test_top_level_await() {
  #[op2]
  async fn op_sleep_forever() {
    tokio::time::sleep(Duration::MAX).await
  }

  deno_core::extension!(testing, ops = [op_sleep_forever]);

  let loader = StaticModuleLoader::new([
    (
      Url::parse("http://x/main.js").unwrap(),
      r#"
const { op_sleep_forever } = Deno.core.ops;
(async () => await op_sleep_forever())();
await import('./mod.js');
    "#,
    ),
    (
      Url::parse("http://x/mod.js").unwrap(),
      r#"
const { op_void_async_deferred } = Deno.core.ops;
await op_void_async_deferred();
    "#,
    ),
  ]);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(loader)),
    extensions: vec![testing::init()],
    ..Default::default()
  });

  let id = runtime
    .load_main_es_module(&Url::parse("http://x/main.js").unwrap())
    .await
    .unwrap();
  let mut rx = runtime.mod_evaluate(id);

  tokio::select! {
    // Not using biased mode leads to non-determinism for relatively simple
    // programs.
    biased;

    maybe_result = &mut rx => {
      maybe_result
    }

    event_loop_result = runtime.run_event_loop(Default::default()) => {
      event_loop_result.unwrap();

      rx.await
    }
  }
  .expect("Failed to get module result");
}

#[op2]
pub async fn op_async() {
  println!("op_async!");
}

#[op2]
#[allow(unreachable_code)]
pub fn op_async_impl_future_error()
-> Result<impl Future<Output = ()>, JsErrorBox> {
  return Err(JsErrorBox::generic("dead"));
  Ok(async {})
}

#[op2]
pub async fn op_async_yield() {
  tokio::task::yield_now().await;
  println!("op_async_yield!");
}

#[op2]
pub async fn op_async_yield_error() -> Result<(), JsErrorBox> {
  tokio::task::yield_now().await;
  println!("op_async_yield_error!");
  Err(JsErrorBox::generic("dead"))
}

#[op2]
pub async fn op_async_error() -> Result<(), JsErrorBox> {
  println!("op_async_error!");
  Err(JsErrorBox::generic("dead"))
}

#[op2(async(deferred), fast)]
pub async fn op_async_deferred() {
  println!("op_async_deferred!");
}

#[op2(async(lazy), fast)]
pub async fn op_async_lazy() {
  println!("op_async_lazy!");
}

#[op2(fast)]
pub fn op_sync() {
  println!("op_sync!");
}

#[op2(fast)]
pub fn op_sync_error() -> Result<(), JsErrorBox> {
  Err(JsErrorBox::generic("Always fails"))
}

#[op2(fast)]
pub fn op_sync_arg_error(_: u32) {
  panic!("Should never be called")
}

#[op2]
pub async fn op_async_arg_error(_: u32) {
  panic!("Should never be called")
}

deno_core::extension!(
  test_ext,
  ops = [
    op_async,
    op_async_error,
    op_async_yield,
    op_async_yield_error,
    op_async_lazy,
    op_async_deferred,
    op_async_impl_future_error,
    op_sync,
    op_sync_error,
    op_sync_arg_error,
    op_async_arg_error,
  ],
);

#[tokio::test]
pub async fn test_op_metrics() {
  let out = Rc::new(RefCell::new(vec![]));

  let out_clone = out.clone();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    op_metrics_factory_fn: Some(Box::new(move |_, _, op| {
      let name = op.name;
      if !name.starts_with("op_async") && !name.starts_with("op_sync") {
        return None;
      }
      let out_clone = out_clone.clone();
      Some(Rc::new(move |_, metrics, _| {
        let s = format!("{} {:?}", name, metrics);
        println!("{s}");
        out_clone.borrow_mut().push(s);
      }))
    })),
    ..Default::default()
  });

  let promise = runtime
  .execute_script(
    "filename.js",
    r#"
    const { op_sync, op_sync_error, op_async, op_async_error, op_async_yield, op_async_yield_error, op_async_deferred, op_async_lazy, op_async_impl_future_error, op_sync_arg_error, op_async_arg_error } = Deno.core.ops;
    async function go() {
      op_sync();
      try { op_sync_error(); } catch {}
      await op_async();
      try { await op_async_error() } catch {}
      await op_async_yield();
      try { await op_async_yield_error() } catch {}
      await op_async_deferred();
      await op_async_lazy();
      try { await op_async_impl_future_error() } catch {}
      try { op_sync_arg_error() } catch {}
      try { await op_async_arg_error() } catch {}
    }

    go()
    "#,
  )
  .unwrap();
  #[allow(deprecated)]
  runtime
    .resolve_value(promise)
    .await
    .expect("Failed to await promise");
  drop(runtime);
  let out = Rc::try_unwrap(out).unwrap().into_inner().join("\n");
  assert_eq!(
    out,
    r#"op_sync Dispatched
op_sync Completed
op_sync_error Dispatched
op_sync_error Error
op_async Dispatched
op_async Completed
op_async_error Dispatched
op_async_error Error
op_async_yield Dispatched
op_async_yield CompletedAsync
op_async_yield_error Dispatched
op_async_yield_error ErrorAsync
op_async_deferred Dispatched
op_async_deferred CompletedAsync
op_async_lazy Dispatched
op_async_lazy CompletedAsync
op_async_impl_future_error Dispatched
op_async_impl_future_error Error
op_sync_arg_error Dispatched
op_sync_arg_error Error
op_async_arg_error Dispatched
op_async_arg_error Error"#
  );
}

#[tokio::test]
pub async fn test_op_metrics_summary_tracker() {
  let tracker = Rc::new(OpMetricsSummaryTracker::default());
  // We want to limit the tracker to just the ops we care about
  let op_enabled = |op: &OpDecl| {
    op.name.starts_with("op_async") || op.name.starts_with("op_sync")
  };
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    op_metrics_factory_fn: Some(
      tracker.clone().op_metrics_factory_fn(op_enabled),
    ),
    ..Default::default()
  });

  let promise = runtime
  .execute_script(
    "filename.js",
    r#"
    const { op_sync, op_sync_error, op_async, op_async_error, op_async_yield, op_async_yield_error, op_async_deferred, op_async_lazy, op_async_impl_future_error, op_sync_arg_error, op_async_arg_error } = Deno.core.ops;
    async function go() {
      op_sync();
      try { op_sync_error(); } catch {}
      await op_async();
      try { await op_async_error() } catch {}
      await op_async_yield();
      try { await op_async_yield_error() } catch {}
      await op_async_deferred();
      await op_async_lazy();
      try { await op_async_impl_future_error() } catch {}
      try { op_sync_arg_error() } catch {}
      try { await op_async_arg_error() } catch {}
    }

    go()
    "#,
  )
  .unwrap();
  #[allow(deprecated)]
  runtime
    .resolve_value(promise)
    .await
    .expect("Failed to await promise");
  drop(runtime);
  assert_eq!(
    tracker.aggregate(),
    OpMetricsSummary {
      ops_completed_async: 8,
      ops_dispatched_async: 8,
      ops_dispatched_sync: 3,
      ops_dispatched_fast: 0,
    }
  );
}
