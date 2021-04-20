// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::serialize_op_result;
use deno_core::JsRuntime;
use deno_core::Op;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;

use bench_util::bench_or_profile;
use bench_util::bencher::{benchmark_group, Bencher};
use bench_util::{bench_js_async, bench_js_sync};

use std::cell::RefCell;
use std::rc::Rc;

fn setup(rt: &mut JsRuntime) {
  rt.register_op("pi_json", op_sync(|_, _: (), _| Ok(314159)));
  rt.register_op("pi_async", op_async(op_pi_async));
  rt.register_op("nop", |state, _, _| {
    Op::Sync(serialize_op_result(Ok(9), state))
  });
}

// this is a function since async closures aren't stable
async fn op_pi_async(
  _: Rc<RefCell<OpState>>,
  _: (),
  _: Option<ZeroCopyBuf>,
) -> Result<i64, AnyError> {
  Ok(314159)
}

fn bench_op_pi_json(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.opSync("pi_json");"#, setup);
}

fn bench_op_nop(b: &mut Bencher) {
  bench_js_sync(
    b,
    r#"Deno.core.dispatchByName("nop", null, null, null);"#,
    setup,
  );
}

fn bench_op_async(b: &mut Bencher) {
  bench_js_async(b, r#"Deno.core.opAsync("pi_async");"#, setup);
}

benchmark_group!(benches, bench_op_pi_json, bench_op_nop, bench_op_async);
bench_or_profile!(benches);
