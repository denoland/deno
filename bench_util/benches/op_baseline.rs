// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_bench_util::bench_js_async;
use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::Bencher;

use deno_core::op;
use deno_core::Extension;

deno_core::extension!(
  bench_setup,
  ops = [
    // op_pi_json,
    op_pi_async,
    op_nop
  ]
);

fn setup() -> Vec<Extension> {
  vec![bench_setup::init_ops()]
}

#[op]
fn op_nop() {}

// TODO(bartlomieju): reenable, currently this op generates a fast function,
// which is wrong, because i64 is not a compatible type for fast call.
// #[op]
// fn op_pi_json() -> i64 {
//   314159
// }

// this is a function since async closures aren't stable
#[op]
async fn op_pi_async() -> i64 {
  314159
}

// fn bench_op_pi_json(b: &mut Bencher) {
//   bench_js_sync(b, r#"Deno.core.ops.op_pi_json();"#, setup);
// }

fn bench_op_nop(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.ops.op_nop();"#, setup);
}

fn bench_op_async(b: &mut Bencher) {
  bench_js_async(b, r#"Deno.core.opAsync("op_pi_async");"#, setup);
}

benchmark_group!(
  benches,
  // bench_op_pi_json,
  bench_op_nop,
  bench_op_async,
);

bench_or_profile!(benches);
