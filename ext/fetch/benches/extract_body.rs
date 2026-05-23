// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  // 22_body.js registers `extractBody` as part of its IIFE return value; the
  // bench setup module pulls it out via `core.loadExtScript` and stashes it
  // on globalThis so the inner bench loops can call it without further
  // indirection. Each iter freshly allocates a `Uint8Array(N)` so the bench
  // measures the per-call cost of body extraction on a freshly allocated
  // BufferSource — i.e. the shape of every `new Response(new Uint8Array(N))`
  // inside a `Deno.serve` handler.
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = [
      "ext:deno_fetch/21_formdata.js" = "21_formdata.js",
      "ext:deno_fetch/22_body.js" = "22_body.js",
      "ext:bench_setup/setup" = {
        source = r#"
          import "ext:deno_fetch/22_body.js";
          const { extractBody } = Deno.core.loadExtScript(
            "ext:deno_fetch/22_body.js",
          );
          globalThis.__extractBody = extractBody;
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

fn bench_extract_slice_1mb(b: &mut Bencher) {
  bench_js_sync(
    b,
    r#"
      // Default behavior: spec-mandated `slice()` copy of the 1 MB buffer
      const buf = new Uint8Array(1048576);
      __extractBody(buf);
    "#,
    setup,
  );
}

fn bench_extract_transfer_1mb(b: &mut Bencher) {
  bench_js_sync(
    b,
    r#"
      // Opt-in: detach the source ArrayBuffer; no memcpy
      const buf = new Uint8Array(1048576);
      __extractBody(buf, true);
    "#,
    setup,
  );
}

fn bench_extract_slice_64kb(b: &mut Bencher) {
  bench_js_sync(
    b,
    r#"
      const buf = new Uint8Array(65536);
      __extractBody(buf);
    "#,
    setup,
  );
}

fn bench_extract_transfer_64kb(b: &mut Bencher) {
  bench_js_sync(
    b,
    r#"
      const buf = new Uint8Array(65536);
      __extractBody(buf, true);
    "#,
    setup,
  );
}

benchmark_group!(
  benches,
  bench_extract_slice_1mb,
  bench_extract_transfer_1mb,
  bench_extract_slice_64kb,
  bench_extract_transfer_64kb,
);
bench_or_profile!(benches);
