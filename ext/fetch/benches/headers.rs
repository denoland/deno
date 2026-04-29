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
    esm = [
      "ext:deno_fetch/20_headers.js" = "20_headers.js",
      "ext:bench_setup/setup" = {
        source = r#"
          import { Headers } from "ext:deno_fetch/20_headers.js";
          globalThis.Headers = Headers;
          globalThis.makeHeaders = () => new Headers({
            "content-type": "application/json",
            "content-length": "1234",
            "cache-control": "no-cache",
            "x-request-id": "abc-123",
            "accept-encoding": "gzip",
            "x-forwarded-for": "10.0.0.1",
            "user-agent": "deno-bench/1.0",
          });
          globalThis.h7 = makeHeaders();
        "#
      },
    ]
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

fn bench_iter_headers_7(b: &mut Bencher) {
  // First call builds the cache, subsequent calls hit the cache. This is
  // the hot path that HTTP-server / fetch consumers exercise after a
  // Response or Request is constructed: read .entries(), then no further
  // mutations happen.
  bench_js_sync(b, r#"for (const _ of h7) {}"#, setup);
}

fn bench_iter_headers_7_fresh(b: &mut Bencher) {
  // Construct a fresh Headers and iterate once. This is the pessimistic
  // case where the cache never helps because the Headers is short-lived.
  bench_js_sync(b, r#"for (const _ of makeHeaders()) {}"#, setup);
}

fn bench_iter_headers_7_after_set(b: &mut Bencher) {
  // Set always invalidates the cache. We then iterate, which has to
  // rebuild. This bench captures the cost of "mutate then iterate".
  bench_js_sync(
    b,
    r#"h7.set("x-request-id", "xyz-456"); for (const _ of h7) {}"#,
    setup,
  );
}

benchmark_group!(
  benches,
  bench_iter_headers_7,
  bench_iter_headers_7_fresh,
  bench_iter_headers_7_after_set,
);
bench_or_profile!(benches);
