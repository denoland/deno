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
        import { Request } from "ext:deno_fetch/23_request.js";
        globalThis.Request = Request;
        globalThis.URL_STR = "https://example.com/path?x=1";
        globalThis.HEADERS = { "content-type": "application/json" };
        globalThis.BODY = '{"hello":"world"}';
      "#
    }],
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

fn bench_request_construct_url_only(b: &mut Bencher) {
  // `new Request(url)` -- the common case. Previously walked the
  // RequestInit dictionary converter on every call; now short-circuits to a
  // null-prototype empty object when no init is passed.
  bench_js_sync(b, r#"new Request(URL_STR);"#, setup);
}

fn bench_request_construct_with_method(b: &mut Bencher) {
  // `new Request(url, { method })` -- exercises the plain-data InnerRequest
  // shape (no accessor descriptors) and the dictionary converter path.
  bench_js_sync(b, r#"new Request(URL_STR, { method: "POST" });"#, setup);
}

fn bench_request_construct_full(b: &mut Bencher) {
  // `new Request(url, { method, headers, body })` -- the full constructor
  // path including headerList population and body extraction.
  bench_js_sync(
    b,
    r#"new Request(URL_STR, { method: "POST", headers: HEADERS, body: BODY });"#,
    setup,
  );
}

benchmark_group!(
  benches,
  bench_request_construct_url_only,
  bench_request_construct_with_method,
  bench_request_construct_full,
);
bench_or_profile!(benches);
