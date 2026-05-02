// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

#[derive(Clone)]
struct Permissions;

fn setup() -> Vec<Extension> {
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = ["ext:bench_setup/setup" = {
      source = r#"
        import { TextDecoder, TextEncoder } from "ext:deno_web/08_text_encoding.js";
        globalThis.TextDecoder = TextDecoder;
        globalThis.TextEncoder = TextEncoder;
        globalThis.hello12k = Deno.core.encode("hello world\n".repeat(1e3));
        globalThis.helloShort = "hello world\n";
        globalThis.encInto = new TextEncoder();
        globalThis.dest = new Uint8Array(64);
      "#
    }],
    state = |state| {
      state.put(Permissions {});
    },
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

fn bench_encode_12kb(b: &mut Bencher) {
  bench_js_sync(b, r#"new TextDecoder().decode(hello12k);"#, setup);
}

fn bench_encode_into_short(b: &mut Bencher) {
  bench_js_sync(b, r#"encInto.encodeInto(helloShort, dest);"#, setup);
}

benchmark_group!(benches, bench_encode_12kb, bench_encode_into_short);
bench_or_profile!(benches);
