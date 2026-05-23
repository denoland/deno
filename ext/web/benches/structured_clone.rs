// Copyright 2018-2026 the Deno authors. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  // 13_message_port.js exposes `structuredClone` via `core.loadExtScript`;
  // pull it directly here so the bench drives the user-visible API.
  // The fast paths shipped in this PR live inside that function's
  // `options.transfer.length === 0` branch and short-circuit before the
  // V8 ValueSerializer/Deserializer round-trip for Date, RegExp, and
  // flat plain Object/Array of primitives.
  deno_core::extension!(
    bench_setup,
    esm_entry_point = "ext:bench_setup/setup",
    esm = ["ext:bench_setup/setup" = {
      source = r#"
          const { structuredClone } =
            __bootstrap.core.loadExtScript("ext:deno_web/13_message_port.js");

          // Date and RegExp are the simplest fast paths - no nesting, no
          // iteration, both implemented as a direct constructor call.
          globalThis.dateVal = new Date();
          globalThis.regexpVal = new RegExp("foo", "g");

          // Flat plain object - the common state-clone shape in real
          // apps (Redux/Zustand store fragments, JSON-shape request
          // bodies). All values are primitives so the fast path
          // assignment loop wins over the V8 serializer's binary walk.
          globalThis.smallObj = { a: 1, b: 2, c: 3 };

          // Flat plain array of primitives - same shape as above but
          // indexed rather than keyed.
          globalThis.smallArr = [1, 2, 3, 4, 5, 6, 7, 8];

          globalThis.cloneDate = () => structuredClone(globalThis.dateVal);
          globalThis.cloneRegExp = () => structuredClone(globalThis.regexpVal);
          globalThis.cloneFlatObj = () => structuredClone(globalThis.smallObj);
          globalThis.cloneFlatArr = () => structuredClone(globalThis.smallArr);
        "#
    },],
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

fn bench_structured_clone_date(b: &mut Bencher) {
  bench_js_sync(b, r#"cloneDate();"#, setup);
}

fn bench_structured_clone_regexp(b: &mut Bencher) {
  bench_js_sync(b, r#"cloneRegExp();"#, setup);
}

fn bench_structured_clone_flat_object(b: &mut Bencher) {
  bench_js_sync(b, r#"cloneFlatObj();"#, setup);
}

fn bench_structured_clone_flat_array(b: &mut Bencher) {
  bench_js_sync(b, r#"cloneFlatArr();"#, setup);
}

benchmark_group!(
  benches,
  bench_structured_clone_date,
  bench_structured_clone_regexp,
  bench_structured_clone_flat_object,
  bench_structured_clone_flat_array,
);
bench_or_profile!(benches);
