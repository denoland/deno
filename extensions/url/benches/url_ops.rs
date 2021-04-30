use bencher::{benchmark_group, benchmark_main, Bencher};

use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

fn create_js_runtime() -> JsRuntime {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![deno_url::init()],
    ..Default::default()
  });

  runtime
    .execute("setup", "const { URL } = globalThis.__bootstrap.url;")
    .unwrap();

  runtime
}

pub fn bench_runtime_js(b: &mut Bencher, src: &str) {
  let mut runtime = create_js_runtime();
  let scope = &mut runtime.handle_scope();
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
