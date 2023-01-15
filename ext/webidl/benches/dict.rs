// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::Bencher;

use deno_core::Extension;

fn setup() -> Vec<Extension> {
  vec![
    deno_webidl::init(),
    Extension::builder("deno_webidl_bench")
      .js(vec![("setup", include_str!("dict.js"))])
      .build(),
  ]
}

fn converter_undefined(b: &mut Bencher) {
  bench_js_sync(b, r#"TextDecodeOptions(undefined);"#, setup);
}

fn handwritten_baseline_undefined(b: &mut Bencher) {
  bench_js_sync(b, r#"handwrittenConverter(undefined)"#, setup);
}

fn converter_object(b: &mut Bencher) {
  bench_js_sync(b, r#"TextDecodeOptions({});"#, setup);
}

fn handwritten_baseline_object(b: &mut Bencher) {
  bench_js_sync(b, r#"handwrittenConverter({})"#, setup);
}

benchmark_group!(
  benches,
  converter_undefined,
  handwritten_baseline_undefined,
  converter_object,
  handwritten_baseline_object,
);
bench_or_profile!(benches);
