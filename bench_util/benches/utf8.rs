use deno_bench_util::bench_js_sync_with;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::BenchOptions;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  vec![Extension::builder("bench_setup")
    .js(vec![(
      "setup.js",
      r#"
      const hello = "hello world\n";
      const hello1k = hello.repeat(1e3);
      const hello1m = hello.repeat(1e6);
      const helloEncoded = Deno.core.encode(hello);
      const hello1kEncoded = Deno.core.encode(hello1k);
      const hello1mEncoded = Deno.core.encode(hello1m);
      "#,
    )])
    .build()]
}

fn bench_utf8_encode_12_b(b: &mut Bencher) {
  bench_js_sync_with(
    b,
    r#"Deno.core.encode(hello);"#,
    setup,
    BenchOptions {
      benching_inner: 1,
      ..Default::default()
    },
  );
}

fn bench_utf8_encode_12_kb(b: &mut Bencher) {
  bench_js_sync_with(
    b,
    r#"Deno.core.encode(hello1k);"#,
    setup,
    BenchOptions {
      benching_inner: 1,
      ..Default::default()
    },
  );
}

fn bench_utf8_encode_12_mb(b: &mut Bencher) {
  bench_js_sync_with(
    b,
    r#"Deno.core.encode(hello1m);"#,
    setup,
    BenchOptions {
      benching_inner: 1,
      profiling_inner: 10,
      profiling_outer: 10,
    },
  );
}

fn bench_utf8_decode_12_b(b: &mut Bencher) {
  bench_js_sync_with(
    b,
    r#"Deno.core.decode(helloEncoded);"#,
    setup,
    BenchOptions {
      benching_inner: 1,
      ..Default::default()
    },
  );
}

fn bench_utf8_decode_12_kb(b: &mut Bencher) {
  bench_js_sync_with(
    b,
    r#"Deno.core.decode(hello1kEncoded);"#,
    setup,
    BenchOptions {
      benching_inner: 1,
      ..Default::default()
    },
  );
}

fn bench_utf8_decode_12_mb(b: &mut Bencher) {
  bench_js_sync_with(
    b,
    r#"Deno.core.decode(hello1mEncoded);"#,
    setup,
    BenchOptions {
      benching_inner: 1,
      profiling_inner: 10,
      profiling_outer: 10,
    },
  );
}

benchmark_group!(
  benches,
  bench_utf8_encode_12_b,
  bench_utf8_encode_12_kb,
  bench_utf8_encode_12_mb,
  bench_utf8_decode_12_b,
  bench_utf8_decode_12_kb,
  bench_utf8_decode_12_mb,
);
bench_or_profile!(benches);
