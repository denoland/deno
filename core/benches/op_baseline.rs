use bench_util::bencher::{benchmark_group, Bencher};
use bench_util::{is_profiling, bench_or_profile};

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

fn loop_code(iters: u64, src: &str) -> String {
  format!(
    r#"for(let i=0; i < {}; i++) {{ {} }}"#,
    iters,
    src,
  )
}

pub fn bench_js_sync(b: &mut Bencher, src: &str) {  
  let mut runtime = create_js_runtime();
  let context = runtime.global_context();
  let scope = &mut v8::HandleScope::with_context(runtime.v8_isolate(), context);
  
  // Increase JS iterations if profiling for nicer flamegraphs
  let inner_iters = 1000 * if is_profiling() { 10000 } else { 1 };
  // Looped code
  let looped_src = loop_code(inner_iters, src);
  
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

pub fn bench_js_async(b: &mut Bencher, src: &str) {
  let mut runtime = create_js_runtime();
  let tokio_runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();
    
  // Looped code
  let looped = loop_code(1000, src);
  let src = looped.as_ref();

  if is_profiling() {
    for _ in 0..10000 {
      runtime.execute("inner_loop", src).unwrap();
      let future = runtime.run_event_loop();
      tokio_runtime.block_on(future).unwrap();
    }
  } else {  
    b.iter(|| {
      runtime.execute("inner_loop", src).unwrap();
      let future = runtime.run_event_loop();
      tokio_runtime.block_on(future).unwrap();
    });
  }
}

fn bench_op_pi_json(b: &mut Bencher) {
  bench_js_sync(
    b,
    r#"Deno.core.opSync("pi_json");"#,
  );
}

fn bench_op_nop(b: &mut Bencher) {
  bench_js_sync(
    b,
    r#"Deno.core.dispatchByName("nop", null, null, null);"#,
  );
}

fn bench_op_async(b: &mut Bencher) {
  bench_js_async(
    b,
    r#"Deno.core.opAsync("pi_async");"#,
  );
}

benchmark_group!(
  benches,
  bench_op_pi_json,
  bench_op_nop,
  bench_op_async
);
bench_or_profile!(benches);
