// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_bench_util::bench_js_async;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::Bencher;
use deno_core::Extension;
use deno_core::ExtensionFileSource;
use deno_web::BlobStore;

struct Permissions;

impl deno_web::TimersPermission for Permissions {
  fn allow_hrtime(&mut self) -> bool {
    true
  }
  fn check_unstable(
    &self,
    _state: &deno_core::OpState,
    _api_name: &'static str,
  ) {
  }
}

fn setup() -> Vec<Extension> {
  vec![
    deno_webidl::init(),
    deno_url::init(),
    deno_console::init(),
    deno_web::init::<Permissions>(BlobStore::default(), None),
    Extension::builder("bench_setup")
    .esm(vec![
      ExtensionFileSource {
        specifier: "internal:setup".to_string(), 
        code: r#"
      import { setTimeout, handleTimerMacrotask } from "internal:deno_web/02_timers.js";
      globalThis.setTimeout = setTimeout;
      Deno.core.setMacrotaskCallback(handleTimerMacrotask);
      "#
      },
    ])
    .state(|state| {
      state.put(Permissions{});
      Ok(())
    })
    .build()
  ]
}

fn bench_set_timeout(b: &mut Bencher) {
  bench_js_async(b, r#"setTimeout(() => {}, 0);"#, setup);
}

benchmark_group!(benches, bench_set_timeout,);
bench_or_profile!(benches);
