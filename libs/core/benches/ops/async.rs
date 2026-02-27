// Copyright 2018-2025 the Deno authors. MIT license.

use bencher::*;
use deno_core::*;
use deno_error::JsErrorBox;
use std::ffi::c_void;
use tokio::runtime::Runtime;

deno_core::extension!(
  testing,
  ops = [
    op_void,
    op_make_external,
    op_call_promise_resolver,
    op_resolve_promise,
    op_async_void,
    op_async_void_lazy,
    op_async_void_lazy_nofast,
    op_async_void_deferred,
    op_async_void_deferred_nofast,
    op_async_void_deferred_return,
    op_async_yield,
    op_async_yield_lazy,
    op_async_yield_lazy_nofast,
    op_async_yield_deferred,
    op_async_yield_deferred_nofast,
  ],
);

#[op2(fast)]
pub fn op_call_promise_resolver(scope: &mut v8::PinScope, f: &v8::Function) {
  let recv = v8::undefined(scope).into();
  f.call(scope, recv, &[]);
}

#[op2]
pub fn op_resolve_promise<'s>(
  scope: &'s mut v8::PinScope,
) -> v8::Local<'s, v8::Promise> {
  let resolver = v8::PromiseResolver::new(scope).unwrap();
  let value = v8::undefined(scope).into();
  resolver.resolve(scope, value).unwrap();
  resolver.get_promise(scope)
}

#[op2(fast)]
pub fn op_void() {}

#[op2(fast)]
pub fn op_make_external() -> *const c_void {
  std::ptr::null()
}

#[op2]
pub async fn op_async_void() {}

#[op2]
pub async fn op_async_yield() {
  tokio::task::yield_now().await
}

#[op2(async(lazy), fast)]
pub async fn op_async_yield_lazy() {
  tokio::task::yield_now().await
}

#[op2(async(lazy), nofast)]
pub async fn op_async_yield_lazy_nofast() {
  tokio::task::yield_now().await
}

#[op2(async(deferred), fast)]
pub async fn op_async_yield_deferred() {
  tokio::task::yield_now().await
}

#[op2(async(deferred), nofast)]
pub async fn op_async_yield_deferred_nofast() {
  tokio::task::yield_now().await
}

#[op2(async(lazy), fast)]
pub async fn op_async_void_lazy() {}

#[op2(async(lazy), nofast)]
pub async fn op_async_void_lazy_nofast() {}

#[op2(async(deferred), fast)]
pub async fn op_async_void_deferred_return() -> u32 {
  1
}

#[op2(async(deferred), fast)]
pub async fn op_async_void_deferred() {}

#[op2(async(deferred), nofast)]
pub async fn op_async_void_deferred_nofast() {}

fn bench_op(
  b: &mut Bencher,
  count: usize,
  op: &str,
  arg_count: usize,
  call: &str,
) {
  #[cfg(not(feature = "unsafe_runtime_options"))]
  unreachable!(
    "This benchmark must be run with --features=unsafe_runtime_options"
  );

  let tokio = tokio::runtime::Builder::new_current_thread()
    .build()
    .unwrap();
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![testing::init()],
    // We need to feature gate this here to prevent IDE errors
    #[cfg(feature = "unsafe_runtime_options")]
    unsafe_expose_natives_and_gc: true,
    ..Default::default()
  });
  let err_mapper =
    |err| JsErrorBox::generic(format!("{op} test failed ({call}): {err:?}"));

  let args = (0..arg_count)
    .map(|n| format!("arg{n}"))
    .collect::<Vec<_>>()
    .join(", ");

  let mut harness = include_str!("async_harness.js").to_owned();
  for (key, value) in [
    ("PERCENT", "%"),
    ("CALL", call),
    ("COUNT", &format!("{count}")),
    ("ARGS", &args.to_string()),
    ("OP", op),
  ] {
    harness = harness.replace(&format!("__{key}__"), value);
  }

  // Prime the optimizer
  runtime
    .execute_script("", harness)
    .map_err(err_mapper)
    .unwrap();
  let guard = tokio.enter();
  let run = runtime.execute_script("", ascii_str!("run()")).unwrap();
  #[allow(deprecated)]
  let bench = tokio.block_on(runtime.resolve_value(run)).unwrap();
  let bench = {
    deno_core::scope!(scope, &mut runtime);
    let bench: v8::Local<v8::Function> =
      v8::Local::new(scope, bench).try_into().unwrap();

    v8::Global::new(scope, bench)
  };
  drop(guard);
  b.iter(move || do_benchmark(&bench, &tokio, &mut runtime));
}

#[inline(never)]
fn do_benchmark(
  bench: &v8::Global<v8::Function>,
  tokio: &Runtime,
  runtime: &mut JsRuntime,
) {
  tokio.block_on(async {
    let guard = tokio.enter();
    let call = runtime.call(bench);
    runtime
      .with_event_loop_promise(call, PollEventLoopOptions::default())
      .await
      .unwrap();
    drop(guard);
  });
}

const BENCH_COUNT: usize = 1000;

/// Tests the overhead of execute_script.
fn baseline(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_async_void", 0, "accum += __index__;");
}

/// Tests the overhead of execute_script with a promise.
fn baseline_promise(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_async_void",
    0,
    "await Promise.resolve(null);",
  );
}

/// Tests the overhead of execute_script with a promise.
fn baseline_promise_with_resolver(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_async_void",
    0,
    "{ let { promise, resolve } = Promise.withResolvers(); resolve(null); await promise; }",
  );
}

/// Tests the overhead of execute_script with a promise.
fn baseline_op_promise_resolver(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_call_promise_resolver",
    1,
    "{ let { promise, resolve } = Promise.withResolvers(); op_call_promise_resolver(resolve); await promise; }",
  );
}

/// Tests the overhead of execute_script with a promise.
fn baseline_op_promise(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_resolve_promise",
    0,
    "await op_resolve_promise();",
  );
}

/// Tests the overhead of execute_script, but also returns a value so we can make sure things are
/// working.
fn bench_op_async_void_deferred_return(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_async_void_deferred_return",
    0,
    "accum += await op_async_void_deferred_return();",
  );
}

macro_rules! bench_void {
  ($bench:ident, $op:ident) => {
    fn $bench(b: &mut Bencher) {
      bench_op(
        b,
        BENCH_COUNT,
        stringify!($op),
        0,
        concat!("await ", stringify!($op), "()"),
      );
    }
  };
}

bench_void!(baseline_sync, op_void);
bench_void!(bench_op_async_yield, op_async_yield);
bench_void!(bench_op_async_yield_lazy, op_async_yield_lazy);
bench_void!(bench_op_async_yield_lazy_nofast, op_async_yield_lazy_nofast);
bench_void!(bench_op_async_yield_deferred, op_async_yield_deferred);
bench_void!(
  bench_op_async_yield_deferred_nofast,
  op_async_yield_deferred_nofast
);
bench_void!(bench_op_async_void, op_async_void);
bench_void!(bench_op_async_void_lazy, op_async_void_lazy);
bench_void!(bench_op_async_void_lazy_nofast, op_async_void_lazy_nofast);
bench_void!(bench_op_async_void_deferred, op_async_void_deferred);
bench_void!(
  bench_op_async_void_deferred_nofast,
  op_async_void_deferred_nofast
);

benchmark_group!(
  benches,
  baseline,
  baseline_promise,
  baseline_promise_with_resolver,
  baseline_op_promise_resolver,
  baseline_op_promise,
  baseline_sync,
  bench_op_async_yield,
  bench_op_async_yield_lazy,
  bench_op_async_yield_lazy_nofast,
  bench_op_async_yield_deferred,
  bench_op_async_yield_deferred_nofast,
  bench_op_async_void,
  bench_op_async_void_lazy,
  bench_op_async_void_lazy_nofast,
  bench_op_async_void_deferred,
  bench_op_async_void_deferred_nofast,
  bench_op_async_void_deferred_return,
);

benchmark_main!(benches);
