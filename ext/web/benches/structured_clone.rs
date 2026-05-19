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
        import { structuredClone } from "ext:deno_web/13_message_port.js";
        globalThis.structuredClone = structuredClone;
        globalThis.smallObj = { a: 1, b: "two", c: true };
        globalThis.u8_256 = new Uint8Array(256).fill(0x41);
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

// Primitive clone — the hot path the perf fix targets. With the wrapper
// inlinable, V8 elides the call into structuredCloneSlow entirely.
fn bench_clone_number(b: &mut Bencher) {
  bench_js_sync(b, r#"structuredClone(42);"#, setup);
}

fn bench_clone_short_string(b: &mut Bencher) {
  bench_js_sync(b, r#"structuredClone("hello world");"#, setup);
}

fn bench_clone_boolean(b: &mut Bencher) {
  bench_js_sync(b, r#"structuredClone(true);"#, setup);
}

// Non-primitive: exercises the slow path. Should be unchanged after the fix.
fn bench_clone_small_obj(b: &mut Bencher) {
  bench_js_sync(b, r#"structuredClone(smallObj);"#, setup);
}

fn bench_clone_u8_256(b: &mut Bencher) {
  bench_js_sync(b, r#"structuredClone(u8_256);"#, setup);
}

benchmark_group!(
  benches,
  bench_clone_number,
  bench_clone_short_string,
  bench_clone_boolean,
  bench_clone_small_obj,
  bench_clone_u8_256,
);
bench_or_profile!(benches);
