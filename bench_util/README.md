# Benching utility for `deno_core` op system

Example:

```rust
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};
use deno_bench_util::bench_js_sync};

use deno_core::op_sync;
use deno_core::serialize_op_result;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::Op;
use deno_core::OpState;

fn setup() -> Vec<Extension> {
  let custom_ext = Extension::builder()
    .ops(vec![
      ("op_nop", |state, _| {
        Op::Sync(serialize_op_result(Ok(9), state))
      }),
    ])
    .build();
  
  vec![
    // deno_{ext}::init(...),
    custom_ext,
  ]
}

fn bench_op_nop(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.ops.op_nop();"#, setup);
}

benchmark_group!(benches, bench_op_nop);
bench_or_profile!(benches);
```
