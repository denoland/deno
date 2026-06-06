// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  // 22_body.js registers `webidl.converters["BodyInit_DOMString"]` (and the
  // nullable wrapper) as a side effect. We bench it directly to isolate the
  // converter cost from the rest of `new Response()`.
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = [
      "ext:deno_fetch/21_formdata.js" = "21_formdata.js",
      "ext:deno_fetch/22_body.js" = "22_body.js",
      "ext:bench_setup/setup" = {
        source = r#"
          import * as webidl from "ext:deno_webidl/00_webidl.js";
          import "ext:deno_fetch/22_body.js";
          const conv = webidl.converters["BodyInit_DOMString?"];
          // Stable references so the bench loop only measures the call.
          globalThis.convertString = () => conv("hello world", "p", "c");
          globalThis.convertEmpty = () => conv("", "p", "c");
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

fn bench_body_init_string(b: &mut Bencher) {
  bench_js_sync(b, r#"convertString();"#, setup);
}

fn bench_body_init_empty_string(b: &mut Bencher) {
  bench_js_sync(b, r#"convertEmpty();"#, setup);
}

benchmark_group!(
  benches,
  bench_body_init_string,
  bench_body_init_empty_string,
);
bench_or_profile!(benches);
