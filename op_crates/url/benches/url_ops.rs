use bencher::{benchmark_group, benchmark_main, Bencher};

use deno_core::op_sync;
use deno_core::v8;
use deno_core::JsRuntime;

fn create_js_runtime() -> JsRuntime {
  let mut runtime = JsRuntime::new(Default::default());
  runtime.register_op("op_url_parse", op_sync(deno_url::op_url_parse));
  runtime.register_op(
    "op_url_parse_search_params",
    op_sync(deno_url::op_url_parse_search_params),
  );
  runtime.register_op(
    "op_url_stringify_search_params",
    op_sync(deno_url::op_url_stringify_search_params),
  );

  runtime
    .execute(
      "bootstrap",
      "globalThis.__bootstrap = (globalThis.__bootstrap || {});",
    )
    .unwrap();
  deno_url::init(&mut runtime);
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
    .execute("setup", "const { URL } = globalThis.__bootstrap.url;")
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

fn bench_url_parse(b: &mut Bencher) {
  bench_runtime_js(b, r#"new URL(`http://www.google.com/`);"#);
}

benchmark_group!(benches, bench_url_parse,);
benchmark_main!(benches);
