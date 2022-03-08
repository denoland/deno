use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};
use deno_bench_util::{bench_js_async, bench_js_sync};

use deno_core::error::AnyError;
use deno_core::op;

use deno_core::Extension;
use deno_core::OpState;

use std::cell::RefCell;
use std::rc::Rc;

fn setup() -> Vec<Extension> {
  vec![Extension::builder()
    .ops(|ctx| {
      ctx.register("pi_json", op_pi_json);
      ctx.register("pi_async", op_pi_async);
      ctx.register("nop", op_nop);
    })
    .build()]
}

#[op]
fn op_nop(_: &mut OpState, _: (), _: ()) -> Result<(), AnyError> {
  Ok(())
}

#[op]
fn op_pi_json(_: &mut OpState, _: (), _: ()) -> Result<i64, AnyError> {
  Ok(314159)
}

// this is a function since async closures aren't stable
#[op]
async fn op_pi_async(
  _: Rc<RefCell<OpState>>,
  _: (),
  _: (),
) -> Result<i64, AnyError> {
  Ok(314159)
}

fn bench_op_pi_json(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.opSync("pi_json", null);"#, setup);
}

fn bench_op_nop(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.opSync("nop", null, null, null);"#, setup);
}

fn bench_op_async(b: &mut Bencher) {
  bench_js_async(b, r#"Deno.core.opAsync("pi_async", null);"#, setup);
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
