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
        globalThis.uspSmallSafe = new URLSearchParams(
          "a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8",
        );
        globalThis.uspSmallSpaces = new URLSearchParams([
          ["q", "hello world"],
          ["page", "1"],
          ["filter", "year:2026"],
        ]);
        globalThis.uspUnicode = new URLSearchParams([
          ["q", "caf\u{00e9}"],
          ["name", "Fran\u{00e7}ois"],
          ["emoji", "\u{1f600}"],
        ]);
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

fn bench_usp_tostring_safe(b: &mut Bencher) {
  // Common shape: 8 short ASCII-only safe pairs (no escapes triggered).
  bench_js_sync(b, r#"uspSmallSafe.toString();"#, setup);
}

fn bench_usp_tostring_spaces(b: &mut Bencher) {
  // Common shape: small number of pairs with spaces / colons that hit the
  // ASCII-with-escapes path but not the UTF-8 path.
  bench_js_sync(b, r#"uspSmallSpaces.toString();"#, setup);
}

fn bench_usp_tostring_unicode(b: &mut Bencher) {
  // Exercises the UTF-8 + percent-encode path with combining accents,
  // 3-byte sequences, and a 4-byte supplementary-plane code point.
  bench_js_sync(b, r#"uspUnicode.toString();"#, setup);
}

benchmark_group!(
  benches,
  bench_url_parse,
  bench_usp_tostring_safe,
  bench_usp_tostring_spaces,
  bench_usp_tostring_unicode,
);
bench_or_profile!(benches);
