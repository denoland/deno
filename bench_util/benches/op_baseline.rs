use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};
use deno_bench_util::{bench_js_async, bench_js_sync};

use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::serialize_op_result;
use deno_core::Extension;
use deno_core::Op;
use deno_core::OpState;

use std::cell::RefCell;
use std::rc::Rc;

fn setup() -> Vec<Extension> {
  vec![Extension::builder()
    .ops(vec![
      ("pi_json", op_sync(|_, _: (), _: ()| Ok(314159))),
      ("pi_async", op_async(op_pi_async)),
      (
        "nop",
        Box::new(|state, _| Op::Sync(serialize_op_result(Ok(9), state))),
      ),
    ])
    .build()]
}

// this is a function since async closures aren't stable
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

benchmark_group!(benches, bench_op_pi_json, bench_op_nop, bench_op_async);
bench_or_profile!(benches);
