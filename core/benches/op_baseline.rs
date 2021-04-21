use bencher::{benchmark_group, benchmark_main, Bencher};

use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::serialize_op_result;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::Op;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;

use std::cell::RefCell;
use std::rc::Rc;

fn create_js_runtime() -> JsRuntime {
  let mut runtime = JsRuntime::new(Default::default());
  runtime.register_op("pi_json", op_sync(|_, _: (), _| Ok(314159)));
  runtime.register_op("pi_async", op_async(op_pi_async));
  runtime.register_op("nop", |state, _, _| {
    Op::Sync(serialize_op_result(Ok(9), state))
  });

  // Init ops
  runtime
    .execute(
      "init",
      r#"
      Deno.core.ops();
      Deno.core.registerErrorClass('Error', Error);
    "#,
    )
    .unwrap();

  runtime
}

// this is a function since async closures aren't stable
async fn op_pi_async(
  _: Rc<RefCell<OpState>>,
  _: (),
  _: Option<ZeroCopyBuf>,
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

fn bench_op_pi_json(b: &mut Bencher) {
  bench_runtime_js(
    b,
    r#"for(let i=0; i < 1e3; i++) {
      Deno.core.opSync("pi_json", null);
    }"#,
  );
}

fn bench_op_nop(b: &mut Bencher) {
  bench_runtime_js(
    b,
    r#"for(let i=0; i < 1e3; i++) {
      Deno.core.dispatchByName("nop", null, null, null);
    }"#,
  );
}

fn bench_op_async(b: &mut Bencher) {
  bench_runtime_js_async(
    b,
    r#"for(let i=0; i < 1e3; i++) {
      Deno.core.opAsync("pi_async", null);
    }"#,
  );
}

benchmark_group!(benches, bench_op_pi_json, bench_op_nop, bench_op_async);
benchmark_main!(benches);
