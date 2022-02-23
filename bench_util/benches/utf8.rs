use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};

use deno_core::Extension;

fn setup() -> Vec<Extension> {
  vec![Extension::builder()
    .js(vec![(
      "setup.js",
      Box::new(|| {
        Ok(
          r#"
      const hello = "hello world\n";
      const hello1k = hello.repeat(1e3);
      const helloEncoded = Deno.core.encode(hello);
      const hello1kEncoded = Deno.core.encode(hello1k);
      "#
          .into(),
        )
      }),
    )])
    .build()]
}

fn bench_utf8_encode_single(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.encode(hello);"#, setup);
}

fn bench_utf8_encode_1k(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.encode(hello1k);"#, setup);
}

fn bench_utf8_decode_single(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.decode(helloEncoded);"#, setup);
}

fn bench_utf8_decode_1k(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.decode(hello1kEncoded);"#, setup);
}

benchmark_group!(
  benches,
  bench_utf8_encode_single,
  bench_utf8_encode_1k,
  bench_utf8_decode_single,
  bench_utf8_decode_1k
);
bench_or_profile!(benches);
