// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  deno_core::extension!(
    deno_webidl_bench,
    esm_entry_point = "ext:deno_webidl_bench/setup.js",
    esm = ["ext:deno_webidl_bench/setup.js" = {
      source = r#"
        import { converters } from "ext:deno_webidl/00_webidl.js";
        globalThis.usv = converters.USVString;
        globalThis.SHORT_ASCII = "id";
        globalThis.LONG_ASCII = "this-is-a-longer-but-still-pure-ascii-key-name";
        globalThis.BMP_NON_ASCII = "caf\u{00e9}";
        globalThis.WITH_SURROGATE = "abc\u{1f600}xyz"; // valid surrogate pair
      "#
    }]
  );

  vec![deno_webidl::deno_webidl::init(), deno_webidl_bench::init()]
}

fn bench_usv_short_ascii(b: &mut Bencher) {
  bench_js_sync(b, r#"usv(SHORT_ASCII, "p", "c");"#, setup);
}

fn bench_usv_long_ascii(b: &mut Bencher) {
  bench_js_sync(b, r#"usv(LONG_ASCII, "p", "c");"#, setup);
}

fn bench_usv_bmp_non_ascii(b: &mut Bencher) {
  bench_js_sync(b, r#"usv(BMP_NON_ASCII, "p", "c");"#, setup);
}

fn bench_usv_with_surrogate(b: &mut Bencher) {
  // Slow path -- has surrogate pair, falls through to V8 builtin.
  bench_js_sync(b, r#"usv(WITH_SURROGATE, "p", "c");"#, setup);
}

benchmark_group!(
  benches,
  bench_usv_short_ascii,
  bench_usv_long_ascii,
  bench_usv_bmp_non_ascii,
  bench_usv_with_surrogate,
);
bench_or_profile!(benches);
