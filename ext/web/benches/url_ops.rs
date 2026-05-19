// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = ["ext:bench_setup/setup" = {
      source = r#"
        import { URL, URLSearchParams } from "ext:deno_web/00_url.js";
        globalThis.URL = URL;
        globalThis.URLSearchParams = URLSearchParams;
        globalThis.uspShort = "a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8";
        globalThis.uspShortWithPlus =
          "q=hello+world&page=1&filter=year:2026";
        globalThis.uspShortWithPercent =
          "q=hello%20world&page=1&filter=year%3A2026";
        globalThis.uspShortUnicode = "q=caf%C3%A9&name=Fran%C3%A7ois";
      "#
    }]
  );

  vec![
    deno_webidl::deno_webidl::init(),
    deno_web::deno_web::init(
      Default::default(),
      None,
      Default::default(),
      Default::default(),
    ),
    bench_setup::init(),
  ]
}

fn bench_url_parse(b: &mut Bencher) {
  bench_js_sync(b, r#"new URL(`http://www.google.com/`);"#, setup);
}

fn bench_usp_construct_short_safe(b: &mut Bencher) {
  // 8 alphanumeric pairs, no escapes -- exercises the JS fast path
  // (no `%`, no non-ASCII).
  bench_js_sync(b, r#"new URLSearchParams(uspShort);"#, setup);
}

fn bench_usp_construct_short_with_plus(b: &mut Bencher) {
  // 3 pairs with `+`-encoded spaces -- still on the JS fast path
  // (`+` -> U+0020 substitution is ASCII-only).
  bench_js_sync(b, r#"new URLSearchParams(uspShortWithPlus);"#, setup);
}

fn bench_usp_construct_short_with_percent(b: &mut Bencher) {
  // 3 pairs with `%XX` escapes -- input contains `%`, falls through to
  // the Rust op since the decoded bytes might be non-ASCII.
  bench_js_sync(b, r#"new URLSearchParams(uspShortWithPercent);"#, setup);
}

fn bench_usp_construct_short_unicode(b: &mut Bencher) {
  // 2 pairs with non-ASCII content -- falls through to the Rust op.
  bench_js_sync(b, r#"new URLSearchParams(uspShortUnicode);"#, setup);
}

benchmark_group!(
  benches,
  bench_url_parse,
  bench_usp_construct_short_safe,
  bench_usp_construct_short_with_plus,
  bench_usp_construct_short_with_percent,
  bench_usp_construct_short_unicode,
);
bench_or_profile!(benches);
