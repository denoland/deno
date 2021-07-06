use deno_core::Extension;

use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::{benchmark_group, Bencher};
use deno_bench_util::{bench_js_async, bench_js_sync};
use deno_web::BlobStore;

fn setup() -> Vec<Extension> {
  vec![
    deno_webidl::init(),
    deno_url::init(),
    deno_web::init(BlobStore::default(), None),
    deno_timers::init::<deno_timers::NoTimersPermission>(),
    Extension::builder()
    .js(vec![
      ("setup",
        Box::new(|| Ok(r#"
        const { opNow, setTimeout, handleTimerMacrotask } = globalThis.__bootstrap.timers;
        Deno.core.setMacrotaskCallback(handleTimerMacrotask);
        "#.to_owned())),
      ),
    ])
    .state(|state| {
      state.put(deno_timers::NoTimersPermission{});
      Ok(())
    })
    .build()
  ]
}

fn bench_op_now(b: &mut Bencher) {
  bench_js_sync(b, r#"opNow();"#, setup);
}

fn bench_set_timeout(b: &mut Bencher) {
  bench_js_async(b, r#"setTimeout(() => {}, 0);"#, setup);
}

benchmark_group!(benches, bench_op_now, bench_set_timeout,);
bench_or_profile!(benches);
