use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};

use deno_core::JsRuntime;

fn setup(runtime: &mut JsRuntime) {
  // TODO(@AaronO): support caller provided extensions in deno_bench_util
  let mut ext = deno_url::init();

  for (name, op_fn) in ext.init_ops().unwrap() {
    runtime.register_op(name, op_fn);
  }
  for (filename, src) in ext.init_js() {
    runtime.execute(filename, src).unwrap();
  }

  runtime
    .execute("setup", "const { URL } = globalThis.__bootstrap.url;")
    .unwrap();
}

fn bench_url_parse(b: &mut Bencher) {
  bench_js_sync(b, r#"new URL(`http://www.google.com/`);"#, setup);
}

benchmark_group!(benches, bench_url_parse,);
bench_or_profile!(benches);
