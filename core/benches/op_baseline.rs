use bencher::{benchmark_group, benchmark_main, Bencher};

use deno_core::bin_op_sync;
use deno_core::error::AnyError;
use deno_core::json_op_async;
use deno_core::json_op_sync;
use deno_core::v8;
use deno_core::BufVec;
use deno_core::JsRuntime;
use deno_core::Op;
use deno_core::OpResponse;
use deno_core::OpState;

use std::cell::RefCell;
use std::rc::Rc;

fn create_js_runtime() -> JsRuntime {
  let mut runtime = JsRuntime::new(Default::default());
  runtime.register_op("pi_bin", bin_op_sync(|_, _, _| Ok(314159)));
  runtime.register_op("pi_json", json_op_sync(|_, _: (), _| Ok(314159)));
  runtime.register_op("pi_async", json_op_async(op_pi_async));
  runtime
    .register_op("nop", |_, _, _| Op::Sync(OpResponse::Value(Box::new(9))));

  // Init ops
  runtime
    .execute(
      "init",
      r#"
      Deno.core.ops();
      Deno.core.registerErrorClass('Error', Error);
      const nopBuffer = new ArrayBuffer(10);
      const nopView = new DataView(nopBuffer);
    "#,
    )
    .unwrap();

  runtime
}

// this is a function since async closures aren't stable
async fn op_pi_async(
  _: Rc<RefCell<OpState>>,
  _: (),
  _: BufVec,
) -> Result<i64, AnyError> {
  Ok(314159)
}

pub fn bench_runtime_js(b: &mut Bencher, src: &str) {
  let mut runtime = create_js_runtime();
  let context = runtime.global_context();
  let scope = &mut v8::HandleScope::with_context(runtime.v8_isolate(), context);
  let code = v8::String::new(scope, src).unwrap();
  let script = v8::Script::compile(scope, code, None).unwrap();
  b.iter(|| {
    script.run(scope).unwrap();
  });
}

pub fn bench_runtime_js_async(b: &mut Bencher, src: &str) {
  let mut runtime = create_js_runtime();
  let tokio_runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  b.iter(|| {
    runtime.execute("inner_loop", src).unwrap();
    let future = runtime.run_event_loop();
    tokio_runtime.block_on(future).unwrap();
  });
}

fn bench_op_pi_bin(b: &mut Bencher) {
  bench_runtime_js(
    b,
    r#"for(let i=0; i < 1e3; i++) {
      Deno.core.binOpSync("pi_bin", 0, nopView);
    }"#,
  );
}

fn bench_op_pi_json(b: &mut Bencher) {
  bench_runtime_js(
    b,
    r#"for(let i=0; i < 1e3; i++) {
      Deno.core.jsonOpSync("pi_json", null);
    }"#,
  );
}

fn bench_op_nop(b: &mut Bencher) {
  bench_runtime_js(
    b,
    r#"for(let i=0; i < 1e3; i++) {
      Deno.core.dispatchByName("nop", null, null, nopView);
    }"#,
  );
}

fn bench_op_async(b: &mut Bencher) {
  bench_runtime_js_async(
    b,
    r#"for(let i=0; i < 1e3; i++) {
      Deno.core.jsonOpAsync("pi_async", null);
    }"#,
  );
}

benchmark_group!(
  benches,
  bench_op_pi_bin,
  bench_op_pi_json,
  bench_op_nop,
  bench_op_async
);
benchmark_main!(benches);
