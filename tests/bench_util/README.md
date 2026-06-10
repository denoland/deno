# Benching utility for `deno_core` op system

Example:

```rust
use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::Bencher;
use deno_bench_util::bencher::benchmark_group;
use deno_core::Extension;

#[op2]
#[number]
fn op_nop() -> usize {
  9
}

fn setup() -> Vec<Extension> {
  vec![Extension {
    name: "my_ext",
    ops: std::borrow::Cow::Borrowed(&[op_nop::DECL]),
  }]
}

fn bench_op_nop(b: &mut Bencher) {
  bench_js_sync(b, r#"Deno.core.ops.op_nop();"#, setup);
}

benchmark_group!(benches, bench_op_nop);
bench_or_profile!(benches);
```
