// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_bench_util::bench_js_sync;
use deno_bench_util::bench_or_profile;
use deno_bench_util::bencher::benchmark_group;
use deno_bench_util::bencher::Bencher;
use deno_core::Extension;
use deno_core::ExtensionFileSource;
use deno_core::ExtensionFileSourceCode;
use deno_core::OpState;
use deno_web::BlobStore;

#[derive(Clone)]
struct Permissions;

impl deno_web::TimersPermission for Permissions {
  fn allow_hrtime(&mut self) -> bool {
    false
  }
  fn check_unstable(&self, _state: &OpState, _api_name: &'static str) {
    unreachable!()
  }
}

fn setup() -> Vec<Extension> {
  vec![
    deno_webidl::deno_webidl::init_ops_and_esm(),
    deno_url::deno_url::init_ops_and_esm(),
    deno_console::deno_console::init_ops_and_esm(),
    deno_web::deno_web::init_ops_and_esm::<Permissions>(
      BlobStore::default(),
      None,
    ),
    Extension::builder("bench_setup")
      .esm(vec![ExtensionFileSource {
        specifier: "ext:bench_setup/setup",
        code: ExtensionFileSourceCode::IncludedInBinary(
          r#"
        import { TextDecoder } from "ext:deno_web/08_text_encoding.js";
        globalThis.TextDecoder = TextDecoder;
        globalThis.hello12k = Deno.core.encode("hello world\n".repeat(1e3));
        "#,
        ),
      }])
      .state(|state| {
        state.put(Permissions {});
      })
      .esm_entry_point("ext:bench_setup/setup")
      .build(),
  ]
}

fn bench_encode_12kb(b: &mut Bencher) {
  bench_js_sync(b, r#"new TextDecoder().decode(hello12k);"#, setup);
}

benchmark_group!(benches, bench_encode_12kb);
bench_or_profile!(benches);
