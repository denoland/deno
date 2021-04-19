use deno_core::op_sync;
use deno_core::JsRuntime;

use bench_util::bench_js_sync;
use bench_util::bench_or_profile;
use bench_util::bencher::{benchmark_group, Bencher};

fn setup(rt: &mut JsRuntime) {
  rt.register_op("op_url_parse", op_sync(deno_url::op_url_parse));
  rt.register_op(
    "op_url_parse_search_params",
    op_sync(deno_url::op_url_parse_search_params),
  );
  rt.register_op(
    "op_url_stringify_search_params",
    op_sync(deno_url::op_url_stringify_search_params),
  );

  deno_url::init(rt);
  rt.execute("setup", "const { URL } = globalThis.__bootstrap.url;")
    .unwrap();
}

fn bench_url_parse(b: &mut Bencher) {
  bench_js_sync(b, r#"new URL(`http://www.google.com/${i}`);"#, setup);
}

benchmark_group!(benches, bench_url_parse,);
bench_or_profile!(benches);
