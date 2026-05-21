// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  // Bring up the Fetch JS surface enough to instantiate `Headers` so the
  // bench loop can call `[Symbol.iterator]()` / `Object.fromEntries(...)`.
  // The `_iterableHeaders` getter that this bench exercises lives in
  // `20_headers.js`; the file is wired in via `lazy_loaded_js` to match
  // how production loads it.
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = ["ext:bench_setup/setup" = {
      source = r#"
          const { Headers } = Deno.core.loadExtScript(
            "ext:deno_fetch/20_headers.js",
          );
          globalThis.Headers = Headers;
          globalThis.SEVEN_HEADERS = new Headers({
            "content-type": "application/json",
            "content-length": "123",
            "accept": "*/*",
            "accept-encoding": "gzip, deflate",
            "user-agent": "deno-bench/1.0",
            "x-request-id": "abc-123-def-456",
            "authorization": "Bearer abc.def.ghi",
          });
        "#
    },],
    lazy_loaded_js = ["20_headers.js",]
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

fn bench_headers_iter_for_of(b: &mut Bencher) {
  // `for ([k,v] of headers)` -- the dominant iteration shape. Hits the
  // `[_iterableHeaders]` getter once per for-of, which on main rebuilds
  // a sorted + lowercased entries array every call.
  bench_js_sync(
    b,
    r#"for (const [k, v] of SEVEN_HEADERS) { void k; void v; }"#,
    setup,
  );
}

fn bench_headers_iter_object_from_entries(b: &mut Bencher) {
  // `Object.fromEntries(headers)` -- the common shape for converting
  // headers into a plain object (e.g. spreading into a JSON payload).
  bench_js_sync(b, r#"Object.fromEntries(SEVEN_HEADERS);"#, setup);
}

fn bench_headers_iter_keys(b: &mut Bencher) {
  // `for (k of headers.keys())` -- hits the same iter machinery.
  bench_js_sync(
    b,
    r#"for (const k of SEVEN_HEADERS.keys()) { void k; }"#,
    setup,
  );
}

benchmark_group!(
  benches,
  bench_headers_iter_for_of,
  bench_headers_iter_object_from_entries,
  bench_headers_iter_keys,
);
bench_or_profile!(benches);
