use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};

use deno_core::Extension;

fn setup() -> Vec<Extension> {
  vec![
    deno_url::init(),
    Extension::builder()
      .js(vec![(
        "setup",
        Box::new(|| {
          Ok(r#"const { URL } = globalThis.__bootstrap.url;"#.to_owned())
        }),
      )])
      .build(),
  ]
}

fn bench_url_parse(b: &mut Bencher) {
  bench_js_sync(b, r#"new URL(`http://www.google.com/`);"#, setup);
}

benchmark_group!(benches, bench_url_parse,);
bench_or_profile!(benches);
