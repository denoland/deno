// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use bencher::Bencher;
use deno_core::v8;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

use crate::profiling::is_profiling;

pub fn create_js_runtime(setup: impl FnOnce() -> Vec<Extension>) -> JsRuntime {
  JsRuntime::new(RuntimeOptions {
    extensions: setup(),
    ..Default::default()
  })
}

fn loop_code(iters: u64) -> String {
  format!(r#"for(let i=0; i < {}; i++) {{ bench() }}"#, iters)
}

#[derive(Copy, Clone)]
pub struct BenchOptions {
  pub benching_inner: u64,
  pub profiling_inner: u64,
  pub profiling_outer: u64,
}

impl Default for BenchOptions {
  fn default() -> Self {
    Self {
      benching_inner: 1_000,
      profiling_inner: 1_000,
      profiling_outer: 10_000,
    }
  }
}

pub fn bench_js_sync(
  b: &mut Bencher,
  src: &str,
  setup: impl FnOnce() -> Vec<Extension>,
) {
  bench_js_sync_with(b, src, setup, Default::default())
}

pub fn bench_js_sync_with(
  b: &mut Bencher,
  src: &str,
  setup: impl FnOnce() -> Vec<Extension>,
  opts: BenchOptions,
) {
  let mut runtime = create_js_runtime(setup);
  let scope = &mut runtime.handle_scope();

  // Increase JS iterations if profiling for nicer flamegraphs
  let inner_iters = if is_profiling() {
    opts.profiling_inner * opts.profiling_outer
  } else {
    opts.benching_inner
  };

  {
    let prep_src = format!(
      r#"function add(a, b) {{ return Deno.core.ops.op_add_fast(a, b) }};  %PrepareFunctionForOptimization(add); %OptimizeFunctionOnNextCall(add); add(1, 2); function bench() {{ {} }}; "#,
      src
    );
    let code = v8::String::new(scope, prep_src.as_ref()).unwrap();
    let script = v8::Script::compile(scope, code, None).unwrap();
    script.run(scope).unwrap();
  }
  // Looped code
  let looped_src = loop_code(inner_iters);

  let code = v8::String::new(scope, looped_src.as_ref()).unwrap();
  let script = v8::Script::compile(scope, code, None).unwrap();

  // Run once if profiling, otherwise regular bench loop
  if is_profiling() {
    script.run(scope).unwrap();
  } else {
    b.iter(|| {
      script.run(scope).unwrap();
    });
  }
}

pub fn bench_js_async(
  b: &mut Bencher,
  src: &str,
  setup: impl FnOnce() -> Vec<Extension>,
) {
  bench_js_async_with(b, src, setup, Default::default())
}

pub fn bench_js_async_with(
  b: &mut Bencher,
  src: &str,
  setup: impl FnOnce() -> Vec<Extension>,
  opts: BenchOptions,
) {
  let mut runtime = create_js_runtime(setup);
  let tokio_runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  // Looped code
  let inner_iters = if is_profiling() {
    opts.profiling_inner
  } else {
    opts.benching_inner
  };
  {
    let prep_src = format!(
      r#"async function bench() {{ {} }}; %PrepareFunctionForOptimization(bench); bench(); %OptimizeFunctionOnNextCall(bench);"#,
      src
    );
    runtime.execute_script("prep_code", &prep_src).unwrap();
  }
  let looped = loop_code(inner_iters);
  let src = looped.as_ref();

  if is_profiling() {
    for _ in 0..opts.profiling_outer {
      tokio_runtime.block_on(inner_async(src, &mut runtime));
    }
  } else {
    b.iter(|| {
      tokio_runtime.block_on(inner_async(src, &mut runtime));
    });
  }
}

async fn inner_async(src: &str, runtime: &mut JsRuntime) {
  runtime.execute_script("inner_loop", src).unwrap();
  runtime.run_event_loop(false).await.unwrap();
}
