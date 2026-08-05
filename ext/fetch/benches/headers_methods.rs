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
          const { Headers } = __bootstrap.core.loadExtScript(
            "ext:deno_fetch/20_headers.js",
          );
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
          globalThis.headerSeq100 = Array.from(
            { length: 100 },
            (_, i) => [`x-many-${i}`, `value-${i}`],
          );
          globalThis.headerRecord100 = Object.fromEntries(headerSeq100);
          globalThis.headerDupSeq100 = Array.from(
            { length: 100 },
            (_, i) => [`x-dup-${i % 10}`, `value-${i}`],
          );
          globalThis.h100 = new Headers(headerSeq100);
          globalThis.iterateHeaders = (headers) => {
            let total = 0;
            for (const [key, value] of headers) {
              total += key.length + value.length;
            }
            return total;
          };
        "#
    },],
    lazy_loaded_js = ["ext:deno_fetch/20_headers.js" = "20_headers.js",]
  );

  vec![
    deno_webidl::deno_webidl::init(),
    deno_web::deno_web::init(
      deno_web::BlobStore::default_arc(),
      None,
      Default::default(),
      Default::default(),
    ),
    bench_setup::init(),
  ]
}

fn bench_construct_headers_7(b: &mut Bencher) {
  // `new Headers({...})` invokes `appendHeader` once per key, and each
  // `appendHeader` previously did an O(n) `byteLowerCase` scan over all
  // existing entries to find the first matching name's case. With 7 keys
  // that's 0+1+2+3+4+5+6 = 21 redundant `toLowerCase` allocations on top
  // of the 7 input-name lowercasings.
  bench_js_sync(b, r#"makeHeaders();"#, setup);
}

fn bench_construct_headers_sequence_100(b: &mut Bencher) {
  bench_js_sync(b, r#"new Headers(headerSeq100);"#, setup);
}

fn bench_construct_headers_record_100(b: &mut Bencher) {
  bench_js_sync(b, r#"new Headers(headerRecord100);"#, setup);
}

fn bench_construct_headers_duplicate_sequence_100(b: &mut Bencher) {
  bench_js_sync(b, r#"new Headers(headerDupSeq100);"#, setup);
}

fn bench_iterate_headers_100(b: &mut Bencher) {
  bench_js_sync(b, r#"iterateHeaders(h100);"#, setup);
}

fn bench_construct_iterate_headers_100(b: &mut Bencher) {
  bench_js_sync(b, r#"iterateHeaders(new Headers(headerSeq100));"#, setup);
}

fn bench_has_hit(b: &mut Bencher) {
  // .has() of a name that exists. Pre-cache: 1 + N/2 toLowerCase calls.
  bench_js_sync(b, r#"h7.has("x-request-id");"#, setup);
}

fn bench_has_miss(b: &mut Bencher) {
  // .has() of a name that doesn't exist. Pre-cache: 1 + N toLowerCase calls.
  bench_js_sync(b, r#"h7.has("authorization");"#, setup);
}

fn bench_set_replace(b: &mut Bencher) {
  // .set() of a name that exists: pre-cache walks all entries lowercasing
  // each, replaces the value of the first match, splices duplicates.
  bench_js_sync(b, r#"h7.set("x-request-id", "xyz-456");"#, setup);
}

fn bench_get_hit(b: &mut Bencher) {
  // .get() of a name that exists. Pre-cache: 1 + N/2 toLowerCase calls plus
  // an entries array allocation and a Join. After: cached lowercase compare
  // + a single string assignment for the typical single-value match.
  bench_js_sync(b, r#"h7.get("x-request-id");"#, setup);
}

fn bench_get_miss(b: &mut Bencher) {
  // .get() of a name that doesn't exist. Pre-cache: 1 + N toLowerCase calls.
  bench_js_sync(b, r#"h7.get("authorization");"#, setup);
}

benchmark_group!(
  benches,
  bench_construct_headers_7,
  bench_construct_headers_sequence_100,
  bench_construct_headers_record_100,
  bench_construct_headers_duplicate_sequence_100,
  bench_iterate_headers_100,
  bench_construct_iterate_headers_100,
  bench_has_hit,
  bench_has_miss,
  bench_set_replace,
  bench_get_hit,
  bench_get_miss,
);
bench_or_profile!(benches);
