use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};
use deno_bench_util::{bench_js_async, bench_js_sync};

use deno_core::op;
use deno_core::Extension;

fn setup() -> Vec<Extension> {
  vec![Extension::builder()
    .ops(vec![
      op_pi_json::decl(),
      op_pi_async::decl(),
      op_nop::decl(),
    ])
    .build()]
}

#[op]
fn op_nop() {}

#[op]
fn op_pi_json() -> i64 {
  314159
}

// this is a function since async closures aren't stable
#[op]
async fn op_pi_async() -> i64 {
  314159
}

fn bench_op_pi_json(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.ops.op_pi_json();"#, setup);
}

fn bench_op_nop(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.ops.op_nop();"#, setup);
}

fn bench_op_async(b: &mut Bencher) {
  bench_js_async(b, r#"Deno.core.opAsync("op_pi_async");"#, setup);
}

fn bench_is_proxy(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.isProxy(42);"#, setup);
}

benchmark_group!(
  benches,
  bench_op_pi_json,
  bench_op_nop,
  bench_op_async,
  bench_is_proxy
);

bench_or_profile!(benches);
