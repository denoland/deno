// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::Bencher;

use deno_core::Extension;
use deno_core::ExtensionFileSource;
use deno_core::ExtensionFileSourceCode;

fn setup() -> Vec<Extension> {
  vec![
    deno_webidl::deno_webidl::init_ops_and_esm(),
    Extension::builder("deno_webidl_bench")
      .esm(vec![ExtensionFileSource {
        specifier: "ext:deno_webidl_bench/setup.js",
        code: ExtensionFileSourceCode::IncludedInBinary(include_str!(
          "dict.js"
        )),
      }])
      .esm_entry_point("ext:deno_webidl_bench/setup.js")
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
