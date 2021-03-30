use bencher::{benchmark_group, benchmark_main, Bencher};

use deno_core::bin_op_sync;
use deno_core::json_op_sync;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::Op;
use deno_core::OpResponse;

fn create_js_runtime() -> JsRuntime {
  let mut runtime = JsRuntime::new(Default::default());
  runtime.register_op("pi_bin", bin_op_sync(|_, _, _| Ok(314159)));
  runtime.register_op("pi_json", json_op_sync(|_, _: (), _| Ok(314159)));
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

benchmark_group!(benches, bench_op_pi_bin, bench_op_pi_json, bench_op_nop);
benchmark_main!(benches);
