# Benching utility for `deno_core` op system

Example:

```rust
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};
use deno_bench_util::bench_js_sync};

use deno_core::op_sync;
use deno_core::serialize_op_result;
use deno_core::JsRuntime;
use deno_core::Op;
use deno_core::OpState;

fn setup(runtime: &mut JsRuntime) {
  runtime.register_op("nop", |state, _| {
    Op::Sync(serialize_op_result(Ok(9), state))
  });
  runtime.sync_ops_cache();
}

fn bench_op_nop(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.opSync("nop", null, null, null);"#, setup);
}

benchmark_group!(benches, bench_op_nop);
bench_or_profile!(benches);
```
